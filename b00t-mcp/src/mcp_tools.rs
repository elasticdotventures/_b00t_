use clap::Parser;
use crate::clap_reflection::{McpReflection, McpCommandRegistry};
use crate::impl_mcp_tool;
// use b00t_c0re_lib::GrokClient;

// Re-export b00t-cli command structures for MCP use
// This creates a compile-time dependency but ensures type safety

/// MCP command for listing MCP servers
#[derive(Parser, Clone)]
pub struct McpListCommand {
    #[arg(long, help = "Output in JSON format")]
    pub json: bool,
}

impl_mcp_tool!(McpListCommand, "b00t_mcp_list", ["mcp", "list"]);

/// MCP command for adding MCP servers
#[derive(Parser, Clone)]
pub struct McpAddCommand {
    #[arg(help = "MCP server configuration JSON or '-' for stdin")]
    pub json: String,

    #[arg(long, help = "Enable Do What I Want mode for enhanced parsing")]
    pub dwiw: bool,

    #[arg(long, help = "Server hint/description")]
    pub hint: Option<String>,
}

impl_mcp_tool!(McpAddCommand, "b00t_mcp_add", ["mcp", "register"]);

/// MCP command for MCP server output generation
#[derive(Parser, Clone)]
pub struct McpOutputCommand {
    #[arg(help = "Comma-separated list of server names")]
    pub servers: String,

    #[arg(long, help = "Output raw JSON instead of mcpServers wrapper")]
    pub json: bool,
}

impl_mcp_tool!(McpOutputCommand, "b00t_mcp_output", ["mcp", "output"]);

/// CLI detect command
#[derive(Parser, Clone)]
pub struct CliDetectCommand {
    #[arg(help = "Command to detect version for")]
    pub command: String,
}

impl_mcp_tool!(CliDetectCommand, "b00t_cli_detect", ["cli", "detect"]);

/// CLI desires command
#[derive(Parser, Clone)]
pub struct CliDesiresCommand {
    #[arg(help = "Command to show desired version for")]
    pub command: String,
}

impl_mcp_tool!(CliDesiresCommand, "b00t_cli_desires", ["cli", "desires"]);

/// CLI check command
#[derive(Parser, Clone)]
pub struct CliCheckCommand {
    #[arg(help = "Command to check version alignment for")]
    pub command: String,
}

impl_mcp_tool!(CliCheckCommand, "b00t_cli_check", ["cli", "check"]);

/// CLI install command
#[derive(Parser, Clone)]
pub struct CliInstallCommand {
    #[arg(help = "Command to install")]
    pub command: String,
}

impl_mcp_tool!(CliInstallCommand, "b00t_cli_install", ["cli", "install"]);

/// CLI update command
#[derive(Parser, Clone)]
pub struct CliUpdateCommand {
    #[arg(help = "Command to update")]
    pub command: String,
}

impl_mcp_tool!(CliUpdateCommand, "b00t_cli_update", ["cli", "update"]);

/// CLI up command (update all)
#[derive(Parser, Clone)]
pub struct CliUpCommand {
    #[arg(long, help = "Dry run - show what would be updated")]
    pub dry_run: bool,
}

impl_mcp_tool!(CliUpCommand, "b00t_cli_up", ["cli", "up"]);

/// LFMF command
#[derive(Parser, Clone)]
pub struct LfmfCommand {
    #[arg(long, help = "Tool name")]
    pub tool: String,
    #[arg(long, help = "Summary hint or lesson learned")]
    pub lesson: String,
    #[arg(long, group = "scope", help = "Record lesson for this repo (default)")]
    pub repo: bool,
    #[arg(long, group = "scope", help = "Record lesson globally (mutually exclusive with --repo)")]
    pub global: bool,
}

impl_mcp_tool!(LfmfCommand, "b00t_lfmf", ["lfmf"]);

