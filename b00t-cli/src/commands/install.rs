use crate::dependency_resolver::DependencyResolver;
use crate::hook_engine::HookResult;
use crate::{BootDatum, UnifiedConfig};
use anyhow::{Context, Result, anyhow};
use clap::Parser;
use duct::cmd;
use shellexpand;
use std::collections::HashMap;
use std::path::PathBuf;
use toml;

#[derive(Parser)]
pub enum InstallCommands {
    #[clap(
        about = "Run 'just install' to install b00t components",
        long_about = "Executes the justfile install recipe which:\n  - Installs b00t-mcp via cargo\n  - Installs b00t-cli via cargo\n  - Installs cocogitto\n  - Sets up git commit hooks\n\nExamples:\n  b00t install\n  b00t install --dry-run"
    )]
    Run {
        #[clap(long, help = "Show what would be installed without installing")]
        dry_run: bool,
    },
}

impl InstallCommands {
    pub fn execute(&self, _path: &str) -> Result<()> {
        match self {
            InstallCommands::Run { dry_run } => run_just_install(*dry_run),
        }
    }
}

pub fn run_just_install(dry_run: bool) -> Result<()> {
    let workspace_root = crate::utils::get_workspace_root();

    if dry_run {
        println!(
            "🔍 Dry run: Would execute 'just install' from {}",
            workspace_root
        );
        println!("\nThe justfile install recipe would:");
        println!("  1. cargo install --path b00t-mcp --force");
        println!("  2. cargo install --path b00t-cli --force");
        println!("  3. cargo install cocogitto --locked --force");
        println!("  4. just install-commit-hook");
        return Ok(());
    }

    println!("🥾 Running 'just install' from {}", workspace_root);

    let output = cmd!("just", "install")
        .dir(&workspace_root)
        .stdout_capture()
        .stderr_capture()
        .unchecked()
        .run()
        .context("Failed to execute 'just install'")?;

    // Print stdout
    if !output.stdout.is_empty() {
        println!("{}", String::from_utf8_lossy(&output.stdout));
    }

    // Print stderr
    if !output.stderr.is_empty() {
        eprintln!("{}", String::from_utf8_lossy(&output.stderr));
    }

    if !output.status.success() {
        anyhow::bail!(
            "just install failed with exit code: {}",
            output.status.code().unwrap_or(-1)
        );
    }

    println!("✅ Installation complete!");

    Ok(())
}

/// Install any datum (cli/mcp/ai/etc) by name with dependency resolution.
pub fn install_datum(path: &str, name: &str, dry_run: bool) -> Result<()> {
    let all_datums = load_all_datums(path)?;

    // Prefer exact key match. When multiple datums share the same plain `b00t.name`,
    // prefer an installable datum over a non-installable one.
    let target_key = if all_datums.contains_key(name) {
        name.to_string()
    } else {
        let matches: Vec<(&String, &BootDatum)> = all_datums
            .iter()
            .filter(|(_, datum)| datum.name == name)
            .collect();

        if matches.is_empty() {
            return Err(anyhow!("Datum '{}' not found", name));
        }

        if let Some((key, _)) = matches
            .iter()
            .find(|(_, datum)| datum.install_command().is_some())
        {
            (*key).clone()
        } else {
            matches[0].0.clone()
        }
    };

    // Resolve dependencies using datum graph
    let datum_refs: Vec<&BootDatum> = all_datums.values().collect();
    let resolver = DependencyResolver::new(datum_refs);
    let install_order = resolver
        .resolve(&target_key)
        .context(format!("Failed to resolve dependencies for {}", name))?;

    if dry_run {
        println!("[dry-run] would install {} items:", install_order.len());
        for (idx, item) in install_order.iter().enumerate() {
            println!("   {}. {}", idx + 1, item);
        }
        return Ok(());
    }

    println!("📋 Installation order ({} items):", install_order.len());
    for (idx, item) in install_order.iter().enumerate() {
        println!("   {}. {}", idx + 1, item);
    }
    println!();

    for key in install_order {
        let datum = all_datums
            .get(&key)
            .ok_or_else(|| anyhow!("Missing datum during install: {}", key))?;

        // Evaluate hook_detect if set (e.g. hook_detect = "gates")
        if let Some(hook) = &datum.hook_detect {
            // Set env vars so gates.rhai can find the datum file
            let datum_file = format!("{}/{}.toml", shellexpand::tilde(path), key.replace('.', "."));
            unsafe { std::env::set_var("_B00T_DATUM_FILE", &datum_file); }
            unsafe { std::env::set_var("_B00T_DATUM_NAME", &datum.name); }
            let result = crate::hook_engine::run_hook(hook);
            match &result {
                crate::hook_engine::HookResult::Ok => {},
                crate::hook_engine::HookResult::Warn(msg) => {
                    eprintln!("⚠️  {} hook_detect: {}", key, msg);
                }
                crate::hook_engine::HookResult::Redirect(alt) => {
                    eprintln!("⏭️  {} redirected to {} by hook_detect", key, alt);
                    continue;
                }
                crate::hook_engine::HookResult::Missing(msg) => {
                    eprintln!("⏭️  {} hook_detect: {}", key, msg);
                    continue;
                }
                _ => {}
            }
        }

        // Skip if already installed (best-effort using version command)
        if let Some(version_cmd) = &datum.version {
            match cmd!("bash", "-c", version_cmd).run() {
                Ok(_) => {
                    println!("✅ {} already installed, skipping", key);
                    continue;
                }
                Err(e) => {
                    eprintln!(
                        "⚠️  Version check for '{}' failed: {}. Proceeding with installation.",
                        key, e
                    );
                }
            }
        }

        if let Some(install_cmd) = datum.install_command() {
            println!("🚀 Installing {}...", key);
            cmd!("bash", "-c", install_cmd)
                .run()
                .with_context(|| format!("Failed to install {}", key))?;
            println!("✅ Installed {}", key);
        } else {
            println!("⚠️  No install command for {}, skipping", key);
        }
    }

    Ok(())
}

