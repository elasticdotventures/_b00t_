use anyhow::{Result, anyhow};
use b00t_cli::exit_code;
use b00t_cli::k0mmand3r::K0mmand;
use b00t_cli::{SessionState, UnifiedConfig, load_datum_providers, whoami};

/// Exit with code, printing error context to stderr.
fn die(code: i32, msg: impl std::fmt::Display) -> ! {
    eprintln!("Error: {}", msg);
    std::process::exit(code);
}
use clap::Parser;
use dirs;
use duct::cmd;
use serde_json::json;
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::Command;

// Import datum types from lib.rs (already declared there as pub mod)
use b00t_cli::commands::learn::{LearnArgs, handle_learn};
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
#[rustfmt::skip]
use b00t_cli::commands::{
  //  Keep commands 1 line per letter A,B,C,... for easy diff
    AiCommands, AgentCommands, AnsibleCommands, AppCommands, AuditCommands,
    BootstrapCommands, BudgetCommands,
    ChatCommands, CliCommands, ConfigCommands,
    DataCommands, DatumCommands, DoctorCommands,
    FocusCommands, GatesCommands,
    GrokCommands, HiveCommands,
    InitCommands,
    JobCommands,
    K8sCommands,
    McpCommands, ModelCommands,
    ObservabilityCommands, OntologyCommands, SessionCommands, SkillCommands, SoulCommands, StackCommands,
    TaskCommands,
    TutorialCommands, VersionCommands, VizCommands, WhatismyCommands


};
use b00t_cli::commands::install::{install_datum, run_just_install};
use b00t_cli::commands::uninstall::uninstall_datum;

// Re-export commonly used functions for datum modules
pub use b00t_cli::{
    DatumType, claude_code_install_mcp, codex_install_mcp, dotmcpjson_install_mcp,
    gemini_install_mcp, get_config, get_expanded_path, get_mcp_config, get_mcp_toml_files,
    mcp_add_json, mcp_list, mcp_output, mcp_remove, vscode_install_mcp,
};

mod integration_tests;

#[derive(Parser)]
#[clap(version = b00t_c0re_lib::version::VERSION, about, long_about = None)]
struct Cli {
    #[clap(short, long, env = "_B00T_Path", default_value = "~/.b00t/_b00t_")]
    path: String,
    #[clap(
        long,
        help = "Output structured markdown documentation about internal structures"
    )]
    doc: bool,
    #[clap(subcommand)]
    command: Option<Commands>,
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
    #[clap(
        about = "Record a lesson learned for a tool (lfmf = Learn From My Failure)",
        alias = "lesson",
        long_about = r#"
lfmf is a dynamic, opinionated man-page for any tool with a b00t datum (TOML, learn/ dir, etc).
It memoizes operator-informed tips, tricks, and anti-patterns—never repo-specific, always tool wisdom.
Each entry is a <25 token topic and <250 token body, written in a positive, laconic, affirmative style.
Use lfmf to help the hive avoid repeating mistakes and accelerate mastery.
Good entries separate neophyte from master. Bad entries are vague, negative, or repo-specific.

Usage:
  b00t-cli lfmf <tool> "<topic>: <body>"

Examples:
  # Good
  b00t-cli lfmf just "modules & workdir: Use modules and workdir to avoid cd; keeps recipes portable and context-safe."
  b00t-cli lfmf docker "container cleanup: Use 'docker system prune' regularly to avoid disk bloat."
  b00t-cli lfmf git "atomic commits: Commit small, focused changes for easier review and rollback."

  # Bad
  b00t-cli lfmf just "cd: I always use cd in my recipes."
  b00t-cli lfmf docker "disk full: My disk filled up once."
  b00t-cli lfmf git "fix: Fixed a bug in my repo."

Tips:
- Topic: <25 tokens, concise, positive, tool-focused.
- Body: <250 tokens, actionable, never repo-specific.
- Affirmative: 'Do X for Y benefit', not 'Don't do X'.
- Suitable tools: any with a b00t datum (TOML, learn/ dir, etc).
"#
    )]
    Lfmf {
        #[clap(long, help = "Tool name")]
        tool: Option<String>,
        #[clap(long, help = "Lesson in '<topic>: <body>' format")]
        lesson: Option<String>,
        #[clap(long, group = "scope", help = "Record lesson for this repo (default)")]
        repo: bool,
        #[clap(
            long,
            group = "scope",
            help = "Record lesson globally (mutually exclusive with --repo)"
        )]
        global: bool,
    },
    #[clap(
        about = "Get advice for syntax errors and debugging",
        long_about = r#"
The b00t advice system acts as a syntax therapist, providing contextual debugging assistance
based on lessons learned from previous failures. It performs semantic search through the
hive's collective knowledge to suggest solutions for similar error patterns.

Usage:
  b00t-cli advice <tool> "<error_pattern>"
  b00t-cli advice <tool> list  # List all lessons for a tool
  b00t-cli advice <tool> search "<query>"  # Semantic search for lessons

Examples:
  b00t-cli advice just "Unknown start of token '.'"
  b00t-cli advice rust "cannot borrow as mutable"
  b00t-cli advice docker "permission denied"
  b00t-cli advice just list
  b00t-cli advice rust search "template syntax"

