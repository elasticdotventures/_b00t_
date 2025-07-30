use anyhow::Result;
use clap::Parser;
use crate::datum_cli::CliDatum;
use crate::traits::*;
use crate::{get_expanded_path, load_datum_providers};
use duct::cmd;
// use std::fs;

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
    #[clap(about = "Update all CLI commands")]
    Up,
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
            CliCommands::Up => cli_up(path),
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

fn cli_update(command: &str, path: &str) -> Result<()> {
    let cli_datum = CliDatum::from_config(command, path)?;

    // Try update command first, fall back to install command
    let update_cmd = cli_datum.datum.update.as_ref()
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
    let current = cli_datum.current_version().unwrap_or_else(|| "not found".to_string());

    let status_text = match version_status {
        VersionStatus::Match => format!("🥾👍🏻 {} {} (matches desired)", command, current),
        VersionStatus::Newer => format!("🥾🐣 {} {} (newer than desired)", command, current),
        VersionStatus::Older => format!("🥾😭 {} {} (older than desired)", command, current),
        VersionStatus::Missing => format!("🥾😱 {} (not installed)", command),
        VersionStatus::Unknown => format!("🥾⏹️ {} {} (version comparison unavailable)", command, current),
    };

    println!("{}", status_text);

    // Set exit code based on status
    match version_status {
        VersionStatus::Match | VersionStatus::Newer | VersionStatus::Unknown => Ok(()),
        VersionStatus::Older => std::process::exit(1),
        VersionStatus::Missing => std::process::exit(2),
    }
}

fn cli_up(path: &str) -> Result<()> {
    println!("🔄 Checking all CLI commands for updates...");

    // Load all CLI datum providers
    let cli_tools: Vec<Box<dyn DatumProvider>> = load_datum_providers::<CliDatum>(path, ".cli.toml")?;

    let mut updated_count = 0;
    let mut total_count = 0;

    for tool in cli_tools {
        total_count += 1;
        let name = tool.name();
        let version_status = tool.version_status();

        match version_status {
            VersionStatus::Older | VersionStatus::Missing => {
                println!("📦 Updating {}...", name);
                if let Ok(cli_datum) = CliDatum::from_config(name, path) {
                    let update_cmd = cli_datum.datum.update.as_ref()
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
            }
            VersionStatus::Match => {
                println!("✅ {} is up to date", name);
            }
            VersionStatus::Newer => {
                println!("🐣 {} is newer than desired", name);
            }
            VersionStatus::Unknown => {
                println!("⏹️ {} version status unknown", name);
            }
        }
    }

    println!("🏁 Updated {} of {} CLI commands", updated_count, total_count);
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
        let _detect = CliCommands::Detect { command: "test".to_string() };
        let _desires = CliCommands::Desires { command: "test".to_string() };
        let _install = CliCommands::Install { command: "test".to_string() };
        let _update = CliCommands::Update { command: "test".to_string() };
        let _check = CliCommands::Check { command: "test".to_string() };
        let _up = CliCommands::Up;
        let _run = CliCommands::Run {
            script_name: "test".to_string(),
            args: vec![]
        };

        // If we got here, all variants exist
        assert!(true);
    }
}