/// MCP command for advice/syntax therapist functionality
#[derive(Parser, Clone)]
pub struct AdviceCommand {
    #[arg(help = "Tool name")]
    pub tool: String,
    #[arg(help = "Error pattern to get advice for, 'list' to show all lessons, or 'search <query>'")]
    pub query: String,
    #[arg(long, help = "Maximum number of results to return (default: 5)")]
    pub count: Option<usize>,
}

impl_mcp_tool!(AdviceCommand, "b00t_advice", ["advice"]);

/// Whoami command
#[derive(Parser, Clone)]
pub struct WhoamiCommand;

impl_mcp_tool!(WhoamiCommand, "b00t_whoami", ["whoami"]);

/// Status command
// 🤓 ENTANGLED: b00t-cli/src/main.rs Commands::Status
// When this changes, update b00t-cli Status command structure
#[derive(Parser, Clone)]
pub struct StatusCommand {
    #[arg(long, help = "Filter by subsystem")]
    pub filter: Option<String>,

    #[arg(long, help = "Show only installed tools")]
    pub installed: bool,

    #[arg(long, help = "Show only available tools")]
    pub available: bool,
}

impl_mcp_tool!(StatusCommand, "b00t_status", ["status"]);

/// AI list command
#[derive(Parser, Clone)]
pub struct AiListCommand {
    #[arg(long, help = "Output in JSON format")]
    pub json: bool,
}

impl_mcp_tool!(AiListCommand, "b00t_ai_list", ["ai", "list"]);

/// AI output command
#[derive(Parser, Clone)]
pub struct AiOutputCommand {
    #[arg(help = "Comma-separated list of AI provider names")]
    pub providers: String,

    #[arg(long, help = "Output key-value pairs")]
    pub kv: bool,

    #[arg(long, help = "Output b00t format")]
    pub b00t: bool,
}

impl_mcp_tool!(AiOutputCommand, "b00t_ai_output", ["ai", "output"]);

// Agent coordination MCP commands

/// MCP command for agent discovery
#[derive(Parser, Clone)]
pub struct AgentDiscoverCommand {
    #[arg(long, help = "Filter by agent role")]
    pub role: Option<String>,

    #[arg(long, help = "Filter by crew membership")]
    pub crew: Option<String>,

    #[arg(long, help = "Required capabilities (comma-separated)")]
    pub capabilities: Option<String>,

    #[arg(long, help = "Output in JSON format")]
    pub json: bool,
}

impl_mcp_tool!(AgentDiscoverCommand, "b00t_agent_discover", ["agent", "discover"]);

/// MCP command for sending messages to agents
#[derive(Parser, Clone)]
pub struct AgentMessageCommand {
    #[arg(help = "Target agent ID")]
    pub to_agent: String,

    #[arg(help = "Message subject")]
    pub subject: String,

    #[arg(help = "Message content")]
    pub content: String,

    #[arg(long, help = "Require acknowledgment")]
    pub ack: bool,
}

impl_mcp_tool!(AgentMessageCommand, "b00t_agent_message", ["agent", "message"]);

/// MCP command for task delegation (captain only)
#[derive(Parser, Clone)]
pub struct AgentDelegateCommand {
    #[arg(help = "Worker agent ID")]
    pub worker: String,

    #[arg(help = "Task ID")]
    pub task_id: String,

    #[arg(help = "Task description")]
    pub description: String,

    #[arg(long, help = "Priority level", value_enum)]
    pub priority: Option<String>, // Will be parsed as TaskPriority

    #[arg(long, help = "Deadline in minutes")]
    pub deadline: Option<u64>,

    #[arg(long, help = "Required capabilities (comma-separated)")]
    pub capabilities: Option<String>,

    #[arg(long, help = "Block until completion")]
    pub blocking: bool,
}

impl_mcp_tool!(AgentDelegateCommand, "b00t_agent_delegate", ["agent", "delegate"]);

/// MCP command for completing tasks (worker response)
#[derive(Parser, Clone)]
pub struct AgentCompleteCommand {
    #[arg(help = "Captain agent ID")]
    pub captain: String,

    #[arg(help = "Task ID")]
    pub task_id: String,

