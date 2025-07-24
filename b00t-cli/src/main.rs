
use clap::Parser;
use std::path::PathBuf;
use std::fs;
use std::io::{self, Read};
use regex::Regex;
use duct::cmd;
use semver::Version;
use anyhow::{Result, Context};
use b00t_cli::{normalize_mcp_json, McpListOutput, McpListItem, UnifiedConfig, BootPackage, PackageType, create_unified_toml_config};

mod integration_tests;

#[derive(Parser)]
#[clap(version, about, long_about = None)]
struct Cli {
    #[clap(subcommand)]
    command: Commands,
    #[clap(short, long, default_value = "~/.dotfiles/_b00t_")]
    path: String,
}

#[derive(Parser)]
enum Commands {
    #[clap(about = "MCP (Model Context Protocol) server management")]
    Mcp {
        #[clap(subcommand)]
        mcp_command: McpCommands,
    },
    #[clap(about = "VSCode integration")]
    Vscode {
        #[clap(subcommand)]
        vscode_command: VscodeCommands,
    },
    #[clap(about = "Claude Code integration")]
    ClaudeCode {
        #[clap(subcommand)]
        claude_command: ClaudeCodeCommands,
    },
    #[clap(about = "CLI script management")]
    Cli {
        #[clap(subcommand)]
        cli_command: CliCommands,
    },
}

#[derive(Parser)]
enum McpCommands {
    #[clap(about = "Add MCP server configuration")]
    Add {
        #[clap(help = "MCP server JSON configuration")]
        json: String,
        #[clap(long, help = "Do What I Want - auto-cleanup and format JSON (default: enabled)")]
        dwiw: bool,
        #[clap(long, help = "Disable auto-cleanup and format JSON", conflicts_with = "dwiw")]
        no_dwiw: bool,
    },
    #[clap(about = "List available MCP server configurations")]
    List {
        #[clap(long, help = "Output in JSON format")]
        json: bool,
    },
    #[clap(about = "Install MCP server to a target (claudecode, vscode)")]
    Install {
        #[clap(help = "MCP server name")]
        name: String,
        #[clap(help = "Installation target: claudecode, vscode")]
        target: String,
    },
}

#[derive(Parser)]
enum VscodeCommands {
    #[clap(about = "Install MCP server to VSCode")]
    Install {
        #[clap(subcommand)]
        install_command: VscodeInstallCommands,
    },
}

#[derive(Parser)]
enum VscodeInstallCommands {
    #[clap(about = "Install MCP server by name")]
    Mcp {
        #[clap(help = "Name of the MCP server to install")]
        name: String,
    },
}

#[derive(Parser)]
enum ClaudeCodeCommands {
    #[clap(about = "Install MCP server to Claude Code")]
    Install {
        #[clap(subcommand)]
        install_command: ClaudeCodeInstallCommands,
    },
}

#[derive(Parser)]
enum ClaudeCodeInstallCommands {
    #[clap(about = "Install MCP server by name")]
    Mcp {
        #[clap(help = "Name of the MCP server to install")]
        name: String,
    },
}

#[derive(Parser)]
enum CliCommands {
    #[clap(about = "Run a CLI script by name")]
    Run {
        #[clap(help = "Name of the CLI script to run")]
        name: String,
    },
    #[clap(about = "Detect the version of a CLI command")]
    Detect {
        #[clap(help = "The command to detect")]
        command: String,
    },
    #[clap(about = "Show the desired version of a CLI command")]
    Desires {
        #[clap(help = "The command to check")]
        command: String,
    },
    #[clap(about = "Install a CLI command")]
    Install {
        #[clap(help = "The command to install")]
        command: String,
    },
    #[clap(about = "Update a CLI command")]
    Update {
        #[clap(help = "The command to update")]
        command: String,
    },
    #[clap(about = "Check installed vs desired versions for CLI command")]
    Check {
        #[clap(help = "The command to check")]
        command: String,
    },
    #[clap(about = "Update all CLI commands")]
    Up,
}

// Using unified config from lib.rs
type Config = UnifiedConfig;


