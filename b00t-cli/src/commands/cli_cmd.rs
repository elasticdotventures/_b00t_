use crate::datum_cli::CliDatum;
use crate::dependency_resolver::DependencyResolver;
use crate::load_datum_providers;
use crate::traits::*;
use crate::{BootDatum, UnifiedConfig};
use anyhow::{Context, Result};
use clap::Parser;
use duct::cmd;
use once_cell::sync::Lazy;
use shellexpand;
use std::collections::HashMap;
use std::path::PathBuf;

static IS_ROOT: Lazy<bool> = Lazy::new(|| {
    cmd!("id", "-u")
        .read()
        .map(|out| out.trim() == "0")
        .unwrap_or(false)
});

#[derive(Parser)]
pub enum CliCommands {
    #[clap(
        about = "Run a CLI script by name",
        long_about = "Run a CLI script by name.\n\nExamples:\n  b00t-cli cli run setup-dev\n  b00t-cli cli run deploy"
    )]
    Run {
        #[clap(help = "Script name to run")]
        script_name: String,
        #[clap(
            help = "Arguments to pass to the script",
            raw = true,
            trailing_var_arg = true
        )]
        args: Vec<String>,
    },
    #[clap(about = "Detect the installed version of a CLI command")]
    Detect {
        #[clap(help = "Command name to detect version for")]
        command: String,
    },
    #[clap(about = "Show the desired version of a CLI command")]
    Desires {
        #[clap(help = "Command name to show desired version for")]
        command: String,
    },
    #[clap(about = "Install a CLI command")]
    Install {
        #[clap(help = "Command name to install")]
        command: String,
    },
    #[clap(about = "Update a CLI command")]
    Update {
        #[clap(help = "Command name to update")]
        command: String,
    },
    #[clap(about = "Check installed vs desired versions for CLI command")]
    Check {
        #[clap(help = "Command name to check")]
        command: String,
    },
    #[clap(
        about = "Check all CLI commands for updates",
        long_about = "Check all CLI commands for updates. By default, only reports which tools need updating.\n\nUse --yes to actually perform the updates.\n\nExamples:\n  b00t-cli cli up          # Check versions only\n  b00t-cli cli up --yes    # Update outdated tools\n  b00t-cli cli up -y       # Same as --yes"
    )]
    Up {
        #[clap(
            short = 'y',
            long = "yes",
            help = "Actually perform updates (default: check only)"
        )]
        yes: bool,
    },
}

impl CliCommands {
    pub fn execute(&self, path: &str) -> Result<()> {
        match self {
            CliCommands::Run { .. } => {
                println!("🚀 CLI run functionality coming soon...");
                Ok(())
            }
            CliCommands::Detect { command } => cli_detect(command, path),
            CliCommands::Desires { command } => cli_desires(command, path),
            CliCommands::Install { command } => cli_install(command, path),
            CliCommands::Update { command } => cli_update(command, path),
            CliCommands::Check { command } => cli_check(command, path),
            CliCommands::Up { yes } => cli_up(path, *yes),
        }
    }
}

fn cli_detect(command: &str, path: &str) -> Result<()> {
    let cli_datum = CliDatum::from_config(command, path)?;
    match cli_datum.current_version() {
        Some(version) => {
            println!("{}", version);
            Ok(())
        }
        None => {
            anyhow::bail!("Could not detect version for {}", command);
        }
    }
}

fn cli_desires(command: &str, path: &str) -> Result<()> {
    let cli_datum = CliDatum::from_config(command, path)?;
    match cli_datum.desired_version() {
        Some(version) => {
            println!("{}", version);
            Ok(())
        }
        None => {
            anyhow::bail!("No desired version specified for {}", command);
        }
    }
}

