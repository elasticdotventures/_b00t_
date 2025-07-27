use anyhow::{Context, Result};
use clap::Parser;
use duct::cmd;
use regex::Regex;
use semver::Version;
use std::fs;
use std::io::{self, Read};
use std::path::PathBuf;
// 🤓 cleaned up unused Tera import after switching to simple string replacement
use b00t_cli::{
    AiConfig, AiListItem, AiListOutput, BootDatum, DatumType, McpListItem, McpListOutput,
    SessionState, UnifiedConfig, create_ai_toml_config, create_unified_toml_config,
    normalize_mcp_json,
};

mod datum_ai;
mod datum_apt;
mod datum_bash;
mod datum_cli;
mod datum_docker;
mod datum_gemini;
mod datum_mcp;
mod datum_vscode;
mod traits;
mod utils;

// 🦨 REMOVED unused K8sDatum import - not used in main.rs
use datum_ai::AiDatum;
use datum_apt::AptDatum;
use datum_bash::BashDatum;
use datum_cli::CliDatum;
use datum_docker::DockerDatum;
use datum_gemini::gemini_install_mcp;
use datum_mcp::McpDatum;
use datum_vscode::VscodeDatum;
use traits::*;

mod integration_tests;

#[derive(Parser)]
#[clap(version, about, long_about = None)]
struct Cli {
    #[clap(subcommand)]
    command: Option<Commands>,
    #[clap(short, long, env = "_B00T_Path", default_value = "~/.dotfiles/_b00t_")]
    path: String,
    #[clap(
        long,
        help = "Output structured markdown documentation about internal structures"
    )]
    doc: bool,
}

#[derive(Parser)]
enum Commands {
    #[clap(about = "MCP (Model Context Protocol) server management")]
    Mcp {
        #[clap(subcommand)]
        mcp_command: McpCommands,
    },
    #[clap(about = "AI provider management")]
    Ai {
        #[clap(subcommand)]
        ai_command: AiCommands,
    },
    #[clap(about = "Application integration commands")]
    App {
        #[clap(subcommand)]
        app_command: AppCommands,
    },
    #[clap(about = "CLI script management")]
    Cli {
        #[clap(subcommand)]
        cli_command: CliCommands,
    },
    #[clap(about = "Initialize system settings and aliases")]
    Init {
        #[clap(subcommand)]
        init_command: InitCommands,
    },
    #[clap(about = "Show agent identity and context information")]
    Whoami,
    #[clap(about = "Create checkpoint: commit all files and run tests")]
    Checkpoint {
        #[clap(short, long, help = "Commit message for the checkpoint")]
        message: Option<String>,
        #[clap(long, help = "Skip running tests (not recommended)")]
        skip_tests: bool,
    },
    #[clap(about = "Query system information")]
    Whatismy {
        #[clap(subcommand)]
        whatismy_command: WhatismyCommands,
    },
    #[clap(about = "Show status dashboard of all available tools and services")]
    Status {
        #[clap(
            long,
            help = "Filter by subsystem: cli, mcp, ai, vscode, docker, apt, nix, bash"
        )]
        filter: Option<String>,
        #[clap(long, help = "Show only installed tools")]
        installed: bool,
        #[clap(long, help = "Show only available (not installed) tools")]
        available: bool,
    },
    #[clap(about = "Session management")]
    Session {
        #[clap(subcommand)]
        session_command: SessionCommands,
    },
}

#[derive(Parser)]
enum McpCommands {
    #[clap(
        about = "Create MCP server configuration",
        long_about = "Create MCP server configuration from JSON or command.\n\nJSON Examples:\n  b00t-cli mcp create '{\"name\":\"filesystem\",\"command\":\"npx\",\"args\":[\"-y\",\"@modelcontextprotocol/server-filesystem\"]}'\n  echo '{...}' | b00t-cli mcp create -\n\nCommand Examples:\n  b00t-cli mcp create brave-search -- npx -y @modelcontextprotocol/server-brave-search\n  b00t-cli mcp create filesystem --hint \"File system access\" -- npx -y @modelcontextprotocol/server-filesystem\n\nInstallation Examples:\n  b00t-cli mcp install brave-search claudecode\n  b00t-cli app vscode mcp install filesystem"
    )]
    Create {
        #[clap(help = "MCP server name (for command mode) or JSON configuration (for JSON mode)")]
        name_or_json: String,
        #[clap(long, help = "Description/hint for the MCP server")]
        hint: Option<String>,
        #[clap(
            long,
            help = "Do What I Want - auto-cleanup and format JSON (default: enabled)"
        )]
        dwiw: bool,
        #[clap(
            long,
            help = "Disable auto-cleanup and format JSON",
            conflicts_with = "dwiw"
        )]
        no_dwiw: bool,
        #[clap(
            last = true,
            help = "Command and arguments (after --) for command mode"
        )]
        command_args: Vec<String>,
    },
    #[clap(
        about = "List available MCP server configurations",
        long_about = "List available MCP server configurations.\n\nExamples:\n  b00t-cli mcp list\n  b00t-cli mcp list --json"
    )]
    List {
        #[clap(long, help = "Output in JSON format")]
        json: bool,
    },
    #[clap(
        about = "Install MCP server to a target (claudecode, vscode, geminicli)",
        long_about = "Install MCP server to a target application.\n\nExamples:\n  b00t-cli mcp install gh claudecode\n  b00t-cli mcp install filesystem geminicli --repo\n  b00t-cli app vscode mcp install filesystem"
    )]
    Install {
        #[clap(help = "MCP server name")]
        name: String,
        #[clap(help = "Installation target: claudecode, vscode, geminicli")]
        target: String,
        #[clap(long, help = "Install to repository-specific location (for geminicli)")]
        repo: bool,
        #[clap(long, help = "Install to user-global location (for geminicli)")]
        user: bool,
    },
    #[clap(
        about = "Output MCP servers in various formats",
        long_about = "Output MCP servers in various formats for configuration files.\n\nExamples:\n  b00t-cli mcp output filesystem,brave-search\n  b00t-cli mcp output --json filesystem\n  b00t-cli mcp output --mcpServers filesystem,brave-search"
    )]
    Output {
        #[clap(long = "json", help = "Output raw JSON format without wrapper", action = clap::ArgAction::SetTrue)]
        json: bool,
        #[clap(long = "mcpServers", help = "Output in mcpServers format (default)", action = clap::ArgAction::SetTrue)]
        mcp_servers: bool,
        #[clap(help = "Comma-separated list of MCP server names to output")]
        servers: String,
    },
}

#[derive(Parser)]
enum AiCommands {
    #[clap(
        about = "Add AI provider configuration from TOML file",
        long_about = "Add AI provider configuration from TOML file.\n\nExamples:\n  b00t-cli ai add ./openai.ai.toml\n  b00t-cli ai add ~/.dotfiles/_b00t_/anthropic.ai.toml"
    )]
    Add {
        #[clap(help = "Path to AI provider TOML file")]
        file: String,
    },
    #[clap(
        about = "List available AI provider configurations",
        long_about = "List available AI provider configurations.\n\nExamples:\n  b00t-cli ai list\n  b00t-cli ai list --json"
    )]
    List {
        #[clap(long, help = "Output in JSON format")]
        json: bool,
    },
    #[clap(
        about = "Output AI providers in various formats",
        long_about = "Output AI providers in various formats.\n\nExamples:\n  b00t-cli ai output --kv openai,anthropic\n  b00t-cli ai output --b00t openai\n  b00t-cli ai output anthropic"
    )]
    Output {
        #[clap(long = "b00t", help = "Output in b00t TOML format (default)", action = clap::ArgAction::SetTrue)]
        b00t: bool,
        #[clap(long = "kv", help = "Output environment variables in KEY=VALUE format", action = clap::ArgAction::SetTrue)]
        kv: bool,
        #[clap(help = "Comma-separated list of AI provider names to output")]
        providers: String,
    },
}

#[derive(Parser)]
enum AppCommands {
    #[clap(
        about = "VSCode integration commands",
        long_about = "VSCode integration commands.\n\nMCP Examples:\n  b00t-cli app vscode mcp install gh\n  b00t-cli mcp install gh vscode\n\nExtension Examples:\n  b00t-cli app vscode extension list\n  b00t-cli app vscode extension install rust-lang.rust-analyzer"
    )]
    Vscode {
        #[clap(subcommand)]
        vscode_command: AppVscodeCommands,
    },
    #[clap(
        about = "Claude Code integration commands",
        long_about = "Claude Code integration commands.\n\nExamples:\n  b00t-cli app claudecode mcp install gh\n  b00t-cli mcp install gh claudecode"
    )]
    Claudecode {
        #[clap(subcommand)]
        claudecode_command: AppClaudecodeCommands,
    },
    #[clap(
        about = "Gemini CLI integration commands",
        long_about = "Gemini CLI integration commands.\n\nExamples:\n  b00t-cli app geminicli mcp install gh --repo\n  b00t-cli mcp install gh geminicli --user"
    )]
    Geminicli {
        #[clap(subcommand)]
        geminicli_command: AppGeminicliCommands,
    },
}