fn main() {
    let cli = Cli::parse();

    match &cli.command {
        Commands::Mcp { mcp_command } => match mcp_command {
            McpCommands::Add { json, dwiw: _, no_dwiw } => {
                // Default to true for dwiw behavior, unless explicitly disabled
                let use_dwiw = if *no_dwiw { false } else { true };
                if let Err(e) = mcp_add(json, use_dwiw, &cli.path) {
                    eprintln!("Error adding MCP server: {}", e);
                    std::process::exit(1);
                }
            }
            McpCommands::List { json } => {
                if let Err(e) = mcp_list(&cli.path, *json) {
                    eprintln!("Error listing MCP servers: {}", e);
                    std::process::exit(1);
                }
            }
            McpCommands::Install { name, target } => {
                let result = match target.as_str() {
                    "claudecode" => claude_code_install_mcp(name, &cli.path),
                    "vscode" => vscode_install_mcp(name, &cli.path),
                    _ => {
                        eprintln!("Error: Invalid target '{}'. Valid targets are: claudecode, vscode", target);
                        std::process::exit(1);
                    }
                };
                if let Err(e) = result {
                    eprintln!("Error installing MCP server to {}: {}", target, e);
                    std::process::exit(1);
                }
            }
        },
        Commands::Vscode { vscode_command } => match vscode_command {
            VscodeCommands::Install { install_command } => match install_command {
                VscodeInstallCommands::Mcp { name } => {
                    if let Err(e) = vscode_install_mcp(name, &cli.path) {
                        eprintln!("Error installing MCP server to VSCode: {}", e);
                        std::process::exit(1);
                    }
                }
            }
        },
        Commands::ClaudeCode { claude_command } => match claude_command {
            ClaudeCodeCommands::Install { install_command } => match install_command {
                ClaudeCodeInstallCommands::Mcp { name } => {
                    if let Err(e) = claude_code_install_mcp(name, &cli.path) {
                        eprintln!("Error installing MCP server to Claude Code: {}", e);
                        std::process::exit(1);
                    }
                }
            }
        },
        Commands::Cli { cli_command } => match cli_command {
            CliCommands::Run { name } => {
                if let Err(e) = cli_run(name, &cli.path) {
                    eprintln!("Error running CLI script: {}", e);
                    std::process::exit(1);
                }
            }
            CliCommands::Detect { command } => cli_detect(command, &cli.path),
            CliCommands::Desires { command } => cli_desires(command, &cli.path),
            CliCommands::Install { command } => cli_install(command, &cli.path),
            CliCommands::Update { command } => cli_update(command, &cli.path),
            CliCommands::Check { command } => cli_check(command, &cli.path),
            CliCommands::Up => cli_up(&cli.path),
        }
    }
}

#[allow(dead_code)]
fn get_config(command: &str, path: &str) -> Result<(Config, String), Box<dyn std::error::Error>> {
    // Try different file extensions in order of preference
    let extensions = [".cli.toml", ".mcp.toml", ".vscode.toml", ".docker.toml", ".apt.toml", ".nix.toml", ".bash.toml", ".toml"];
    
    let mut path_buf = PathBuf::new();
    path_buf.push(shellexpand::tilde(path).to_string());
    
    for ext in &extensions {
        let filename = format!("{}{}", command, ext);
        path_buf.set_file_name(&filename);
        if path_buf.exists() {
            let content = fs::read_to_string(&path_buf)?;
            let config: Config = toml::from_str(&content)?;
            return Ok((config, filename));
        }
    }
    
    eprintln!("{} UNDEFINED", command);
    std::process::exit(100);
}

fn get_cli_unified_config(command: &str, path: &str) -> Result<(Config, String), Box<dyn std::error::Error>> {
    // Only look for CLI-specific files
    let extensions = [".cli.toml"];
    
    let base_path = PathBuf::from(shellexpand::tilde(path).to_string());
    
    for ext in &extensions {
        let filename = format!("{}{}", command, ext);
        let full_path = base_path.join(&filename);
        if full_path.exists() {
            let content = fs::read_to_string(&full_path)?;
            let config: Config = toml::from_str(&content)?;
            return Ok((config, filename));
        }
    }
    
    eprintln!("CLI package {} UNDEFINED", command);
    std::process::exit(100);
}

#[allow(dead_code)]
fn detect(command: &str, path: &str) {
    if let Some(version) = get_installed_version(command, path) {
        println!("{}", version);
    } else {
        eprintln!("🥾😱 {} MISSING", command);
        std::process::exit(2);
    }
}

#[allow(dead_code)]
fn desires(command: &str, path: &str) {
    match get_config(command, path) {
        Ok((config, _)) => {
            println!("{}", config.b00t.desires.as_deref().unwrap_or("N/A"));
        }
        Err(e) => eprintln!("Error reading config for {}: {}", command, e),
    }
}

