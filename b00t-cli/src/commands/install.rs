use crate::dependency_resolver::DependencyResolver;
use crate::{BootDatum, UnifiedConfig, evaluate_gates};
use anyhow::{Context, Result, anyhow};
use chrono::Utc;
use clap::Parser;
use duct::cmd;
use serde_json;
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

    // 🤓 Dogfood: checkpoint install state in the b00t knowledge store.
    checkpoint_installed(&workspace_root)?;

    Ok(())
}

/// Dogfood the b00t store: write install metadata so `b00t up --self` can
/// check whether a newer build exists.
pub(crate) fn checkpoint_installed(workspace_root: &str) -> Result<()> {
    let version = b00t_c0re_lib::version::VERSION.to_string();

    let commit = std::process::Command::new("git")
        .args(["rev-parse", "--short", "HEAD"])
        .current_dir(workspace_root)
        .output()
        .ok()
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
        .unwrap_or_else(|| "unknown".to_string());

    let json = serde_json::json!({
        "name": "b00t-cli",
        "version": version,
        "commit": commit,
        "installed_at": Utc::now().to_rfc3339(),
        "workspace": workspace_root,
    });

    let tmp = std::env::temp_dir().join("b00t-install-checkpoint.json");
    std::fs::write(&tmp, serde_json::to_string_pretty(&json)?)?;

    let mut tags = std::collections::BTreeMap::new();
    tags.insert("name".to_string(), "b00t-cli".to_string());
    tags.insert("version".to_string(), version.clone());

    match b00t_c0re_lib::store::put(&tmp, "b00t:InstalledBinary", "b00t-cli", &tags) {
        Ok(_) => eprintln!("  📦 store checkpointed: b00t-cli v{} ({})", version, commit),
        Err(e) => eprintln!("  ⚠️  store checkpoint failed (non-fatal): {}", e),
    }
    let _ = std::fs::remove_file(&tmp);
    Ok(())
}

