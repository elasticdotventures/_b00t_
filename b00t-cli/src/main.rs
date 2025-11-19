use anyhow::{Context, Result};
use clap::Parser;
use duct::cmd;
// use regex::Regex;
// use semver::Version;
use std::fs;
// use std::io::{Read};
// use std::path::PathBuf;
// 🤓 cleaned up unused Tera import after switching to simple string replacement
use b00t_cli::{SessionState, UnifiedConfig, whoami};

// 🦨 Module declarations removed - these are declared in lib.rs now
// Import from b00t_cli:: instead
use b00t_cli::datum_ai::AiDatum;
use b00t_cli::datum_ai_model::AiModelDatumEntry;
use b00t_cli::datum_apt::AptDatum;
use b00t_cli::datum_bash::BashDatum;
use b00t_cli::datum_cli::CliDatum;
use b00t_cli::datum_docker::DockerDatum;
use b00t_cli::datum_mcp::McpDatum;
use b00t_cli::datum_vscode::VscodeDatum;
use b00t_cli::traits::*;
use b00t_cli::utils::get_workspace_root;

use b00t_cli::commands::learn::{LearnArgs, handle_learn};
use b00t_cli::commands::{
    AiCommands, AppCommands, BootstrapCommands, BudgetCommands, ChatCommands, CliCommands,
    DatumCommands, GrokCommands, InitCommands, InstallCommands, K8sCommands, McpCommands,
    SessionCommands, StackCommands, WhatismyCommands,
};

// Re-export commonly used functions for datum modules
pub use b00t_cli::{
    claude_code_install_mcp, codex_install_mcp, dotmcpjson_install_mcp, gemini_install_mcp,
    get_config, get_expanded_path, get_mcp_config, get_mcp_toml_files, mcp_add_json, mcp_list,
    mcp_output, mcp_remove, vscode_install_mcp,
};

mod integration_tests;

#[derive(Parser)]
#[clap(version = b00t_c0re_lib::version::VERSION, about, long_about = None)]
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
    #[clap(
        about = "Count tokens in a string using tiktoken",
        long_about = r#"
Count tokens in a string using OpenAI's tiktoken tokenizer.

Usage:
  b00t-cli tiktoken "your text here"

Example:
  b00t-cli tiktoken "This is a test sentence."
  # Output: Token count: 7
