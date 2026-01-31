use crate::{BootDatum, UnifiedConfig};
use crate::dependency_resolver::DependencyResolver;
use anyhow::{anyhow, Context, Result};
use clap::Parser;
use duct::cmd;
use std::collections::HashMap;
use std::path::PathBuf;
use shellexpand;
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
pub fn install_datum(path: &str, name: &str) -> Result<()> {
    let all_datums = load_all_datums(path)?;

    // Find matching datum key (supports suffix or plain name)
    let mut target_key: Option<String> = None;
    for (key, datum) in &all_datums {
        if datum.name == name || key == name {
            target_key = Some(key.clone());
            break;
        }
    }

    let target_key = target_key.ok_or_else(|| anyhow!("Datum '{}' not found", name))?;

    // Resolve dependencies using datum graph
    let datum_refs: Vec<&BootDatum> = all_datums.values().collect();
    let resolver = DependencyResolver::new(datum_refs);
    let install_order = resolver
        .resolve(&target_key)
        .context(format!("Failed to resolve dependencies for {}", name))?;

    println!("📋 Installation order ({} items):", install_order.len());
    for (idx, item) in install_order.iter().enumerate() {
        println!("   {}. {}", idx + 1, item);
    }
    println!();

    for key in install_order {
        let datum = all_datums
            .get(&key)
            .ok_or_else(|| anyhow!("Missing datum during install: {}", key))?;

        // Skip if already installed (best-effort using version command)
        if let Some(version_cmd) = &datum.version {
            match cmd!("bash", "-c", version_cmd).run() {
                Ok(_) => {
                    println!("✅ {} already installed, skipping", key);
                    continue;
                }
                Err(e) => {
                    eprintln!("⚠️  Version check for '{}' failed: {}. Proceeding with installation.", key, e);
                }
            }
        }

        if let Some(install_cmd) = &datum.install {
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

    #[test]
    fn test_install_dry_run() {
        let result = run_just_install(true);
        assert!(result.is_ok());
    }
}
