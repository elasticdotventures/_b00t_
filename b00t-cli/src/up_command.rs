use anyhow::Result;
use crate::datum_ai::AiDatum;
use crate::datum_cli::CliDatum;
use crate::datum_config::B00tConfig;
use crate::datum_docker::DockerDatum;
use crate::datum_mcp::McpDatum;
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
    let cli_tools = load_cli_providers(b00t_path)?;
    let mcp_tools = load_mcp_providers(b00t_path)?;
    let docker_tools = load_docker_providers(b00t_path)?;
    let ai_tools = load_ai_providers(b00t_path)?;

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

/// Load CLI datum providers
fn load_cli_providers(b00t_path: &str) -> Result<Vec<Box<dyn DatumProvider>>> {
    let mut tools: Vec<Box<dyn DatumProvider>> = Vec::new();
    let expanded_path = shellexpand::tilde(b00t_path).to_string();
    let b00t_dir = std::path::PathBuf::from(&expanded_path);

    if let Ok(entries) = std::fs::read_dir(&b00t_dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if let Some(file_name) = path.file_name() {
                let name_str = file_name.to_string_lossy();
                if name_str.ends_with(".cli.toml") {
                    let name = name_str.trim_end_matches(".cli.toml");
                    if let Ok(datum) = CliDatum::from_config(name, b00t_path) {
                        tools.push(Box::new(datum));
                    }
                }
            }
        }
    }

    Ok(tools)
}

/// Load MCP datum providers
fn load_mcp_providers(b00t_path: &str) -> Result<Vec<Box<dyn DatumProvider>>> {
    let mut tools: Vec<Box<dyn DatumProvider>> = Vec::new();
    let expanded_path = shellexpand::tilde(b00t_path).to_string();
    let b00t_dir = std::path::PathBuf::from(&expanded_path);

    if let Ok(entries) = std::fs::read_dir(&b00t_dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if let Some(file_name) = path.file_name() {
                let name_str = file_name.to_string_lossy();
                if name_str.ends_with(".mcp.toml") {
                    let name = name_str.trim_end_matches(".mcp.toml");
                    if let Ok(datum) = McpDatum::from_config(name, b00t_path) {
                        tools.push(Box::new(datum));
                    }
                }
            }
        }
    }

    Ok(tools)
}

/// Load Docker datum providers
fn load_docker_providers(b00t_path: &str) -> Result<Vec<Box<dyn DatumProvider>>> {
    let mut tools: Vec<Box<dyn DatumProvider>> = Vec::new();
    let expanded_path = shellexpand::tilde(b00t_path).to_string();
    let b00t_dir = std::path::PathBuf::from(&expanded_path);

    if let Ok(entries) = std::fs::read_dir(&b00t_dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if let Some(file_name) = path.file_name() {
                let name_str = file_name.to_string_lossy();
                if name_str.ends_with(".docker.toml") {
                    let name = name_str.trim_end_matches(".docker.toml");
                    if let Ok(datum) = DockerDatum::from_config(name, b00t_path) {
                        tools.push(Box::new(datum));
                    }
                }
            }
        }
    }

    Ok(tools)
}

/// Load AI datum providers
fn load_ai_providers(b00t_path: &str) -> Result<Vec<Box<dyn DatumProvider>>> {
    let mut tools: Vec<Box<dyn DatumProvider>> = Vec::new();
    let expanded_path = shellexpand::tilde(b00t_path).to_string();
    let b00t_dir = std::path::PathBuf::from(&expanded_path);

    if let Ok(entries) = std::fs::read_dir(&b00t_dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if let Some(file_name) = path.file_name() {
                let name_str = file_name.to_string_lossy();
                if name_str.ends_with(".ai.toml") {
                    let name = name_str.trim_end_matches(".ai.toml");
                    if let Ok(datum) = AiDatum::try_from((name, b00t_path as &str)) {
                        tools.push(Box::new(datum));
                    }
                }
            }
        }
    }

    Ok(tools)
}