    #[arg(long, help = "Completion status", value_enum)]
    pub status: String, // "success", "failed", "partial", "cancelled"

    #[arg(long, help = "Result description")]
    pub result: Option<String>,

    #[arg(long, help = "Output artifacts (comma-separated paths)")]
    pub artifacts: Option<String>,
}

impl_mcp_tool!(AgentCompleteCommand, "b00t_agent_complete", ["agent", "complete"]);

/// MCP command for reporting progress
#[derive(Parser, Clone)]
pub struct AgentProgressCommand {
    #[arg(help = "Task ID")]
    pub task_id: String,

    #[arg(help = "Progress percentage (0-100)")]
    pub progress: f32,

    #[arg(help = "Status message")]
    pub message: String,

    #[arg(long, help = "Estimated completion in minutes")]
    pub eta: Option<u64>,
}

impl_mcp_tool!(AgentProgressCommand, "b00t_agent_progress", ["agent", "progress"]);

/// MCP command for creating voting proposals (captain only)
#[derive(Parser, Clone)]
pub struct AgentVoteCreateCommand {
    #[arg(help = "Proposal subject")]
    pub subject: String,

    #[arg(help = "Proposal description")]
    pub description: String,

    #[arg(help = "Voting options (JSON array)")]
    pub options: String,

    #[arg(long, help = "Voting type", value_enum)]
    pub vote_type: String, // "single", "ranked", "approval", "veto"

    #[arg(long, help = "Deadline in minutes")]
    pub deadline: u64,

    #[arg(help = "Eligible voters (comma-separated agent IDs)")]
    pub voters: String,
}

impl_mcp_tool!(AgentVoteCreateCommand, "b00t_agent_vote_create", ["agent", "vote", "create"]);

/// MCP command for submitting votes
#[derive(Parser, Clone)]
pub struct AgentVoteSubmitCommand {
    #[arg(help = "Proposal ID")]
    pub proposal_id: String,

    #[arg(help = "Vote choice (JSON)")]
    pub vote: String,

    #[arg(long, help = "Vote reasoning")]
    pub reasoning: Option<String>,
}

impl_mcp_tool!(AgentVoteSubmitCommand, "b00t_agent_vote_submit", ["agent", "vote", "submit"]);

/// MCP command for waiting for messages (blocking)
#[derive(Parser, Clone)]
pub struct AgentWaitCommand {
    #[arg(long, help = "Timeout in seconds", default_value = "300")]
    pub timeout: u64,

    #[arg(long, help = "Filter by message type")]
    pub message_type: Option<String>,

    #[arg(long, help = "Filter by sender agent")]
    pub from_agent: Option<String>,

    #[arg(long, help = "Filter by task ID")]
    pub task_id: Option<String>,

    #[arg(long, help = "Filter by subject")]
    pub subject: Option<String>,
}

impl_mcp_tool!(AgentWaitCommand, "b00t_agent_wait", ["agent", "wait"]);

/// MCP command for sending event notifications
#[derive(Parser, Clone)]
pub struct AgentNotifyCommand {
    #[arg(help = "Event type (e.g., 'file_created', 'pr_opened')")]
    pub event_type: String,

    #[arg(help = "Event source")]
    pub source: String,

    #[arg(help = "Event details (JSON)")]
    pub details: String,

    #[arg(long, help = "Target specific agents (comma-separated)")]
    pub agents: Option<String>,
}

impl_mcp_tool!(AgentNotifyCommand, "b00t_agent_notify", ["agent", "notify"]);

/// MCP command for capability requests
#[derive(Parser, Clone)]
pub struct AgentCapabilityCommand {
    #[arg(help = "Required capabilities (comma-separated)")]
    pub capabilities: String,

    #[arg(help = "Task description")]
    pub description: String,

    #[arg(long, help = "Request urgency", value_enum)]
    pub urgency: Option<String>, // "low", "normal", "high", "emergency"
}

impl_mcp_tool!(AgentCapabilityCommand, "b00t_agent_capability", ["agent", "capability"]);