fn cli_install(command: &str, path: &str) -> Result<()> {
    let cli_datum = CliDatum::from_config(command, path)?;

    // Check if datum has dependencies
    if let Some(deps) = &cli_datum.datum.depends_on {
        if !deps.is_empty() {
            println!("📦 Resolving dependencies for {}...", command);

            // Load all available datums for dependency resolution
            let all_datums = load_all_datums(path)?;

            // Build dependency resolver
            let datum_refs: Vec<&BootDatum> = all_datums.values().collect();
            let resolver = DependencyResolver::new(datum_refs);

            // Resolve installation order (includes command itself and all deps)
            let datum_key = format!("{}.cli", command);
            let install_order = resolver
                .resolve(&datum_key)
                .context(format!("Failed to resolve dependencies for {}", command))?;

            println!("📋 Installation order ({} items):", install_order.len());
            for (idx, item) in install_order.iter().enumerate() {
                println!("   {}. {}", idx + 1, item);
            }
            println!();

            // Install each datum in dependency order
            for datum_key in &install_order {
                // Skip if already installed (check for the datum)
                if let Some(datum) = all_datums.get(datum_key) {
                    // Check if installed
                    if let Some(version_cmd) = &datum.version {
                        if cmd!("bash", "-c", version_cmd).read().is_ok() {
                            println!("✅ {} already installed, skipping", datum_key);
                            continue;
                        }
                    }

                    // Install this datum
                    if let Some(install_cmd) = &datum.install {
                        println!("🚀 Installing {}...", datum_key);
                        let result = cmd!("bash", "-c", install_cmd).run();
                        match result {
                            Ok(_) => {
                                println!("✅ Successfully installed {}", datum_key);
                            }
                            Err(e) => {
                                anyhow::bail!("Failed to install {}: {}", datum_key, e);
                            }
                        }
                    } else {
                        println!("⚠️ No install command for {}, skipping", datum_key);
                    }
                } else {
                    anyhow::bail!("Datum not found in registry: {}", datum_key);
                }
            }

            return Ok(());
        }
    }

    // No dependencies - install directly
    if let Some(install_cmd) = &cli_datum.datum.install {
        println!("🚀 Installing {}...", command);
        let result = cmd!("bash", "-c", install_cmd).run();
        match result {
            Ok(_) => {
                println!("✅ Successfully installed {}", command);
                Ok(())
            }
            Err(e) => {
                anyhow::bail!("Failed to install {}: {}", command, e);
            }
        }
    } else {
        anyhow::bail!("No install command specified for {}", command);
    }
}

/// Load all datums from _b00t_ directory for dependency resolution
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
                // Skip stack files
                if file_name.ends_with(".stack.toml") {
                    continue;
                }

                // Load other datum types
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

fn cli_update(command: &str, path: &str) -> Result<()> {
    let cli_datum = CliDatum::from_config(command, path)?;

    // Try update command first, fall back to install command
    let update_cmd = cli_datum
        .datum
        .update
        .as_ref()
        .or(cli_datum.datum.install.as_ref());

    if let Some(cmd_str) = update_cmd {
        println!("🔄 Updating {}...", command);
        let result = cmd!("bash", "-c", cmd_str).run();
        match result {
            Ok(_) => {
                println!("✅ Successfully updated {}", command);
                Ok(())
            }
            Err(e) => {
                anyhow::bail!("Failed to update {}: {}", command, e);
            }
        }
    } else {
        anyhow::bail!("No update or install command specified for {}", command);
    }
}

fn cli_check(command: &str, path: &str) -> Result<()> {
    let cli_datum = CliDatum::from_config(command, path)?;
    let version_status = cli_datum.version_status();
    let current = cli_datum
        .current_version()
        .unwrap_or_else(|| "not found".to_string());

    let status_text = match version_status {
        VersionStatus::Match => format!("🥾👍🏻 {} {} (matches desired)", command, current),
        VersionStatus::Newer => format!("🥾🐣 {} {} (newer than desired)", command, current),
        VersionStatus::Older => format!("🥾😭 {} {} (older than desired)", command, current),
        VersionStatus::Missing => format!("🥾😱 {} (not installed)", command),
        VersionStatus::Unknown => format!(
            "🥾⏹️ {} {} (version comparison unavailable)",
            command, current
        ),
    };

    println!("{}", status_text);

    // Set exit code based on status
    match version_status {
        VersionStatus::Match | VersionStatus::Newer | VersionStatus::Unknown => Ok(()),
        VersionStatus::Older => std::process::exit(1),
        VersionStatus::Missing => std::process::exit(2),
    }
}