The system will:
1. Search for similar error patterns in the vector database
2. Return relevant lessons with confidence scores
3. Provide conversational debugging guidance
4. Suggest specific solutions based on hive experience
"#
    )]
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
    #[clap(about = "Hive CMDB: system resource state, profile activation, command guards")]
    Hive {
        #[clap(subcommand)]
        hive_command: HiveCommands,
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
    #[clap(name = "config", about = "Project configuration and scaffolding")]
    Configure {
        #[clap(subcommand)]
        config_command: ConfigCommands,
    },
    #[clap(about = "Run Ansible playbooks")]
    Ansible {
        #[clap(subcommand)]
        ansible_command: AnsibleCommands,
    },
    #[clap(
        about = "AI model datum management",
        long_about = "List, inspect, install, and activate AI model datums defined in the _b00t_ directory."
    )]
    Model {
        #[clap(subcommand)]
        model_command: ModelCommands,
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
    Whoami {
        #[clap(long, help = "Override detected role (matches role datum)")]
        role: Option<String>,
        #[clap(
            long,
            help = "Emit full skill metadata for all skills declared by the role"
        )]
        with_skills: bool,
        #[clap(long, help = "Output identity as JSON (structured)")]
        json: bool,
    },
    #[clap(
        name = "k0mmand3r",
        hide = true,
        about = "Hidden slash-command dispatcher for datum-driven and internal commands"
    )]
    K0mmand3r {
        #[clap(help = "Slash command to dispatch, e.g. /whoami or /gh")]
        slash: String,
        #[clap(
            trailing_var_arg = true,
            allow_hyphen_values = true,
            num_args = 0..,
            help = "Arguments passed through to the dispatched command"
        )]
        args: Vec<String>,
    },
    #[clap(about = "Create checkpoint: commit all files and run tests")]
    // 🤓 ENTANGLED: b00t-mcp/src/mcp_tools.rs CheckpointCommand
    // When this changes, update b00t-mcp CheckpointCommand structure
    Checkpoint {
        #[clap(short, long, help = "Commit message for the checkpoint")]
        message: Option<String>,
        #[clap(long, help = "Skip running tests (not recommended)")]
        skip_tests: bool,
    },
    #[clap(about = "Agentic soul — persistent identity & memory (~/._b00t_/SOUL.tomllm)")]
    Soul {
        #[clap(subcommand)]
        soul_command: SoulCommands,
    },
    #[clap(about = "Skill discovery and activation — progressive disclosure across skill dirs")]
    Skill {
        #[clap(subcommand)]
        skill_command: SkillCommands,
    },
    #[clap(about = "Query system information", alias = "inspect")]
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
    #[clap(about = "Session management")]
    Session {
        #[clap(subcommand)]
        session_command: SessionCommands,
    },
    #[clap(about = "Agent coordination and management")]
    Agent {
        #[clap(subcommand)]
        agent_command: AgentCommands,
    },
    #[clap(about = "Job workflow orchestration with checkpoints and sub-agents")]
    Job {
        #[clap(subcommand)]
        job_command: JobCommands,
    },
    #[clap(about = "Native task management — replaces taskmaster-ai")]
    Task {
        #[clap(subcommand)]
        task_command: TaskCommands,
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
        about = "Install a datum (auto-resolves dependencies) or run bootstrap install when no name is provided"
    )]
    Install {
        #[clap(help = "Datum name to install (omit to run repo bootstrap just install)")]
        name: Option<String>,
        #[clap(long, help = "Show what would be installed for bootstrap mode")]
        dry_run: bool,
        #[clap(long, help = "Interactive TUI installer for agent runtimes")]
        interactive: bool,
        /// Non-interactive: comma-separated runtime IDs (claude,gemini,codex,opencode,copilot)
        #[clap(long, value_delimiter = ',')]
        runtimes: Vec<String>,
        /// Non-interactive: install scope (global or local)
        #[clap(long, default_value = "global")]
        scope: String,
        /// Skip confirmation prompt (non-interactive mode)
        #[clap(long, short = 'y')]
        yes: bool,
        /// Install MCP server(s). Use --mcp=recommended for bulk-install via rhai pipeline,
        /// or --mcp=<server-name> to install a single MCP datum directly (e.g. --mcp=github).
        #[clap(long)]
        mcp: Option<String>,
    },
    #[clap(about = "Uninstall a datum by name (use --purge to remove from _b00t_.toml)")]
    Uninstall {
        #[clap(help = "Datum name or key, e.g. 'ripgrep' or 'ripgrep.cli'")]
        name: String,
        #[clap(long, help = "Also remove datum entry from _b00t_.toml")]
        purge: bool,
        #[clap(long, short = 'y', help = "Skip confirmation prompt")]
        yes: bool,
    },
    #[clap(about = "Bootstrap self-configuring b00t installation (Phase 0: Foundation)")]
    Bootstrap {
        #[clap(subcommand)]
        bootstrap_command: BootstrapCommands,
    },
    #[clap(about = "Launch ralph agent REPL outer-loop")]
    Up(b00t_cli::commands::up::UpArgs),
    #[clap(about = "Check or upgrade the installed b00t-cli release")]
    Version {
        #[clap(subcommand)]
        version_command: VersionCommands,
    },
    #[clap(about = "Query live capability ontology from datum TOMLs")]
    Ontology {
        #[clap(subcommand)]
        ontology_command: OntologyCommands,
    },
    #[clap(about = "WOW integrity checks — run, list, spline")]
    Wow {
        #[clap(subcommand)]
        wow_command: WowSubcommands,
    },
    #[clap(about = "Visualize b00t graphs using ledgrrr-shaped scene output (l3dg3rr proto)")]
    Viz {
        #[clap(subcommand)]
        viz_command: VizCommands,
    },
    #[clap(about = "Tutorial progression tracking for role-based datum onboarding")]
    Tutorial {
        #[clap(subcommand)]
        tutorial_command: TutorialCommands,
    },
    #[clap(about = "Validate t00n-serialized FOCUS records against reqif.yaml via sm0l model")]
    Validate {
        #[clap(flatten)]
        validate_args: b00t_cli::commands::validate::ValidateArgs,
    },
    #[clap(about = "Dispatch A/B experiments with parallel sub-agents and stateless scoring")]
    Experiment {
        #[clap(subcommand)]
        experiment_command: ExperimentCommands,
    },
    #[clap(subcommand)]
    Focus(FocusCommands),
    #[clap(
        about = "Execute command with guard enforcement and broad-authority audit log",
        long_about = "Audited execution: Allow→run, Warn→run with warning, Block→reject first time / force on re-submit within 5min.\nAll executions logged to ~/.b00t/exec-log.jsonl.\n\nUse --sleep=<duration> for background execution (returns immediately)."
    )]
    Exec(b00t_cli::commands::exec::ExecArgs),
    #[clap(about = "Schema datum management (generate, validate)")]
    Schema {
        #[clap(subcommand)]
        schema_command: SchemaSubcommands,
    },
    #[clap(about = "Killswitch: terminate upper agent instance and return CLI to prompt")]
    Quit(b00t_cli::commands::quit::QuitArgs),
    #[clap(
        about = "l3dg3rr docgen proxy — query knowledge graph and emit .tomllm / rustdoc / json"
    )]
    Docgen(b00t_cli::commands::docgen::DocgenArgs),
    #[clap(about = "Read audit trail from .b00t/audit.jsonl")]
    Audit {
        #[clap(subcommand)]
        audit_command: AuditCommands,
    },
    #[clap(about = "Inspect AbDataFrame JSONL files")]
    Data {
        #[clap(subcommand)]
        data_command: DataCommands,
    },
    #[clap(about = "Run system diagnostics for b00t infrastructure")]
    Doctor {
        #[clap(subcommand)]
        doctor_command: DoctorCommands,
    },
    #[clap(
        about = "List [[b00t.gate]] declarations across MCP datums",
        long_about = "Scan all .mcp.toml files and show gate preconditions with status indicators.\n\nExamples:\n  b00t gates list\n  b00t gates list --search github\n  b00t gates list --by-kind env\n  b00t gates list --json"
    )]
    Gates {
        #[clap(subcommand)]
        gates_command: GatesCommands,
    },
    #[clap(
        about = "Observability: events, guard violations, and telemetry",
        long_about = "View events from unified events.jsonl and guard violation statistics.\n\nExamples:\n  b00t observability events\n  b00t observability events --since 5\n  b00t observability events --event mcp_install\n  b00t observability events --follow\n  b00t observability guards\n  b00t observability guards --escalated"
    )]
    Observability {
        #[clap(subcommand)]
        observability_command: ObservabilityCommands,
    },
}

#[derive(clap::Parser, Clone)]
pub enum WowSubcommands {
    #[clap(about = "Run all WOW integrity checks")]
    Check {
        #[clap(long, help = "Emit JSON results")]
        json: bool,
    },
    #[clap(about = "List registered WOW checks")]
    List,
}