/// App VSCode MCP install command
#[derive(Parser, Clone)]
pub struct AppVscodeMcpInstallCommand {
    #[arg(help = "MCP server name to install")]
    pub name: String,
}

impl_mcp_tool!(AppVscodeMcpInstallCommand, "b00t_app_vscode_mcp_install", ["app", "vscode", "mcp", "install"]);

/// App Claude Code MCP install command
#[derive(Parser, Clone)]
pub struct AppClaudecodeMcpInstallCommand {
    #[arg(help = "MCP server name to install")]
    pub name: String,
}

impl_mcp_tool!(AppClaudecodeMcpInstallCommand, "b00t_app_claudecode_mcp_install", ["app", "claudecode", "mcp", "install"]);

/// MCP install command with full target and parameter support
// 🤓 ENTANGLED: b00t-cli/src/commands/mcp.rs McpCommands::Install
// When this changes, update b00t-cli McpCommands::Install structure
#[derive(Parser, Clone)]
pub struct McpInstallCommand {
    #[arg(help = "MCP server name")]
    pub name: String,

    #[arg(help = "Installation target: claudecode, vscode, geminicli, dotmcpjson")]
    pub target: String,

    #[arg(long, help = "Install to repository-specific location (for geminicli)")]
    pub repo: bool,

    #[arg(long, help = "Install to user-global location (for geminicli)")]
    pub user: bool,

    #[arg(long, help = "Select stdio method by command (for multi-source MCP configs)")]
    pub stdio_command: Option<String>,

    #[arg(long, help = "Use httpstream method (for multi-source MCP configs)")]
    pub httpstream: bool,
}

impl_mcp_tool!(McpInstallCommand, "b00t_mcp_install", ["mcp", "install"]);

/// Session init command
// 🤓 ENTANGLED: b00t-cli/src/commands/session.rs SessionCommands::Init
// When this changes, update b00t-cli SessionCommands::Init structure
#[derive(Parser, Clone)]
pub struct SessionInitCommand {
    #[arg(long, help = "Budget limit in dollars")]
    pub budget: Option<f64>,

    #[arg(long, help = "Time limit in minutes")]
    pub time_limit: Option<u32>,

    #[arg(long, help = "Agent name")]
    pub agent: Option<String>,
}

impl_mcp_tool!(SessionInitCommand, "b00t_session_init", ["session", "init"]);

/// Session status command
#[derive(Parser, Clone)]
pub struct SessionStatusCommand;

impl_mcp_tool!(SessionStatusCommand, "b00t_session_status", ["session", "status"]);

/// Session end command
#[derive(Parser, Clone)]
pub struct SessionEndCommand;

impl_mcp_tool!(SessionEndCommand, "b00t_session_end", ["session", "end"]);

/// Learn command
// 🤓 ENTANGLED: b00t-cli/src/main.rs Commands::Learn
// When this changes, update b00t-cli Learn command structure
#[derive(Parser, Clone)]
pub struct LearnCommand {
    #[arg(help = "Topic to learn about")]
    pub topic: Option<String>,
}

impl_mcp_tool!(LearnCommand, "b00t_learn", ["learn"]);

/// Checkpoint command
// 🤓 ENTANGLED: b00t-cli/src/main.rs Commands::Checkpoint
// When this changes, update b00t-cli Checkpoint command structure
#[derive(Parser, Clone)]
pub struct CheckpointCommand {
    #[arg(short, long, help = "Commit message")]
    pub message: Option<String>,

    #[arg(long, help = "Skip running tests")]
    pub skip_tests: bool,
}

impl_mcp_tool!(CheckpointCommand, "b00t_checkpoint", ["checkpoint"]);

// Grok knowledgebase MCP tools

/// MCP command for digesting content into chunks about a topic
/// 🤓 ENTANGLED: b00t-cli/src/commands/grok.rs GrokCommands::Digest
#[derive(Parser, Clone)]
pub struct GrokDigestCommand {
    #[arg(help = "Topic to digest content about")]
    pub topic: String,

    #[arg(help = "Content to digest")]
    pub content: String,
}