fn cli_up(path: &str, yes: bool) -> Result<()> {
    if yes {
        println!("🔄 Checking and updating all CLI commands...");
    } else {
        println!("🔍 Checking all CLI commands (use --yes to update)...");
    }

    // Load all CLI datum providers
    let cli_tools: Vec<Box<dyn DatumProvider>> =
        load_datum_providers::<CliDatum>(path, ".cli.toml")?;

    let mut updated_count = 0;
    let mut needs_update_count = 0;
    let mut total_count = 0;

    for tool in cli_tools {
        total_count += 1;
        let name = tool.name();
        let version_status = tool.version_status();
        let current = tool
            .current_version()
            .unwrap_or_else(|| "not found".to_string());
        let desired = tool
            .desired_version()
            .unwrap_or_else(|| "unknown".to_string());

        let datum = tool.datum();

        if datum.auto_install.map(|v| !v).unwrap_or(false) {
            println!(
                "⏭️ {} (auto_install=false) - skipping install/update in cli up",
                name
            );
            continue;
        }

        if datum.requires_sudo && !*IS_ROOT {
            println!(
                "⏭️ {} (requires sudo) - run b00t cli up with sudo to install/update",
                name
            );
            continue;
        }

        match version_status {
            VersionStatus::Older | VersionStatus::Missing => {
                needs_update_count += 1;
                if yes {
                    println!("📦 Updating {}...", name);
                    if let Ok(cli_datum) = CliDatum::from_config(name, path) {
                        let update_cmd = cli_datum
                            .datum
                            .update
                            .as_ref()
                            .or(cli_datum.datum.install.as_ref());

                        if let Some(cmd_str) = update_cmd {
                            match cmd!("bash", "-c", cmd_str).run() {
                                Ok(_) => {
                                    println!("✅ Updated {}", name);
                                    updated_count += 1;
                                }
                                Err(e) => {
                                    eprintln!("❌ Failed to update {}: {}", name, e);
                                }
                            }
                        } else {
                            eprintln!("⚠️ No update command for {}", name);
                        }
                    }
                } else {
                    if version_status == VersionStatus::Missing {
                        println!("🥾😱 {} (not installed) -> desires: {}", name, desired);
                    } else {
                        println!("🥾😭 {} (current: {}, desires: {})", name, current, desired);
                    }
                }
            }
            VersionStatus::Match => {
                println!("🥾👍🏻 {} {} (up to date)", name, current);
            }
            VersionStatus::Newer => {
                println!(
                    "🥾🐣 {} {} (newer than desired: {})",
                    name, current, desired
                );
            }
            VersionStatus::Unknown => {
                println!("🥾⏹️ {} {} (version status unknown)", name, current);
            }
        }
    }

    if yes {
        println!(
            "🏁 Updated {} of {} CLI commands",
            updated_count, total_count
        );
    } else {
        if needs_update_count > 0 {
            println!(
                "\n💡 {} of {} commands need updates. Run 'b00t cli up --yes' to update them.",
                needs_update_count, total_count
            );
        } else {
            println!("\n🎉 All {} CLI commands are up to date!", total_count);
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cli_commands_exist() {
        let run_cmd = CliCommands::Run {
            script_name: "test-script".to_string(),
            args: vec![],
        };

        assert!(run_cmd.execute("test").is_ok());
    }

    #[test]
    fn test_all_cli_commands_have_variants() {
        // Test that all expected CLI command variants exist
        let _detect = CliCommands::Detect {
            command: "test".to_string(),
        };
        let _desires = CliCommands::Desires {
            command: "test".to_string(),
        };
        let _install = CliCommands::Install {
            command: "test".to_string(),
        };
        let _update = CliCommands::Update {
            command: "test".to_string(),
        };
        let _check = CliCommands::Check {
            command: "test".to_string(),
        };
        let _up = CliCommands::Up { yes: false };
        let _up_yes = CliCommands::Up { yes: true };
        let _run = CliCommands::Run {
            script_name: "test".to_string(),
            args: vec![],
        };

        // If we got here, all variants exist
        assert!(true);
    }
}