#[allow(dead_code)]
fn install(command: &str, path: &str) {
    match get_config(command, path) {
        Ok((config, filename)) => {
            match config.b00t.get_package_type(Some(&filename)) {
                PackageType::Traditional => {
                    if let Some(install_cmd) = &config.b00t.install {
                        println!("Installing {}...", command);
                        let result = cmd!("bash", "-c", install_cmd).run();
                        match result {
                            Ok(_) => println!("Installation complete."),
                            Err(e) => eprintln!("Installation failed: {}", e),
                        }
                    } else {
                        eprintln!("No install command defined for {}", command);
                    }
                }
                PackageType::Mcp => {
                    eprintln!("Use 'b00t-cli vscode install mcp {}' or 'b00t-cli claude-code install mcp {}' instead", command, command);
                }
                _ => {
                    eprintln!("Installation not yet supported for {:?} packages", config.b00t.get_package_type(Some(&filename)));
                }
            }
        }
        Err(e) => eprintln!("Error reading config for {}: {}", command, e),
    }
}

#[allow(dead_code)]
fn update(command: &str, path: &str) {
    match get_config(command, path) {
        Ok((config, filename)) => {
            match config.b00t.get_package_type(Some(&filename)) {
                PackageType::Traditional => {
                    if let Some(update_cmd) = config.b00t.update.as_ref().or(config.b00t.install.as_ref()) {
                        println!("Updating {}...", command);
                        let result = cmd!("bash", "-c", update_cmd).run();
                        match result {
                            Ok(_) => println!("Update complete."),
                            Err(e) => eprintln!("Update failed: {}", e),
                        }
                    } else {
                        eprintln!("No update or install command defined for {}", command);
                    }
                }
                _ => {
                    eprintln!("Update not yet supported for {:?} packages", config.b00t.get_package_type(Some(&filename)));
                }
            }
        }
        Err(e) => eprintln!("Error reading config for {}: {}", command, e),
    }
}

#[allow(dead_code)]
fn get_installed_version(command: &str, path: &str) -> Option<String> {
    if let Ok((config, _)) = get_config(command, path) {
        if let Some(version_cmd) = &config.b00t.version {
            if let Ok(output) = cmd!("bash", "-c", version_cmd).read() {
                let re = Regex::new(config.b00t.version_regex.as_deref().unwrap_or("\\d+\\.\\d+\\.\\d+")).unwrap();
                if let Some(caps) = re.captures(&output) {
                    return Some(caps[0].to_string());
                }
            }
        }
    }
    None
}

#[allow(dead_code)]
fn check(command: &str, path: &str) {
    let desired_version_str = match get_config(command, path) {
        Ok((config, _)) => config.b00t.desires.clone().unwrap_or_else(|| {
            eprintln!("No desired version specified for {}", command);
            std::process::exit(1);
        }),
        Err(e) => {
            eprintln!("Error reading config for {}: {}", command, e);
            std::process::exit(1);
        }
    };

    let desired_version = Version::parse(&desired_version_str).unwrap();

    if let Some(installed_version_str) = get_installed_version(command, path) {
        let installed_version = Version::parse(&installed_version_str).unwrap();

        if installed_version == desired_version {
            println!("🥾👍🏻 {}", command);
            std::process::exit(0);
        } else if installed_version > desired_version {
            println!("🥾🐣 {} {}", command, installed_version);
            std::process::exit(0);
        } else {
            println!("🥾😭 {} IS {} WANTS {}", command, installed_version, desired_version);
            std::process::exit(1);
        }
    } else {
        println!("🥾😱 {} MISSING", command);
        std::process::exit(2);
    }
}

#[allow(dead_code)]
fn up(path: &str) {
    let expanded_path = shellexpand::tilde(path).to_string();
    let entries = match fs::read_dir(&expanded_path) {
        Ok(entries) => entries,
        Err(e) => {
            eprintln!("Error reading directory {}: {}", &expanded_path, e);
            return;
        }
    };

    for entry in entries {
        if let Ok(entry) = entry {
            let entry_path = entry.path();
            if entry_path.extension().and_then(|s| s.to_str()) == Some("toml") {
                if let Some(command) = entry_path.file_stem().and_then(|s| s.to_str()) {
                    let desired_version_str = match get_config(command, path) {
                        Ok((config, _)) => match config.b00t.desires.clone() {
                            Some(version) => version,
                            None => continue,
                        },
                        Err(_) => continue,
                    };

                    let desired_version = Version::parse(&desired_version_str).unwrap();

                    if let Some(installed_version_str) = get_installed_version(command, path) {
                        let installed_version = Version::parse(&installed_version_str).unwrap();

                        if installed_version < desired_version {
                            println!("Updating {}...", command);
                            update(command, path);
                        }
                    } else {
                        println!("Installing {}...", command);
                        install(command, path);
                    }
                }
            }
        }
    }
}