impl_mcp_tool!(GrokDigestCommand, "b00t_grok_digest", ["grok", "digest"]);

/// MCP command for asking questions and searching the knowledgebase
/// 🤓 ENTANGLED: b00t-cli/src/commands/grok.rs GrokCommands::Ask
#[derive(Parser, Clone)]
pub struct GrokAskCommand {
    #[arg(help = "Query to search for")]
    pub query: String,

    #[arg(long, help = "Optional topic to filter by")]
    pub topic: Option<String>,

    #[arg(long, help = "Maximum number of results to return", default_value = "10")]
    pub limit: Option<usize>,
}

impl_mcp_tool!(GrokAskCommand, "b00t_grok_ask", ["grok", "ask"]);

/// MCP command for learning from URLs or content
/// 🤓 ENTANGLED: b00t-cli/src/commands/grok.rs GrokCommands::Learn
#[derive(Parser, Clone)]
pub struct GrokLearnCommand {
    #[arg(help = "Content to learn from")]
    pub content: String,

    #[arg(long, help = "Source URL or file path")]
    pub source: Option<String>,
}

impl_mcp_tool!(GrokLearnCommand, "b00t_grok_learn", ["grok", "learn"]);

/// MCP command for getting grok system status
#[derive(Parser, Clone)]
pub struct GrokStatusCommand;

impl_mcp_tool!(GrokStatusCommand, "b00t_grok_status", ["grok", "status"]);

// ACP Hive coordination MCP tools

/// MCP command for joining a hive mission
#[derive(Parser, Clone)]
pub struct AcpHiveJoinCommand {
    #[arg(help = "Mission identifier")]
    pub mission_id: String,

    #[arg(help = "Agent role in the mission")]
    pub role: String,

    #[arg(long, help = "Agent namespace (defaults to account.username)")]
    pub namespace: Option<String>,

    #[arg(long, help = "NATS server URL (defaults to c010.promptexecution.com:4222)")]
    pub nats_url: Option<String>,
}

impl_mcp_tool!(AcpHiveJoinCommand, "b00t_acp_hive_join", ["acp", "hive", "join"]);

/// MCP command for creating a hive mission
#[derive(Parser, Clone)]
pub struct AcpHiveCreateCommand {
    #[arg(help = "Mission identifier")]
    pub mission_id: String,

    #[arg(help = "Expected number of agents")]
    pub expected_agents: usize,

    #[arg(help = "Mission description")]
    pub description: String,

    #[arg(help = "Agent role in the mission")]
    pub role: String,

    #[arg(long, help = "Agent namespace (defaults to account.username)")]
    pub namespace: Option<String>,

    #[arg(long, help = "NATS server URL (defaults to c010.promptexecution.com:4222)")]
    pub nats_url: Option<String>,
}

impl_mcp_tool!(AcpHiveCreateCommand, "b00t_acp_hive_create", ["acp", "hive", "create"]);

/// MCP command for sending status to hive
#[derive(Parser, Clone)]
pub struct AcpHiveStatusCommand {
    #[arg(help = "Mission identifier")]
    pub mission_id: String,

    #[arg(help = "Status description")]
    pub description: String,

    #[arg(long, help = "Optional payload data (JSON)")]
    pub payload: Option<String>,
}

impl_mcp_tool!(AcpHiveStatusCommand, "b00t_acp_hive_status", ["acp", "hive", "status"]);

/// MCP command for proposing actions to hive
#[derive(Parser, Clone)]
pub struct AcpHiveProposeCommand {
    #[arg(help = "Mission identifier")]
    pub mission_id: String,

    #[arg(help = "Action to propose")]
    pub action: String,

    #[arg(long, help = "Optional action payload (JSON)")]
    pub payload: Option<String>,
}

impl_mcp_tool!(AcpHiveProposeCommand, "b00t_acp_hive_propose", ["acp", "hive", "propose"]);

/// MCP command for step synchronization
#[derive(Parser, Clone)]
pub struct AcpHiveSyncCommand {
    #[arg(help = "Mission identifier")]
    pub mission_id: String,