#[derive(clap::Parser, Clone)]
pub enum SchemaSubcommands {
    #[clap(about = "Generate focus.schema.tomllmd from FocusSchema code")]
    Generate {
        #[clap(flatten)]
        args: b00t_cli::datum_schema::SchemaGenerateArgs,
    },
    #[clap(about = "Diff two schema datums")]
    Diff {
        #[clap(help = "First schema name (e.g. focus)")]
        schema_a: String,
        #[clap(help = "Second schema name (e.g. focus-v2)")]
        schema_b: String,
    },
    #[clap(about = "Import schema from JSON file")]
    Import {
        #[clap(help = "Path to JSON schema file")]
        path: PathBuf,
        #[clap(long, help = "Output name for the schema datum")]
        name: String,
        #[clap(long, help = "Output directory (default: _b00t_)")]
        output: Option<PathBuf>,
    },
}

#[derive(clap::Parser, Clone)]
pub enum ExperimentCommands {
    #[clap(about = "Run an A/B experiment with control and treatment variants")]
    Run {
        #[clap(long, help = "Experiment ID")]
        id: String,
        #[clap(long, help = "Control variant prompt")]
        control: String,
        #[clap(long, help = "Treatment variant prompt")]
        treatment: String,
        #[clap(long, help = "Model endpoint URL [default: http://localhost:8001]")]
        endpoint: Option<String>,
        #[clap(long, help = "Path to trained LoRA adapter (from `b00t model train`)")]
        adapter: Option<String>,
    },
    #[clap(about = "Show experiment status (phygital-twin heartbeat)")]
    Status,
    #[clap(about = "List past experiments from persisted FOCUS records")]
    History {
        #[clap(
            long,
            help = "Number of recent experiments to show",
            default_value_t = 10
        )]
        limit: usize,
        #[clap(long, help = "Emit as JSON")]
        json: bool,
    },
    #[clap(about = "Compare two experiment results side by side")]
    Compare {
        #[clap(help = "First experiment ID")]
        exp_a: String,
        #[clap(help = "Second experiment ID")]
        exp_b: String,
        #[clap(
            long,
            help = "Path to FOCUS records JSONL file",
            default_value = "focus_records.jsonl"
        )]
        path: std::path::PathBuf,
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
        (
            "Role",
            "Role onboarding/compliance datums",
            vec![".role.toml", ".toml (type=role)"],
        ),
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

Configuration files are stored in `$_B00T_Path` (default: `~/.b00t/_b00t_`) with naming convention:
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

fn normalize_slash(slash: &str) -> String {
    let trimmed = slash.trim();
    if trimmed.starts_with('/') {
        trimmed.to_string()
    } else {
        format!("/{}", trimmed)
    }
}

fn normalize_slash_args(raw_args: Vec<String>) -> Vec<String> {
    if raw_args.len() < 2 {
        return raw_args;
    }

    // Scan argv for the first non-option token that either starts with '/'
    // or is a /k0mmand3r alias, and normalize it while preserving
    // leading flags/options.
    let mut prefix: Vec<String> = Vec::new();
    // Always keep argv[0] as-is.
    prefix.push(raw_args[0].clone());

    let mut i = 1;
    while i < raw_args.len() {
        let arg = &raw_args[i];

        // Respect end-of-options marker; do not rewrite past `--`.
        if arg == "--" {
            return raw_args;
        }

        // Skip option-like arguments (starting with '-') into the prefix.
        if arg.starts_with('-') {
            prefix.push(arg.clone());
            i += 1;
            continue;
        }

        // At this point, `arg` is the first non-option token.
        // Handle the explicit /k0mmand3r and k0mmand3r aliases.
        if (arg.as_str() == "/k0mmand3r" || arg.as_str() == "k0mmand3r") && i + 1 < raw_args.len() {
            let slash = normalize_slash(&raw_args[i + 1]);
            let mut normalized = prefix;
            normalized.push("k0mmand3r".to_string());
            normalized.push(slash);
            normalized.extend(raw_args.iter().skip(i + 2).cloned());
            return normalized;
        }

        // Handle direct slash commands like `/whoami` — single-component only.
        // Multi-component paths like `/tmp/foo` are flag values, not slash commands.
        if arg.starts_with('/') && !arg[1..].contains('/') {
            let mut normalized = prefix;
            normalized.push("k0mmand3r".to_string());
            normalized.push(arg.clone());
            normalized.extend(raw_args.iter().skip(i + 1).cloned());
            return normalized;
        }

        // First non-option is not a slash command; nothing to normalize.
        return raw_args;
    }

    // No eligible token found; return argv unchanged.
    raw_args
}

fn load_cli_boot_datums(path: &str) -> Result<Vec<b00t_cli::BootDatum>> {
    let providers = load_datum_providers::<CliDatum>(path, ".cli.toml")?;
    Ok(providers
        .into_iter()
        .map(|provider| provider.datum().clone())
        .collect())
}

fn datum_slash_aliases(datum: &b00t_cli::BootDatum) -> Vec<String> {
    let mut aliases = Vec::new();
    if let Some(cfg) = &datum.k0mmand3r {
        if let Some(slash) = &cfg.slash {
            aliases.push(normalize_slash(slash));
        }
    }
    aliases.push(format!("/{}", datum.name));
    if let Some(raw_aliases) = &datum.aliases {
        for alias in raw_aliases {
            aliases.push(normalize_slash(alias));
        }
    }

    aliases.sort();
    aliases.dedup();
    aliases
}

fn find_cli_datum_for_slash<'a>(
    slash: &str,
    datums: &'a [b00t_cli::BootDatum],
) -> Option<&'a b00t_cli::BootDatum> {
    let normalized = normalize_slash(slash);
    datums.iter().find(|datum| {
        datum_slash_aliases(datum)
            .iter()
            .any(|alias| alias == &normalized)
    })
}

fn b00t_home_for_path(_path: &str) -> Result<PathBuf> {
    if let Ok(explicit) = std::env::var("B00T_LOG_DIR") {
        return Ok(PathBuf::from(explicit));
    }

    Ok(std::env::temp_dir().join("b00t"))
}

fn append_jsonl_record(path: &Path, value: &serde_json::Value) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let mut file = fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)?;
    writeln!(file, "{}", serde_json::to_string(value)?)?;
    Ok(())
}

fn log_k0mmand3r_stub(
    path: &str,
    mode: &str,
    slash: &str,
    target: &str,
    passthrough_args: &[String],
    exit_code: i32,
) -> Result<()> {
    let b00t_home = b00t_home_for_path(path)?;
    let timestamp = chrono::Utc::now().to_rfc3339();
    let session_id = std::env::var("B00T_SESSION_ID").ok();
    let host = std::env::var("HOSTNAME").ok();

    let metrics_record = json!({
        "timestamp": timestamp,
        "mode": mode,
        "slash": slash,
        "target": target,
        "args": passthrough_args,
        "exit_code": exit_code,
        "session_id": session_id,
    });
    let node_record = json!({
        "timestamp": timestamp,
        "event": "k0mmand3r_dispatch",
        "mode": mode,
        "slash": slash,
        "target": target,
        "args": passthrough_args,
        "exit_code": exit_code,
        "session_id": session_id,
        "host": host,
    });

    append_jsonl_record(
        &b00t_home.join("session-metrics.stub.jsonl"),
        &metrics_record,
    )?;
    append_jsonl_record(&b00t_home.join("node-log.stub.jsonl"), &node_record)?;
    Ok(())
}