"#
    )]
    Tiktoken {
        #[clap(help = "Text to tokenize")]
        text: String,
    },
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
    #[clap(about = "Software stack management")]
    Stack {
        #[clap(subcommand)]
        stack_command: StackCommands,
    },
    #[clap(about = "Budget-aware scheduling and tracking")]
    Budget {
        #[clap(subcommand)]
        budget_command: BudgetCommands,
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
    #[clap(
        about = "AI model datum management",
        long_about = "List, inspect, install, and activate AI model datums defined in the _b00t_ directory."
    )]
    Model {
        #[clap(subcommand)]
        model_command: b00t_cli::commands::ModelCommands,
    },
    #[clap(
        name = ".",
        about = "Check installed vs desired version for CLI command",
        long_about = "Check if a CLI tool's installed version matches the desired version.\n\nThis is a shorthand for: b00t-cli cli check <command>\n\nExamples:\n  b00t-cli . dagu\n  b00t-cli . git\n  b00t-cli . just"
    )]
    DotCheck {
        #[clap(help = "Command name to check")]
        command: String,
    },
    #[clap(about = "Execute RHAI scripts with b00t context")]
    Script {
        #[clap(subcommand)]
        script_command: b00t_cli::commands::script::ScriptCommands,
    },
    #[clap(about = "Initialize system settings and aliases")]
    Init {
        #[clap(subcommand)]
        init_command: InitCommands,
    },
    #[clap(about = "Show agent identity and context information")]
    Whoami,
    #[clap(about = "Create checkpoint: commit all files and run tests")]
    // 🤓 ENTANGLED: b00t-mcp/src/mcp_tools.rs CheckpointCommand
    // When this changes, update b00t-mcp CheckpointCommand structure
    Checkpoint {
        #[clap(short, long, help = "Commit message for the checkpoint")]
        message: Option<String>,
        #[clap(long, help = "Skip running tests (not recommended)")]
        skip_tests: bool,

        #[clap(long = "message", help = "Commit message (MCP compatibility)")]
        message_flag: Option<String>, // 🦨 MCP compatibility: accept --message flag
    },
    #[clap(about = "Query system information")]
    Whatismy {
        #[clap(subcommand)]
        whatismy_command: WhatismyCommands,
    },
    #[clap(about = "Show status dashboard of all available tools and services")]
    // 🤓 ENTANGLED: b00t-mcp/src/mcp_tools.rs StatusCommand
    // When this changes, update b00t-mcp StatusCommand structure
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
    #[clap(about = "Kubernetes (k8s) cluster and pod management")]
    K8s {
        #[clap(subcommand)]
        k8s_command: K8sCommands,
    },
    #[clap(about = "Run 'just install' to install b00t components")]
    Install {
        #[clap(subcommand)]
        install_command: InstallCommands,
    },
    #[clap(about = "Session management")]
    Session {
        #[clap(subcommand)]
        session_command: SessionCommands,
    },
    #[clap(about = "Agent coordination and management")]
    Agent {
        #[clap(subcommand)]
        agent_command: b00t_cli::commands::AgentCommands,
    },
    #[clap(about = "Job workflow orchestration with checkpoints and sub-agents")]
    Job {
        #[clap(subcommand)]
        job_command: b00t_cli::commands::JobCommands,
    },
    #[clap(about = "Agent Coordination Protocol (ACP) - send messages to agents")]
    Chat {
        #[clap(subcommand)]
        chat_command: ChatCommands,
    },
    #[clap(about = "Learn about topics with unified knowledge management")]
    // 🤓 ENTANGLED (synchronized): b00t-mcp/src/mcp_tools.rs LearnCommand now uses LearnArgs wrapper, matching CLI structure.
    // Unified knowledge command: LFMF lessons, learn docs, man pages, RAG
    Learn(LearnArgs),
    #[clap(about = "Datum management and inspection")]
    Datum {
        #[clap(subcommand)]
        datum_command: DatumCommands,
    },
    #[clap(about = "Grok knowledgebase RAG system")]
    Grok {
        #[clap(subcommand)]
        grok_command: GrokCommands,
    },
    #[clap(
        about = "Update all datums defined in _b00t_.toml",
        long_about = "Check and update all datums according to _b00t_.toml configuration.\n\nBy default, checks versions only. Use --yes to actually perform updates.\n\nConfiguration file priority:\n1. <git_root>/_b00t_.toml (project-specific, if in a git repo)\n2. ~/.b00t/_b00t_.toml (user-level)\n\nNote: Projects may have a _b00t_/ directory for project-specific datums.\n\nExamples:\n  b00t up           # Check all datums\n  b00t up --yes     # Update outdated datums\n  b00t up -y        # Same as --yes"
    )]
    Up {
        #[clap(
            short = 'y',
            long = "yes",
            help = "Actually perform updates (default: check only)"
        )]
        yes: bool,
    },
    #[clap(about = "Bootstrap self-configuring b00t installation (Phase 0: Foundation)")]
    Bootstrap {
        #[clap(subcommand)]
        bootstrap_command: BootstrapCommands,
    },
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