#[derive(Parser)]
enum AppVscodeCommands {
    #[clap(about = "MCP server management for VSCode")]
    Mcp {
        #[clap(subcommand)]
        mcp_command: AppMcpCommands,
    },
    #[clap(about = "VS Code extension management")]
    Extension {
        #[clap(subcommand)]
        extension_command: AppVscodeExtensionCommands,
    },
}

#[derive(Parser)]
enum AppClaudecodeCommands {
    #[clap(about = "MCP server management for Claude Code")]
    Mcp {
        #[clap(subcommand)]
        mcp_command: AppMcpCommands,
    },
}

#[derive(Parser)]
enum AppGeminicliCommands {
    #[clap(about = "MCP server management for Gemini CLI")]
    Mcp {
        #[clap(subcommand)]
        mcp_command: AppGeminicliMcpCommands,
    },
}

#[derive(Parser)]
enum AppMcpCommands {
    #[clap(
        about = "Install MCP server",
        long_about = "Install MCP server to the target application.\n\nExamples:\n  b00t-cli app vscode mcp install gh\n  b00t-cli app claudecode mcp install filesystem"
    )]
    Install {
        #[clap(help = "Name of the MCP server to install")]
        name: String,
    },
}

#[derive(Parser)]
enum AppVscodeExtensionCommands {
    #[clap(
        about = "List installed VS Code extensions",
        long_about = "List all installed VS Code extensions.\n\nExamples:\n  b00t-cli app vscode extension list\n  b00t-cli app vscode extension list --json"
    )]
    List {
        #[clap(long, help = "Output in JSON format")]
        json: bool,
    },
    #[clap(
        about = "Install VS Code extension",
        long_about = "Install VS Code extension by ID.\n\nExamples:\n  b00t-cli app vscode extension install ms-vscode.vscode-typescript-next\n  b00t-cli app vscode extension install rust-lang.rust-analyzer"
    )]
    Install {
        #[clap(help = "Extension ID to install (e.g., ms-vscode.vscode-typescript-next)")]
        extension_id: String,
    },
    #[clap(
        about = "Uninstall VS Code extension",
        long_about = "Uninstall VS Code extension by ID.\n\nExamples:\n  b00t-cli app vscode extension uninstall ms-vscode.vscode-typescript-next\n  b00t-cli app vscode extension uninstall rust-lang.rust-analyzer"
    )]
    Uninstall {
        #[clap(help = "Extension ID to uninstall")]
        extension_id: String,
    },
}

#[derive(Parser)]
enum AppGeminicliMcpCommands {
    #[clap(
        about = "Install MCP server to Gemini CLI",
        long_about = "Install MCP server to Gemini CLI extension.\n\nExamples:\n  b00t-cli app geminicli mcp install gh --repo\n  b00t-cli app geminicli mcp install filesystem --user"
    )]
    Install {
        #[clap(help = "Name of the MCP server to install")]
        name: String,
        #[clap(
            long,
            help = "Install to repository-specific extension (default if in git repo)"
        )]
        repo: bool,
        #[clap(long, help = "Install to user-global extension")]
        user: bool,
    },
}

#[derive(Parser)]
enum CliCommands {
    #[clap(
        about = "Run a CLI script by name",
        long_about = "Run a CLI script by name.\n\nExamples:\n  b00t-cli cli run setup-dev\n  b00t-cli cli run deploy"
    )]
    Run {
        #[clap(help = "Name of the CLI script to run")]
        name: String,
    },
    #[clap(
        about = "Detect the version of a CLI command",
        long_about = "Detect the version of a CLI command.\n\nExamples:\n  b00t-cli cli detect node\n  b00t-cli cli detect git"
    )]
    Detect {
        #[clap(help = "The command to detect")]
        command: String,
    },
    #[clap(
        about = "Show the desired version of a CLI command",
        long_about = "Show the desired version of a CLI command.\n\nExamples:\n  b00t-cli cli desires node\n  b00t-cli cli desires docker"
    )]
    Desires {
        #[clap(help = "The command to check")]
        command: String,
    },
    #[clap(
        about = "Install a CLI command",
        long_about = "Install a CLI command using its configuration.\n\nExamples:\n  b00t-cli cli install node\n  b00t-cli cli install docker"
    )]
    Install {
        #[clap(help = "The command to install")]
        command: String,
    },
    #[clap(
        about = "Update a CLI command",
        long_about = "Update a CLI command using its configuration.\n\nExamples:\n  b00t-cli cli update node\n  b00t-cli cli update docker"
    )]
    Update {
        #[clap(help = "The command to update")]
        command: String,
    },
    #[clap(
        about = "Check installed vs desired versions for CLI command",
        long_about = "Check installed vs desired versions for CLI command.\n\nExamples:\n  b00t-cli cli check node\n  b00t-cli cli check docker\n\nOutput emojis:\n  🥾👍🏻 = versions match\n  🥾🐣 = installed version newer than desired\n  🥾😭 = installed version older than desired\n  🥾😱 = command missing"
    )]
    Check {
        #[clap(help = "The command to check")]
        command: String,
    },
    #[clap(
        about = "Update all CLI commands",
        long_about = "Update all CLI commands that have outdated versions.\n\nExamples:\n  b00t-cli cli up"
    )]
    Up,
}

#[derive(Parser)]
enum InitCommands {
    #[clap(
        about = "Initialize command aliases",
        long_about = "Initialize command aliases for CLI tools.\n\nExamples:\n  b00t-cli init aliases"
    )]
    Aliases,
}

#[derive(Parser)]
enum WhatismyCommands {
    #[clap(about = "Detect current AI agent (claude, gemini, etc.)")]
    Agent {
        #[clap(long, help = "Ignore _B00T_Agent environment variable")]
        no_env: bool,
    },
}

#[derive(Parser)]
enum SessionCommands {
    #[clap(about = "Initialize new session")]
    Init {
        #[clap(long, help = "Budget limit in USD")]
        budget: Option<f64>,
        #[clap(long, help = "Time limit in minutes")]
        time_limit: Option<u32>,
        #[clap(long, help = "Agent name override")]
        agent: Option<String>,
    },
    #[clap(about = "Show current session status")]
    Status,
    #[clap(about = "Update session metrics")]
    Update {
        #[clap(long, help = "Estimated cost to add")]
        cost: Option<f64>,
        #[clap(long, help = "Hint message to add")]
        hint: Option<String>,
    },
    #[clap(about = "End current session")]
    End,
    #[clap(about = "Get compact status for prompt")]
    Prompt,
}

// Using unified config from lib.rs
type Config = UnifiedConfig;

#[derive(Debug, Clone)]
struct ToolStatus {
    name: String,
    subsystem: String,
    installed: bool,
    available: bool,
    disabled: bool,
    version_status: Option<String>, // emoji for version status
    current_version: Option<String>,
    desired_version: Option<String>,
    hint: String,
}

impl ToolStatus {
    fn status_icon(&self) -> &'static str {
        if self.disabled {
            "🔴"
        } else if self.installed {
            "☑️"
        } else if self.available {
            "⏹️"
        } else {
            "❌"
        }
    }

    fn version_emoji(&self) -> &str {
        self.version_status.as_deref().unwrap_or("")
    }
}

// Bridge function to convert trait-based DatumProviders to legacy ToolStatus
fn datum_providers_to_tool_status(providers: Vec<Box<dyn DatumProvider>>) -> Vec<ToolStatus> {
    providers
        .into_iter()
        .map(|provider| {
            let is_installed = DatumChecker::is_installed(provider.as_ref());
            let is_disabled = StatusProvider::is_disabled(provider.as_ref());
            let version_status = DatumChecker::version_status(provider.as_ref());

            ToolStatus {
                name: StatusProvider::name(provider.as_ref()).to_string(),
                subsystem: StatusProvider::subsystem(provider.as_ref()).to_string(),
                installed: is_installed,
                available: FilterLogic::is_available(provider.as_ref()),
                disabled: is_disabled,
                version_status: Some(version_status.emoji().to_string()),
                current_version: DatumChecker::current_version(provider.as_ref()),
                desired_version: DatumChecker::desired_version(provider.as_ref()),
                hint: StatusProvider::hint(provider.as_ref()).to_string(),
            }
        })
        .collect()
}

fn whoami(path: &str) -> Result<()> {
    let expanded_path = get_expanded_path(path)?;
    let agent_md_path = expanded_path.join("AGENT.md");

    if !agent_md_path.exists() {
        anyhow::bail!(
            "AGENT.md not found in {}. This file contains agent identity information.",
            expanded_path.display()
        );
    }

    let template_content = fs::read_to_string(&agent_md_path).context(format!(
        "Failed to read AGENT.md from {}",
        agent_md_path.display()
    ))?;

    // Prepare template variables
    let timestamp = chrono::Utc::now()
        .format("%Y-%m-%d %H:%M:%S UTC")
        .to_string();
    let user = std::env::var("USER").unwrap_or_else(|_| "unknown".to_string());
    let branch = cmd!("git", "branch", "--show-current")
        .read()
        .unwrap_or_else(|_| "no-git".to_string())
        .trim()
        .to_string();
    let agent = detect_agent(false);
    let model_size = std::env::var("MODEL_SIZE").unwrap_or_else(|_| "unknown".to_string());
    let privacy = std::env::var("PRIVACY").unwrap_or_else(|_| "standard".to_string());

    // Simple string replacement approach instead of Tera due to complex template syntax
    let mut rendered = template_content;

    // Replace variables manually
    rendered = rendered.replace("{{PID}}", &std::process::id().to_string());
    rendered = rendered.replace("{{TIMESTAMP}}", &timestamp);
    rendered = rendered.replace("{{USER}}", &user);
    rendered = rendered.replace("{{BRANCH}}", &branch);
    rendered = rendered.replace("{{_B00T_Agent}}", &agent);
    rendered = rendered.replace("{{_B00T_AGENT}}", &agent);
    rendered = rendered.replace("{{MODEL_SIZE}}", &model_size);
    rendered = rendered.replace("{{PRIVACY}}", &privacy);

    println!("{}", rendered);

    Ok(())
}