fn create_mcp_toml_config(package: &BootPackage, path: &str) -> Result<()> {
    create_unified_toml_config(package, path)
}


fn mcp_add(json: &str, dwiw: bool, path: &str) -> Result<()> {
    let json_content = if json == "-" {
        let mut buffer = String::new();
        match io::stdin().read_to_string(&mut buffer) {
            Ok(_) => {
                let trimmed = buffer.trim();
                if trimmed.is_empty() {
                    anyhow::bail!("No input provided. Pipe JSON content or press Ctrl+D after pasting.");
                }
                trimmed.to_string()
            }
            Err(e) => {
                anyhow::bail!("Failed to read from stdin: {}. Pipe JSON content or use Ctrl+D after input.", e);
            }
        }
    } else {
        json.trim().to_string()
    };
    
    let package = normalize_mcp_json(&json_content, dwiw)?;
    
    create_mcp_toml_config(&package, path)?;
    
    println!("MCP server '{}' configuration saved.", package.name);
    println!("To install to VSCode: b00t-cli vscode install mcp {}", package.name);
    
    Ok(())
}

fn mcp_list(path: &str, json_output: bool) -> Result<()> {
    let expanded_path = shellexpand::tilde(path).to_string();
    let entries = match fs::read_dir(&expanded_path) {
        Ok(entries) => entries,
        Err(e) => {
            anyhow::bail!("Error reading directory {}: {}", &expanded_path, e);
        }
    };

    let mut mcp_items = Vec::new();

    for entry in entries {
        if let Ok(entry) = entry {
            let entry_path = entry.path();
            if let Some(file_name) = entry_path.file_name().and_then(|s| s.to_str()) {
                if file_name.ends_with(".mcp.toml") {
                    if let Some(server_name) = file_name.strip_suffix(".mcp.toml") {
                        // Try to read the config to get details
                        match get_mcp_config(server_name, path) {
                            Ok(package) => {
                                mcp_items.push(McpListItem {
                                    name: server_name.to_string(),
                                    command: package.command.clone(),
                                    args: package.args.clone(),
                                    error: None,
                                });
                            }
                            Err(e) => {
                                mcp_items.push(McpListItem {
                                    name: server_name.to_string(),
                                    command: None,
                                    args: None,
                                    error: Some(e.to_string()),
                                });
                            }
                        }
                    }
                }
            }
        }
    }

    if json_output {
        let output = McpListOutput {
            servers: mcp_items,
            path: expanded_path,
        };
        let json_str = serde_json::to_string_pretty(&output)
            .context("Failed to serialize MCP list to JSON")?;
        println!("{}", json_str);
    } else {
        if mcp_items.is_empty() {
            println!("No MCP server configurations found in {}", expanded_path);
            println!("Use 'b00t-cli mcp add <json>' to add MCP server configurations.");
        } else {
            println!("Available MCP servers in {}:", expanded_path);
            println!();
            for item in mcp_items {
                match (&item.command, &item.args) {
                    (Some(command), Some(args)) => {
                        println!("📋 {} ({})", item.name, command);
                        if !args.is_empty() {
                            println!("   args: {}", args.join(" "));
                        }
                    }
                    _ => {
                        println!("❌ {} (error reading config)", item.name);
                    }
                }
            }
            println!();
            println!("To install to VSCode: b00t-cli vscode install mcp <name>");
            println!("To install to Claude Code: b00t-cli claude-code install mcp <name>");
        }
    }

    Ok(())
}

fn get_mcp_config(name: &str, path: &str) -> Result<BootPackage> {
    let mut path_buf = PathBuf::new();
    path_buf.push(shellexpand::tilde(path).to_string());
    path_buf.push(format!("{}.mcp.toml", name));

    if !path_buf.exists() {
        anyhow::bail!("MCP server '{}' not found. Use 'b00t-cli mcp add' to create it first.", name);
    }

    let content = fs::read_to_string(&path_buf)
        .context(format!("Failed to read MCP config from {}", path_buf.display()))?;
    
    let config: UnifiedConfig = toml::from_str(&content)
        .context("Failed to parse MCP config TOML")?;

    Ok(config.b00t)
}