fn handle_up_command(_b00t_path: &str, yes: bool) -> Result<()> {
    use b00t_cli::datum_config::B00tConfig;

    // Load or create configuration
    let (config, config_path) = B00tConfig::load_or_create()?;

    if yes {
        println!("🔄 Updating all datums from {}...", config_path.display());
    } else {
        println!(
            "🔍 Checking all datums from {} (use --yes to update)...",
            config_path.display()
        );
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

    println!("\n📋 Configuration loaded:");
    println!("   Version: {}", config.version);
    println!("   Datums: {:?}", config.datums);
    println!("   Install methods: {:?}", config.install_methods);

    // 🤓 TODO: Implement datum loading and updating
    // Currently blocked by trait version conflicts - needs refactoring
    println!("\n⚠️  Full datum checking not yet implemented");
    println!("   Next steps:");
    println!("   1. Load datums from _b00t_ path");
    println!("   2. Match against configured patterns");
    println!("   3. Check versions and update if --yes flag is set");

    Ok(())
}

fn checkpoint(message: Option<&str>, skip_tests: bool) -> Result<()> {
    println!("🥾 Creating checkpoint...");

    // Check if we're in a git repository
    let git_status = cmd!("git", "status", "--porcelain").read();
    if git_status.is_err() {
        anyhow::bail!("Not in a git repository. Run 'git init' first.");
    }

    // Track checkpoint attempt in session memory
    let mut memory = b00t_cli::session_memory::SessionMemory::load().unwrap_or_default();
    let checkpoint_count = memory.incr("checkpoint_count").unwrap_or(1);

    // Check if this is a Rust project and run cargo check
    if std::path::Path::new("Cargo.toml").exists() {
        println!("🦀 Rust project detected. Running cargo check...");
        let cargo_check = cmd!("cargo", "check").run();
        if let Err(e) = cargo_check {
            let _ = memory.incr("failed_builds");
            anyhow::bail!(
                "🚨 cargo check failed: {}. Fix compilation errors before checkpoint.",
                e
            );
        }
        println!("✅ cargo check passed");
    }

    // Generate commit message with checkpoint number
    let default_msg = format!(
        "🥾 checkpoint #{}: automated commit via b00t-cli",
        checkpoint_count
    );
    let commit_msg = message.unwrap_or(&default_msg);

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
            let _ = memory.incr("successful_commits");

            // Show the commit hash
            if let Ok(commit_hash) = cmd!("git", "rev-parse", "--short", "HEAD").read() {
                println!("📍 Commit: {}", commit_hash.trim());
                let _ = memory.set("last_commit_hash", commit_hash.trim());
            }

            // Show current branch
            if let Ok(branch) = cmd!("git", "branch", "--show-current").read() {
                println!("🌳 Branch: {}", branch.trim());
                let _ = memory.set("current_branch", branch.trim());
            }

            if !skip_tests {
                println!("🧪 Tests executed via git pre-commit hooks");
            }

            // CI integration hints
            println!("💡 Next steps:");
            println!("   • Run `git push` to trigger CI pipeline");
            println!(
                "   • Create PR: `gh pr create --title \"{}\"` (if ready)",
                commit_msg
            );
        }
        Err(e) => {
            let _ = memory.incr("failed_commits");
            anyhow::bail!(
                "Commit failed: {}. This usually means git pre-commit hooks (including tests) failed.",
                e
            );
        }
    }

    Ok(())
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
        AiModelDatumEntry,
    >(
        path, ".ai_model.toml"
    )?));
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
pub fn handle_session_init(
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

    // Initialize session memory and check README.md
    let mut memory = b00t_cli::session_memory::SessionMemory::load()?;
    check_readme_status(&mut memory)?;

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

pub fn handle_session_status() -> Result<()> {
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

pub fn handle_session_update(cost: &Option<f64>, hint: Option<&str>) -> Result<()> {
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

pub fn handle_session_end() -> Result<()> {
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

pub fn handle_session_prompt() -> Result<()> {
    let session = SessionState::load()?;
    print!("{}", session.get_status_line());
    Ok(())
}

/// Check if README.md exists and track reading status
fn check_readme_status(memory: &mut b00t_cli::session_memory::SessionMemory) -> Result<()> {
    let git_root = get_workspace_root();
    let readme_path = std::path::PathBuf::from(&git_root).join("README.md");

    if readme_path.exists() {
        if !memory.is_readme_read() {
            println!("📖 README.md found but not yet marked as read");
            println!("💡 Run `b00t-cli session mark-readme-read` after reading it");
        } else {
            println!("✅ README.md already read this session");
        }
    } else {
        println!("ℹ️  No README.md found in git root");
    }

    Ok(())
}

#[tokio::main]
async fn main() {
    let cli = Cli::parse();

    if cli.doc {
        generate_documentation();
        return;
    }

    match &cli.command {
        Some(Commands::Tiktoken { text }) => {
            if let Err(e) = b00t_cli::commands::tiktoken::handle_tiktoken(text) {
                eprintln!("Error: {}", e);
                std::process::exit(1);
            }
        }
        Some(Commands::Mcp { mcp_command }) => {
            if let Err(e) = mcp_command.execute_async(&cli.path).await {
                eprintln!("Error: {}", e);
                std::process::exit(1);
            }
        }
        Some(Commands::Ai { ai_command }) => {
            if let Err(e) = ai_command.execute(&cli.path) {
                eprintln!("Error: {}", e);
                std::process::exit(1);
            }
        }
        Some(Commands::Stack { stack_command }) => {
            if let Err(e) = stack_command.execute(&cli.path) {
                eprintln!("Error: {}", e);
                std::process::exit(1);
            }
        }
        Some(Commands::Budget { budget_command }) => {
            if let Err(e) = budget_command.execute(&cli.path) {
                eprintln!("Error: {}", e);
                std::process::exit(1);
            }
        }
        Some(Commands::App { app_command }) => {
            if let Err(e) = app_command.execute(&cli.path) {
                eprintln!("Error: {}", e);
                std::process::exit(1);
            }
        }
        Some(Commands::Cli { cli_command }) => {
            if let Err(e) = cli_command.execute(&cli.path) {
                eprintln!("Error: {}", e);
                std::process::exit(1);
            }
        }
        Some(Commands::Model { model_command }) => {
            if let Err(e) = model_command.execute(&cli.path) {
                eprintln!("Error: {}", e);
                std::process::exit(1);
            }
        }
        Some(Commands::DotCheck { command }) => {
            // Shorthand for cli check
            let check_cmd = CliCommands::Check {
                command: command.clone(),
            };
            if let Err(e) = check_cmd.execute(&cli.path) {
                eprintln!("Error: {}", e);
                std::process::exit(1);
            }
        }
        Some(Commands::Init { init_command }) => {
            if let Err(e) = init_command.execute(&cli.path) {
                eprintln!("Error: {}", e);
                std::process::exit(1);
            }
        }
        Some(Commands::Whoami) => {
            if let Err(e) = whoami::whoami(&cli.path) {
                eprintln!("Error: {}", e);
                std::process::exit(1);
            }
        }
        Some(Commands::Checkpoint {
            message,
            skip_tests,
            message_flag,
        }) => {
            // 🦨 MCP compatibility: merge positional and flag arguments
            let effective_message = message.as_ref().or(message_flag.as_ref());
            if let Err(e) = checkpoint(effective_message.map(|s| s.as_str()), *skip_tests) {
                eprintln!("Error: {}", e);
                std::process::exit(1);
            }
        }
        Some(Commands::Whatismy { whatismy_command }) => {
            if let Err(e) = whatismy_command.execute(&cli.path) {
                eprintln!("Error: {}", e);
                std::process::exit(1);
            }
        }
        Some(Commands::Status {
            filter,
            installed,
            available,
        }) => {
            if let Err(e) = show_status(
                &cli.path,
                filter.as_ref().map(|s| s.as_str()),
                *installed,
                *available,
            ) {
                eprintln!("Error: {}", e);
                std::process::exit(1);
            }
        }
        Some(Commands::K8s { k8s_command }) => {
            if let Err(e) = k8s_command.execute(&cli.path) {
                eprintln!("Error: {}", e);
                std::process::exit(1);
            }
        }
        Some(Commands::Install { install_command }) => {
            if let Err(e) = install_command.execute(&cli.path) {
                eprintln!("Error: {}", e);
                std::process::exit(1);
            }
        }
        Some(Commands::Session { session_command }) => {
            if let Err(e) = session_command.execute(&cli.path) {
                eprintln!("Error: {}", e);
                std::process::exit(1);
            }
        }
        Some(Commands::Agent { agent_command }) => {
            if let Err(e) =
                b00t_cli::commands::agent::handle_agent_command(agent_command.clone()).await
            {
                eprintln!("Agent Error: {}", e);
                std::process::exit(1);
            }
        }
        Some(Commands::Job { job_command }) => {
            if let Err(e) = job_command.execute_async(&cli.path).await {
                eprintln!("Job Error: {}", e);
                std::process::exit(1);
            }
        }
        Some(Commands::Chat { chat_command }) => {
            if let Err(e) = chat_command.execute().await {
                eprintln!("Chat Error: {}", e);
                std::process::exit(1);
            }
        }
        Some(Commands::Learn(args)) => {
            if let Err(e) = handle_learn(&cli.path, args.clone()) {
                eprintln!("Error: {}", e);
                std::process::exit(1);
            }
        }
        Some(Commands::Datum { datum_command }) => {
            use b00t_cli::commands::datum::handle_datum_command;
            if let Err(e) = handle_datum_command(&cli.path, datum_command) {
                eprintln!("Error: {}", e);
                std::process::exit(1);
            }
        }
        Some(Commands::Grok { grok_command }) => {
            use b00t_cli::commands::grok::handle_grok_command;

            // 🤓 No need for nested runtime - already in #[tokio::main]
            if let Err(e) = handle_grok_command(grok_command.clone()).await {
                eprintln!("Error: {}", e);
                std::process::exit(1);
            }
        }
        Some(Commands::Up { yes }) => {
            if let Err(e) = handle_up_command(&cli.path, *yes) {
                eprintln!("Error: {}", e);
                std::process::exit(1);
            }
        }
        Some(Commands::Bootstrap { bootstrap_command }) => {
            use b00t_cli::commands::bootstrap::handle_bootstrap_command;

            if let Err(e) = handle_bootstrap_command(bootstrap_command.clone()).await {
                eprintln!("Error: {}", e);
                std::process::exit(1);
            }
        }
        Some(Commands::Script { script_command }) => {
            use b00t_cli::commands::script::handle_script_command;

            if let Err(e) = handle_script_command(script_command.clone()) {
                eprintln!("Error: {}", e);
                std::process::exit(1);
            }
        }
        None => {
            eprintln!("No command provided. Use --help for usage information.");
            std::process::exit(1);
        }
    }
}