fn checkpoint(message: Option<&str>, skip_tests: bool) -> Result<()> {
    println!("🥾 Creating checkpoint...");

    // Check if we're in a git repository
    let git_status = cmd!("git", "status", "--porcelain").read();
    if git_status.is_err() {
        anyhow::bail!("Not in a git repository. Run 'git init' first.");
    }

    // Check if this is a Rust project and run cargo check
    if std::path::Path::new("Cargo.toml").exists() {
        println!("🦀 Rust project detected. Running cargo check...");
        let cargo_check = cmd!("cargo", "check").run();
        if let Err(e) = cargo_check {
            anyhow::bail!(
                "🚨 cargo check failed: {}. Fix compilation errors before checkpoint.",
                e
            );
        }
        println!("✅ cargo check passed");
    }

    // Generate commit message
    let commit_msg = message.unwrap_or("🥾 checkpoint: automated commit via b00t-cli");

    // Add all files (including untracked)
    println!("📦 Adding all files to staging area...");
    let add_result = cmd!("git", "add", "-A").run();
    if let Err(e) = add_result {
        anyhow::bail!("Failed to add files to git staging area: {}", e);
    }

    // Check if there are any changes to commit
    let staged_changes = cmd!("git", "diff", "--cached", "--name-only")
        .read()
        .unwrap_or_default();

    if staged_changes.trim().is_empty() {
        println!("✅ No changes to commit. Repository is clean.");
        return Ok(());
    }

    println!("📝 Files staged for commit:");
    let staged_files = cmd!("git", "diff", "--cached", "--name-only")
        .read()
        .unwrap_or_default();
    for file in staged_files.lines() {
        if !file.trim().is_empty() {
            println!("   • {}", file.trim());
        }
    }

    // Create the commit (this will trigger pre-commit hooks including tests)
    println!("💾 Creating commit with message: '{}'", commit_msg);
    let commit_result = cmd!("git", "commit", "-m", commit_msg).run();

    match commit_result {
        Ok(_) => {
            println!("✅ Checkpoint created successfully!");

            // Show the commit hash
            if let Ok(commit_hash) = cmd!("git", "rev-parse", "--short", "HEAD").read() {
                println!("📍 Commit: {}", commit_hash.trim());
            }

            // Show current branch
            if let Ok(branch) = cmd!("git", "branch", "--show-current").read() {
                println!("🌳 Branch: {}", branch.trim());
            }

            if !skip_tests {
                println!("🧪 Tests executed via git pre-commit hooks");
            }
        }
        Err(e) => {
            anyhow::bail!(
                "Commit failed: {}. This usually means git pre-commit hooks (including tests) failed.",
                e
            );
        }
    }

    Ok(())
}

/// Detect current AI agent based on environment variables
fn detect_agent(ignore_env: bool) -> String {
    // Check if _B00T_Agent is already set and we're not ignoring env
    if !ignore_env {
        if let Ok(agent) = std::env::var("_B00T_Agent") {
            if !agent.is_empty() {
                return agent;
            }
        }
    }

    // Check for Claude Code
    if std::env::var("CLAUDECODE").unwrap_or_default() == "1" {
        return "claude".to_string();
    }

    // TODO: Add detection for other agents based on their shell environment:
    // - gemini: specific environment vars set by gemini-cli shell
    // - codex: specific environment vars set by codex shell
    // - other agents: their respective shell environment indicators

    // Return empty string if no agent detected
    "".to_string()
}

/// Generic function to load datum providers for a specific file extension
/// Replaces the 7 duplicate get_*_tools_status functions
fn load_datum_providers<T>(path: &str, extension: &str) -> Result<Vec<Box<dyn DatumProvider>>>
where
    T: DatumProvider + 'static,
    T: for<'a> TryFrom<(&'a str, &'a str), Error = anyhow::Error>,
{
    let mut tools: Vec<Box<dyn DatumProvider>> = Vec::new();
    let expanded_path = get_expanded_path(path)?;

    if let Ok(entries) = std::fs::read_dir(&expanded_path) {
        for entry in entries {
            if let Ok(entry) = entry {
                let entry_path = entry.path();
                if let Some(file_name) = entry_path.file_name().and_then(|s| s.to_str()) {
                    if file_name.ends_with(extension) {
                        if let Some(tool_name) = file_name.strip_suffix(extension) {
                            if let Ok(datum) = T::try_from((tool_name, path)) {
                                tools.push(Box::new(datum));
                            }
                        }
                    }
                }
            }
        }
    }

    Ok(tools)
}

fn show_status(
    path: &str,
    filter: Option<&str>,
    only_installed: bool,
    only_available: bool,
) -> Result<()> {
    let mut all_tools = Vec::new();

    // Collect tools from all subsystems using new generic trait-based architecture
    all_tools.extend(datum_providers_to_tool_status(load_datum_providers::<
        CliDatum,
    >(path, ".cli.toml")?));
    all_tools.extend(datum_providers_to_tool_status(load_datum_providers::<
        McpDatum,
    >(path, ".mcp.toml")?));
    all_tools.extend(datum_providers_to_tool_status(load_datum_providers::<
        AiDatum,
    >(path, ".ai.toml")?));
    all_tools.extend(datum_providers_to_tool_status(load_datum_providers::<
        AptDatum,
    >(path, ".apt.toml")?));
    all_tools.extend(datum_providers_to_tool_status(load_datum_providers::<
        BashDatum,
    >(path, ".bash.toml")?));
    all_tools.extend(datum_providers_to_tool_status(load_datum_providers::<
        DockerDatum,
    >(path, ".docker.toml")?));
    all_tools.extend(datum_providers_to_tool_status(load_datum_providers::<
        VscodeDatum,
    >(path, ".vscode.toml")?));
    all_tools.extend(get_other_tools_status(path)?);

    // Apply filters
    let filtered_tools: Vec<ToolStatus> = all_tools
        .into_iter()
        .filter(|tool| {
            if let Some(f) = filter {
                if tool.subsystem != f {
                    return false;
                }
            }
            if only_installed && !tool.installed {
                return false;
            }
            if only_available && (tool.installed || tool.disabled) {
                return false;
            }
            true
        })
        .collect();

    // Group by subsystem and display
    let mut subsystems: std::collections::HashMap<String, Vec<ToolStatus>> =
        std::collections::HashMap::new();
    for tool in filtered_tools {
        subsystems
            .entry(tool.subsystem.clone())
            .or_insert_with(Vec::new)
            .push(tool);
    }

    // Sort subsystems for consistent output
    let mut sorted_subsystems: Vec<_> = subsystems.into_iter().collect();
    sorted_subsystems.sort_by(|a, b| a.0.cmp(&b.0));

    println!("# 🥾 b00t Tool Status Dashboard\n");

    for (subsystem_name, mut tools) in sorted_subsystems {
        tools.sort_by(|a, b| a.name.cmp(&b.name));

        let subsystem_upper = subsystem_name.to_uppercase();
        let display_name = match subsystem_upper.as_str() {
            "DOCKER" => "Docker Containers",
            "VSCODE" => "VSCode Extensions",
            "APT" => "Linux/Ubuntu Packages",
            "AI" => "AI Providers",
            other => other,
        };
        println!("## {}", display_name);
        println!();

        if tools.is_empty() {
            println!("No tools found for {}", subsystem_name);
            println!();
            continue;
        }

        // Table header
        println!("| Status | Tool | Version | Hint |");
        println!("| ------ | ---- | ------- | ---- |");

        for tool in tools {
            let version_info = match (&tool.current_version, &tool.desired_version) {
                (Some(current), Some(desired)) => {
                    format!("{} {} → {}", tool.version_emoji(), current, desired)
                }
                (Some(current), None) => {
                    format!("{} {}", tool.version_emoji(), current)
                }
                (None, Some(desired)) => {
                    format!("⏹️ → {}", desired)
                }
                (None, None) => {
                    if tool.installed {
                        "✓".to_string()
                    } else {
                        "—".to_string()
                    }
                }
            };

            println!(
                "| {} | {} | {} | {} |",
                tool.status_icon(),
                tool.name,
                version_info,
                tool.hint
            );
        }
        println!();
    }

    Ok(())
}