fn vscode_install_mcp(name: &str, path: &str) -> Result<()> {
    let package = get_mcp_config(name, path)?;
    
    let vscode_json = serde_json::json!({
        "name": package.name,
        "command": package.command.as_ref().unwrap_or(&"npx".to_string()),
        "args": package.args.as_ref().unwrap_or(&vec![])
    });

    let json_str = serde_json::to_string(&vscode_json)
        .context("Failed to serialize JSON for VSCode")?;

    let result = cmd!("code", "--add-mcp", &json_str).run();
    
    match result {
        Ok(_) => {
            println!("Successfully installed MCP server '{}' to VSCode", package.name);
            println!("VSCode command: code --add-mcp '{}'", json_str);
        },
        Err(e) => {
            eprintln!("Failed to install MCP server to VSCode: {}", e);
            eprintln!("Manual command: code --add-mcp '{}'", json_str);
            std::process::exit(1);
        }
    }
    
    Ok(())
}

fn claude_code_install_mcp(name: &str, path: &str) -> Result<()> {
    let package = get_mcp_config(name, path)?;
    
    // Claude Code uses claude-code config add-mcp command
    let claude_json = serde_json::json!({
        "name": package.name,
        "command": package.command.as_ref().unwrap_or(&"npx".to_string()),
        "args": package.args.as_ref().unwrap_or(&vec![])
    });

    let json_str = serde_json::to_string(&claude_json)
        .context("Failed to serialize JSON for Claude Code")?;

    let result = cmd!("claude-code", "config", "add-mcp", &json_str).run();
    
    match result {
        Ok(_) => {
            println!("Successfully installed MCP server '{}' to Claude Code", package.name);
            println!("Claude Code command: claude-code config add-mcp '{}'", json_str);
        },
        Err(e) => {
            eprintln!("Failed to install MCP server to Claude Code: {}", e);
            eprintln!("Manual command: claude-code config add-mcp '{}'", json_str);
            std::process::exit(1);
        }
    }
    
    Ok(())
}

fn cli_run(name: &str, path: &str) -> Result<()> {
    let package = get_cli_config(name, path)?;
    
    if let Some(command) = &package.command {
        println!("Running CLI script '{}'...", name);
        let result = cmd!("bash", "-c", command).run();
        match result {
            Ok(_) => {
                println!("CLI script '{}' completed successfully.", name);
            },
            Err(e) => {
                eprintln!("CLI script '{}' failed: {}", name, e);
                std::process::exit(1);
            }
        }
    } else {
        eprintln!("No command defined for CLI script '{}'.", name);
        std::process::exit(1);
    }
    
    Ok(())
}

fn cli_check(command: &str, path: &str) {
    let desired_version_str = match get_cli_unified_config(command, path) {
        Ok((config, _)) => config.b00t.desires.clone().unwrap_or_else(|| {
            eprintln!("No desired version specified for {}", command);
            std::process::exit(1);
        }),
        Err(e) => {
            eprintln!("Error reading config for {}: {}", command, e);
            std::process::exit(1);
        }
    };

    let desired_version = Version::parse(&desired_version_str).unwrap();

    if let Some(installed_version_str) = get_cli_installed_version(command, path) {
        let installed_version = Version::parse(&installed_version_str).unwrap();

        if installed_version == desired_version {
            println!("🥾👍🏻 {}", command);
            std::process::exit(0);
        } else if installed_version > desired_version {
            println!("🥾🐣 {} {}", command, installed_version);
            std::process::exit(0);
        } else {
            println!("🥾😭 {} IS {} WANTS {}", command, installed_version, desired_version);
            std::process::exit(1);
        }
    } else {
        println!("🥾😱 {} MISSING", command);
        std::process::exit(2);
    }
}

fn get_cli_installed_version(command: &str, path: &str) -> Option<String> {
    if let Ok((config, _)) = get_cli_unified_config(command, path) {
        if let Some(version_cmd) = &config.b00t.version {
            if let Ok(output) = cmd!("bash", "-c", version_cmd).read() {
                let re = Regex::new(config.b00t.version_regex.as_deref().unwrap_or("\\d+\\.\\d+\\.\\d+")).unwrap();
                if let Some(caps) = re.captures(&output) {
                    return Some(caps[0].to_string());
                }
            }
        }
    }
    None
}