    #[arg(help = "Target step to synchronize to")]
    pub target_step: u64,

    #[arg(long, help = "Timeout in seconds", default_value = "60")]
    pub timeout_seconds: u64,
}

impl_mcp_tool!(AcpHiveSyncCommand, "b00t_acp_hive_sync", ["acp", "hive", "sync"]);

/// MCP command for signaling step readiness
#[derive(Parser, Clone)]
pub struct AcpHiveReadyCommand {
    #[arg(help = "Mission identifier")]
    pub mission_id: String,

    #[arg(help = "Step to signal readiness for")]
    pub target_step: u64,
}

impl_mcp_tool!(AcpHiveReadyCommand, "b00t_acp_hive_ready", ["acp", "hive", "ready"]);

/// MCP command for showing hive status
#[derive(Parser, Clone)]
pub struct AcpHiveShowCommand {
    #[arg(help = "Mission identifier (optional - shows all missions if not specified)")]
    pub mission_id: Option<String>,
}

impl_mcp_tool!(AcpHiveShowCommand, "b00t_acp_hive_show", ["acp", "hive", "show"]);

/// MCP command for leaving hive mission
#[derive(Parser, Clone)]
pub struct AcpHiveLeaveCommand {
    #[arg(help = "Mission identifier")]
    pub mission_id: String,
}

impl_mcp_tool!(AcpHiveLeaveCommand, "b00t_acp_hive_leave", ["acp", "hive", "leave"]);

// Custom implementations for ACP hive tools
use crate::acp_tools::*;

impl crate::clap_reflection::McpExecutor for AcpHiveJoinCommand {
    fn execute_mcp_call(params: &std::collections::HashMap<String, serde_json::Value>) -> anyhow::Result<String> {
        let join_params = crate::acp_tools::JoinHiveParams {
            mission_id: params.get("mission_id")
                .and_then(|v| v.as_str())
                .ok_or_else(|| anyhow::anyhow!("Missing mission_id parameter"))?
                .to_string(),
            role: params.get("role")
                .and_then(|v| v.as_str())
                .ok_or_else(|| anyhow::anyhow!("Missing role parameter"))?
                .to_string(),
            namespace: params.get("namespace").and_then(|v| v.as_str()).map(|s| s.to_string()),
            nats_url: params.get("nats_url").and_then(|v| v.as_str()).map(|s| s.to_string()),
        };
        
        tokio::runtime::Runtime::new()?.block_on(acp_hive_join(join_params))
    }
}

impl crate::clap_reflection::McpExecutor for AcpHiveCreateCommand {
    fn execute_mcp_call(params: &std::collections::HashMap<String, serde_json::Value>) -> anyhow::Result<String> {
        let create_params = crate::acp_tools::CreateHiveParams {
            mission_id: params.get("mission_id")
                .and_then(|v| v.as_str())
                .ok_or_else(|| anyhow::anyhow!("Missing mission_id parameter"))?
                .to_string(),
            expected_agents: params.get("expected_agents")
                .and_then(|v| v.as_u64())
                .ok_or_else(|| anyhow::anyhow!("Missing expected_agents parameter"))? as usize,
            description: params.get("description")
                .and_then(|v| v.as_str())
                .ok_or_else(|| anyhow::anyhow!("Missing description parameter"))?
                .to_string(),
            role: params.get("role")
                .and_then(|v| v.as_str())
                .ok_or_else(|| anyhow::anyhow!("Missing role parameter"))?
                .to_string(),
            namespace: params.get("namespace").and_then(|v| v.as_str()).map(|s| s.to_string()),
            nats_url: params.get("nats_url").and_then(|v| v.as_str()).map(|s| s.to_string()),
        };
        
        tokio::runtime::Runtime::new()?.block_on(acp_hive_create(create_params))
    }
}