fn get_other_tools_status(path: &str) -> Result<Vec<ToolStatus>> {
    let mut tools = Vec::new();
    let expanded_path = get_expanded_path(path)?;

    let other_extensions = [".nix.toml"]; // Only handle unimplemented subsystems

    if let Ok(entries) = fs::read_dir(&expanded_path) {
        for entry in entries {
            if let Ok(entry) = entry {
                let entry_path = entry.path();
                if let Some(file_name) = entry_path.file_name().and_then(|s| s.to_str()) {
                    for ext in &other_extensions {
                        if file_name.ends_with(ext) {
                            if let Some(tool_name) = file_name.strip_suffix(ext) {
                                let subsystem =
                                    ext.trim_start_matches('.').trim_end_matches(".toml");

                                let tool_status =
                                    check_other_tool_status(tool_name, subsystem, path)?;
                                tools.push(tool_status);
                            }
                        }
                    }
                }
            }
        }
    }

    Ok(tools)
}

fn check_other_tool_status(tool_name: &str, subsystem: &str, path: &str) -> Result<ToolStatus> {
    // Try to read the config file directly instead of using get_config which may exit
    let mut path_buf = get_expanded_path(path)?;
    path_buf.push(format!("{}.{}.toml", tool_name, subsystem));

    if !path_buf.exists() {
        return Ok(ToolStatus {
            name: tool_name.to_string(),
            subsystem: subsystem.to_string(),
            installed: false,
            available: false,
            disabled: true,
            version_status: Some("🔴".to_string()),
            current_version: None,
            desired_version: None,
            hint: "Configuration file not found".to_string(),
        });
    }

    let config_result = fs::read_to_string(&path_buf).and_then(|content| {
        toml::from_str::<Config>(&content)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))
    });

    match config_result {
        Ok(config) => {
            // For other tools, we'll make a best guess about installation status
            let installed = match subsystem {
                "apt" => {
                    // Check if the package is installed via dpkg
                    if let Some(package_name) = &config.b00t.package_name {
                        cmd!("dpkg", "-l", package_name).read().is_ok()
                    } else {
                        check_command_available(tool_name)
                    }
                }
                "bash" => {
                    // Bash scripts are always "available" if configured
                    true
                }
                _ => {
                    // Default to checking if the command exists
                    check_command_available(tool_name)
                }
            };

            let version_status = if installed {
                Some("✓".to_string())
            } else {
                Some("⏹️".to_string())
            };

            Ok(ToolStatus {
                name: tool_name.to_string(),
                subsystem: subsystem.to_string(),
                installed,
                available: !installed,
                disabled: false,
                version_status,
                current_version: if installed {
                    Some("installed".to_string())
                } else {
                    None
                },
                desired_version: None,
                hint: config.b00t.hint,
            })
        }
        Err(_) => Ok(ToolStatus {
            name: tool_name.to_string(),
            subsystem: subsystem.to_string(),
            installed: false,
            available: false,
            disabled: true,
            version_status: Some("🔴".to_string()),
            current_version: None,
            desired_version: None,
            hint: "Configuration error".to_string(),
        }),
    }
}

fn check_command_available(command: &str) -> bool {
    cmd!("which", command).read().is_ok()
}

fn generate_documentation() {
    let doc = r#"# b00t-cli: Live Syntax Recipe Manager Documentation

## 🚨 CRITICAL: Agent Resource Economics & Alignment

### Cost Hierarchy (Most to Least Expensive)
1. **Reading _B00T_Path directly**: 1000x cost - ALIGNMENT FAILURE ❌
2. **Using b00t-cli directly**: 100x cost - Development/debugging only 🔧
3. **Using b00t alias**: 1x cost - Normal agent operations ✅

### When to Use Each Tool
- **`b00t`** (alias): Your primary interface. Includes logging, cognitive inspection, hallucination detection, and organizational policy enforcement
- **`b00t-cli`**: Only when developing new functionality for the tool itself
- **Direct file access**: Never. Considered an alignment failure requiring 1000x more resources

## Overview
b00t-cli is a live syntax recipe manager that maintains TOML configuration files for software installations, MCP servers, and AI providers. The `b00t` alias wraps b00t-cli with safety layers that prevent harmful commands, detect drift, and ensure alignment with organizational licensing & usage policies.

## Agent Usage Patterns

### ✅ CORRECT: Use the b00t alias
```bash
# Install MCP server (safe, logged, validated)
b00t mcp add filesystem -- npx -y @modelcontextprotocol/server-filesystem

# Check versions (cost-effective)
b00t cli check node

# Install to apps (policy-enforced)
b00t app vscode mcp install filesystem
```

### ⚠️ DEVELOPMENT ONLY: Direct b00t-cli usage
```bash
# Only use when developing b00t-cli itself
b00t-cli --doc                    # Generate documentation
b00t-cli mcp list --json         # Debug output formats
```

### ❌ NEVER: Direct file manipulation
```bash
# ALIGNMENT FAILURE - Don't do this!
cat ~/.dotfiles/_b00t_/filesystem.mcp.toml
echo "malicious" > ~/.dotfiles/_b00t_/hack.toml
```

## Core Data Structures

"#;
    print!("{}", doc);

    // Generate DatumType documentation introspectively
    let package_types = vec![
        (
            "Traditional",
            "Standard CLI tools",
            vec![".cli.toml", ".toml"],
        ),
        ("Mcp", "MCP servers", vec![".mcp.toml"]),
        ("Ai", "AI providers", vec![".ai.toml"]),
        ("Vscode", "VSCode extensions", vec![".vscode.toml"]),
        ("Docker", "Docker containers", vec![".docker.toml"]),
        ("Apt", "APT packages", vec![".apt.toml"]),
        ("Nix", "Nix packages", vec![".nix.toml"]),
        ("Bash", "Bash scripts", vec![".bash.toml"]),
    ];

    println!("### DatumType Enum");
    println!("Determines package behavior based on file extension:");
    for (variant, description, extensions) in &package_types {
        println!(
            "- `{}`: {} ({})",
            variant,
            description,
            extensions.join(", ")
        );
    }
    println!();

    let file_org_doc = r#"## File Organization

Configuration files are stored in `$_B00T_Path` (default: `~/.dotfiles/_b00t_/`) with naming convention:
"#;
    print!("{}", file_org_doc);

    for (_, description, extensions) in &package_types {
        for ext in extensions {
            println!("- `<name>{}` - {}", ext, description);
        }
    }

    let workflow_doc = r#"

## Common Agent Workflows

### Adding New MCP Servers
```bash
# Method 1: Command syntax (recommended)
b00t mcp add brave-search --hint "Web search integration" -- npx -y @modelcontextprotocol/server-brave-search

# Method 2: JSON input
b00t mcp add '{"name":"github","command":"npx","args":["-y","@modelcontextprotocol/server-github"]}'

# Method 3: Pipe JSON from stdin
echo '{"name":"lsp","command":"npx","args":["-y","@modelcontextprotocol/server-lsp"]}' | b00t mcp add -
```

### Installing to Applications
```bash
# New hierarchical syntax (intuitive)
b00t app vscode mcp install filesystem
b00t app claudecode mcp install github

# Legacy syntax (still supported)
b00t mcp install filesystem vscode
b00t mcp install github claudecode
```

### Managing AI Providers
```bash
# Add AI provider from TOML file
b00t ai add ./openai.ai.toml

# List available providers
b00t ai list

# Export environment variables for use
b00t ai output --kv openai,anthropic
# Output: OPENAI_API_KEY=sk-... ANTHROPIC_API_KEY=sk-...

# Export TOML format
b00t ai output --b00t anthropic
```

### CLI Tool Management
```bash
# Detect installed version
b00t cli detect node
# Output: 20.11.0

# Show desired version from config
b00t cli desires node
# Output: 20.0.0

# Check version alignment with status emoji
b00t cli check node
# Output: 🥾🐣 node 20.11.0  (newer than desired)

# Install missing tool
b00t cli install rustc

# Update single tool
b00t cli update node

# Update all outdated tools
b00t cli up
```

## Safety & Validation Features

### Whitelisted Package Managers
Only these package managers are allowed in MCP add commands:
- `npx` - Node.js package executor
- `uvx` - Python package executor
- `pnpm` - Alternative Node.js package manager (requires `dlx`)
- `bunx` - Bun package executor
- `docker` - Docker container execution
- `just` - Command runner

### Example Safety Validation
```bash
# ✅ ALLOWED: Whitelisted package manager
b00t mcp add safe-server -- npx -y @safe/server

# ❌ BLOCKED: Non-whitelisted command
b00t mcp add malicious -- rm -rf /
# Error: Package manager 'rm' is not whitelisted
```

## Configuration Examples

### MCP Server Configuration
```toml
# ~/.dotfiles/_b00t_/filesystem.mcp.toml
[b00t]
name = "filesystem"
type = "mcp"
hint = "File system access for MCP"
command = "npx"
args = ["-y", "@modelcontextprotocol/server-filesystem", "--", "/allowed/path"]
```

### CLI Tool Configuration
```toml
# ~/.dotfiles/_b00t_/node.cli.toml
[b00t]
name = "node"
desires = "20.0.0"
hint = "Node.js JavaScript runtime"
install = "curl -fsSL https://deb.nodesource.com/setup_20.x | sudo -E bash - && sudo apt-get install -y nodejs"
version = "node --version"
version_regex = "v?(\\d+\\.\\d+\\.\\d+)"
```

### AI Provider Configuration
```toml
# ~/.dotfiles/_b00t_/openai.ai.toml
[b00t]
name = "openai"

[models]
"gpt-4" = "gpt-4"
"gpt-3.5-turbo" = "gpt-3.5-turbo"
"gpt-4-turbo" = "gpt-4-turbo-preview"

[env]
OPENAI_API_KEY = "${OPENAI_API_KEY}"
OPENAI_ORG_ID = "${OPENAI_ORG_ID}"
```

## Status Indicators & Exit Codes

### Version Status Emojis
- 🥾👍🏻 = Installed version matches desired exactly
- 🥾🐣 = Installed version newer than desired (acceptable)
- 🥾😭 = Installed version older than desired (needs update)
- 🥾😱 = Command/package missing entirely

### Exit Codes
- `0` = Success
- `1` = Version mismatch (older than desired)
- `2` = Package/command missing
- `100` = Configuration file not found

## Advanced Features

### Environment Variable Override
```bash
# Override default config path
export _B00T_Path="/custom/config/path"
b00t mcp list  # Uses custom path

# Or per-command
_B00T_Path="/tmp/test" b00t mcp add test -- npx test-server
```

### JSON Output for Integration
```bash
# Get structured data for automation
b00t mcp list --json
b00t ai list --json

# Generate MCP configuration for apps
b00t mcp output filesystem,github  # mcpServers format
b00t mcp output --json filesystem  # Raw JSON
```

## Development & Debugging

### Documentation Generation
```bash
# Generate this documentation (development only)
b00t-cli --doc > ARCHITECTURE.md
```

### Integration Testing
The codebase includes comprehensive integration tests that verify:
- Command mode functionality with whitelisted packages
- Security validation (rejection of harmful commands)
- Environment variable path overrides
- Both command syntaxes (hierarchical and legacy)

## Remember: Use `b00t`, Not `b00t-cli`
Unless you're developing b00t-cli itself, always use the `b00t` alias. It provides essential safety layers while being 10x more cost-effective than direct b00t-cli usage and 1000x more cost-effective than direct file manipulation.
"#;
    print!("{}", workflow_doc);
}