fn get_cli_config(name: &str, path: &str) -> Result<BootPackage> {
    let mut path_buf = PathBuf::new();
    path_buf.push(shellexpand::tilde(path).to_string());
    path_buf.push(format!("{}.cli.toml", name));

    if !path_buf.exists() {
        anyhow::bail!("CLI script '{}' not found. Create a {}.cli.toml file first.", name, name);
    }

    let content = fs::read_to_string(&path_buf)
        .context(format!("Failed to read CLI config from {}", path_buf.display()))?;
    
    let config: UnifiedConfig = toml::from_str(&content)
        .context("Failed to parse CLI config TOML")?;

    Ok(config.b00t)
}

fn cli_detect(command: &str, path: &str) {
    if let Some(version) = get_cli_installed_version(command, path) {
        println!("{}", version);
    } else {
        eprintln!("🥾😱 {} MISSING", command);
        std::process::exit(2);
    }
}

fn cli_desires(command: &str, path: &str) {
    match get_cli_unified_config(command, path) {
        Ok((config, _)) => {
            println!("{}", config.b00t.desires.as_deref().unwrap_or("N/A"));
        }
        Err(e) => eprintln!("Error reading config for {}: {}", command, e),
    }
}

fn cli_install(command: &str, path: &str) {
    match get_cli_unified_config(command, path) {
        Ok((config, filename)) => {
            match config.b00t.get_package_type(Some(&filename)) {
                PackageType::Traditional => {
                    if let Some(install_cmd) = &config.b00t.install {
                        println!("Installing {}...", command);
                        let result = cmd!("bash", "-c", install_cmd).run();
                        match result {
                            Ok(_) => println!("Installation complete."),
                            Err(e) => eprintln!("Installation failed: {}", e),
                        }
                    } else {
                        eprintln!("No install command defined for {}", command);
                    }
                }
                PackageType::Mcp => {
                    eprintln!("Use 'b00t-cli vscode install mcp {}' or 'b00t-cli claude-code install mcp {}' instead", command, command);
                }
                _ => {
                    eprintln!("Installation not yet supported for {:?} packages", config.b00t.get_package_type(Some(&filename)));
                }
            }
        }
        Err(e) => eprintln!("Error reading config for {}: {}", command, e),
    }
}

fn cli_update(command: &str, path: &str) {
    match get_cli_unified_config(command, path) {
        Ok((config, filename)) => {
            match config.b00t.get_package_type(Some(&filename)) {
                PackageType::Traditional => {
                    if let Some(update_cmd) = config.b00t.update.as_ref().or(config.b00t.install.as_ref()) {
                        println!("Updating {}...", command);
                        let result = cmd!("bash", "-c", update_cmd).run();
                        match result {
                            Ok(_) => println!("Update complete."),
                            Err(e) => eprintln!("Update failed: {}", e),
                        }
                    } else {
                        eprintln!("No update or install command defined for {}", command);
                    }
                }
                _ => {
                    eprintln!("Update not yet supported for {:?} packages", config.b00t.get_package_type(Some(&filename)));
                }
            }
        }
        Err(e) => eprintln!("Error reading config for {}: {}", command, e),
    }
}

fn cli_up(path: &str) {
    let path_buf = PathBuf::from(shellexpand::tilde(path).to_string());
    let entries = match fs::read_dir(&path_buf) {
        Ok(entries) => entries,
        Err(e) => {
            eprintln!("Error reading directory {}: {}", path_buf.display(), e);
            return;
        }
    };

    for entry in entries {
        if let Ok(entry) = entry {
            let entry_path = entry.path();
            if entry_path.extension().and_then(|s| s.to_str()) == Some("toml") {
                let filename = entry_path.file_name().and_then(|s| s.to_str()).unwrap_or("");
                if filename.ends_with(".cli.toml") {
                    if let Some(command) = entry_path.file_stem().and_then(|s| s.to_str()) {
                        let command = command.trim_end_matches(".cli");
                        let desired_version_str = match get_cli_unified_config(command, path) {
                            Ok((config, _)) => match config.b00t.desires.clone() {
                                Some(version) => version,
                                None => continue,
                            },
                            Err(_) => continue,
                        };

                        if let Some(installed_version_str) = get_cli_installed_version(command, path) {
                            if let (Ok(desired_version), Ok(installed_version)) = (
                                semver::Version::parse(&desired_version_str),
                                semver::Version::parse(&installed_version_str),
                            ) {
                                if installed_version < desired_version {
                                    println!("Updating {} from {} to {}...", command, installed_version, desired_version);
                                    cli_update(command, path);
                                }
                            }
                        } else {
                            println!("Installing missing package {}...", command);
                            cli_install(command, path);
                        }
                    }
                }
            }
        }
    }
}