/// Check the store for the last-installed version of a b00t binary.
pub fn last_installed_version(name: &str) -> Option<String> {
    let mut tags = std::collections::BTreeMap::new();
    tags.insert("name".to_string(), name.to_string());
    b00t_c0re_lib::store::query(&tags)
        .ok()?
        .last()
        .and_then(|e| e.tags.get("version").cloned())
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
            .find(|(_, datum)| datum.install_command_string().is_some())
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

        // Evaluate gates before installing
        if let Some(ref gates) = datum.gate {
            if !gates.is_empty() {
                let gate_results = evaluate_gates(gates, path);
                let all_passed = gate_results.iter().all(|r| r.passed);
                if !all_passed {
                    for result in &gate_results {
                        if !result.passed {
                            println!("⏭️  {} gate blocked: {}", key, result.reason);
                        }
                    }
                    continue;
                }
            }
        }

        if let Some(install_cmd) = datum.install_command_string() {
            println!("🚀 Installing {}...", key);
            cmd!("bash", "-c", &install_cmd)
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
        println!("🔍 Dry run: would configure hermes: hermes config set terminal.backend local");
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

// Canonical paths for hermes MCP config — resolved from $HOME at runtime.
// Tests duplicate these functions to verify round-trip correctness.
fn home_dir_str() -> String {
    std::env::var("HOME").unwrap_or_default()
}

fn hermes_b00t_mcp_command() -> String {
    format!("{}/.cargo/bin/b00t-mcp", home_dir_str())
}

fn hermes_b00t_mcp_args() -> Vec<String> {
    let home = home_dir_str();
    vec!["stdio".into(), "-d".into(), format!("{}/.b00t", home)]
}

fn codebase_memory_mcp_path() -> String {
    format!(
        "{}/.b00t/vendor/codebase-memory-mcp-b00t-ir0n-ledg3rr/build/c/codebase-memory-mcp",
        home_dir_str()
    )
}

/// Update (or create) the hermes `config.yaml` at `config_path` with canonical
/// b00t MCP server entries.
///
/// - Creates parent directories if needed.
/// - Parses the existing YAML if the file exists.
/// - Merges/overwrites the `b00t-mcp` entry with canonical `command` and `args`.
/// - Adds the `codebase-memory` entry if the binary exists on disk.
/// - Preserves all other top-level keys and unrelated `mcp_servers` entries.
/// - Returns `Err` if the parent cannot be created, YAML is unparseable, or
///   `mcp_servers` is not a mapping.
pub fn update_hermes_mcp_config(config_path: &std::path::Path) -> Result<()> {
    // Ensure parent directory exists.
    if let Some(parent) = config_path.parent() {
        std::fs::create_dir_all(parent)
            .with_context(|| format!("cannot create directory: {}", parent.display()))?;
    }

    // Load existing YAML or start with an empty mapping.
    let mut doc: serde_yaml::Mapping = if config_path.exists() {
        let raw = std::fs::read_to_string(config_path)
            .with_context(|| format!("cannot read {}", config_path.display()))?;
        if raw.trim().is_empty() {
            serde_yaml::Mapping::new()
        } else {
            serde_yaml::from_str::<serde_yaml::Mapping>(&raw)
                .with_context(|| format!("cannot parse YAML: {}", config_path.display()))?
        }
    } else {
        serde_yaml::Mapping::new()
    };

    // Get or create the mcp_servers mapping.
    let servers_key = serde_yaml::Value::String("mcp_servers".into());
    let servers = doc
        .entry(servers_key)
        .or_insert_with(|| serde_yaml::Value::Mapping(serde_yaml::Mapping::new()));
    let servers = servers
        .as_mapping_mut()
        .context("mcp_servers must be a mapping")?;

    // Build canonical b00t-mcp entry.
    let mut b00t_entry = serde_yaml::Mapping::new();
    b00t_entry.insert(
        serde_yaml::Value::String("command".into()),
        serde_yaml::Value::String(hermes_b00t_mcp_command()),
    );
    b00t_entry.insert(
        serde_yaml::Value::String("args".into()),
        serde_yaml::Value::Sequence(
            hermes_b00t_mcp_args()
                .into_iter()
                .map(serde_yaml::Value::String)
                .collect(),
        ),
    );
    servers.insert(
        serde_yaml::Value::String("b00t-mcp".into()),
        serde_yaml::Value::Mapping(b00t_entry),
    );

    // Add codebase-memory entry only when the binary is present on disk.
    let cm_path = codebase_memory_mcp_path();
    if std::path::Path::new(&cm_path).exists() {
        let mut cm_entry = serde_yaml::Mapping::new();
        cm_entry.insert(
            serde_yaml::Value::String("command".into()),
            serde_yaml::Value::String(cm_path),
        );
        cm_entry.insert(
            serde_yaml::Value::String("args".into()),
            serde_yaml::Value::Sequence(vec![]),
        );
        servers.insert(
            serde_yaml::Value::String("codebase-memory".into()),
            serde_yaml::Value::Mapping(cm_entry),
        );
    }

    // Write back.
    let yaml_out = serde_yaml::to_string(&doc).context("cannot serialize YAML")?;
    std::fs::write(config_path, yaml_out)
        .with_context(|| format!("cannot write {}", config_path.display()))?;

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
    use crate::{DatumType, GateSpec};
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

    // ── Gate evaluation tests ────────────────────────────────────────────────

    #[test]
    fn test_gate_command_fails() {
        // A gate requiring a non-existent command should fail
        let gates = vec![GateSpec {
            command: Some("this-command-definitely-does-not-exist-xyzzy".to_string()),
            file: None,
            env: None,
            rhai: None,
            knowledge_backend: None,
            hint: Some("test command gate".to_string()),
        }];
        let results = evaluate_gates(&gates, "/tmp");
        assert!(!results[0].passed);
        assert!(results[0].reason.contains("not found"));
    }

    #[test]
    fn test_gate_file_passes() {
        // A gate requiring an existing file should pass
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("test_file.txt");
        fs::write(&file_path, "test content").unwrap();

        let gates = vec![GateSpec {
            command: None,
            file: Some(file_path.to_str().unwrap().to_string()),
            env: None,
            rhai: None,
            knowledge_backend: None,
            hint: Some("test file gate".to_string()),
        }];
        let results = evaluate_gates(&gates, "/tmp");
        assert!(results[0].passed);
    }

    #[test]
    fn test_gate_env_fails() {
        // A gate requiring a non-existent env var should fail
        let gates = vec![GateSpec {
            command: None,
            file: None,
            env: Some("THIS_ENV_VAR_DOES_NOT_EXIST_12345".to_string()),
            rhai: None,
            knowledge_backend: None,
            hint: Some("test env gate".to_string()),
        }];
        let results = evaluate_gates(&gates, "/tmp");
        assert!(!results[0].passed);
        assert!(results[0].reason.contains("not set"));
    }

    #[test]
    fn test_gate_knowledge_backend_passes_for_compiled_backend() {
        let gates = vec![GateSpec {
            command: None,
            file: None,
            env: None,
            rhai: None,
            knowledge_backend: Some(b00t_c0re_lib::compiled_knowledge_backend().to_string()),
            hint: Some("knowledge backend gate".to_string()),
        }];
        let results = evaluate_gates(&gates, "/tmp");
        assert!(results[0].passed);
    }

    #[test]
    fn test_gate_knowledge_backend_fails_for_mismatch() {
        let active = b00t_c0re_lib::compiled_knowledge_backend();
        let mismatched = match active {
            "helixdb" => "oxigraph",
            "oxigraph" => "helixdb",
            "neumann" => "oxigraph",
            _ => "helixdb",
        };
        let gates = vec![GateSpec {
            command: None,
            file: None,
            env: None,
            rhai: None,
            knowledge_backend: Some(mismatched.to_string()),
            hint: Some("knowledge backend gate".to_string()),
        }];
        let results = evaluate_gates(&gates, "/tmp");
        assert!(!results[0].passed);
        assert!(results[0].reason.contains("compiled backend"));
    }

    #[test]
    fn test_multiple_gates_all_pass() {
        // Multiple gates that should all pass (file exists + env var that exists)
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("multi_test.txt");
        fs::write(&file_path, "test").unwrap();

        // PATH is always set
        let gates = vec![
            GateSpec {
                command: None,
                file: Some(file_path.to_str().unwrap().to_string()),
                env: None,
                rhai: None,
                knowledge_backend: None,
                hint: Some("file gate".to_string()),
            },
            GateSpec {
                command: None,
                file: None,
                env: Some("PATH".to_string()),
                rhai: None,
                knowledge_backend: None,
                hint: Some("env gate".to_string()),
            },
        ];
        let results = evaluate_gates(&gates, "/tmp");
        assert!(results.len() == 2);
        assert!(results[0].passed, "file gate should pass: {}", results[0].reason);
        assert!(results[1].passed, "env gate should pass: {}", results[1].reason);
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