// Session management functions
fn handle_session_init(
    budget: &Option<f64>,
    time_limit: &Option<u32>,
    agent: Option<&str>,
) -> Result<()> {
    let agent_name = agent
        .map(|s| s.to_string())
        .or_else(|| std::env::var("_B00T_Agent").ok())
        .filter(|s| !s.is_empty());

    let mut session = SessionState::new(agent_name);

    if let Some(budget) = budget {
        session.budget_limit = Some(*budget);
    }

    if let Some(time_limit) = time_limit {
        session.time_limit_minutes = Some(*time_limit);
    }

    // Set session ID in environment
    unsafe {
        std::env::set_var("B00T_SESSION_ID", &session.session_id);
    }

    session.save()?;
    println!("🥾 Session {} initialized", session.session_id);

    if let Some(agent) = &session.agent_info {
        println!("🤖 Agent: {}", agent.name);
    }

    if let Some(budget) = session.budget_limit {
        println!("💰 Budget: ${:.2}", budget);
    }

    if let Some(time_limit) = session.time_limit_minutes {
        println!("⏱️  Time limit: {}m", time_limit);
    }

    Ok(())
}

fn handle_session_status() -> Result<()> {
    let session = SessionState::load()?;
    println!("{}", session.get_status_line());

    if !session.hints.is_empty() {
        println!("💡 Hints:");
        for hint in &session.hints {
            println!("   • {}", hint);
        }
    }

    Ok(())
}

fn handle_session_update(cost: &Option<f64>, hint: Option<&str>) -> Result<()> {
    let mut session = SessionState::load()?;

    if let Some(cost) = cost {
        session.increment_command(*cost);
    } else {
        session.increment_command(0.0);
    }

    if let Some(hint) = hint {
        session.hints.push(hint.to_string());
    }

    session.save()?;
    Ok(())
}

fn handle_session_end() -> Result<()> {
    let session = SessionState::load()?;
    let path = SessionState::get_session_file_path()?;

    println!("🥾 Session {} ended", session.session_id);
    println!("📊 Final stats: {}", session.get_status_line());

    if path.exists() {
        std::fs::remove_file(&path).context("Failed to remove session file")?;
    }

    unsafe {
        std::env::remove_var("B00T_SESSION_ID");
    }
    Ok(())
}

fn handle_session_prompt() -> Result<()> {
    let session = SessionState::load()?;
    print!("{}", session.get_status_line());
    Ok(())
}

fn main() {
    let cli = Cli::parse();

    if cli.doc {
        generate_documentation();
        return;
    }

    match &cli.command {
        Some(Commands::Mcp { mcp_command }) => match mcp_command {
            McpCommands::Create {
                name_or_json,
                hint,
                dwiw: _,
                no_dwiw,
                command_args,
            } => {
                if !command_args.is_empty() {
                    // Command mode: b00t-cli mcp create name -- command args...
                    if let Err(e) =
                        mcp_add_command(name_or_json, hint.as_deref(), command_args, &cli.path)
                    {
                        eprintln!("Error adding MCP server: {}", e);
                        std::process::exit(1);
                    }
                } else {
                    // JSON mode: b00t-cli mcp create '{"name":"..."}'
                    let use_dwiw = if *no_dwiw { false } else { true };
                    if let Err(e) = mcp_add_json(name_or_json, use_dwiw, &cli.path) {
                        eprintln!("Error adding MCP server: {}", e);
                        std::process::exit(1);
                    }
                }
            }
            McpCommands::List { json } => {
                if let Err(e) = mcp_list(&cli.path, *json) {
                    eprintln!("Error listing MCP servers: {}", e);
                    std::process::exit(1);
                }
            }
            McpCommands::Install {
                name,
                target,
                repo,
                user,
            } => {
                let result = match target.as_str() {
                    "claudecode" => claude_code_install_mcp(name, &cli.path),
                    "vscode" => vscode_install_mcp(name, &cli.path),
                    "geminicli" => {
                        // Determine installation location: default to repo if in git repo, otherwise user
                        let use_repo = if *repo && *user {
                            eprintln!("Error: Cannot specify both --repo and --user flags");
                            std::process::exit(1);
                        } else if *repo {
                            true
                        } else if *user {
                            false
                        } else {
                            // Default behavior: repo if in git repo, otherwise user
                            crate::utils::is_git_repo()
                        };
                        gemini_install_mcp(name, &cli.path, use_repo)
                    }
                    _ => {
                        eprintln!(
                            "Error: Invalid target '{}'. Valid targets are: claudecode, vscode, geminicli",
                            target
                        );
                        std::process::exit(1);
                    }
                };
                if let Err(e) = result {
                    eprintln!("Error installing MCP server to {}: {}", target, e);
                    std::process::exit(1);
                }
            }
            McpCommands::Output {
                json,
                mcp_servers: _,
                servers,
            } => {
                // Default to mcpServers format unless --json is specified
                let use_wrapper = if *json {
                    false
                } else {
                    true // Default behavior or explicit --mcpServers
                };

                if let Err(e) = mcp_output(&cli.path, use_wrapper, servers) {
                    eprintln!("Error outputting MCP servers: {}", e);
                    std::process::exit(1);
                }
            }
        },
        Some(Commands::Ai { ai_command }) => match ai_command {
            AiCommands::Add { file } => {
                if let Err(e) = ai_add(file, &cli.path) {
                    eprintln!("Error adding AI provider: {}", e);
                    std::process::exit(1);
                }
            }
            AiCommands::List { json } => {
                if let Err(e) = ai_list(&cli.path, *json) {
                    eprintln!("Error listing AI providers: {}", e);
                    std::process::exit(1);
                }
            }
            AiCommands::Output {
                b00t: _,
                kv,
                providers,
            } => {
                let format = if *kv { "kv" } else { "b00t" };
                if let Err(e) = ai_output(&cli.path, format, providers) {
                    eprintln!("Error outputting AI providers: {}", e);
                    std::process::exit(1);
                }
            }
        },
        Some(Commands::App { app_command }) => match app_command {
            AppCommands::Vscode { vscode_command } => match vscode_command {
                AppVscodeCommands::Mcp { mcp_command } => match mcp_command {
                    AppMcpCommands::Install { name } => {
                        if let Err(e) = vscode_install_mcp(name, &cli.path) {
                            eprintln!("Error installing MCP server to VSCode: {}", e);
                            std::process::exit(1);
                        }
                    }
                },
                AppVscodeCommands::Extension { extension_command } => match extension_command {
                    AppVscodeExtensionCommands::List { json } => {
                        if let Err(e) = vscode_extension_list(*json) {
                            eprintln!("Error listing VS Code extensions: {}", e);
                            std::process::exit(1);
                        }
                    }
                    AppVscodeExtensionCommands::Install { extension_id } => {
                        if let Err(e) = vscode_extension_install(extension_id) {
                            eprintln!("Error installing VS Code extension: {}", e);
                            std::process::exit(1);
                        }
                    }
                    AppVscodeExtensionCommands::Uninstall { extension_id } => {
                        if let Err(e) = vscode_extension_uninstall(extension_id) {
                            eprintln!("Error uninstalling VS Code extension: {}", e);
                            std::process::exit(1);
                        }
                    }
                },
            },
            AppCommands::Claudecode { claudecode_command } => match claudecode_command {
                AppClaudecodeCommands::Mcp { mcp_command } => match mcp_command {
                    AppMcpCommands::Install { name } => {
                        if let Err(e) = claude_code_install_mcp(name, &cli.path) {
                            eprintln!("Error installing MCP server to Claude Code: {}", e);
                            std::process::exit(1);
                        }
                    }
                },
            },
            AppCommands::Geminicli { geminicli_command } => match geminicli_command {
                AppGeminicliCommands::Mcp { mcp_command } => match mcp_command {
                    AppGeminicliMcpCommands::Install { name, repo, user } => {
                        // Determine installation location: default to repo if in git repo, otherwise user
                        let use_repo = if *repo && *user {
                            eprintln!("Error: Cannot specify both --repo and --user flags");
                            std::process::exit(1);
                        } else if *repo {
                            true
                        } else if *user {
                            false
                        } else {
                            // Default behavior: repo if in git repo, otherwise user
                            crate::utils::is_git_repo()
                        };

                        if let Err(e) = gemini_install_mcp(name, &cli.path, use_repo) {
                            eprintln!("Error installing MCP server to Gemini CLI: {}", e);
                            std::process::exit(1);
                        }
                    }
                },
            },
        },
        Some(Commands::Cli { cli_command }) => match cli_command {
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
        },
        Some(Commands::Init { init_command }) => match init_command {
            InitCommands::Aliases => {
                println!("Aliases initialization not yet implemented.");
                println!(
                    "This will scan for CLI tools with 'aliases' field and create ~/.local/bin scripts."
                );
            }
        },
        Some(Commands::Whoami) => {
            if let Err(e) = whoami(&cli.path) {
                eprintln!("Error displaying agent identity: {}", e);
                std::process::exit(1);
            }
        }
        Some(Commands::Checkpoint {
            message,
            skip_tests,
        }) => {
            if let Err(e) = checkpoint(message.as_deref(), *skip_tests) {
                eprintln!("Error creating checkpoint: {}", e);
                std::process::exit(1);
            }
        }
        Some(Commands::Whatismy { whatismy_command }) => match whatismy_command {
            WhatismyCommands::Agent { no_env } => {
                let agent = detect_agent(*no_env);
                println!("{}", agent);
            }
        },
        Some(Commands::Status {
            filter,
            installed,
            available,
        }) => {
            if let Err(e) = show_status(&cli.path, filter.as_deref(), *installed, *available) {
                eprintln!("Error displaying status: {}", e);
                std::process::exit(1);
            }
        }
        Some(Commands::Session { session_command }) => match session_command {
            SessionCommands::Init {
                budget,
                time_limit,
                agent,
            } => {
                if let Err(e) = handle_session_init(budget, time_limit, agent.as_deref()) {
                    eprintln!("Error initializing session: {}", e);
                    std::process::exit(1);
                }
            }
            SessionCommands::Status => {
                if let Err(e) = handle_session_status() {
                    eprintln!("Error getting session status: {}", e);
                    std::process::exit(1);
                }
            }
            SessionCommands::Update { cost, hint } => {
                if let Err(e) = handle_session_update(cost, hint.as_deref()) {
                    eprintln!("Error updating session: {}", e);
                    std::process::exit(1);
                }
            }
            SessionCommands::End => {
                if let Err(e) = handle_session_end() {
                    eprintln!("Error ending session: {}", e);
                    std::process::exit(1);
                }
            }
            SessionCommands::Prompt => {
                if let Err(e) = handle_session_prompt() {
                    eprintln!("Error getting session prompt: {}", e);
                    std::process::exit(1);
                }
            }
        },
        None => {
            eprintln!("No command provided. Use --help for usage information.");
            std::process::exit(1);
        }
    }
}