impl crate::clap_reflection::McpExecutor for AcpHiveStatusCommand {
    fn execute_mcp_call(params: &std::collections::HashMap<String, serde_json::Value>) -> anyhow::Result<String> {
        let status_params = crate::acp_tools::HiveStatusParams {
            mission_id: params.get("mission_id")
                .and_then(|v| v.as_str())
                .ok_or_else(|| anyhow::anyhow!("Missing mission_id parameter"))?
                .to_string(),
            description: params.get("description")
                .and_then(|v| v.as_str())
                .ok_or_else(|| anyhow::anyhow!("Missing description parameter"))?
                .to_string(),
            payload: params.get("payload").and_then(|v| {
                if v.is_string() {
                    v.as_str().and_then(|s| serde_json::from_str(s).ok())
                } else {
                    Some(v.clone())
                }
            }),
        };
        
        tokio::runtime::Runtime::new()?.block_on(acp_hive_status(status_params))
    }
}

impl crate::clap_reflection::McpExecutor for AcpHiveProposeCommand {
    fn execute_mcp_call(params: &std::collections::HashMap<String, serde_json::Value>) -> anyhow::Result<String> {
        let propose_params = crate::acp_tools::HiveProposeParams {
            mission_id: params.get("mission_id")
                .and_then(|v| v.as_str())
                .ok_or_else(|| anyhow::anyhow!("Missing mission_id parameter"))?
                .to_string(),
            action: params.get("action")
                .and_then(|v| v.as_str())
                .ok_or_else(|| anyhow::anyhow!("Missing action parameter"))?
                .to_string(),
            payload: params.get("payload").and_then(|v| {
                if v.is_string() {
                    v.as_str().and_then(|s| serde_json::from_str(s).ok())
                } else {
                    Some(v.clone())
                }
            }),
        };
        
        tokio::runtime::Runtime::new()?.block_on(acp_hive_propose(propose_params))
    }
}

impl crate::clap_reflection::McpExecutor for AcpHiveSyncCommand {
    fn execute_mcp_call(params: &std::collections::HashMap<String, serde_json::Value>) -> anyhow::Result<String> {
        let sync_params = crate::acp_tools::HiveStepSyncParams {
            mission_id: params.get("mission_id")
                .and_then(|v| v.as_str())
                .ok_or_else(|| anyhow::anyhow!("Missing mission_id parameter"))?
                .to_string(),
            target_step: params.get("target_step")
                .and_then(|v| v.as_u64())
                .ok_or_else(|| anyhow::anyhow!("Missing target_step parameter"))?,
            timeout_seconds: params.get("timeout_seconds").and_then(|v| v.as_u64()),
        };
        
        tokio::runtime::Runtime::new()?.block_on(acp_hive_sync(sync_params))
    }
}

impl crate::clap_reflection::McpExecutor for AcpHiveReadyCommand {
    fn execute_mcp_call(params: &std::collections::HashMap<String, serde_json::Value>) -> anyhow::Result<String> {
        let ready_params = crate::acp_tools::HiveStepSyncParams {
            mission_id: params.get("mission_id")
                .and_then(|v| v.as_str())
                .ok_or_else(|| anyhow::anyhow!("Missing mission_id parameter"))?
                .to_string(),
            target_step: params.get("target_step")
                .and_then(|v| v.as_u64())
                .ok_or_else(|| anyhow::anyhow!("Missing target_step parameter"))?,
            timeout_seconds: None,
        };
        
        tokio::runtime::Runtime::new()?.block_on(acp_hive_ready(ready_params))
    }
}

impl crate::clap_reflection::McpExecutor for AcpHiveShowCommand {
    fn execute_mcp_call(params: &std::collections::HashMap<String, serde_json::Value>) -> anyhow::Result<String> {
        let show_params = crate::acp_tools::HiveShowParams {
            mission_id: params.get("mission_id").and_then(|v| v.as_str()).map(|s| s.to_string()),
        };
        
        tokio::runtime::Runtime::new()?.block_on(acp_hive_show(show_params))
    }
}