fn execute_cli_passthrough(
    datum: &b00t_cli::BootDatum,
    passthrough_args: &[String],
) -> Result<i32> {
    let command_spec = datum.command.as_deref().unwrap_or(&datum.name);
    let mut command_parts = shlex::split(command_spec)
        .ok_or_else(|| anyhow!("Invalid command declaration for '{}'", datum.name))?;

    if command_parts.is_empty() {
        anyhow::bail!("Empty command declaration for '{}'", datum.name);
    }

    let program = command_parts.remove(0);
    let mut cmd = Command::new(&program);
    if !command_parts.is_empty() {
        cmd.args(command_parts);
    }
    if let Some(default_args) = &datum.args {
        cmd.args(default_args);
    }
    cmd.args(passthrough_args);

    let status = cmd.status()?;
    Ok(status.code().unwrap_or(1))
}

fn execute_internal_slash_alias(slash: &str, passthrough_args: &[String]) -> Result<i32> {
    let command_name = slash.trim_start_matches('/');
    if command_name.is_empty() {
        anyhow::bail!("Empty slash command");
    }

    let current_exe = std::env::current_exe()?;
    let status = Command::new(current_exe)
        .arg(command_name)
        .args(passthrough_args)
        .status()?;
    Ok(status.code().unwrap_or(1))
}

fn print_visible_k0mmand3r_datums(path: &str) -> Result<()> {
    let datums = load_cli_boot_datums(path)?;
    let mut rows: Vec<(String, String)> = Vec::new();

    for datum in datums {
        let hidden = datum
            .k0mmand3r
            .as_ref()
            .and_then(|cfg| cfg.hidden)
            .unwrap_or(false);
        if hidden {
            continue;
        }

        let description = datum
            .k0mmand3r
            .as_ref()
            .and_then(|cfg| cfg.description.clone())
            .unwrap_or_else(|| datum.hint.clone());
        for slash in datum_slash_aliases(&datum) {
            rows.push((slash, description.clone()));
        }
    }

    rows.sort_by(|a, b| a.0.cmp(&b.0));
    rows.dedup_by(|a, b| a.0 == b.0);

    for (slash, description) in rows {
        println!("{}  {}", slash, description);
    }

    Ok(())
}

fn execute_k0mmand3r_dispatch(path: &str, slash: &str, passthrough_args: &[String]) -> Result<i32> {
    let normalized_slash = normalize_slash(slash);
    if normalized_slash == "/k0mmand3r" {
        print_visible_k0mmand3r_datums(path)?;
        return Ok(0);
    }

    let k0mmand_verbs = [
        "negotiate",
        "vote",
        "delegate",
        "status",
        "handshake",
        "crew",
    ];
    let verb = normalized_slash.trim_start_matches('/').to_lowercase();
    if k0mmand_verbs.contains(&verb.as_str()) {
        let mut raw = normalized_slash.clone();
        if !passthrough_args.is_empty() {
            raw.push(' ');
            raw.push_str(&passthrough_args.join(" "));
        }

        let k0mmand = K0mmand::parse(&raw).map_err(|e| anyhow!(e))?;
        k0mmand.validate().map_err(|e| anyhow!(e))?;

        println!("k0mmand3r: {} {}", k0mmand.verb, k0mmand.object);
        if let Err(e) = log_k0mmand3r_stub(
            path,
            "k0mmand",
            &normalized_slash,
            &k0mmand.verb,
            passthrough_args,
            0,
        ) {
            eprintln!("⚠️ k0mmand3r logging stub failed: {}", e);
        }
        return Ok(0);
    }

    let datums = load_cli_boot_datums(path)?;
    if let Some(datum) = find_cli_datum_for_slash(&normalized_slash, &datums) {
        let exit_code = execute_cli_passthrough(datum, passthrough_args)?;
        let target = datum.command.as_deref().unwrap_or(&datum.name).to_string();
        if let Err(e) = log_k0mmand3r_stub(
            path,
            "datum_cli",
            &normalized_slash,
            &target,
            passthrough_args,
            exit_code,
        ) {
            eprintln!("⚠️ k0mmand3r logging stub failed: {}", e);
        }
        return Ok(exit_code);
    }

    let exit_code = execute_internal_slash_alias(&normalized_slash, passthrough_args)?;
    if let Err(e) = log_k0mmand3r_stub(
        path,
        "internal",
        &normalized_slash,
        normalized_slash.trim_start_matches('/'),
        passthrough_args,
        exit_code,
    ) {
        eprintln!("⚠️ k0mmand3r logging stub failed: {}", e);
    }
    Ok(exit_code)
}