#[allow(dead_code)]
fn get_config(command: &str, path: &str) -> Result<(Config, String), Box<dyn std::error::Error>> {
    // Try different file extensions in order of preference
    let extensions = [
        ".cli.toml",
        ".mcp.toml",
        ".vscode.toml",
        ".docker.toml",
        ".apt.toml",
        ".nix.toml",
        ".bash.toml",
        ".toml",
    ];

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

fn get_cli_unified_config(
    command: &str,
    path: &str,
) -> Result<(Config, String), Box<dyn std::error::Error>> {
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
        Ok((config, filename)) => match config.b00t.get_datum_type(Some(&filename)) {
            DatumType::Unknown => {
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
            DatumType::Mcp => {
                eprintln!(
                    "Use 'b00t-cli vscode install mcp {}' or 'b00t-cli claude-code install mcp {}' instead",
                    command, command
                );
            }
            _ => {
                eprintln!(
                    "Installation not yet supported for {:?} packages",
                    config.b00t.get_datum_type(Some(&filename))
                );
            }
        },
        Err(e) => eprintln!("Error reading config for {}: {}", command, e),
    }
}

#[allow(dead_code)]
fn update(command: &str, path: &str) {
    match get_config(command, path) {
        Ok((config, filename)) => match config.b00t.get_datum_type(Some(&filename)) {
            DatumType::Unknown => {
                if let Some(update_cmd) =
                    config.b00t.update.as_ref().or(config.b00t.install.as_ref())
                {
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
                eprintln!(
                    "Update not yet supported for {:?} packages",
                    config.b00t.get_datum_type(Some(&filename))
                );
            }
        },
        Err(e) => eprintln!("Error reading config for {}: {}", command, e),
    }
}

#[allow(dead_code)]
fn get_installed_version(command: &str, path: &str) -> Option<String> {
    if let Ok((config, _)) = get_config(command, path) {
        if let Some(version_cmd) = &config.b00t.version {
            if let Ok(output) = cmd!("bash", "-c", version_cmd).read() {
                let re = Regex::new(
                    config
                        .b00t
                        .version_regex
                        .as_deref()
                        .unwrap_or("\\d+\\.\\d+\\.\\d+"),
                )
                .unwrap();
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
            println!(
                "🥾😭 {} IS {} WANTS {}",
                command, installed_version, desired_version
            );
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

fn create_mcp_toml_config(datum: &BootDatum, path: &str) -> Result<()> {
    create_unified_toml_config(datum, path)
}

fn is_whitelisted_package_manager(command: &str) -> bool {
    matches!(
        command,
        "bash" | "just" | "npx" | "uvx" | "pnpm" | "bunx" | "docker"
    )
}

fn mcp_add_command(
    name: &str,
    hint: Option<&str>,
    command_args: &[String],
    path: &str,
) -> Result<()> {
    if command_args.is_empty() {
        anyhow::bail!("No command provided. Use -- followed by the command and arguments.");
    }

    let command = &command_args[0];
    if !is_whitelisted_package_manager(command) {
        anyhow::bail!(
            "Package manager '{}' is not whitelisted. Allowed: just, npx, uvx, pnpm, bunx, docker",
            command
        );
    }

    // Handle special cases for package managers
    let (actual_command, args) = match command.as_str() {
        "pnpm" => {
            // For pnpm, we expect "pnpm dlx ..." so we need to validate "dlx" is present
            if command_args.len() < 2 || command_args[1] != "dlx" {
                anyhow::bail!("pnpm must be used with 'dlx' subcommand: pnpm dlx <package>");
            }
            ("pnpm".to_string(), command_args[1..].to_vec())
        }
        _ => (command.clone(), command_args[1..].to_vec()),
    };

    let datum = BootDatum {
        name: name.to_string(),
        datum_type: Some(DatumType::Mcp),
        desires: None,
        hint: hint.unwrap_or("MCP server").to_string(),
        install: None,
        update: None,
        version: None,
        version_regex: None,
        command: Some(actual_command),
        args: Some(args),
        vsix_id: None,
        script: None,
        image: None,
        docker_args: None,
        oci_uri: None,
        resource_path: None,
        chart_path: None,
        namespace: None,
        values_file: None,
        keywords: None,
        package_name: None,
        env: None,
        require: None,
        aliases: None,
    };

    create_mcp_toml_config(&datum, path)?;

    println!("MCP server '{}' configuration saved.", datum.name);
    println!(
        "Command: {} {}",
        datum.command.as_ref().unwrap(),
        datum.args.as_ref().unwrap().join(" ")
    );
    println!(
        "To install to VSCode: b00t-cli vscode install mcp {}",
        datum.name
    );
    println!(
        "To install to Claude Code: b00t-cli claude-code install mcp {}",
        datum.name
    );

    Ok(())
}

fn mcp_add_json(json: &str, dwiw: bool, path: &str) -> Result<()> {
    let json_content = if json == "-" {
        let mut buffer = String::new();
        match io::stdin().read_to_string(&mut buffer) {
            Ok(_) => {
                let trimmed = buffer.trim();
                if trimmed.is_empty() {
                    anyhow::bail!(
                        "No input provided. Pipe JSON content or press Ctrl+D after pasting."
                    );
                }
                trimmed.to_string()
            }
            Err(e) => {
                anyhow::bail!(
                    "Failed to read from stdin: {}. Pipe JSON content or use Ctrl+D after input.",
                    e
                );
            }
        }
    } else {
        json.trim().to_string()
    };

    let datum = normalize_mcp_json(&json_content, dwiw)?;

    create_mcp_toml_config(&datum, path)?;

    println!("MCP server '{}' configuration saved.", datum.name);
    println!(
        "To install to VSCode: b00t-cli vscode install mcp {}",
        datum.name
    );

    Ok(())
}

fn get_expanded_path(path: &str) -> Result<PathBuf> {
    Ok(PathBuf::from(shellexpand::tilde(path).to_string()))
}

fn get_mcp_toml_files(path: &str) -> Result<Vec<String>> {
    let expanded_path = get_expanded_path(path)?;
    let entries = fs::read_dir(&expanded_path)
        .with_context(|| format!("Error reading directory {}", expanded_path.display()))?;

    let mut mcp_files = Vec::new();
    for entry in entries {
        if let Ok(entry) = entry {
            let entry_path = entry.path();
            if let Some(file_name) = entry_path.file_name().and_then(|s| s.to_str()) {
                if file_name.ends_with(".mcp.toml") {
                    if let Some(server_name) = file_name.strip_suffix(".mcp.toml") {
                        mcp_files.push(server_name.to_string());
                    }
                }
            }
        }
    }
    Ok(mcp_files)
}

fn mcp_list(path: &str, json_output: bool) -> Result<()> {
    let mcp_files = get_mcp_toml_files(path)?;
    let mut mcp_items = Vec::new();

    for server_name in mcp_files {
        match get_mcp_config(&server_name, path) {
            Ok(datum) => {
                mcp_items.push(McpListItem {
                    name: server_name,
                    command: datum.command.clone(),
                    args: datum.args.clone(),
                    hint: Some(datum.hint.clone()),
                    error: None,
                });
            }
            Err(e) => {
                mcp_items.push(McpListItem {
                    name: server_name,
                    command: None,
                    args: None,
                    hint: None,
                    error: Some(e.to_string()),
                });
            }
        }
    }

    if json_output {
        let expanded_path = get_expanded_path(path)?;
        let output = McpListOutput {
            servers: mcp_items,
            path: expanded_path.display().to_string(),
        };
        let json_str = serde_json::to_string_pretty(&output)
            .context("Failed to serialize MCP list to JSON")?;
        println!("{}", json_str);
    } else {
        let expanded_path = get_expanded_path(path)?;
        if mcp_items.is_empty() {
            println!(
                "No MCP server configurations found in {}",
                expanded_path.display()
            );
            println!("Use 'b00t-cli mcp add <json>' to add MCP server configurations.");
        } else {
            println!("Available MCP servers in {}:", expanded_path.display());
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

fn get_mcp_config(name: &str, path: &str) -> Result<BootDatum> {
    let mut path_buf = get_expanded_path(path)?;
    path_buf.push(format!("{}.mcp.toml", name));

    if !path_buf.exists() {
        anyhow::bail!(
            "MCP server '{}' not found. Use 'b00t-cli mcp add' to create it first.",
            name
        );
    }

    let content = fs::read_to_string(&path_buf).context(format!(
        "Failed to read MCP config from {}",
        path_buf.display()
    ))?;

    let config: UnifiedConfig =
        toml::from_str(&content).context("Failed to parse MCP config TOML")?;

    Ok(config.b00t)
}

fn vscode_install_mcp(name: &str, path: &str) -> Result<()> {
    let datum = get_mcp_config(name, path)?;

    let vscode_json = serde_json::json!({
        "name": datum.name,
        "command": datum.command.as_ref().unwrap_or(&"npx".to_string()),
        "args": datum.args.as_ref().unwrap_or(&vec![])
    });

    let json_str =
        serde_json::to_string(&vscode_json).context("Failed to serialize JSON for VSCode")?;

    let result = cmd!("code", "--add-mcp", &json_str).run();

    match result {
        Ok(_) => {
            println!(
                "Successfully installed MCP server '{}' to VSCode",
                datum.name
            );
            println!("VSCode command: code --add-mcp '{}'", json_str);
        }
        Err(e) => {
            eprintln!("Failed to install MCP server to VSCode: {}", e);
            eprintln!("Manual command: code --add-mcp '{}'", json_str);
            std::process::exit(1);
        }
    }

    Ok(())
}

fn claude_code_install_mcp(name: &str, path: &str) -> Result<()> {
    let datum = get_mcp_config(name, path)?;

    // Claude Code uses claude-code config add-mcp command
    let claude_json = serde_json::json!({
        "name": datum.name,
        "command": datum.command.as_ref().unwrap_or(&"npx".to_string()),
        "args": datum.args.as_ref().unwrap_or(&vec![])
    });

    let json_str =
        serde_json::to_string(&claude_json).context("Failed to serialize JSON for Claude Code")?;

    let result = cmd!("claude", "mcp", "add-json", &datum.name, &json_str).run();

    match result {
        Ok(_) => {
            println!(
                "Successfully installed MCP server '{}' to Claude Code",
                datum.name
            );
            println!(
                "Claude Code command: claude mcp add-json {} '{}'",
                datum.name, json_str
            );
        }
        Err(e) => {
            eprintln!("Failed to install MCP server to Claude Code: {}", e);
            eprintln!(
                "Manual command: claude mcp add-json {} '{}'",
                datum.name, json_str
            );
            std::process::exit(1);
        }
    }

    Ok(())
}

fn cli_run(name: &str, path: &str) -> Result<()> {
    let datum = get_cli_config(name, path)?;

    if let Some(command) = &datum.command {
        println!("Running CLI script '{}'...", name);
        let result = cmd!("bash", "-c", command).run();
        match result {
            Ok(_) => {
                println!("CLI script '{}' completed successfully.", name);
            }
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
            println!(
                "🥾😭 {} IS {} WANTS {}",
                command, installed_version, desired_version
            );
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
                let re = Regex::new(
                    config
                        .b00t
                        .version_regex
                        .as_deref()
                        .unwrap_or("\\d+\\.\\d+\\.\\d+"),
                )
                .unwrap();
                if let Some(caps) = re.captures(&output) {
                    return Some(caps[0].to_string());
                }
            }
        }
    }
    None
}

fn get_cli_config(name: &str, path: &str) -> Result<BootDatum> {
    let mut path_buf = PathBuf::new();
    path_buf.push(shellexpand::tilde(path).to_string());
    path_buf.push(format!("{}.cli.toml", name));

    if !path_buf.exists() {
        anyhow::bail!(
            "CLI script '{}' not found. Create a {}.cli.toml file first.",
            name,
            name
        );
    }

    let content = fs::read_to_string(&path_buf).context(format!(
        "Failed to read CLI config from {}",
        path_buf.display()
    ))?;

    let config: UnifiedConfig =
        toml::from_str(&content).context("Failed to parse CLI config TOML")?;

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
        Ok((config, filename)) => match config.b00t.get_datum_type(Some(&filename)) {
            DatumType::Unknown => {
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
            DatumType::Mcp => {
                eprintln!(
                    "Use 'b00t-cli vscode install mcp {}' or 'b00t-cli claude-code install mcp {}' instead",
                    command, command
                );
            }
            _ => {
                eprintln!(
                    "Installation not yet supported for {:?} packages",
                    config.b00t.get_datum_type(Some(&filename))
                );
            }
        },
        Err(e) => eprintln!("Error reading config for {}: {}", command, e),
    }
}

fn cli_update(command: &str, path: &str) {
    match get_cli_unified_config(command, path) {
        Ok((config, filename)) => match config.b00t.get_datum_type(Some(&filename)) {
            DatumType::Unknown => {
                if let Some(update_cmd) =
                    config.b00t.update.as_ref().or(config.b00t.install.as_ref())
                {
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
                eprintln!(
                    "Update not yet supported for {:?} packages",
                    config.b00t.get_datum_type(Some(&filename))
                );
            }
        },
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
                let filename = entry_path
                    .file_name()
                    .and_then(|s| s.to_str())
                    .unwrap_or("");
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

                        if let Some(installed_version_str) =
                            get_cli_installed_version(command, path)
                        {
                            if let (Ok(desired_version), Ok(installed_version)) = (
                                semver::Version::parse(&desired_version_str),
                                semver::Version::parse(&installed_version_str),
                            ) {
                                if installed_version < desired_version {
                                    println!(
                                        "Updating {} from {} to {}...",
                                        command, installed_version, desired_version
                                    );
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

fn mcp_output(path: &str, use_mcp_servers_wrapper: bool, servers: &str) -> Result<()> {
    let requested_servers: Vec<&str> = servers.split(',').map(|s| s.trim()).collect();
    let mut server_configs = serde_json::Map::new();

    for server_name in requested_servers {
        if server_name.is_empty() {
            continue;
        }

        match get_mcp_config(server_name, path) {
            Ok(datum) => {
                let mut server_config = serde_json::Map::new();
                server_config.insert(
                    "command".to_string(),
                    serde_json::Value::String(datum.command.unwrap_or_else(|| "npx".to_string())),
                );
                server_config.insert(
                    "args".to_string(),
                    serde_json::Value::Array(
                        datum
                            .args
                            .unwrap_or_default()
                            .into_iter()
                            .map(serde_json::Value::String)
                            .collect(),
                    ),
                );

                server_configs.insert(
                    server_name.to_string(),
                    serde_json::Value::Object(server_config),
                );
            }
            Err(_) => {
                // Create a cute poopy log error indicator instead of stderr warning
                let mut error_config = serde_json::Map::new();
                error_config.insert(
                    "command".to_string(),
                    serde_json::Value::String("b00t:💩🪵".to_string()),
                );

                let utc_timestamp = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_secs();
                let utc_time = chrono::DateTime::from_timestamp(utc_timestamp as i64, 0)
                    .unwrap()
                    .format("%Y-%m-%dT%H:%M:%SZ")
                    .to_string();

                error_config.insert(
                    "args".to_string(),
                    serde_json::Value::Array(vec![
                        serde_json::Value::String(utc_time),
                        serde_json::Value::String(format!(
                            "server '{}' not found in _b00t_ directory",
                            server_name
                        )),
                    ]),
                );

                server_configs.insert(
                    server_name.to_string(),
                    serde_json::Value::Object(error_config),
                );
            }
        }
    }

    let output = if use_mcp_servers_wrapper {
        let mut wrapper = serde_json::Map::new();
        wrapper.insert(
            "mcpServers".to_string(),
            serde_json::Value::Object(server_configs),
        );
        serde_json::Value::Object(wrapper)
    } else {
        serde_json::Value::Object(server_configs)
    };

    let json_str =
        serde_json::to_string_pretty(&output).context("Failed to serialize MCP servers to JSON")?;
    println!("{}", json_str);

    Ok(())
}

fn get_ai_toml_files(path: &str) -> Result<Vec<String>> {
    let expanded_path = get_expanded_path(path)?;
    let entries = fs::read_dir(&expanded_path)
        .with_context(|| format!("Error reading directory {}", expanded_path.display()))?;

    let mut ai_files = Vec::new();
    for entry in entries {
        if let Ok(entry) = entry {
            let entry_path = entry.path();
            if let Some(file_name) = entry_path.file_name().and_then(|s| s.to_str()) {
                if file_name.ends_with(".ai.toml") {
                    if let Some(provider_name) = file_name.strip_suffix(".ai.toml") {
                        ai_files.push(provider_name.to_string());
                    }
                }
            }
        }
    }
    Ok(ai_files)
}

fn get_ai_config(name: &str, path: &str) -> Result<AiConfig> {
    let mut path_buf = get_expanded_path(path)?;
    path_buf.push(format!("{}.ai.toml", name));

    if !path_buf.exists() {
        anyhow::bail!(
            "AI provider '{}' not found. Use 'b00t-cli ai add' to create it first.",
            name
        );
    }

    let content = fs::read_to_string(&path_buf).context(format!(
        "Failed to read AI config from {}",
        path_buf.display()
    ))?;

    let config: AiConfig = toml::from_str(&content).context("Failed to parse AI config TOML")?;

    Ok(config)
}

fn ai_add(file_path: &str, path: &str) -> Result<()> {
    let content = fs::read_to_string(file_path)
        .context(format!("Failed to read AI config file: {}", file_path))?;

    let config: AiConfig = toml::from_str(&content).context("Failed to parse AI config TOML")?;

    create_ai_toml_config(&config, path)?;

    println!("AI provider '{}' configuration saved.", config.b00t.name);

    Ok(())
}

fn ai_list(path: &str, json_output: bool) -> Result<()> {
    let ai_files = get_ai_toml_files(path)?;
    let mut ai_items = Vec::new();

    for provider_name in ai_files {
        match get_ai_config(&provider_name, path) {
            Ok(config) => {
                let model_names = config
                    .models
                    .as_ref()
                    .map(|models| models.keys().cloned().collect())
                    .unwrap_or_default();
                let env_keys = config
                    .env
                    .as_ref()
                    .map(|env| env.keys().cloned().collect())
                    .unwrap_or_default();

                ai_items.push(AiListItem {
                    name: provider_name,
                    models: Some(model_names),
                    env_keys: Some(env_keys),
                    error: None,
                });
            }
            Err(e) => {
                ai_items.push(AiListItem {
                    name: provider_name,
                    models: None,
                    env_keys: None,
                    error: Some(e.to_string()),
                });
            }
        }
    }

    if json_output {
        let expanded_path = get_expanded_path(path)?;
        let output = AiListOutput {
            providers: ai_items,
            path: expanded_path.display().to_string(),
        };
        let json_str =
            serde_json::to_string_pretty(&output).context("Failed to serialize AI list to JSON")?;
        println!("{}", json_str);
    } else {
        let expanded_path = get_expanded_path(path)?;
        if ai_items.is_empty() {
            println!(
                "No AI provider configurations found in {}",
                expanded_path.display()
            );
            println!("Use 'b00t-cli ai add <file>' to add AI provider configurations.");
        } else {
            println!("Available AI providers in {}:", expanded_path.display());
            println!();
            for item in ai_items {
                match (&item.models, &item.env_keys) {
                    (Some(models), Some(env_keys)) => {
                        println!(
                            "🤖 {} ({} models, {} env vars)",
                            item.name,
                            models.len(),
                            env_keys.len()
                        );
                        if !models.is_empty() {
                            println!("   models: {}", models.join(", "));
                        }
                        if !env_keys.is_empty() {
                            println!("   env: {}", env_keys.join(", "));
                        }
                    }
                    _ => {
                        println!("❌ {} (error reading config)", item.name);
                    }
                }
            }
            println!();
            println!("Use 'b00t-cli ai output --kv <providers>' for environment variables");
            println!("Use 'b00t-cli ai output --b00t <providers>' for TOML format");
        }
    }

    Ok(())
}

fn substitute_env_vars(template: &str) -> String {
    let mut result = template.to_string();
    let env_var_pattern = Regex::new(r"\$\{([^}]+)\}").unwrap();

    while let Some(captures) = env_var_pattern.captures(&result) {
        let full_match = captures.get(0).unwrap().as_str();
        let var_name = captures.get(1).unwrap().as_str();
        let var_value = std::env::var(var_name).unwrap_or_default();
        result = result.replace(full_match, &var_value);
    }

    result
}

fn ai_output(path: &str, format: &str, providers: &str) -> Result<()> {
    let requested_providers: Vec<&str> = providers.split(',').map(|s| s.trim()).collect();

    for provider_name in requested_providers {
        if provider_name.is_empty() {
            continue;
        }

        match get_ai_config(provider_name, path) {
            Ok(config) => {
                match format {
                    "kv" => {
                        if let Some(env_vars) = &config.env {
                            for (key, value) in env_vars {
                                let final_value = if value.is_empty() {
                                    // If value is empty, try to get from environment
                                    std::env::var(key).unwrap_or_default()
                                } else {
                                    // Manual environment variable substitution
                                    substitute_env_vars(value)
                                };
                                println!("{}={}", key, final_value);
                            }
                        }
                    }
                    "b00t" | _ => {
                        let toml_str = toml::to_string(&config)
                            .context("Failed to serialize AI config to TOML")?;
                        println!("{}", toml_str);
                    }
                }
            }
            Err(e) => {
                eprintln!(
                    "Warning: Failed to read config for '{}': {}",
                    provider_name, e
                );
                continue;
            }
        }
    }

    Ok(())
}

fn vscode_extension_list(json_output: bool) -> Result<()> {
    let result = cmd!("code", "--list-extensions").read();

    match result {
        Ok(output) => {
            if json_output {
                let extensions: Vec<String> = output
                    .lines()
                    .map(|line| line.trim().to_string())
                    .filter(|line| !line.is_empty())
                    .collect();

                let json_data = serde_json::json!({
                    "extensions": extensions
                });
                let json_str = serde_json::to_string_pretty(&json_data)
                    .context("Failed to serialize extensions to JSON")?;
                println!("{}", json_str);
            } else {
                if output.trim().is_empty() {
                    println!("No VS Code extensions installed");
                } else {
                    println!("Installed VS Code extensions:");
                    for line in output.lines() {
                        let extension = line.trim();
                        if !extension.is_empty() {
                            println!("📦 {}", extension);
                        }
                    }
                }
            }
        }
        Err(e) => {
            anyhow::bail!(
                "Failed to list VS Code extensions: {}. Make sure VS Code CLI is installed and available.",
                e
            );
        }
    }

    Ok(())
}

fn vscode_extension_install(extension_id: &str) -> Result<()> {
    println!("Installing VS Code extension: {}", extension_id);

    let result = cmd!("code", "--install-extension", extension_id).run();

    match result {
        Ok(_) => {
            println!("✅ Successfully installed extension: {}", extension_id);
        }
        Err(e) => {
            anyhow::bail!(
                "Failed to install VS Code extension '{}': {}. Check that the extension ID is correct.",
                extension_id,
                e
            );
        }
    }

    Ok(())
}

fn vscode_extension_uninstall(extension_id: &str) -> Result<()> {
    println!("Uninstalling VS Code extension: {}", extension_id);

    let result = cmd!("code", "--uninstall-extension", extension_id).run();

    match result {
        Ok(_) => {
            println!("✅ Successfully uninstalled extension: {}", extension_id);
        }
        Err(e) => {
            anyhow::bail!(
                "Failed to uninstall VS Code extension '{}': {}. Check that the extension is installed.",
                extension_id,
                e
            );
        }
    }

    Ok(())
}
