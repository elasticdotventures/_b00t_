use anyhow::Result;
use crate::datum_ai::AiDatum;
use crate::datum_cli::CliDatum;
use crate::datum_config::B00tConfig;
use crate::datum_docker::DockerDatum;
use crate::datum_mcp::McpDatum;
use crate::load_datum_providers;
use crate::traits::{DatumProvider, VersionStatus};
use crate::DatumType;
use duct::cmd;

/// Handle the `b00t up` command - check and optionally update all datums from _b00t_.toml
pub fn handle_up_command(b00t_path: &str, yes: bool) -> Result<()> {
    // Load or create configuration
    let (mut config, config_path) = B00tConfig::load_or_create()?;

    if yes {
        println!("🔄 Updating all datums from {}...", config_path.display());
    } else {
        println!("🔍 Checking all datums from {} (use --yes to update)...", config_path.display());
    }

    // If config file doesn't exist yet, show helpful message
    if !config_path.exists() {
        println!("\n⚠️  No _b00t_.toml found at {}", config_path.display());
        println!("   Create one to track your installed datums:\n");
        println!("   Example _b00t_.toml:");
        println!("   ---");
        println!("   version = \"{}\"", b00t_c0re_lib::version::VERSION);
        println!("   initialized = \"{}\"", chrono::Utc::now().to_rfc3339());
        println!("   install_methods = [\"docker\", \"pkgx\", \"apt\", \"curl\"]");
        println!("   datums = [");
        println!("     \"git.cli\",");
        println!("     \"docker.docker\",");
        println!("     \"rust.*\",    # All rust-related datums");
        println!("     \"ai.*\",      # All AI providers");
        println!("   ]");
        println!("   ---\n");
        println!("💡 Run `b00t install <datum>` to auto-create and update this file.");
        return Ok(());
    }

    // Pre-load all datum providers by type - work entirely within lib.rs trait system
    let cli_tools = crate::load_datum_providers::<CliDatum>(b00t_path, ".cli.toml")?;
    let mcp_tools = crate::load_datum_providers::<McpDatum>(b00t_path, ".mcp.toml")?;
    let docker_tools = crate::load_datum_providers::<DockerDatum>(b00t_path, ".docker.toml")?;
    let ai_tools = crate::load_datum_providers::<AiDatum>(b00t_path, ".ai.toml")?;

    // Build all_datums list from loaded providers
    let mut all_datums: Vec<(String, DatumType)> = Vec::new();

    for tool in &cli_tools {
        all_datums.push((tool.name().to_string(), DatumType::Cli));
    }
    for tool in &mcp_tools {
        all_datums.push((tool.name().to_string(), DatumType::Mcp));
    }
    for tool in &docker_tools {
        all_datums.push((tool.name().to_string(), DatumType::Docker));
    }
    for tool in &ai_tools {
        all_datums.push((tool.name().to_string(), DatumType::Ai));
    }

    // Get datums matching the configured patterns
    let matching_datums = config.get_matching_datums(&all_datums);

    if matching_datums.is_empty() {
        println!("\n⚠️  No datums match the configured patterns");
        println!("   Configured patterns: {:?}", config.datums);
        println!("   Available datums: {} total", all_datums.len());
        for (name, dtype) in &all_datums {
            println!("     - {}.{}", name, B00tConfig::datum_type_str(dtype));
        }
        return Ok(());
    }

    let mut needs_update_count = 0;
    let mut updated_count = 0;
    let total_count = matching_datums.len();

    println!("\n📋 Found {} matching datums:\n", total_count);

    // Process each matching datum
    for datum_spec in &matching_datums {
        let parts: Vec<&str> = datum_spec.split('.').collect();
        if parts.len() != 2 {
            continue;
        }

        let (name, dtype_str) = (parts[0], parts[1]);

        // Find the appropriate provider based on type (work entirely with trait objects)
        let provider: Option<&Box<dyn DatumProvider>> = match dtype_str {
            "cli" => cli_tools.iter().find(|t| t.name() == name),
            "mcp" => mcp_tools.iter().find(|t| t.name() == name),
            "docker" => docker_tools.iter().find(|t| t.name() == name),
            "ai" => ai_tools.iter().find(|t| t.name() == name),
            _ => None,
        };

        if let Some(tool) = provider {
            let version_status = tool.version_status();
            let current = tool.current_version().unwrap_or_else(|| "not found".to_string());
            let desired = tool.desired_version().unwrap_or_else(|| "unknown".to_string());

            match version_status {
                VersionStatus::Older | VersionStatus::Missing => {
                    needs_update_count += 1;
                    if yes {
                        println!("📦 Updating {}...", datum_spec);
                        let datum = tool.datum();
                        let update_cmd = datum.update.as_ref().or(datum.install.as_ref());

                        if let Some(cmd_str) = update_cmd {
                            match cmd!("bash", "-c", cmd_str).run() {
                                Ok(_) => {
                                    println!("✅ Updated {}", datum_spec);
                                    updated_count += 1;
                                    config.add_history(
                                        datum_spec.clone(),
                                        Some(desired.clone()),
                                        config.install_methods[0].clone(),
                                        Some("success".to_string()),
                                    );
                                }
                                Err(e) => {
                                    eprintln!("❌ Failed to update {}: {}", datum_spec, e);
                                    config.add_history(
                                        datum_spec.clone(),
                                        None,
                                        config.install_methods[0].clone(),
                                        Some("failed".to_string()),
                                    );
                                }
                            }
                        } else {
                            eprintln!("⚠️  No update command for {}", datum_spec);
                        }
                    } else {
                        if version_status == VersionStatus::Missing {
                            println!("🥾😱 {} (not installed) -> desires: {}", datum_spec, desired);
                        } else {
                            println!("🥾😭 {} (current: {}, desires: {})", datum_spec, current, desired);
                        }
                    }
                }
                VersionStatus::Match => {
                    println!("🥾👍🏻 {} {} (up to date)", datum_spec, current);
                }
                VersionStatus::Newer => {
                    println!("🥾🐣 {} {} (newer than desired: {})", datum_spec, current, desired);
                }
                VersionStatus::Unknown => {
                    println!("🥾⏹️  {} {} (version status unknown)", datum_spec, current);
                }
            }
        } else {
            println!("⚠️  {} not found", datum_spec);
        }
    }

    if yes {
        println!("\n🏁 Updated {} of {} datums", updated_count, total_count);
        // Save updated config with history
        if updated_count > 0 || !config.history.is_empty() {
            config.save(&config_path)?;
            println!("💾 Saved history to {}", config_path.display());
        }
    } else {
        if needs_update_count > 0 {
            println!("\n💡 {} of {} datums need updates. Run 'b00t up --yes' to update them.", needs_update_count, total_count);
        } else {
            println!("\n🎉 All {} datums are up to date!", total_count);
        }
    }

    Ok(())
}