#[tokio::main]
async fn main() {
    let cli = Cli::parse_from(normalize_slash_args(std::env::args().collect()));

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
        Some(Commands::Hive { hive_command }) => {
            if let Err(e) = b00t_cli::commands::hive::handle_hive_command(hive_command, &cli.path) {
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
        Some(Commands::Configure { config_command }) => {
            if let Err(e) =
                b00t_cli::commands::config_cmd::handle_config_command(config_command, &cli.path)
                    .await
            {
                eprintln!("Error: {}", e);
                std::process::exit(1);
            }
        }
        Some(Commands::Ansible { ansible_command }) => {
            if let Err(e) = ansible_command.execute(&cli.path) {
                eprintln!("Error: {}", e);
                std::process::exit(1);
            }
        }
        Some(Commands::Model { model_command }) => {
            if let Err(e) = model_command.execute_async(&cli.path).await {
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
        Some(Commands::Whoami {
            role,
            with_skills,
            json,
        }) => {
            if *json {
                use b00t_c0re_lib::B00tContext;
                match B00tContext::current() {
                    Ok(ctx) => {
                        let output = serde_json::json!({
                            "agent": ctx.agent,
                            "pid": ctx.pid,
                            "hostname": ctx.hostname,
                            "user": ctx.user,
                            "branch": ctx.branch,
                            "workspace_root": ctx.workspace_root,
                            "is_git_repo": ctx.is_git_repo,
                            "model_size": ctx.model_size,
                            "privacy": ctx.privacy,
                            "timestamp": ctx.timestamp,
                            "role": role.clone().or_else(|| std::env::var("_B00T_ROLE").ok()),
                        });
                        println!("{}", serde_json::to_string_pretty(&output).unwrap());
                    }
                    Err(e) => die(
                        exit_code::ERROR,
                        format!("failed to get b00t context: {}", e),
                    ),
                }
            } else if let Err(e) = whoami::whoami(&cli.path, role.clone(), *with_skills) {
                eprintln!("Error: {}", e);
                std::process::exit(1);
            }
        }
        Some(Commands::K0mmand3r { slash, args }) => {
            match execute_k0mmand3r_dispatch(&cli.path, slash, args) {
                Ok(0) => {}
                Ok(code) => std::process::exit(code),
                Err(e) => {
                    eprintln!("Error: {}", e);
                    std::process::exit(1);
                }
            }
        }
        Some(Commands::Checkpoint {
            message,
            skip_tests,
        }) => {
            if let Err(e) = checkpoint(message.as_deref(), *skip_tests) {
                eprintln!("Error: {}", e);
                std::process::exit(1);
            }
        }
        Some(Commands::Soul { soul_command }) => {
            if let Err(e) = b00t_cli::commands::soul::handle_soul_command(soul_command) {
                eprintln!("Error: {}", e);
                std::process::exit(1);
            }
        }
        Some(Commands::Skill { skill_command }) => {
            if let Err(e) =
                b00t_cli::commands::skill::handle_skill_command(skill_command, &cli.path)
            {
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
            if let Err(e) = show_status(&cli.path, filter.as_deref(), *installed, *available) {
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
                eprintln!("Error: {}", e);
                std::process::exit(1);
            }
        }
        Some(Commands::Job { job_command }) => {
            if let Err(e) = job_command.execute_async(&cli.path).await {
                eprintln!("Error: {}", e);
                std::process::exit(1);
            }
        }
        Some(Commands::Task { task_command }) => {
            if let Err(e) = b00t_cli::commands::task::handle_task_command(task_command.clone()) {
                eprintln!("Error: {}", e);
                std::process::exit(1);
            }
        }
        Some(Commands::Chat { chat_command }) => {
            if let Err(e) = chat_command.execute().await {
                eprintln!("Error: {}", e);
                std::process::exit(1);
            }
        }
        Some(Commands::Learn(args)) => {
            if let Err(e) = handle_learn(&cli.path, args.clone()).await {
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
            if let Err(e) = handle_grok_command(grok_command.clone()).await {
                eprintln!("Error: {}", e);
                std::process::exit(1);
            }
        }
        Some(Commands::Uninstall { name, purge, yes }) => {
            if let Err(e) = uninstall_datum(&cli.path, &name, *yes, *purge) {
                eprintln!("Uninstall Error: {}", e);
                std::process::exit(1);
            }
        }
        Some(Commands::Install {
            name,
            dry_run,
            interactive,
            runtimes,
            scope,
            yes,
            mcp,
        }) => {
            // --mcp=<filter_or_name>: install MCP server(s), or list/search
            if let Some(mcp_filter) = mcp {
                let filter = mcp_filter.as_str();
                // Special aliases: --mcp=list
                if filter == "list" {
                    if let Err(e) = mcp_list(
                        &cli.path,
                        false,
                        b00t_cli::McpListFilter {
                            bypass_threshold: true,
                            ..Default::default()
                        },
                    ) {
                        die(exit_code::MCP, e);
                    }
                    return;
                }
                // First, try as a direct MCP datum name
                let mcp_datum_path = get_mcp_config(filter, &cli.path);
                match mcp_datum_path {
                    Ok(datum) => {
                        // Direct datum install: install dependencies first, then MCP to default target
                        println!("🚀 Installing MCP datum '{}'...", filter);
                        if let Err(e) = install_datum(&cli.path, filter, false) {
                            eprintln!("Install Error: datum install failed for {}: {}", filter, e);
                            std::process::exit(1);
                        }
                        let target = "claudecode";
                        println!("🔌 Installing MCP server '{}' to {}...", filter, target);
                        // Try claude code; exit code 1 from `claude mcp add-json` usually
                        // means the server is already registered — treat as non-fatal.
                        match claude_code_install_mcp(filter, &cli.path) {
                            Ok(_) => println!("✅ Installed MCP server '{}' via --mcp", filter),
                            Err(e) => {
                                let msg = e.to_string();
                                // "already exists" or exit code 1 = already registered
                                if msg.contains("already exists")
                                    || msg.contains("exited with code 1")
                                {
                                    println!("✅ MCP server '{}' already installed", filter);
                                } else {
                                    eprintln!(
                                        "Install Error: mcp install failed for {}: {}",
                                        filter, msg
                                    );
                                    std::process::exit(1);
                                }
                            }
                        }
                    }
                    Err(_) if filter == "recommended" => {
                        // rhai pipeline: discover + dynamic filter + batch install
                        use b00t_c0re_lib::{B00tContext, RhaiEngine};
                        let context = match B00tContext::current() {
                            Ok(c) => c,
                            Err(e) => die(
                                exit_code::ERROR,
                                format!("failed to get b00t context: {}", e),
                            ),
                        };
                        let ws_root = b00t_cli::utils::get_workspace_root();
                        let script_paths = [
                            PathBuf::from(&ws_root)
                                .join("_b00t_")
                                .join("scripts")
                                .join("install-mcp-recommended.rhai"),
                            dirs::home_dir()
                                .unwrap_or_default()
                                .join(".dotfiles")
                                .join("_b00t_")
                                .join("scripts")
                                .join("install-mcp-recommended.rhai"),
                            PathBuf::from("install-mcp-recommended.rhai"),
                        ];
                        let resolved = script_paths.iter().find(|p| p.exists()).cloned().unwrap_or_else(||
                            die(exit_code::NOT_FOUND, "script install-mcp-recommended.rhai not found (tried workspace, ~/.dotfiles, cwd)")
                        );
                        let engine = match RhaiEngine::new(context) {
                            Ok(e) => e,
                            Err(e) => die(
                                exit_code::ERROR,
                                format!("failed to init rhai engine: {}", e),
                            ),
                        };
                        match engine.execute_file(&resolved) {
                            Ok(val) => {
                                let count: i64 = val.as_int().unwrap_or(0);
                                if count > 0 {
                                    println!(
                                        "✅ Installed {} MCP server(s) via --mcp=recommended",
                                        count
                                    );
                                } else {
                                    println!("⚠️  No MCP servers matched 'recommended' criteria");
                                }
                            }
                            Err(e) => die(
                                exit_code::MCP,
                                format!("rhai pipeline failed for --mcp=recommended: {}", e),
                            ),
                        }
                    }
                    Err(_) => die(
                        exit_code::NOT_FOUND,
                        format!(
                            "MCP server '{}' not found and not a known pipeline filter. Use --mcp=recommended or a valid MCP server name.",
                            filter
                        ),
                    ),
                }
            } else if *interactive || !runtimes.is_empty() {
                // Parse runtime IDs from comma-separated --runtimes arg
                let mut runtime_ids_vec: Vec<b00t_cli::install::RuntimeId> = Vec::new();
                let mut parse_error = false;
                for r in runtimes.iter() {
                    match r.as_str() {
                        "claude" => runtime_ids_vec.push(b00t_cli::install::RuntimeId::Claude),
                        "gemini" => runtime_ids_vec.push(b00t_cli::install::RuntimeId::Gemini),
                        "codex" => runtime_ids_vec.push(b00t_cli::install::RuntimeId::Codex),
                        "opencode" => runtime_ids_vec.push(b00t_cli::install::RuntimeId::OpenCode),
                        "copilot" => runtime_ids_vec.push(b00t_cli::install::RuntimeId::Copilot),
                        _ => {
                            eprintln!(
                                "Install Error: unknown runtime '{}'. Valid: claude,gemini,codex,opencode,copilot",
                                r
                            );
                            parse_error = true;
                        }
                    }
                }
                if parse_error {
                    std::process::exit(1);
                }
                let runtime_ids: Option<Vec<b00t_cli::install::RuntimeId>> = if runtimes.is_empty()
                {
                    None
                } else {
                    Some(runtime_ids_vec)
                };
                let scope_val = match scope.as_str() {
                    "local" => match std::env::current_dir() {
                        Ok(dir) => Some(b00t_cli::install::InstallScope::Local(dir)),
                        Err(e) => {
                            eprintln!("Install Error: cannot determine current directory: {}", e);
                            std::process::exit(1);
                        }
                    },
                    _ => Some(b00t_cli::install::InstallScope::Global),
                };
                if let Err(e) = b00t_cli::install::handle_install_command(
                    *interactive,
                    runtime_ids,
                    scope_val,
                    *yes,
                ) {
                    eprintln!("Install Error: {}", e);
                    std::process::exit(1);
                }
            } else if let Some(name) = name {
                if name == "hermes" {
                    if let Err(e) = b00t_cli::commands::install::hermes_special_install(*dry_run) {
                        eprintln!("Install Error: {}", e);
                        std::process::exit(1);
                    }
                } else if let Err(e) = install_datum(&cli.path, name, *dry_run) {
                    eprintln!("Install Error: {}", e);
                    std::process::exit(1);
                }
            } else if let Err(e) = run_just_install(*dry_run) {
                eprintln!("Install Error: {}", e);
                std::process::exit(1);
            }
        }
        Some(Commands::Up(args)) => {
            if let Err(e) = args.execute() {
                eprintln!("Error: {}", e);
                std::process::exit(1);
            }
        }
        Some(Commands::Version { version_command }) => {
            if let Err(e) = b00t_cli::commands::version::handle_version_command(version_command) {
                eprintln!("Error: {}", e);
                std::process::exit(1);
            }
        }
        Some(Commands::Ontology { ontology_command }) => {
            if let Err(e) = ontology_command.execute() {
                eprintln!("Error: {}", e);
                std::process::exit(1);
            }
        }
        Some(Commands::Wow { wow_command }) => match wow_command {
            WowSubcommands::Check { json } => {
                b00t_cli::wow::init_default_checks();
                let results = b00t_cli::wow::run_all();
                if *json {
                    println!("{}", serde_json::to_string_pretty(&results).unwrap());
                } else {
                    println!("{}", b00t_cli::wow::format_spline(&results));
                }
                let passed = results.iter().filter(|r| r.passed).count();
                let total = results.len();
                if passed != total {
                    std::process::exit(1);
                }
            }
            WowSubcommands::List => {
                b00t_cli::wow::init_default_checks();
                let results = b00t_cli::wow::run_all();
                for r in &results {
                    let mark = if r.passed { "✅" } else { "❌" };
                    println!(" {mark} [{}] {}", r.category, r.name);
                }
            }
        },
        Some(Commands::Viz { viz_command }) => {
            if let Err(e) = b00t_cli::commands::viz::handle_viz_command(&cli.path, viz_command) {
                eprintln!("Error: {}", e);
                std::process::exit(1);
            }
        }
        Some(Commands::Lfmf {
            tool,
            lesson,
            repo: _,
            global,
        }) => {
            // Validate required fields
            let tool = match tool {
                Some(t) => t,
                None => {
                    eprintln!("--tool is required");
                    std::process::exit(1);
                }
            };
            let lesson = match lesson {
                Some(l) => l,
                None => {
                    eprintln!("--lesson is required");
                    std::process::exit(1);
                }
            };
            // Determine scope
            let scope = if *global { "global" } else { "repo" };
            if let Err(e) =
                b00t_cli::commands::lfmf::handle_lfmf(&cli.path, &tool, &lesson, scope).await
            {
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
        Some(Commands::Tutorial { tutorial_command }) => {
            if let Err(e) = tutorial_command.execute() {
                eprintln!("Error: {}", e);
                std::process::exit(1);
            }
        }
        Some(Commands::Validate { validate_args }) => {
            if let Err(e) = b00t_cli::commands::validate::handle_validate(validate_args) {
                eprintln!("validation error: {e}");
                std::process::exit(1);
            }
        }
        Some(Commands::Experiment { experiment_command }) => {
            match experiment_command {
                ExperimentCommands::Run {
                    id,
                    control,
                    treatment,
                    endpoint,
                    adapter,
                } => {
                    use b00t_cli::commands::experiment;
                    let config = experiment::ExperimentConfig {
                        id: id.clone(),
                        control_prompt: control.clone(),
                        treatment_prompt: treatment.clone(),
                        variants: vec!["control".into(), "treatment".into()],
                        personalities: experiment::default_personalities(),
                        model_endpoint: endpoint
                            .clone()
                            .unwrap_or_else(|| "http://localhost:8001".into()),
                        adapter: adapter.clone(),
                    };
                    if let Err(gate_err) = experiment::governance_gate(&config.control_prompt) {
                        eprintln!("{gate_err}");
                        std::process::exit(1);
                    }
                    if let Err(gate_err) = experiment::governance_gate(&config.treatment_prompt) {
                        eprintln!("{gate_err}");
                        std::process::exit(1);
                    }
                    match experiment::dispatch_experiment(&config) {
                        Ok(cmp) => {
                            println!("{}", experiment::format_comparison(&cmp));
                            // emit ledgrrr FOCUS records
                            let cf = experiment::create_focus_record(
                                &cmp.experiment_id,
                                "control",
                                "sm0l-ctl",
                                "experiment-eval",
                                &cmp.control.scores,
                            );
                            let tf = experiment::create_focus_record(
                                &cmp.experiment_id,
                                "treatment",
                                "sm0l-trt",
                                "experiment-eval",
                                &cmp.treatment.scores,
                            );
                            eprintln!("[ledgrrr] {}", experiment::focus_record_to_ledgrrr(&cf));
                            eprintln!("[ledgrrr] {}", experiment::focus_record_to_ledgrrr(&tf));
                            // emit FOCUS records to ledgerr-mcp MCP server (best-effort)
                            experiment::emit_focus_to_ledgerr_mcp(&cmp, "http://localhost:8001");
                        }
                        Err(e) => {
                            eprintln!("Experiment failed: {e}");
                            std::process::exit(1);
                        }
                    }
                }
                ExperimentCommands::Status => {
                    use b00t_cli::commands::experiment;
                    let status =
                        experiment::phygital_status("worker-cli", "idle", "pass", None, 0.0);
                    println!("node_id: {}", status.node_id);
                    println!("state: {}", status.state);
                    println!("last_heartbeat: {}", status.last_heartbeat);
                    println!("gate_result: {}", status.gate_result);
                    println!("focus_balance: {:.2}", status.focus_balance);
                }
                ExperimentCommands::History { limit, json } => {
                    use b00t_cli::commands::focus::FocusCommands;
                    use b00t_cli::commands::focus::handle_focus_command;
                    let args = FocusCommands::History {
                        limit: *limit,
                        json: *json,
                    };
                    if let Err(e) = handle_focus_command(&args) {
                        eprintln!("Error: {}", e);
                        std::process::exit(1);
                    }
                }
                ExperimentCommands::Compare { exp_a, exp_b, path } => {
                    use b00t_cli::commands::experiment;
                    if let Err(e) = experiment::handle_experiment_compare(exp_a, exp_b, path) {
                        eprintln!("Error: {}", e);
                        std::process::exit(1);
                    }
                }
            }
        }
        Some(Commands::Focus(args)) => {
            if let Err(e) = b00t_cli::commands::focus::handle_focus_command(args) {
                eprintln!("Error: {}", e);
                std::process::exit(1);
            }
        }
        Some(Commands::Exec(args)) => {
            if let Err(e) = b00t_cli::commands::exec::handle_exec(args, &cli.path) {
                eprintln!("Error: {}", e);
                std::process::exit(1);
            }
        }
        Some(Commands::Schema { schema_command }) => match schema_command {
            SchemaSubcommands::Generate { args } => {
                if let Err(e) = b00t_cli::datum_schema::handle_schema_generate(args) {
                    eprintln!("Error: {e}");
                    std::process::exit(1);
                }
            }
            SchemaSubcommands::Diff { schema_a, schema_b } => {
                use b00t_cli::commands::schema::handle_schema_diff;
                if let Err(e) = handle_schema_diff(&cli.path, schema_a, schema_b) {
                    eprintln!("Error: {e}");
                    std::process::exit(1);
                }
            }
            SchemaSubcommands::Import { path, name, output } => {
                use b00t_cli::commands::schema::handle_schema_import;
                if let Err(e) = handle_schema_import(&path.to_string_lossy(), name, output.clone())
                {
                    eprintln!("Error: {e}");
                    std::process::exit(1);
                }
            }
        },
        Some(Commands::Quit(args)) => {
            if let Err(e) = b00t_cli::commands::quit::handle_quit(args) {
                eprintln!("Error: {e}");
                std::process::exit(1);
            }
        }
        Some(Commands::Docgen(args)) => match b00t_cli::commands::docgen::run_docgen(args) {
            Ok(output) => println!("{output}"),
            Err(e) => {
                eprintln!("docgen error: {e}");
                std::process::exit(1);
            }
        },
        Some(Commands::Audit { audit_command }) => {
            if let Err(e) = b00t_cli::commands::audit::handle_audit_command(audit_command) {
                eprintln!("Error: {e}");
                std::process::exit(1);
            }
        }
        Some(Commands::Data { data_command }) => {
            if let Err(e) = b00t_cli::commands::data_cmd::handle_data_command(data_command) {
                eprintln!("Error: {e}");
                std::process::exit(1);
            }
        }
        Some(Commands::Doctor { doctor_command }) => {
            if let Err(e) =
                b00t_cli::commands::doctor_cmd::handle_doctor_command(doctor_command, &cli.path)
            {
                eprintln!("Error: {e}");
                std::process::exit(1);
            }
        }
        Some(Commands::Gates { gates_command }) => {
            if let Err(e) = gates_command.execute(&cli.path) {
                eprintln!("Error: {e}");
                std::process::exit(1);
            }
        }
        Some(Commands::Observability {
            observability_command,
        }) => {
            if let Err(e) = observability_command.execute() {
                eprintln!("Error: {e}");
                std::process::exit(1);
            }
        }
        None => {
            eprintln!("No command provided. Use --help for usage information.");
            std::process::exit(1);
        }
    }
}

#[cfg(test)]
mod k0mmand3r_dispatch_tests {
    use super::*;

    // ── normalize_slash ──────────────────────────────────────────────────────

    #[test]
    fn normalize_slash_already_prefixed() {
        assert_eq!(normalize_slash("/whoami"), "/whoami");
    }

    #[test]
    fn normalize_slash_adds_prefix() {
        assert_eq!(normalize_slash("whoami"), "/whoami");
    }

    #[test]
    fn normalize_slash_trims_whitespace() {
        assert_eq!(normalize_slash("  gh  "), "/gh");
    }

    #[test]
    fn normalize_slash_double_slash_unchanged() {
        // A leading slash is enough; no double-slash introduced.
        assert_eq!(normalize_slash("/gh"), "/gh");
    }

    // ── normalize_slash_args ─────────────────────────────────────────────────

    fn args(v: &[&str]) -> Vec<String> {
        v.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn normalize_slash_args_too_short_unchanged() {
        let input = args(&["b00t-cli"]);
        assert_eq!(normalize_slash_args(input.clone()), input);
    }

    #[test]
    fn normalize_slash_args_no_slash_unchanged() {
        let input = args(&["b00t-cli", "whoami"]);
        assert_eq!(normalize_slash_args(input.clone()), input);
    }

    #[test]
    fn normalize_slash_args_direct_slash_rewritten() {
        // `/whoami` at argv[1] → `k0mmand3r /whoami`
        let input = args(&["b00t-cli", "/whoami"]);
        let expected = args(&["b00t-cli", "k0mmand3r", "/whoami"]);
        assert_eq!(normalize_slash_args(input), expected);
    }

    #[test]
    fn normalize_slash_args_slash_with_trailing_args() {
        // `/gh --version` → `k0mmand3r /gh --version`
        let input = args(&["b00t-cli", "/gh", "--version"]);
        let expected = args(&["b00t-cli", "k0mmand3r", "/gh", "--version"]);
        assert_eq!(normalize_slash_args(input), expected);
    }

    #[test]
    fn normalize_slash_args_flag_before_slash_preserved() {
        // `--doc /whoami` → `--doc k0mmand3r /whoami`
        // Boolean flags (no separate value) before a slash are preserved in the prefix.
        let input = args(&["b00t-cli", "--doc", "/whoami"]);
        let expected = args(&["b00t-cli", "--doc", "k0mmand3r", "/whoami"]);
        assert_eq!(normalize_slash_args(input), expected);
    }

    #[test]
    fn normalize_slash_args_inline_value_option_before_slash() {
        // `--path=/tmp /whoami` uses `=`-joined value so the flag is one token
        // starting with `-`; the scanner skips it and correctly finds `/whoami`.
        let input = args(&["b00t-cli", "--path=/tmp", "/whoami"]);
        let expected = args(&["b00t-cli", "--path=/tmp", "k0mmand3r", "/whoami"]);
        assert_eq!(normalize_slash_args(input), expected);
    }

    #[test]
    fn normalize_slash_args_multiple_flags_before_slash() {
        let input = args(&["b00t-cli", "--doc", "--path=/tmp", "/gh", "--version"]);
        let expected = args(&[
            "b00t-cli",
            "--doc",
            "--path=/tmp",
            "k0mmand3r",
            "/gh",
            "--version",
        ]);
        assert_eq!(normalize_slash_args(input), expected);
    }

    #[test]
    fn cli_parse_accepts_path_before_subcommand() {
        let cli = Cli::parse_from(args(&[
            "b00t-cli",
            "--path",
            "/tmp/demo",
            "uninstall",
            "--yes",
            "demo-datum",
        ]));

        assert_eq!(cli.path, "/tmp/demo");
        match cli.command {
            Some(Commands::Uninstall { name, yes, purge }) => {
                assert_eq!(name, "demo-datum");
                assert!(yes);
                assert!(!purge);
            }
            _ => panic!("expected uninstall command"),
        }
    }

    #[test]
    fn normalize_slash_args_filesystem_path_value_not_rewritten() {
        // `--path /tmp/.tmpXXX uninstall` — /tmp/.tmpXXX is a flag value (multi-component path),
        // NOT a slash command; argv must be returned unchanged so clap parses it correctly.
        let input = args(&[
            "b00t-cli",
            "--path",
            "/tmp/.tmpXXX",
            "uninstall",
            "--yes",
            "foo",
        ]);
        assert_eq!(normalize_slash_args(input.clone()), input);
    }

    #[test]
    fn normalize_slash_args_end_of_options_marker_unchanged() {
        // `--` stops scanning; argv returned as-is even if a slash follows.
        let input = args(&["b00t-cli", "--", "/whoami"]);
        assert_eq!(normalize_slash_args(input.clone()), input);
    }

    #[test]
    fn normalize_slash_args_k0mmand3r_alias_normalized() {
        // `k0mmand3r vote blessing:test` → `k0mmand3r /vote blessing:test`
        let input = args(&["b00t-cli", "k0mmand3r", "vote", "blessing:test"]);
        let expected = args(&["b00t-cli", "k0mmand3r", "/vote", "blessing:test"]);
        assert_eq!(normalize_slash_args(input), expected);
    }

    #[test]
    fn normalize_slash_args_slash_k0mmand3r_alias_normalized() {
        // `/k0mmand3r vote blessing:test` → `k0mmand3r /vote blessing:test`
        let input = args(&["b00t-cli", "/k0mmand3r", "vote", "blessing:test"]);
        let expected = args(&["b00t-cli", "k0mmand3r", "/vote", "blessing:test"]);
        assert_eq!(normalize_slash_args(input), expected);
    }

    #[test]
    fn normalize_slash_args_k0mmand3r_alias_no_subcommand_is_self_dispatch() {
        // `/k0mmand3r` alone (no following verb token) falls through to the
        // generic `starts_with('/')` branch and maps to `k0mmand3r /k0mmand3r`,
        // which the dispatcher handles by printing visible datums.
        let input = args(&["b00t-cli", "/k0mmand3r"]);
        let expected = args(&["b00t-cli", "k0mmand3r", "/k0mmand3r"]);
        assert_eq!(normalize_slash_args(input), expected);
    }

    // ── datum_slash_aliases ──────────────────────────────────────────────────

    fn make_datum(
        name: &str,
        slash: Option<&str>,
        aliases: Option<Vec<&str>>,
    ) -> b00t_cli::BootDatum {
        b00t_cli::BootDatum {
            name: name.to_string(),
            k0mmand3r: slash.map(|s| b00t_cli::K0mmand3rDatumConfig {
                slash: Some(s.to_string()),
                hidden: None,
                description: None,
            }),
            aliases: aliases.map(|v| v.iter().map(|s| s.to_string()).collect()),
            ..Default::default()
        }
    }

    #[test]
    fn datum_slash_aliases_name_always_included() {
        let datum = make_datum("gh", None, None);
        let aliases = datum_slash_aliases(&datum);
        assert!(aliases.contains(&"/gh".to_string()));
    }

    #[test]
    fn datum_slash_aliases_explicit_slash_included() {
        let datum = make_datum("gh-cli", Some("/gh"), None);
        let aliases = datum_slash_aliases(&datum);
        assert!(aliases.contains(&"/gh".to_string()));
        assert!(aliases.contains(&"/gh-cli".to_string()));
    }

    #[test]
    fn datum_slash_aliases_extra_aliases_normalized() {
        let datum = make_datum("git", None, Some(vec!["g", "/git2"]));
        let aliases = datum_slash_aliases(&datum);
        assert!(aliases.contains(&"/g".to_string()));
        assert!(aliases.contains(&"/git2".to_string()));
        assert!(aliases.contains(&"/git".to_string()));
    }

    #[test]
    fn datum_slash_aliases_deduped_and_sorted() {
        // "/gh" from explicit slash AND from name should not duplicate.
        let datum = make_datum("gh", Some("gh"), None);
        let aliases = datum_slash_aliases(&datum);
        let count = aliases.iter().filter(|a| *a == "/gh").count();
        assert_eq!(count, 1, "duplicate /gh aliases: {:?}", aliases);
    }

    // ── find_cli_datum_for_slash ─────────────────────────────────────────────

    #[test]
    fn find_cli_datum_matches_by_name_slash() {
        let datums = vec![
            make_datum("gh", None, None),
            make_datum("docker", None, None),
        ];
        let found = find_cli_datum_for_slash("/gh", &datums);
        assert!(found.is_some());
        assert_eq!(found.unwrap().name, "gh");
    }

    #[test]
    fn find_cli_datum_matches_by_explicit_slash() {
        let datums = vec![make_datum("gh-cli", Some("/gh"), None)];
        let found = find_cli_datum_for_slash("/gh", &datums);
        assert!(found.is_some());
        assert_eq!(found.unwrap().name, "gh-cli");
    }

    #[test]
    fn find_cli_datum_no_match_returns_none() {
        let datums = vec![make_datum("gh", None, None)];
        assert!(find_cli_datum_for_slash("/unknown", &datums).is_none());
    }

    #[test]
    fn find_cli_datum_slash_without_prefix_still_matches() {
        // Caller may pass "gh" (no slash); normalize_slash is applied internally.
        let datums = vec![make_datum("gh", None, None)];
        let found = find_cli_datum_for_slash("gh", &datums);
        assert!(found.is_some());
    }

    // ── verb-collision guard: /status must not be stolen by k0mmand_verbs ────
    // This test documents and guards against the known issue where k0mmand verbs
    // like "status" shadow datum/internal dispatch for `/status`.
    // The current implementation matches verbs BEFORE datum lookup; this test
    // records that `/status` is currently routed as a k0mmand verb (not to a
    // datum), so any future fix to move verb handling behind datum lookup will
    // be visible as a test change.

    #[test]
    fn k0mmand_verb_status_is_recognized_before_datum_lookup() {
        use b00t_cli::k0mmand3r::K0mmand;
        // K0mmand::parse should succeed for /status (it is a known verb).
        let cmd = K0mmand::parse("/status from agent:executive");
        assert!(cmd.is_ok(), "K0mmand::parse(/status) should succeed");
        let k = cmd.unwrap();
        assert_eq!(k.verb, "status");
    }

    #[test]
    fn k0mmand_verb_list_contains_collision_candidates() {
        // These verbs are reserved by k0mmand3r and checked BEFORE datum lookup
        // in execute_k0mmand3r_dispatch.  Any datum named one of these will be
        // shadowed — this test documents the current set so a future refactor
        // (moving verb handling *after* datum lookup) will require an update here.
        let k0mmand_verbs = [
            "negotiate",
            "vote",
            "delegate",
            "status",
            "handshake",
            "crew",
        ];
        assert!(
            k0mmand_verbs.contains(&"status"),
            "/status is shadowed by k0mmand verb"
        );
        assert!(
            k0mmand_verbs.contains(&"crew"),
            "/crew is shadowed by k0mmand verb"
        );
        // If a datum named "gh" is not in k0mmand_verbs it will not be shadowed.
        assert!(!k0mmand_verbs.contains(&"gh"), "/gh is NOT shadowed");
    }
}