/// Hermes terminal install helper: configure hermes to use local terminal backend.
/// Hermes is the AI terminal; this sets it up for local inference mode.
pub fn hermes_special_install(dry_run: bool) -> Result<()> {
    if dry_run {
        println!("[dry-run] would configure hermes: hermes config set terminal.backend local");
        return Ok(());
    }
    let output = std::process::Command::new("hermes")
        .args(["config", "set", "terminal.backend", "local"])
        .output();
    match output {
        Ok(out) if out.status.success() => {
            println!("✅ Hermes configured: terminal.backend = local");
        }
        Ok(out) => {
            let stderr = String::from_utf8_lossy(&out.stderr);
            anyhow::bail!("hermes config failed: {stderr}");
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            anyhow::bail!(
                "hermes not found in PATH. Install it first: https://github.com/hermes-tui/hermes"
            );
        }
        Err(e) => anyhow::bail!("failed to run hermes: {e}"),
    }
    Ok(())
}

/// Load all datums from the configured path (excluding stack files).
fn load_all_datums(path: &str) -> Result<HashMap<String, BootDatum>> {
    let mut datums = HashMap::new();
    let b00t_dir = PathBuf::from(shellexpand::tilde(path).to_string());

    if !b00t_dir.exists() {
        return Ok(datums);
    }

    for entry in std::fs::read_dir(&b00t_dir)? {
        let entry = entry?;
        let entry_path = entry.path();

        if entry_path.is_file() {
            if let Some(file_name) = entry_path.file_name().and_then(|s| s.to_str()) {
                if file_name.ends_with(".stack.toml") {
                    continue;
                }

                if file_name.ends_with(".toml") {
                    if let Ok(content) = std::fs::read_to_string(&entry_path) {
                        if let Ok(config) = toml::from_str::<UnifiedConfig>(&content) {
                            let datum = config.b00t;
                            let datum_type = datum
                                .datum_type
                                .as_ref()
                                .map(|t| format!("{:?}", t).to_lowercase())
                                .unwrap_or_else(|| "unknown".to_string());
                            let key = format!("{}.{}", datum.name, datum_type);
                            datums.insert(key, datum);
                        }
                    }
                }
            }
        }
    }

    Ok(datums)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::DatumType;
    use std::fs;
    use tempfile::TempDir;

    #[test]
    fn test_install_dry_run() {
        let result = run_just_install(true);
        assert!(result.is_ok());
    }

    // Helper to create a test datum TOML file
    fn create_test_datum_file(
        dir: &TempDir,
        name: &str,
        datum_type: &str,
        depends_on: Option<Vec<String>>,
        install_cmd: Option<&str>,
        version_cmd: Option<&str>,
    ) -> std::io::Result<()> {
        let depends_str = if let Some(deps) = depends_on {
            format!(
                "depends_on = [{}]",
                deps.iter()
                    .map(|d| format!("\"{}\"", d))
                    .collect::<Vec<_>>()
                    .join(", ")
            )
        } else {
            String::new()
        };

        let install_str = if let Some(cmd) = install_cmd {
            format!("install = \"{}\"", cmd)
        } else {
            String::new()
        };

        let version_str = if let Some(cmd) = version_cmd {
            format!("version = \"{}\"", cmd)
        } else {
            String::new()
        };

        let content = format!(
            r#"[b00t]
name = "{}"
type = "{}"
hint = "Test datum {}"
{}
{}
{}
"#,
            name, datum_type, name, depends_str, install_str, version_str
        );

        let filename = format!("{}.{}.toml", name, datum_type);
        let file_path = dir.path().join(filename);
        fs::write(file_path, content)?;
        Ok(())
    }

    #[test]
    fn test_load_all_datums_empty_directory() {
        let temp_dir = TempDir::new().unwrap();
        let path = temp_dir.path().to_str().unwrap();

        let result = load_all_datums(path);
        assert!(result.is_ok());
        let datums = result.unwrap();
        assert_eq!(datums.len(), 0);
    }

    #[test]
    fn test_load_all_datums_single_datum() {
        let temp_dir = TempDir::new().unwrap();
        let path = temp_dir.path().to_str().unwrap();

        create_test_datum_file(&temp_dir, "docker", "cli", None, None, None).unwrap();

        let result = load_all_datums(path);
        assert!(result.is_ok());
        let datums = result.unwrap();
        assert_eq!(datums.len(), 1);
        assert!(datums.contains_key("docker.cli"));
        assert_eq!(datums.get("docker.cli").unwrap().name, "docker");
    }

    #[test]
    fn test_load_all_datums_multiple_datums() {
        let temp_dir = TempDir::new().unwrap();
        let path = temp_dir.path().to_str().unwrap();

        create_test_datum_file(&temp_dir, "docker", "cli", None, None, None).unwrap();
        create_test_datum_file(&temp_dir, "postgres", "docker", None, None, None).unwrap();
        create_test_datum_file(&temp_dir, "kubectl", "cli", None, None, None).unwrap();

        let result = load_all_datums(path);
        assert!(result.is_ok());
        let datums = result.unwrap();
        assert_eq!(datums.len(), 3);
        assert!(datums.contains_key("docker.cli"));
        assert!(datums.contains_key("postgres.docker"));
        assert!(datums.contains_key("kubectl.cli"));
    }

    #[test]
    fn test_load_all_datums_ignores_stack_files() {
        let temp_dir = TempDir::new().unwrap();
        let path = temp_dir.path().to_str().unwrap();

        create_test_datum_file(&temp_dir, "docker", "cli", None, None, None).unwrap();

        // Create a stack file that should be ignored
        let stack_content = r#"[b00t]
name = "test-stack"
type = "stack"
hint = "Test stack"
"#;
        fs::write(temp_dir.path().join("test.stack.toml"), stack_content).unwrap();

        let result = load_all_datums(path);
        assert!(result.is_ok());
        let datums = result.unwrap();
        assert_eq!(datums.len(), 1); // Only docker.cli, stack file ignored
        assert!(datums.contains_key("docker.cli"));
    }

    #[test]
    fn test_load_all_datums_nonexistent_directory() {
        let result = load_all_datums("/nonexistent/path/to/datums");
        assert!(result.is_ok());
        let datums = result.unwrap();
        assert_eq!(datums.len(), 0); // Returns empty hashmap for nonexistent dir
    }

    #[test]
    fn test_install_datum_not_found() {
        let temp_dir = TempDir::new().unwrap();
        let path = temp_dir.path().to_str().unwrap();

        create_test_datum_file(&temp_dir, "docker", "cli", None, None, None).unwrap();

        let result = install_datum(path, "nonexistent", false);
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(err.to_string().contains("not found"));
    }

    #[test]
    fn test_install_datum_by_name() {
        let temp_dir = TempDir::new().unwrap();
        let path = temp_dir.path().to_str().unwrap();

        create_test_datum_file(
            &temp_dir,
            "docker",
            "cli",
            None,
            Some("echo 'Installing docker'"),
            Some("echo '1.0.0'"),
        )
        .unwrap();

        let result = install_datum(path, "docker", false);
        // This will succeed in finding the datum but may fail on actual install
        // We're testing the lookup logic here
        assert!(
            result.is_ok()
                || result
                    .unwrap_err()
                    .to_string()
                    .contains("Failed to install")
        );
    }

    #[test]
    fn test_install_datum_by_key() {
        let temp_dir = TempDir::new().unwrap();
        let path = temp_dir.path().to_str().unwrap();

        create_test_datum_file(
            &temp_dir,
            "docker",
            "cli",
            None,
            Some("echo 'Installing docker'"),
            Some("echo '1.0.0'"),
        )
        .unwrap();

        let result = install_datum(path, "docker.cli", false);
        // This will succeed in finding the datum but may fail on actual install
        // We're testing the lookup logic here
        assert!(
            result.is_ok()
                || result
                    .unwrap_err()
                    .to_string()
                    .contains("Failed to install")
        );
    }

    #[test]
    fn test_install_datum_prefers_installable_name_collision() {
        let temp_dir = TempDir::new().unwrap();
        let path = temp_dir.path().to_str().unwrap();

        // Non-installable datum: no install command; depends on a non-existent datum so
        // that if it is wrongly selected, dependency resolution fails and the test fails.
        create_test_datum_file(
            &temp_dir,
            "pi",
            "agent",
            Some(vec!["nonexistent-dep.cli".to_string()]),
            None,
            None,
        )
        .unwrap();

        // Installable datum: has an install command that creates a sentinel file so we can
        // assert it was actually executed.
        let sentinel = temp_dir.path().join("pi-cli-installed");
        create_test_datum_file(
            &temp_dir,
            "pi",
            "cli",
            None,
            Some(&format!("touch {}", sentinel.display())),
            None,
        )
        .unwrap();

        let result = install_datum(path, "pi", false);
        assert!(
            result.is_ok(),
            "expected installable datum to be selected: {:?}",
            result.err()
        );
        assert!(
            sentinel.exists(),
            "install command was not executed — non-installable datum may have been selected"
        );
    }

    #[test]
    fn test_install_datum_with_circular_dependency() {
        let temp_dir = TempDir::new().unwrap();
        let path = temp_dir.path().to_str().unwrap();

        // Create circular dependency: a -> b -> c -> a
        create_test_datum_file(
            &temp_dir,
            "a",
            "cli",
            Some(vec!["b.cli".to_string()]),
            Some("echo 'Installing a'"),
            None,
        )
        .unwrap();

        create_test_datum_file(
            &temp_dir,
            "b",
            "cli",
            Some(vec!["c.cli".to_string()]),
            Some("echo 'Installing b'"),
            None,
        )
        .unwrap();

        create_test_datum_file(
            &temp_dir,
            "c",
            "cli",
            Some(vec!["a.cli".to_string()]),
            Some("echo 'Installing c'"),
            None,
        )
        .unwrap();

        let result = install_datum(path, "a", false);
        assert!(result.is_err());
        let err = result.unwrap_err();
        let err_msg = format!("{:?}", err).to_lowercase();
        // Check for circular dependency in the error chain
        assert!(
            err_msg.contains("circular"),
            "Expected circular dependency error, got: {:?}",
            err
        );
    }

    #[test]
    fn test_install_datum_with_self_dependency() {
        let temp_dir = TempDir::new().unwrap();
        let path = temp_dir.path().to_str().unwrap();

        // Create self-dependency: a -> a
        create_test_datum_file(
            &temp_dir,
            "a",
            "cli",
            Some(vec!["a.cli".to_string()]),
            Some("echo 'Installing a'"),
            None,
        )
        .unwrap();

        let result = install_datum(path, "a", false);
        assert!(result.is_err());
        let err = result.unwrap_err();
        let err_msg = format!("{:?}", err).to_lowercase();
        // Check for circular dependency in the error chain
        assert!(
            err_msg.contains("circular"),
            "Expected circular dependency error, got: {:?}",
            err
        );
    }

    #[test]
    fn test_install_datum_with_missing_dependency() {
        let temp_dir = TempDir::new().unwrap();
        let path = temp_dir.path().to_str().unwrap();

        // Create datum with missing dependency
        create_test_datum_file(
            &temp_dir,
            "app",
            "cli",
            Some(vec!["missing.cli".to_string()]),
            Some("echo 'Installing app'"),
            None,
        )
        .unwrap();

        let result = install_datum(path, "app", false);
        assert!(result.is_err());
        let err = result.unwrap_err();
        let err_msg = format!("{:?}", err).to_lowercase();
        // Check for "not found" in the error chain
        assert!(
            err_msg.contains("not found"),
            "Expected not found error, got: {:?}",
            err
        );
    }

    #[test]
    fn test_install_datum_with_linear_dependencies() {
        let temp_dir = TempDir::new().unwrap();
        let path = temp_dir.path().to_str().unwrap();

        // Create linear dependency chain: c -> b -> a
        create_test_datum_file(
            &temp_dir,
            "a",
            "cli",
            None,
            Some("echo 'Installing a'"),
            None,
        )
        .unwrap();

        create_test_datum_file(
            &temp_dir,
            "b",
            "cli",
            Some(vec!["a.cli".to_string()]),
            Some("echo 'Installing b'"),
            None,
        )
        .unwrap();

        create_test_datum_file(
            &temp_dir,
            "c",
            "cli",
            Some(vec!["b.cli".to_string()]),
            Some("echo 'Installing c'"),
            None,
        )
        .unwrap();

        let result = install_datum(path, "c", false);
        // Test will succeed in resolving dependencies
        // Actual installation may fail but that's OK for this test
        assert!(
            result.is_ok()
                || result
                    .unwrap_err()
                    .to_string()
                    .contains("Failed to install")
        );
    }

    #[test]
    fn test_install_datum_with_diamond_dependencies() {
        let temp_dir = TempDir::new().unwrap();
        let path = temp_dir.path().to_str().unwrap();

        // Create diamond dependency: d -> [b, c] -> a
        create_test_datum_file(
            &temp_dir,
            "a",
            "cli",
            None,
            Some("echo 'Installing a'"),
            None,
        )
        .unwrap();

        create_test_datum_file(
            &temp_dir,
            "b",
            "cli",
            Some(vec!["a.cli".to_string()]),
            Some("echo 'Installing b'"),
            None,
        )
        .unwrap();

        create_test_datum_file(
            &temp_dir,
            "c",
            "cli",
            Some(vec!["a.cli".to_string()]),
            Some("echo 'Installing c'"),
            None,
        )
        .unwrap();

        create_test_datum_file(
            &temp_dir,
            "d",
            "cli",
            Some(vec!["b.cli".to_string(), "c.cli".to_string()]),
            Some("echo 'Installing d'"),
            None,
        )
        .unwrap();

        let result = install_datum(path, "d", false);
        // Test will succeed in resolving dependencies
        assert!(
            result.is_ok()
                || result
                    .unwrap_err()
                    .to_string()
                    .contains("Failed to install")
        );
    }

    #[test]
    fn test_load_all_datums_with_invalid_toml() {
        let temp_dir = TempDir::new().unwrap();
        let path = temp_dir.path().to_str().unwrap();

        // Create valid datum
        create_test_datum_file(&temp_dir, "docker", "cli", None, None, None).unwrap();

        // Create invalid TOML file
        let invalid_content = "this is not valid toml { [ } ]";
        fs::write(temp_dir.path().join("invalid.toml"), invalid_content).unwrap();

        let result = load_all_datums(path);
        assert!(result.is_ok());
        let datums = result.unwrap();
        // Should skip invalid file and still load the valid one
        assert_eq!(datums.len(), 1);
        assert!(datums.contains_key("docker.cli"));
    }

    #[test]
    fn test_load_all_datums_various_datum_types() {
        let temp_dir = TempDir::new().unwrap();
        let path = temp_dir.path().to_str().unwrap();

        // Create datums of various types
        create_test_datum_file(&temp_dir, "docker", "cli", None, None, None).unwrap();
        create_test_datum_file(&temp_dir, "postgres", "docker", None, None, None).unwrap();
        create_test_datum_file(&temp_dir, "sequential", "mcp", None, None, None).unwrap();
        create_test_datum_file(&temp_dir, "kubectl", "cli", None, None, None).unwrap();

        let result = load_all_datums(path);
        assert!(result.is_ok());
        let datums = result.unwrap();
        assert_eq!(datums.len(), 4);

        // Verify each datum has the correct type
        assert_eq!(
            datums.get("docker.cli").unwrap().datum_type,
            Some(DatumType::Cli)
        );
        assert_eq!(
            datums.get("postgres.docker").unwrap().datum_type,
            Some(DatumType::Docker)
        );
        assert_eq!(
            datums.get("sequential.mcp").unwrap().datum_type,
            Some(DatumType::Mcp)
        );
        assert_eq!(
            datums.get("kubectl.cli").unwrap().datum_type,
            Some(DatumType::Cli)
        );
    }
}