impl crate::clap_reflection::McpExecutor for AcpHiveLeaveCommand {
    fn execute_mcp_call(params: &std::collections::HashMap<String, serde_json::Value>) -> anyhow::Result<String> {
        let leave_params = crate::acp_tools::HiveShowParams {
            mission_id: params.get("mission_id").and_then(|v| v.as_str()).map(|s| s.to_string()),
        };
        
        tokio::runtime::Runtime::new()?.block_on(acp_hive_leave(leave_params))
    }
}

/// Create and populate a registry with all available MCP tools
pub fn create_mcp_registry() -> McpCommandRegistry {
    let mut builder = McpCommandRegistry::builder();

    // Register all MCP tools
    builder
        .register::<McpListCommand>()
        .register::<McpAddCommand>()
        .register::<McpOutputCommand>()
        .register::<CliDetectCommand>()
        .register::<CliDesiresCommand>()
        .register::<CliCheckCommand>()
        .register::<CliInstallCommand>()
        .register::<CliUpdateCommand>()
        .register::<CliUpCommand>()
        .register::<WhoamiCommand>()
        .register::<StatusCommand>()
        .register::<AiListCommand>()
        .register::<AiOutputCommand>()
        .register::<AppVscodeMcpInstallCommand>()
        .register::<AppClaudecodeMcpInstallCommand>()
        .register::<SessionInitCommand>()
        .register::<SessionStatusCommand>()
        .register::<SessionEndCommand>()
        .register::<LearnCommand>()
        .register::<CheckpointCommand>()
        // LFMF and advice system
        .register::<LfmfCommand>()
        .register::<AdviceCommand>()
        // Agent coordination commands
        .register::<AgentDiscoverCommand>()
        .register::<AgentMessageCommand>()
        .register::<AgentDelegateCommand>()
        .register::<AgentCompleteCommand>()
        .register::<AgentProgressCommand>()
        .register::<AgentVoteCreateCommand>()
        .register::<AgentVoteSubmitCommand>()
        .register::<AgentWaitCommand>()
        .register::<AgentNotifyCommand>()
        .register::<AgentCapabilityCommand>()
        // Grok knowledgebase tools
        .register::<GrokDigestCommand>()
        .register::<GrokAskCommand>()
        .register::<GrokLearnCommand>()
        .register::<GrokStatusCommand>()
        // ACP Hive coordination tools
        .register::<AcpHiveJoinCommand>()
        .register::<AcpHiveCreateCommand>()
        .register::<AcpHiveStatusCommand>()
        .register::<AcpHiveProposeCommand>()
        .register::<AcpHiveSyncCommand>()
        .register::<AcpHiveReadyCommand>()
        .register::<AcpHiveShowCommand>()
        .register::<AcpHiveLeaveCommand>();

    builder.build()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::clap_reflection::McpExecutor;
    use std::collections::HashMap;

    #[test]
    fn test_registry_creation() {
        let registry = create_mcp_registry();
        let tools = registry.get_tools();

        // Should have all registered tools
        assert!(!tools.is_empty());

        // Check specific tools exist
        let tool_names: Vec<&str> = tools.iter()
            .map(|t| t.name.as_ref())
            .collect();

        assert!(tool_names.contains(&"b00t_mcp_list"));
        assert!(tool_names.contains(&"b00t_cli_detect"));
        assert!(tool_names.contains(&"b00t_whoami"));
        assert!(tool_names.contains(&"b00t_status"));
    }

    #[test]
    fn test_tool_schema_generation() {
        let tool = McpListCommand::to_mcp_tool();
        assert_eq!(tool.name.as_ref(), "b00t_mcp_list");

        // Check schema has expected properties
        let schema = tool.input_schema.as_ref();
        assert!(schema.contains_key("type"));
        assert!(schema.contains_key("properties"));

        let properties = schema["properties"].as_object().unwrap();
        assert!(properties.contains_key("json"));
    }

    #[test]
    fn test_params_conversion() {
        let mut params = HashMap::new();
        params.insert("json".to_string(), serde_json::json!(true));

        let args = McpListCommand::params_to_args(&params);
        assert!(args.contains(&"--json".to_string()));
    }
}