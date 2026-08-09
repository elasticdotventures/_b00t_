use crate::clap_reflection::{McpCommandRegistry, McpExecutor, McpReflection};
use crate::impl_mcp_tool;
use crate::tools::pipeline::BPipelineCommand;
use anyhow::Result;
use b00t_council::MessageSink;
use clap::Parser;
use serde_json::{Map, Value, json};
use std::collections::HashMap;
// use b00t_c0re_lib::GrokClient;

// Re-export b00t-cli command structures for MCP use
// This creates a compile-time dependency but ensures type safety

/// List MCP servers
#[derive(Parser, Clone)]
pub struct McpListCommand {
    #[arg(long, help = "JSON output")]
    pub json: bool,
}

impl_mcp_tool!(McpListCommand, "b00t_mcp_list", ["mcp", "list"]);

/// Add MCP server
#[derive(Parser, Clone)]
pub struct McpAddCommand {
    #[arg(help = "MCP server config JSON or '-' for stdin")]
    pub json: String,

    #[arg(long, help = "Enable DWIW enhanced parsing")]
    pub dwiw: bool,

    #[arg(long, help = "Server hint/description")]
    pub hint: Option<String>,
}

impl_mcp_tool!(McpAddCommand, "mcp_add", ["mcp", "register"]);

/// Generate MCP server output
#[derive(Parser, Clone)]
pub struct McpOutputCommand {
    #[arg(help = "Comma-separated server names")]
    pub servers: String,

    #[arg(long, help = "Output raw JSON, not mcpServers wrapper")]
    pub json: bool,
}

impl_mcp_tool!(McpOutputCommand, "mcp_output", ["mcp", "output"]);

/// Detect CLI tool version
#[derive(Parser, Clone)]
pub struct CliDetectCommand {
    #[arg(help = "Command to detect")]
    pub command: String,
}

impl_mcp_tool!(CliDetectCommand, "b00t_cli_detect", ["cli", "detect"]);

/// Show desired CLI version
#[derive(Parser, Clone)]
pub struct CliDesiresCommand {
    #[arg(help = "Command name")]
    pub command: String,
}

impl_mcp_tool!(CliDesiresCommand, "cli_desires", ["cli", "desires"]);

/// Check CLI version alignment
#[derive(Parser, Clone)]
pub struct CliCheckCommand {
    #[arg(help = "Command to check")]
    pub command: String,
}

impl_mcp_tool!(CliCheckCommand, "cli_check", ["cli", "check"]);

/// Install CLI tool
#[derive(Parser, Clone)]
pub struct CliInstallCommand {
    #[arg(help = "Command to install")]
    pub command: String,
}

impl_mcp_tool!(CliInstallCommand, "cli_install", ["cli", "install"]);

/// Update CLI tool
#[derive(Parser, Clone)]
pub struct CliUpdateCommand {
    #[arg(help = "Command to update")]
    pub command: String,
}

impl_mcp_tool!(CliUpdateCommand, "cli_update", ["cli", "update"]);

/// Update all CLI tools
#[derive(Parser, Clone)]
pub struct CliUpCommand {
    // 🤓 CLI uses --yes not --dry-run; dry_run was wrong field name (fixed)
    #[arg(
        long,
        short = 'y',
        help = "Actually perform updates (default: report only)"
    )]
    pub yes: bool,
}

impl_mcp_tool!(CliUpCommand, "cli_up", ["cli", "up"]);

/// Holistic upgrade: binary, MCP servers, hooks, Claude settings (NASA MBSE phases)
#[derive(Parser, Clone)]
pub struct UpgradeCommand {
    #[arg(
        long,
        default_value = "all",
        help = "Scope: all|binary|mcp|hooks|settings"
    )]
    pub scope: String,
    #[arg(long, help = "Plan only; apply no changes")]
    pub dry_run: bool,
    #[arg(long, help = "Route compile tasks to ch0nky GPU tier")]
    pub delegate: bool,
    #[arg(long, help = "Emit structured JSON report")]
    pub json: bool,
}

impl_mcp_tool!(UpgradeCommand, "b00t_upgrade", ["upgrade"]);

/// Record lesson from mistake
#[derive(Parser, Clone)]
pub struct LfmfCommand {
    #[arg(long, help = "Tool name")]
    pub tool: String,
    #[arg(long, help = "Lesson learned")]
    pub lesson: String,
    #[arg(long, group = "scope", help = "Record for this repo (default)")]
    pub repo: bool,
    #[arg(
        long,
        group = "scope",
        help = "Record globally (mutually exclusive with --repo)"
    )]
    pub global: bool,
}

impl_mcp_tool!(LfmfCommand, "lfmf", ["lfmf"]);

/// Get syntax advice for errors
#[derive(Parser, Clone)]
pub struct AdviceCommand {
    #[arg(help = "Tool name")]
    pub tool: String,
    #[arg(help = "Error pattern, 'list', or 'search <query>'")]
    pub query: String,
    #[arg(long, help = "Max results (default: 5)")]
    pub count: Option<usize>,
}

impl_mcp_tool!(AdviceCommand, "advice", ["advice"]);

/// Show identity
#[derive(Parser, Clone)]
pub struct WhoamiCommand {
    #[arg(long, help = "Agent role (e.g. operator, executive, worker)")]
    pub role: Option<String>,
    #[arg(long, help = "Show layered system dashboard")]
    pub dashboard: bool,
    #[arg(long, help = "Discover agent capabilities")]
    pub capabilities: bool,
    #[arg(long, alias = "agent", help = "Alias for --role")]
    pub agent: Option<String>,
    #[arg(long, help = "Load and display skills")]
    pub with_skills: bool,
    #[arg(long, help = "Output as JSON")]
    pub json: bool,
}

impl_mcp_tool!(WhoamiCommand, "b00t_whoami", ["whoami"]);

/// Show system status
#[derive(Parser, Clone)]
pub struct StatusCommand {
    #[arg(long, help = "Filter by subsystem")]
    pub filter: Option<String>,

    #[arg(long, help = "Only installed tools")]
    pub installed: bool,

    #[arg(long, help = "Only available tools")]
    pub available: bool,
}

impl_mcp_tool!(StatusCommand, "b00t_status", ["status"]);

/// List AI providers
#[derive(Parser, Clone)]
pub struct AiListCommand {
    #[arg(long, help = "JSON output")]
    pub json: bool,
}

impl_mcp_tool!(AiListCommand, "ai_list", ["ai", "list"]);

/// Show AI provider config
#[derive(Parser, Clone)]
pub struct AiOutputCommand {
    #[arg(help = "Comma-separated AI providers")]
    pub providers: String,

    #[arg(long, help = "Output key-value pairs")]
    pub kv: bool,

    #[arg(long, help = "Output b00t format")]
    pub b00t: bool,
}

impl_mcp_tool!(AiOutputCommand, "ai_output", ["ai", "output"]);

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

impl crate::clap_reflection::McpReflection for AgentDiscoverCommand {
    fn mcp_tool_name() -> String {
        "agent_discover".to_string()
    }
    fn command_path() -> Vec<String> {
        vec!["agent".to_string(), "discover".to_string()]
    }
}

impl crate::clap_reflection::McpExecutor for AgentDiscoverCommand {
    fn execute_mcp_call(
        params: &std::collections::HashMap<String, serde_json::Value>,
    ) -> anyhow::Result<String> {
        use std::time::Duration;

        let json = params
            .get("json")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);

        let _role_filter = params
            .get("role")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());

        let rt = tokio::runtime::Runtime::new()?;
        rt.block_on(async {
            let client = get_nats_client().await?;

            let mut rx = client
                .subscribe_notifications("b00t.>.>")
                .await
                .map_err(|e| anyhow::anyhow!("Failed to subscribe: {}", e))?;

            let mut discovered = Vec::new();
            let deadline = Duration::from_secs(2);
            loop {
                match tokio::time::timeout(deadline, rx.recv()).await {
                    Ok(Some(notif)) => {
                        let entry = serde_json::json!({
                            "source": notif.source,
                            "event": notif.event_type,
                            "timestamp": notif.timestamp.to_rfc3339(),
                        });
                        discovered.push(entry);
                    }
                    _ => break,
                }
            }

            if json {
                Ok(serde_json::to_string_pretty(&serde_json::json!({
                    "agents": discovered,
                    "transport": "nats",
                    "count": discovered.len(),
                }))?)
            } else {
                if discovered.is_empty() {
                    return Err(anyhow::anyhow!("No agents discovered within listen window"));
                }
                let names: Vec<String> = discovered
                    .iter()
                    .map(|a| a["source"].as_str().unwrap_or("unknown").to_string())
                    .collect();
                Ok(format!(
                    "Discovered {} agents: {}",
                    names.len(),
                    names.join(", ")
                ))
            }
        })
    }
}

/// List b00t skills
#[derive(Parser, Clone)]
pub struct SkillListCommand {
    #[arg(long, help = "Filter by agent role")]
    pub role: Option<String>,

    #[arg(long, help = "JSON output")]
    pub json: bool,
}

impl_mcp_tool!(SkillListCommand, "skill_list", ["skill", "list"]);

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

impl crate::clap_reflection::McpReflection for AgentMessageCommand {
    fn mcp_tool_name() -> String {
        "agent_message".to_string()
    }
    fn command_path() -> Vec<String> {
        vec!["agent".to_string(), "message".to_string()]
    }
}

impl crate::clap_reflection::McpExecutor for AgentMessageCommand {
    fn execute_mcp_call(
        params: &std::collections::HashMap<String, serde_json::Value>,
    ) -> anyhow::Result<String> {
        use b00t_chat::NotificationMessage;

        let to_agent = params
            .get("to_agent")
            .and_then(|v| v.as_str())
            .unwrap_or("unknown");
        let content = params.get("content").and_then(|v| v.as_str()).unwrap_or("");
        let subject = params
            .get("subject")
            .and_then(|v| v.as_str())
            .unwrap_or("message");

        record_message(
            &caller_agent_id(),
            b00t_council::Recipient::Direct(to_agent.to_string()),
            serde_json::json!({"subject": subject, "content": content}),
        );

        let rt = tokio::runtime::Runtime::new()?;
        rt.block_on(async {
            let client = get_nats_client().await?;

            let notification = NotificationMessage::new(
                format!("message.{}", subject),
                "agent_message",
                serde_json::json!({"to": to_agent, "content": content, "subject": subject}),
            );

            client
                .publish_notification(&notification)
                .await
                .map_err(|e| anyhow::anyhow!("Failed to send message: {}", e))?;

            Ok(format!(
                "Message sent to {} on subject {}",
                to_agent, subject
            ))
        })
    }
}

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
    pub priority: Option<String>,

    #[arg(long, help = "Deadline in minutes")]
    pub deadline: Option<u64>,

    #[arg(long, help = "Required capabilities (comma-separated)")]
    pub capabilities: Option<String>,

    #[arg(long, help = "Block until completion")]
    pub blocking: bool,
}

impl crate::clap_reflection::McpReflection for AgentDelegateCommand {
    fn mcp_tool_name() -> String {
        "agent_delegate".to_string()
    }
    fn command_path() -> Vec<String> {
        vec!["agent".to_string(), "delegate".to_string()]
    }
}

impl crate::clap_reflection::McpExecutor for AgentDelegateCommand {
    fn execute_mcp_call(
        params: &std::collections::HashMap<String, serde_json::Value>,
    ) -> anyhow::Result<String> {
        use b00t_chat::NotificationMessage;

        let worker = params
            .get("worker")
            .and_then(|v| v.as_str())
            .unwrap_or("unknown");
        let task_id = params
            .get("task_id")
            .and_then(|v| v.as_str())
            .unwrap_or("delegate");
        let description = params
            .get("description")
            .and_then(|v| v.as_str())
            .unwrap_or("");

        let rt = tokio::runtime::Runtime::new()?;
        rt.block_on(async {
            let client = get_nats_client().await?;
            let notification = NotificationMessage::new(
                "delegate",
                task_id,
                serde_json::json!({"worker": worker, "description": description, "task_id": task_id}),
            );
            client.publish_notification(&notification).await
                .map_err(|e| anyhow::anyhow!("Failed to delegate: {}", e))?;
            Ok(format!("Task {} delegated to {}", task_id, worker))
        })
    }
}

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

impl_mcp_tool!(
    AgentCompleteCommand,
    "agent_complete",
    ["agent", "complete"]
);

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

impl_mcp_tool!(
    AgentProgressCommand,
    "agent_progress",
    ["agent", "progress"]
);

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
    pub vote_type: String,

    #[arg(long, help = "Deadline in minutes")]
    pub deadline: u64,

    #[arg(help = "Eligible voters (comma-separated agent IDs)")]
    pub voters: String,
}

impl crate::clap_reflection::McpReflection for AgentVoteCreateCommand {
    fn mcp_tool_name() -> String {
        "agent_vote_create".to_string()
    }
    fn command_path() -> Vec<String> {
        vec![
            "agent".to_string(),
            "vote".to_string(),
            "create".to_string(),
        ]
    }
}

impl crate::clap_reflection::McpExecutor for AgentVoteCreateCommand {
    fn execute_mcp_call(
        params: &std::collections::HashMap<String, serde_json::Value>,
    ) -> anyhow::Result<String> {
        use b00t_chat::NotificationMessage;

        let subject = params
            .get("subject")
            .and_then(|v| v.as_str())
            .unwrap_or("proposal");
        let description = params
            .get("description")
            .and_then(|v| v.as_str())
            .unwrap_or("");
        let vote_type = params
            .get("vote_type")
            .and_then(|v| v.as_str())
            .unwrap_or("");
        let deadline_minutes = params.get("deadline").and_then(|v| v.as_u64()).unwrap_or(0);
        let voters: Vec<String> = params
            .get("voters")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .split(',')
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .collect();

        // `options` arrives as a JSON array (per the tool's own help text);
        // fall back to a single literal option if it isn't valid JSON so a
        // caller passing a bare string doesn't just fail outright.
        let raw_options = params
            .get("options")
            .and_then(|v| v.as_str())
            .unwrap_or("[]");
        let options: Vec<String> = serde_json::from_str(raw_options)
            .unwrap_or_else(|_| vec![raw_options.to_string()]);

        let proposal = b00t_council::Proposal {
            id: uuid::Uuid::new_v4().to_string(),
            subject: subject.to_string(),
            options,
            quorum: quorum_for_vote_type(vote_type),
            deadline: (deadline_minutes > 0)
                .then(|| chrono::Utc::now() + chrono::Duration::minutes(deadline_minutes as i64)),
            eligible_voters: voters,
        };

        record_message(
            &caller_agent_id(),
            b00t_council::Recipient::Channel(proposal.id.clone()),
            serde_json::json!({"kind": "proposal", "description": description, "proposal": proposal}),
        );

        let proposal_id = proposal.id.clone();
        let rt = tokio::runtime::Runtime::new()?;
        rt.block_on(async {
            let client = get_nats_client().await?;
            let notification = NotificationMessage::new(
                "vote",
                "create",
                serde_json::json!({"proposal_id": proposal_id, "subject": subject, "description": description}),
            );
            client
                .publish_notification(&notification)
                .await
                .map_err(|e| anyhow::anyhow!("Failed to create vote: {}", e))?;
            Ok(format!(
                "Vote created: {} (proposal_id={}) — submit with agent_vote_submit, resolve with agent_vote_tally",
                subject, proposal_id
            ))
        })
    }
}

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

impl crate::clap_reflection::McpReflection for AgentVoteSubmitCommand {
    fn mcp_tool_name() -> String {
        "agent_vote_submit".to_string()
    }
    fn command_path() -> Vec<String> {
        vec![
            "agent".to_string(),
            "vote".to_string(),
            "submit".to_string(),
        ]
    }
}

impl crate::clap_reflection::McpExecutor for AgentVoteSubmitCommand {
    fn execute_mcp_call(
        params: &std::collections::HashMap<String, serde_json::Value>,
    ) -> anyhow::Result<String> {
        use b00t_chat::NotificationMessage;

        let proposal_id = params
            .get("proposal_id")
            .and_then(|v| v.as_str())
            .unwrap_or("unknown");
        let vote = params.get("vote").and_then(|v| v.as_str()).unwrap_or("");
        let reasoning = params
            .get("reasoning")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());
        let voter = caller_agent_id();

        record_message(
            &voter,
            b00t_council::Recipient::Channel(proposal_id.to_string()),
            serde_json::json!({"kind": "vote", "voter": voter, "choice": vote, "reasoning": reasoning}),
        );

        let rt = tokio::runtime::Runtime::new()?;
        rt.block_on(async {
            let client = get_nats_client().await?;
            let notification = NotificationMessage::new(
                "vote",
                "submit",
                serde_json::json!({"proposal_id": proposal_id, "vote": vote}),
            );
            client
                .publish_notification(&notification)
                .await
                .map_err(|e| anyhow::anyhow!("Failed to submit vote: {}", e))?;
            Ok(format!(
                "Vote submitted for proposal {} — check outcome with agent_vote_tally",
                proposal_id
            ))
        })
    }
}

/// MCP command for tallying a vote — replays the durable message log for a
/// proposal's `Channel(proposal_id)` traffic and resolves it via
/// `b00t_council::tally`. This is the fix for the two stub tools above: a
/// vote now has an actual, checkable outcome instead of two indistinguishable
/// NATS notifications.
#[derive(Parser, Clone)]
pub struct AgentVoteTallyCommand {
    #[arg(help = "Proposal ID")]
    pub proposal_id: String,
}

impl crate::clap_reflection::McpReflection for AgentVoteTallyCommand {
    fn mcp_tool_name() -> String {
        "agent_vote_tally".to_string()
    }
    fn command_path() -> Vec<String> {
        vec![
            "agent".to_string(),
            "vote".to_string(),
            "tally".to_string(),
        ]
    }
}

impl crate::clap_reflection::McpExecutor for AgentVoteTallyCommand {
    fn execute_mcp_call(
        params: &std::collections::HashMap<String, serde_json::Value>,
    ) -> anyhow::Result<String> {
        let proposal_id = params
            .get("proposal_id")
            .and_then(|v| v.as_str())
            .unwrap_or("unknown");

        let envelopes = message_sink()
            .replay(&b00t_council::ReplayFilter::channel(proposal_id))
            .map_err(|e| anyhow::anyhow!("Failed to replay message log: {}", e))?;

        let mut proposal: Option<b00t_council::Proposal<String>> = None;
        let mut ballots: Vec<(String, b00t_council::Ballot<String>)> = Vec::new();

        for env in &envelopes {
            let kind = env.body.get("kind").and_then(|v| v.as_str()).unwrap_or("");
            match kind {
                "proposal" => {
                    if let Some(p) = env.body.get("proposal") {
                        if let Ok(p) = serde_json::from_value::<b00t_council::Proposal<String>>(p.clone()) {
                            proposal = Some(p);
                        }
                    }
                }
                "vote" => {
                    let voter = env
                        .body
                        .get("voter")
                        .and_then(|v| v.as_str())
                        .unwrap_or(&env.from)
                        .to_string();
                    let choice = env.body.get("choice").and_then(|v| v.as_str()).unwrap_or("");
                    let ballot = if choice.eq_ignore_ascii_case("veto") {
                        b00t_council::Ballot::Veto { alternative: None }
                    } else if choice.eq_ignore_ascii_case("abstain") {
                        b00t_council::Ballot::Abstain
                    } else {
                        b00t_council::Ballot::Cast(choice.to_string())
                    };
                    ballots.push((voter, ballot));
                }
                _ => {}
            }
        }

        let Some(proposal) = proposal else {
            return Ok(format!(
                "No proposal found for {} — has agent_vote_create been called for it?",
                proposal_id
            ));
        };

        let outcome = b00t_council::tally(&proposal.options, &ballots, &proposal.quorum);
        let breakdown: std::collections::HashMap<&str, usize> =
            ballots.iter().fold(std::collections::HashMap::new(), |mut acc, (_, b)| {
                let key = match b {
                    b00t_council::Ballot::Cast(o) => o.as_str(),
                    b00t_council::Ballot::Veto { .. } => "veto",
                    b00t_council::Ballot::Abstain => "abstain",
                };
                *acc.entry(key).or_insert(0) += 1;
                acc
            });

        let outcome_str = match &outcome {
            b00t_council::Outcome::Passed(option) => format!("Passed({option})"),
            b00t_council::Outcome::Rejected => "Rejected".to_string(),
            b00t_council::Outcome::Pending => "Pending".to_string(),
        };

        Ok(format!(
            "Proposal {} ({}): {outcome_str} — {} ballots cast, breakdown: {breakdown:?}",
            proposal_id,
            proposal.subject,
            ballots.len(),
        ))
    }
}

/// MCP tool for datum delegation authorization gate — bridges b00t → ledgrrr cost gate.
///
/// Sends a `ledgerr_b00t_delegate_datum` MCP call to the running `ledgerr-mcp` server
/// (spawned from vendor/l3dg3rr via `LEDGERR_MCP_CMD` or detected path).
/// Returns `DelegateAuthority`-shaped JSON: `{authorized, budget_remaining_cake, resume_token}`.
///
/// Resume token format: `<datum_id>:<task_id>:<epoch_secs>` — opaque to b00t, used
/// by ledgrrr to reconstruct the delegation context on proceed.
///
/// Cost gate: ledgrrr enforces $10.00 per-request limit; denial_reason is set if exceeded.
#[derive(Parser, Clone)]
pub struct DelegateDatumCommand {
    #[arg(help = "Datum ID to authorize")]
    pub datum_id: String,

    #[arg(long, help = "Agent requesting authorization")]
    pub agent_id: String,

    #[arg(long, help = "Task ID this delegation belongs to")]
    pub task_id: String,

    #[arg(long, help = "Estimated cost in 🍰 cake")]
    pub estimated_cost_cake: f64,

    #[arg(long, help = "Human-readable justification for this delegation")]
    pub justification: Option<String>,
}

impl crate::clap_reflection::McpReflection for DelegateDatumCommand {
    fn mcp_tool_name() -> String {
        "delegate".to_string()
    }
    fn command_path() -> Vec<String> {
        vec![]
    }
}

impl crate::clap_reflection::McpExecutor for DelegateDatumCommand {
    fn execute_mcp_call(params: &HashMap<String, Value>) -> Result<String> {
        let datum_id = params
            .get("datum_id")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow::anyhow!("b00t_delegate_datum requires datum_id: string"))?;
        let agent_id = params
            .get("agent_id")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow::anyhow!("b00t_delegate_datum requires agent_id: string"))?;
        let task_id = params
            .get("task_id")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow::anyhow!("b00t_delegate_datum requires task_id: string"))?;
        let estimated_cost_cake = params
            .get("estimated_cost_cake")
            .and_then(|v| v.as_f64())
            .ok_or_else(|| {
                anyhow::anyhow!("b00t_delegate_datum requires estimated_cost_cake: number")
            })?;

        // Bridge: call ledgerr-mcp via MCP stdio subprocess.
        // Discover binary: LEDGERR_MCP_CMD env > vendor/l3dg3rr build path.
        let ledgerr_cmd = std::env::var("LEDGERR_MCP_CMD").ok().or_else(|| {
            // Walk up from b00t home to find vendored binary
            let b00t_home = std::env::var("B00T_HOME")
                .ok()
                .or_else(|| std::env::var("HOME").ok().map(|h| format!("{h}/.b00t")))
                .unwrap_or_else(|| "~/.b00t".to_string());
            let path = std::path::PathBuf::from(&b00t_home)
                .join("vendor/l3dg3rr/target/release/ledgerr-mcp-server");
            if path.exists() {
                Some(path.to_string_lossy().into_owned())
            } else {
                None
            }
        });

        let Some(cmd_path) = ledgerr_cmd else {
            // ⚠️ BRIDGE PENDING: ledgerr-mcp binary not found.
            // Set LEDGERR_MCP_CMD or build vendor/l3dg3rr with:
            //   cd vendor/l3dg3rr && cargo build -p ledgerr-mcp --features b00t --release
            // Until then, fall back to synthetic authorized=true with local resume token.
            let epoch_secs = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs();
            eprintln!("[b00t_delegate] LEDGERR_MCP_CMD unset — using local gate fallback");
            eprintln!(
                "[b00t_delegate] datum={datum_id} agent={agent_id} task={task_id} cost={estimated_cost_cake:.4} 🍰 → local-authorized"
            );
            return Ok(serde_json::to_string_pretty(&json!({
                "authorized": true,
                "datum_id": datum_id,
                "agent_id": agent_id,
                "task_id": task_id,
                "budget_remaining_cake": (100.0 - estimated_cost_cake).max(0.0),
                "resume_token": format!("{datum_id}:{task_id}:{epoch_secs}"),
                "denial_reason": null,
                "_bridge": "⚠️ BRIDGE PENDING: LEDGERR_MCP_CMD not set — local fallback gate"
            }))?);
        };

        // Send initialize + tools/call via MCP stdio
        let payload = json!({
            "jsonrpc": "2.0",
            "method": "tools/call",
            "id": 1,
            "params": {
                "name": "ledgerr_b00t_delegate_datum",
                "arguments": {
                    "datum_id": datum_id,
                    "agent_id": agent_id,
                    "task_id": task_id,
                    "estimated_cost_cake": estimated_cost_cake
                }
            }
        });

        let result = call_ledgerr_mcp_stdio(&cmd_path, &payload).unwrap_or_else(|e| {
            eprintln!("[b00t_delegate] ledgerr-mcp subprocess error: {e}");
            json!({
                "authorized": false,
                "denial_reason": format!("ledgerr-mcp subprocess error: {e}"),
                "_bridge": "subprocess-error"
            })
        });

        Ok(serde_json::to_string_pretty(&result)?)
    }
}

/// Spawn ledgerr-mcp as stdio MCP subprocess, send initialize + one tools/call, return result.
fn call_ledgerr_mcp_stdio(cmd_path: &str, payload: &Value) -> Result<Value> {
    use std::io::{BufRead, BufReader, Write};
    use std::process::{Command, Stdio};
    use std::time::{Duration, Instant};

    let mut child = Command::new(cmd_path)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|e| anyhow::anyhow!("spawn ledgerr-mcp-server failed: {e}"))?;

    let mut stdin = child
        .stdin
        .take()
        .ok_or_else(|| anyhow::anyhow!("no stdin on ledgerr-mcp child"))?;
    let stdout = child
        .stdout
        .take()
        .ok_or_else(|| anyhow::anyhow!("no stdout on ledgerr-mcp child"))?;

    // Step 1: initialize handshake
    let init = json!({
        "jsonrpc": "2.0", "id": 0, "method": "initialize",
        "params": {
            "protocolVersion": "2024-11-05",
            "capabilities": {},
            "clientInfo": { "name": "b00t-mcp", "version": "0.1.0" }
        }
    });
    writeln!(stdin, "{}", serde_json::to_string(&init)?)?;
    stdin.flush()?;

    let mut reader = BufReader::new(stdout);
    let deadline = Instant::now() + Duration::from_secs(8);

    // Read initialize response
    let mut _init_resp = String::new();
    loop {
        if Instant::now() > deadline {
            let _ = child.kill();
            return Err(anyhow::anyhow!(
                "timeout waiting for ledgerr-mcp initialize"
            ));
        }
        let mut line = String::new();
        if reader.read_line(&mut line)? == 0 {
            break;
        }
        if !line.trim().is_empty() {
            _init_resp = line;
            break;
        }
    }

    // Step 2: send tools/call
    writeln!(stdin, "{}", serde_json::to_string(payload)?)?;
    stdin.flush()?;

    // Read tools/call response
    let mut response_line = String::new();
    let deadline2 = Instant::now() + Duration::from_secs(10);
    loop {
        if Instant::now() > deadline2 {
            let _ = child.kill();
            return Err(anyhow::anyhow!(
                "timeout waiting for ledgerr-mcp delegate response"
            ));
        }
        let mut line = String::new();
        if reader.read_line(&mut line)? == 0 {
            break;
        }
        if !line.trim().is_empty() {
            response_line = line;
            break;
        }
    }

    let _ = child.kill();

    if response_line.is_empty() {
        return Err(anyhow::anyhow!("empty response from ledgerr-mcp"));
    }

    let resp: Value = serde_json::from_str(response_line.trim())?;
    // Extract the result field from JSON-RPC envelope
    Ok(resp.get("result").cloned().unwrap_or(resp))
}

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

impl crate::clap_reflection::McpReflection for AgentWaitCommand {
    fn mcp_tool_name() -> String {
        "agent_wait".to_string()
    }
    fn command_path() -> Vec<String> {
        vec!["agent".to_string(), "wait".to_string()]
    }
}

impl crate::clap_reflection::McpExecutor for AgentWaitCommand {
    fn execute_mcp_call(
        params: &std::collections::HashMap<String, serde_json::Value>,
    ) -> anyhow::Result<String> {
        use std::time::Duration;

        let timeout_secs = params
            .get("timeout")
            .and_then(|v| v.as_u64())
            .unwrap_or(300);
        let event_type = params
            .get("message_type")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());
        let source_filter = params
            .get("subject")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());

        let wildcard = if let Some(ref src) = source_filter {
            format!("b00t.notify.{}.>", src)
        } else {
            "b00t.notify.>".to_string()
        };

        let rt = tokio::runtime::Runtime::new()?;
        rt.block_on(async {
            let client = get_nats_client().await?;

            let mut rx = client
                .subscribe_notifications(&wildcard)
                .await
                .map_err(|e| anyhow::anyhow!("Failed to subscribe: {}", e))?;

            let deadline = Duration::from_secs(timeout_secs);
            match tokio::time::timeout(deadline, rx.recv()).await {
                Ok(Some(notification)) => {
                    let matches = event_type
                        .as_ref()
                        .map_or(true, |et| notification.event_type == *et);
                    if matches {
                        Ok(serde_json::to_string_pretty(&notification)?)
                    } else {
                        anyhow::bail!("Notification received but type mismatch")
                    }
                }
                Ok(None) => anyhow::bail!("Notification channel closed"),
                Err(_) => anyhow::bail!("Wait timed out after {} seconds", timeout_secs),
            }
        })
    }
}

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

impl crate::clap_reflection::McpReflection for AgentNotifyCommand {
    fn mcp_tool_name() -> String {
        "agent_notify".to_string()
    }
    fn command_path() -> Vec<String> {
        vec!["agent".to_string(), "notify".to_string()]
    }
}

impl crate::clap_reflection::McpExecutor for AgentNotifyCommand {
    fn execute_mcp_call(
        params: &std::collections::HashMap<String, serde_json::Value>,
    ) -> anyhow::Result<String> {
        use b00t_chat::NotificationMessage;
        use serde_json;

        let source = params
            .get("source")
            .and_then(|v| v.as_str())
            .unwrap_or("mcp");
        let event_type = params
            .get("event_type")
            .and_then(|v| v.as_str())
            .unwrap_or("notification");
        let details = params
            .get("details")
            .map(|v| v.clone())
            .unwrap_or(serde_json::Value::Null);

        record_message(
            source,
            b00t_council::Recipient::Broadcast,
            serde_json::json!({"event_type": event_type, "details": details}),
        );

        let rt = tokio::runtime::Runtime::new()?;
        rt.block_on(async {
            let client = get_nats_client().await?;

            let notification = NotificationMessage::new(source, event_type, details);

            client
                .publish_notification(&notification)
                .await
                .map_err(|e| anyhow::anyhow!("Failed to publish notification: {}", e))?;

            Ok(format!(
                "Notification published: {}.{}",
                notification.source, notification.event_type
            ))
        })
    }
}

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

impl crate::clap_reflection::McpReflection for AgentCapabilityCommand {
    fn mcp_tool_name() -> String {
        "agent_capability".to_string()
    }
    fn command_path() -> Vec<String> {
        vec!["agent".to_string(), "capability".to_string()]
    }
}

impl crate::clap_reflection::McpExecutor for AgentCapabilityCommand {
    fn execute_mcp_call(
        params: &std::collections::HashMap<String, serde_json::Value>,
    ) -> anyhow::Result<String> {
        use b00t_chat::NotificationMessage;
        use std::time::Duration;

        let capabilities = params
            .get("capabilities")
            .and_then(|v| v.as_str())
            .unwrap_or("");
        let description = params
            .get("description")
            .and_then(|v| v.as_str())
            .unwrap_or("");

        let rt = tokio::runtime::Runtime::new()?;
        rt.block_on(async {
            let client = get_nats_client().await?;

            let mut rx = client
                .subscribe_notifications("b00t.notify.capability.>")
                .await
                .map_err(|e| anyhow::anyhow!("Failed to subscribe: {}", e))?;

            client
                .publish_notification(&NotificationMessage::new(
                    "capability",
                    "query",
                    serde_json::json!({"capabilities": capabilities, "description": description}),
                ))
                .await
                .map_err(|e| anyhow::anyhow!("Failed to query capabilities: {}", e))?;

            let mut responses = Vec::new();
            let deadline = Duration::from_secs(3);
            loop {
                match tokio::time::timeout(deadline, rx.recv()).await {
                    Ok(Some(notif)) => {
                        if notif.source == "capability" && notif.event_type == "query" {
                            continue; // skip own query echo
                        }
                        responses.push(serde_json::json!({
                            "source": notif.source,
                            "event": notif.event_type,
                            "timestamp": notif.timestamp.to_rfc3339(),
                            "payload": notif.payload,
                        }));
                    }
                    Ok(None) => break,
                    Err(_) => break,
                }
            }

            if responses.is_empty() {
                return Err(anyhow::anyhow!("No agents responded to capability query"));
            }

            Ok(serde_json::to_string_pretty(&serde_json::json!({
                "query": {"capabilities": capabilities, "description": description},
                "responses": responses,
                "count": responses.len(),
            }))?)
        })
    }
}

/// App VSCode MCP install command
#[derive(Parser, Clone)]
pub struct AppVscodeMcpInstallCommand {
    #[arg(help = "MCP server name to install")]
    pub name: String,
}

impl_mcp_tool!(
    AppVscodeMcpInstallCommand,
    "app_vscode_mcp_install",
    ["app", "vscode", "mcp", "install"]
);

/// App Claude Code MCP install command
#[derive(Parser, Clone)]
pub struct AppClaudecodeMcpInstallCommand {
    #[arg(help = "MCP server name to install")]
    pub name: String,
}

impl_mcp_tool!(
    AppClaudecodeMcpInstallCommand,
    "app_claudecode_mcp_install",
    ["app", "claudecode", "mcp", "install"]
);

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

    #[arg(
        long,
        help = "Select stdio method by command (for multi-source MCP configs)"
    )]
    pub stdio_command: Option<String>,

    #[arg(long, help = "Use httpstream method (for multi-source MCP configs)")]
    pub httpstream: bool,
}

impl_mcp_tool!(McpInstallCommand, "mcp_install", ["mcp", "install"]);

/// Session init command
// 🤓 ENTANGLED: b00t-cli/src/commands/session.rs SessionCommands::Init
// When this changes, update b00t-cli SessionCommands::Init structure
// 🤓 CLI session init accepts only --budget and --name (both flags, no positionals)
//    time_limit and agent params removed — not present in b00t-cli SessionCommands::Init
#[derive(Parser, Clone)]
pub struct SessionInitCommand {
    #[arg(long, help = "Budget limit in dollars")]
    pub budget: Option<f64>,

    #[arg(long, help = "Session name")]
    pub name: Option<String>,
}

impl_mcp_tool!(SessionInitCommand, "session_init", ["session", "init"]);

/// Session status command
#[derive(Parser, Clone)]
pub struct SessionStatusCommand;

impl_mcp_tool!(
    SessionStatusCommand,
    "session_status",
    ["session", "status"]
);

/// Session end command
#[derive(Parser, Clone)]
pub struct SessionEndCommand;

impl_mcp_tool!(SessionEndCommand, "session_end", ["session", "end"]);

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

impl_mcp_tool!(CheckpointCommand, "checkpoint", ["checkpoint"]);

// Grok knowledgebase MCP tools

/// Digest content into chunks about a topic
/// 🤓 ENTANGLED: b00t-cli/src/commands/grok.rs GrokCommands::Digest
/// 🤓 content is positional in CLI: `b00t-cli grok digest --topic <TOPIC> <CONTENT>`
#[derive(Parser, Clone)]
pub struct GrokDigestCommand {
    #[arg(long, short = 't', help = "Topic to digest about")]
    pub topic: String,

    #[arg(help = "Content to digest")]
    pub content: String,

    #[arg(
        long,
        default_value = "both",
        help = "RAG: raglite | irontology | both"
    )]
    pub rag: Option<String>,
}

impl_mcp_tool!(
    GrokDigestCommand,
    "grok_digest",
    ["grok", "digest"],
    positionals: ["content"]
);

/// Ask questions and search the knowledgebase
/// 🤓 ENTANGLED: b00t-cli/src/commands/grok.rs GrokCommands::Ask
/// 🤓 query is positional in CLI: `b00t-cli grok ask <QUERY>`
#[derive(Parser, Clone)]
pub struct GrokAskCommand {
    #[arg(help = "Query to search for")]
    pub query: String,

    #[arg(long, help = "Filter by topic")]
    pub topic: Option<String>,

    #[arg(long, help = "Max results", default_value = "10")]
    pub limit: Option<usize>,

    #[arg(
        long,
        default_value = "both",
        help = "RAG: raglite | irontology | both"
    )]
    pub rag: Option<String>,
}

impl_mcp_tool!(
    GrokAskCommand,
    "grok_ask",
    ["grok", "ask"],
    positionals: ["query"]
);

/// Learn from URLs or content
/// 🤓 ENTANGLED: b00t-cli/src/commands/grok.rs GrokCommands::Learn
/// 🤓 content is positional in CLI: `b00t-cli grok learn [CONTENT]`
#[derive(Parser, Clone)]
pub struct GrokLearnCommand {
    #[arg(help = "Content to learn from")]
    pub content: Option<String>,

    #[arg(short = 't', long, help = "Topic for RAG indexing")]
    pub topic: Option<String>,

    #[arg(short = 's', long, help = "Source URL or file path")]
    pub source: Option<String>,

    #[arg(
        long,
        default_value = "both",
        help = "RAG: raglite | irontology | both"
    )]
    pub rag: Option<String>,
}

impl_mcp_tool!(
    GrokLearnCommand,
    "grok_learn",
    ["grok", "learn"],
    positionals: ["content"]
);

/// MCP command for getting grok system status
#[derive(Parser, Clone)]
pub struct GrokStatusCommand;

impl_mcp_tool!(GrokStatusCommand, "grok_status", ["grok", "status"]);

/// Random walk through the knowledge graph — surfaces unexpected connections
/// 🤓 ENTANGLED: b00t-cli/src/commands/grok.rs GrokCommands::Wander
/// 🤓 #247: gap identified vs Cortex's 44-tool ideation (no exploratory
///    traversal existed — `ontology sparql` is targeted-query only).
/// 🤓 No required fields — zero-friction serendipity by design, per the
///    Reddit post's own lesson: "more than three required fields, nobody
///    will call it voluntarily."
#[derive(Parser, Clone)]
pub struct GrokWanderCommand {
    #[arg(long, help = "Restrict wandering to one topic (default: random)")]
    pub topic: Option<String>,

    #[arg(
        long,
        default_value = "both",
        help = "RAG: raglite | irontology | both"
    )]
    pub rag: Option<String>,
}

impl_mcp_tool!(GrokWanderCommand, "grok_wander", ["grok", "wander"]);

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

    #[arg(
        long,
        help = "NATS server URL (defaults to c010.promptexecution.com:4222)"
    )]
    pub nats_url: Option<String>,
}

impl_mcp_tool!(AcpHiveJoinCommand, "acp_hive_join", ["acp", "hive", "join"]);

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

    #[arg(
        long,
        help = "NATS server URL (defaults to c010.promptexecution.com:4222)"
    )]
    pub nats_url: Option<String>,
}

impl_mcp_tool!(
    AcpHiveCreateCommand,
    "acp_hive_create",
    ["acp", "hive", "create"]
);

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

impl_mcp_tool!(
    AcpHiveStatusCommand,
    "acp_hive_status",
    ["acp", "hive", "status"]
);

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

impl_mcp_tool!(
    AcpHiveProposeCommand,
    "acp_hive_propose",
    ["acp", "hive", "propose"]
);

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

impl_mcp_tool!(AcpHiveSyncCommand, "acp_hive_sync", ["acp", "hive", "sync"]);

/// MCP command for signaling step readiness
#[derive(Parser, Clone)]
pub struct AcpHiveReadyCommand {
    #[arg(help = "Mission identifier")]
    pub mission_id: String,

    #[arg(help = "Step to signal readiness for")]
    pub target_step: u64,
}

impl_mcp_tool!(
    AcpHiveReadyCommand,
    "acp_hive_ready",
    ["acp", "hive", "ready"]
);

/// MCP command for showing hive status
#[derive(Parser, Clone)]
pub struct AcpHiveShowCommand {
    #[arg(help = "Mission identifier (optional - shows all missions if not specified)")]
    pub mission_id: Option<String>,
}

impl_mcp_tool!(AcpHiveShowCommand, "acp_hive_show", ["acp", "hive", "show"]);

/// MCP command for leaving hive mission
#[derive(Parser, Clone)]
pub struct AcpHiveLeaveCommand {
    #[arg(help = "Mission identifier")]
    pub mission_id: String,
}

impl_mcp_tool!(
    AcpHiveLeaveCommand,
    "acp_hive_leave",
    ["acp", "hive", "leave"]
);

// Custom implementations for ACP hive tools
// 🤓 Disabled - acp_hive uses full NATS Agent from old ACP; chat refactor simplified to stubs

/// MCP command for creating a cron-based timer assignment
#[derive(Parser, Clone)]
pub struct B00tTimerCommand {
    #[arg(help = "Rule name (e.g., 'daily-summary')")]
    pub name: String,

    #[arg(
        help = "Cron expression (e.g., '0 9 * * 1-5' for weekdays at 9am, '*/5 * * * *' for every 5 minutes) or interval seconds (e.g., '3600' for hourly)"
    )]
    pub schedule: String,

    #[arg(help = "Target agent ID to dispatch the task to")]
    pub to_agent: String,

    #[arg(help = "Action name to dispatch (e.g., 'summarize', 'check')")]
    pub action: String,

    #[arg(long, help = "Additional payload data as JSON")]
    pub payload: Option<String>,

    #[arg(long, help = "Fire once then disable (default: repeat)")]
    pub once: bool,
}

impl crate::clap_reflection::McpReflection for B00tTimerCommand {
    fn mcp_tool_name() -> String {
        "b00t_timer_create".to_string()
    }
    fn command_path() -> Vec<String> {
        vec![
            "b00t".to_string(),
            "timer".to_string(),
            "create".to_string(),
        ]
    }
}

impl crate::clap_reflection::McpExecutor for B00tTimerCommand {
    fn execute_mcp_call(
        params: &std::collections::HashMap<String, serde_json::Value>,
    ) -> anyhow::Result<String> {
        use b00t_chat::NotificationMessage;

        let name = params
            .get("name")
            .and_then(|v| v.as_str())
            .unwrap_or("timer");
        let schedule = params
            .get("schedule")
            .and_then(|v| v.as_str())
            .unwrap_or("60");
        let to_agent = params
            .get("to_agent")
            .and_then(|v| v.as_str())
            .unwrap_or("default");
        let action = params
            .get("action")
            .and_then(|v| v.as_str())
            .unwrap_or("timer-tick");
        let once = params
            .get("once")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);
        let payload_str = params
            .get("payload")
            .and_then(|v| v.as_str())
            .and_then(|s| serde_json::from_str::<serde_json::Value>(s).ok());

        let is_cron = schedule.contains('*') || schedule.contains('/');
        let timer_type = if is_cron { "cron" } else { "interval" };

        let rt = tokio::runtime::Runtime::new()?;
        rt.block_on(async {
            let client = get_nats_client().await?;

            let notification = NotificationMessage::new(
                "timer",
                "create",
                serde_json::json!({
                    "name": name,
                    "schedule": schedule,
                    "to_agent": to_agent,
                    "action": action,
                    "timer_type": timer_type,
                    "once": once,
                    "payload": payload_str,
                }),
            );

            client
                .publish_notification(&notification)
                .await
                .map_err(|e| anyhow::anyhow!("Failed to create timer: {}", e))?;

            Ok(format!(
                "Timer '{}' created ({}: {}, target: {}, action: {})\n\
                 The timer will publish tasks on schedule to b00t.tasks.{}. \n\
                 Monitor with: b00t_agent_wait --subject={}",
                name, timer_type, schedule, to_agent, action, to_agent, to_agent
            ))
        })
    }
}
// use crate::acp_tools::*;

/// Tutorial status command — show tutorial progression for current agent role
#[derive(Parser, Clone)]
pub struct TutorialStatusCommand;

impl_mcp_tool!(
    TutorialStatusCommand,
    "tutorial_status",
    ["tutorial", "status"]
);

/// Tutorial next command — get next recommended datum to install/validate
#[derive(Parser, Clone)]
pub struct TutorialNextCommand;

impl_mcp_tool!(TutorialNextCommand, "tutorial_next", ["tutorial", "next"]);

/// Query live capability ontology from datum TOMLs
// 🤓 ENTANGLED: b00t-cli/src/commands/ontology.rs OntologyCommands::Query
#[derive(Parser, Clone)]
pub struct OntologyQueryCommand {
    #[arg(long, help = "Filter by role (developer|orchestrator|analyst)")]
    pub role: Option<String>,

    #[arg(long, help = "Output: table or json", default_value = "json")]
    pub format: Option<String>,
}

impl_mcp_tool!(
    OntologyQueryCommand,
    "ontology_query",
    ["ontology", "query"]
);

/// Launch ralph agent REPL outer-loop
// 🤓 ENTANGLED: b00t-cli/src/commands/up.rs UpArgs
#[derive(Parser, Clone)]
pub struct UpCommand {
    #[arg(long, help = "AI tool for ralph loop", default_value = "claude")]
    pub tool: Option<String>,

    #[arg(long, help = "Max iterations per session", default_value = "10")]
    pub max_iter: Option<u32>,

    #[arg(long, help = "Agent role filter")]
    pub role: Option<String>,
}

impl_mcp_tool!(UpCommand, "up", ["up"]);

// ── b00t task MCP tools (BT1) — native task management, replaces taskmaster-ai ─
// 🤓 These proxy directly to `b00t task` CLI subcommands. No external deps.

/// List tasks (active = pending + in-progress by default)
#[derive(Parser, Clone)]
pub struct TaskListCommand {
    #[arg(
        long,
        help = "Filter: pending|in-progress|done|blocked|all (default: active)"
    )]
    pub status: Option<String>,
    #[arg(long, help = "Filter by tag")]
    pub tag: Option<String>,
    #[arg(long, help = "Output as JSON")]
    pub json: bool,
}
impl_mcp_tool!(TaskListCommand, "task_list", ["task", "list"]);

/// Get next actionable task (highest priority with all deps satisfied)
#[derive(Parser, Clone)]
pub struct TaskNextCommand {
    #[arg(long, help = "Output as JSON")]
    pub json: bool,
}
impl_mcp_tool!(TaskNextCommand, "task_next", ["task", "next"]);

/// Add a new task
#[derive(Parser, Clone)]
pub struct TaskAddCommand {
    #[arg(help = "Task title")]
    pub title: String,
    #[arg(long, short, help = "Description")]
    pub description: Option<String>,
    #[arg(long = "criteria", short = 'c', help = "Acceptance criteria")]
    pub acceptance_criteria: Option<String>,
    #[arg(
        long,
        short,
        help = "Priority 1-4 (1=critical, 4=low)",
        default_value = "3"
    )]
    pub priority: u8,
    #[arg(long, short, help = "Tags (comma-separated)")]
    pub tags: Option<String>,
}
impl_mcp_tool!(TaskAddCommand, "task_add", ["task", "add"], positionals: ["title"]);

/// Mark a task done
#[derive(Parser, Clone)]
pub struct TaskDoneCommand {
    #[arg(help = "Task ID")]
    pub id: u32,
}
impl_mcp_tool!(TaskDoneCommand, "task_done", ["task", "done"], positionals: ["id"]);

/// Update task status, title, or append a note
#[derive(Parser, Clone)]
pub struct TaskUpdateCommand {
    #[arg(help = "Task ID")]
    pub id: u32,
    #[arg(long, help = "New status: pending|in-progress|done|blocked|deferred")]
    pub status: Option<String>,
    #[arg(long, help = "New title")]
    pub title: Option<String>,
    #[arg(long, help = "Append to notes")]
    pub note: Option<String>,
    #[arg(long, help = "Priority 1-4")]
    pub priority: Option<u8>,
}
impl_mcp_tool!(TaskUpdateCommand, "task_update", ["task", "update"], positionals: ["id"]);

/// List registered justfile datums and recipe counts
#[derive(Parser, Clone)]
pub struct JustfileListCommand {
    #[arg(long, help = "Output as JSON")]
    pub json: bool,
}
impl_mcp_tool!(JustfileListCommand, "justfile_list", ["justfile", "list"]);

/// Query recipes for a registered justfile datum
#[derive(Parser, Clone)]
pub struct JustfileQueryCommand {
    #[arg(help = "Justfile datum name")]
    pub name: String,
    #[arg(long, help = "Filter recipe names/docs/dependencies by substring")]
    pub recipe: Option<String>,
    #[arg(long, help = "Output as JSON")]
    pub json: bool,
}
impl_mcp_tool!(JustfileQueryCommand, "justfile_query", ["justfile", "query"], positionals: ["name"]);

/// Validate a registered justfile datum through just's AST parser
#[derive(Parser, Clone)]
pub struct JustfileValidateCommand {
    #[arg(help = "Justfile datum name")]
    pub name: String,
}
impl_mcp_tool!(JustfileValidateCommand, "justfile_validate", ["justfile", "validate"], positionals: ["name"]);

/// Emit registered justfile paths for strict just-mcp startup
#[derive(Parser, Clone)]
pub struct JustfileRegistryCommand {
    #[arg(long, default_value = "args", help = "Output format: args|lines|json")]
    pub format: String,
}
impl_mcp_tool!(
    JustfileRegistryCommand,
    "justfile_registry",
    ["justfile", "registry"]
);

/// Run a recipe from a registered justfile datum
#[derive(Parser, Clone)]
pub struct JustfileRunCommand {
    #[arg(help = "Justfile datum name")]
    pub name: String,
    #[arg(help = "Recipe name")]
    pub recipe: String,
    #[arg(long, help = "Additional recipe arguments")]
    pub args: Vec<String>,
}

impl McpReflection for JustfileRunCommand {
    fn mcp_tool_name() -> String {
        "justfile_run".to_string()
    }

    fn command_path() -> Vec<String> {
        vec!["justfile".to_string(), "run".to_string()]
    }

    fn generate_json_schema() -> Map<String, Value> {
        let mut schema = Map::new();
        let mut properties = Map::new();
        schema.insert("type".to_string(), json!("object"));
        properties.insert(
            "name".to_string(),
            json!({"type": "string", "description": "Justfile datum name"}),
        );
        properties.insert(
            "recipe".to_string(),
            json!({"type": "string", "description": "Recipe name"}),
        );
        properties.insert(
            "args".to_string(),
            json!({"type": "array", "items": {"type": "string"}, "description": "Additional recipe arguments"}),
        );
        schema.insert("properties".to_string(), Value::Object(properties));
        schema.insert("required".to_string(), json!(["name", "recipe"]));
        schema
    }
}

impl McpExecutor for JustfileRunCommand {
    fn execute_mcp_call(params: &HashMap<String, Value>) -> Result<String> {
        let name = params
            .get("name")
            .and_then(Value::as_str)
            .ok_or_else(|| anyhow::anyhow!("name is required"))?;
        let recipe = params
            .get("recipe")
            .and_then(Value::as_str)
            .ok_or_else(|| anyhow::anyhow!("recipe is required"))?;

        let mut all_args = Self::command_path();
        all_args.push(name.to_string());
        all_args.push(recipe.to_string());
        if let Some(extra) = params.get("args") {
            match extra {
                Value::Array(items) => {
                    for item in items {
                        all_args.push(item.as_str().unwrap_or("").to_string());
                    }
                }
                Value::String(item) if !item.is_empty() => all_args.push(item.clone()),
                _ => {}
            }
        }

        let output = std::process::Command::new("b00t-cli")
            .args(&all_args)
            .output()
            .map_err(|e| anyhow::anyhow!("Failed to execute b00t-cli: {}", e))?;

        if output.status.success() {
            Ok(String::from_utf8_lossy(&output.stdout).to_string())
        } else {
            let stderr = String::from_utf8_lossy(&output.stderr);
            anyhow::bail!("b00t-cli command failed: {}", stderr)
        }
    }
}

// ── Autodiscovery Proxy ──────────────────────────────────────────────────────
// 🤓 54 tools hidden here; agent discovers via b00t_discover, executes via b00t_exec
// Sub-agents receive only 5 surface tools → lean context, sandboxed capability

/// Catalog entry: (tool_name, description, cli_subcommand)
pub struct ToolCatalogEntry {
    pub name: &'static str,
    pub description: &'static str,
    pub subcommand: &'static str,
}

pub static TOOL_CATALOG: &[ToolCatalogEntry] = &[
    ToolCatalogEntry {
        name: "b00t_mcp_list",
        description: "List registered MCP servers",
        subcommand: "mcp list",
    },
    ToolCatalogEntry {
        name: "b00t_mcp_add",
        description: "Register a new MCP server",
        subcommand: "mcp register",
    },
    ToolCatalogEntry {
        name: "b00t_mcp_install",
        description: "Install an MCP server datum",
        subcommand: "mcp install",
    },
    ToolCatalogEntry {
        name: "b00t_mcp_output",
        description: "Show MCP server output/logs",
        subcommand: "mcp output",
    },
    ToolCatalogEntry {
        name: "b00t_cli_detect",
        description: "Detect installed CLI tool version",
        subcommand: "cli detect",
    },
    ToolCatalogEntry {
        name: "b00t_cli_desires",
        description: "Show desired CLI tool version",
        subcommand: "cli desires",
    },
    ToolCatalogEntry {
        name: "b00t_cli_check",
        description: "Check CLI tool vs desired version",
        subcommand: "cli check",
    },
    ToolCatalogEntry {
        name: "b00t_cli_install",
        description: "Install a CLI tool via datum",
        subcommand: "cli install",
    },
    ToolCatalogEntry {
        name: "b00t_cli_update",
        description: "Update a specific CLI tool",
        subcommand: "cli update",
    },
    ToolCatalogEntry {
        name: "b00t_cli_up",
        description: "Update all CLI tools",
        subcommand: "cli up",
    },
    ToolCatalogEntry {
        name: "b00t_upgrade",
        description: "Holistic upgrade: binary+MCP+hooks",
        subcommand: "upgrade",
    },
    ToolCatalogEntry {
        name: "b00t_lfmf",
        description: "Record a lesson from failure",
        subcommand: "lfmf",
    },
    ToolCatalogEntry {
        name: "b00t_advice",
        description: "Get advice for a tool or error",
        subcommand: "advice",
    },
    ToolCatalogEntry {
        name: "b00t_ai_list",
        description: "List configured AI providers",
        subcommand: "ai list",
    },
    ToolCatalogEntry {
        name: "b00t_ai_output",
        description: "Show AI provider output",
        subcommand: "ai output",
    },
    ToolCatalogEntry {
        name: "b00t_agent_discover",
        description: "Discover available agents",
        subcommand: "agent discover",
    },
    ToolCatalogEntry {
        name: "b00t_agent_capability",
        description: "Query agent capabilities",
        subcommand: "agent capability",
    },
    ToolCatalogEntry {
        name: "b00t_agent_delegate",
        description: "Delegate task to sub-agent",
        subcommand: "agent delegate",
    },
    ToolCatalogEntry {
        name: "b00t_agent_message",
        description: "Send ACP message to agent",
        subcommand: "agent message",
    },
    ToolCatalogEntry {
        name: "b00t_agent_wait",
        description: "Wait for agent to complete",
        subcommand: "agent wait",
    },
    ToolCatalogEntry {
        name: "b00t_agent_notify",
        description: "Broadcast ACP notification",
        subcommand: "agent notify",
    },
    ToolCatalogEntry {
        name: "b00t_agent_progress",
        description: "Report task progress",
        subcommand: "agent progress",
    },
    ToolCatalogEntry {
        name: "justfile_list",
        description: "List registered justfile modules and recipe counts",
        subcommand: "justfile list",
    },
    ToolCatalogEntry {
        name: "justfile_query",
        description: "Query registered recipes by justfile module",
        subcommand: "justfile query",
    },
    ToolCatalogEntry {
        name: "justfile_validate",
        description: "Validate a registered justfile through just AST",
        subcommand: "justfile validate",
    },
    ToolCatalogEntry {
        name: "justfile_registry",
        description: "Emit strict just-mcp --allow registry from justfile datums",
        subcommand: "justfile registry",
    },
    ToolCatalogEntry {
        name: "justfile_run",
        description: "Run a recipe from a registered justfile module",
        subcommand: "justfile run",
    },
    ToolCatalogEntry {
        name: "b00t_agent_complete",
        description: "Mark agent task complete",
        subcommand: "agent complete",
    },
    ToolCatalogEntry {
        name: "b00t_agent_vote_create",
        description: "Create a hive vote",
        subcommand: "agent vote create",
    },
    ToolCatalogEntry {
        name: "b00t_agent_vote_submit",
        description: "Submit a vote",
        subcommand: "agent vote submit",
    },
    ToolCatalogEntry {
        name: "b00t_agent_vote_tally",
        description: "Resolve a vote's outcome from the durable message log",
        subcommand: "agent vote tally",
    },
    ToolCatalogEntry {
        name: "b00t_session_init",
        description: "Initialize a b00t session",
        subcommand: "session init",
    },
    ToolCatalogEntry {
        name: "b00t_session_status",
        description: "Show session status",
        subcommand: "session status",
    },
    ToolCatalogEntry {
        name: "b00t_session_end",
        description: "End current session",
        subcommand: "session end",
    },
    ToolCatalogEntry {
        name: "b00t_checkpoint",
        description: "Create git checkpoint with tests",
        subcommand: "checkpoint",
    },
    ToolCatalogEntry {
        name: "b00t_grok_digest",
        description: "Ingest content into RAG knowledgebase",
        subcommand: "grok digest",
    },
    ToolCatalogEntry {
        name: "b00t_grok_ask",
        description: "Semantic search in knowledgebase",
        subcommand: "grok ask",
    },
    ToolCatalogEntry {
        name: "b00t_grok_learn",
        description: "Learn content into grok RAG",
        subcommand: "grok learn",
    },
    ToolCatalogEntry {
        name: "b00t_grok_status",
        description: "Check grok/RAG backend health",
        subcommand: "grok status",
    },
    ToolCatalogEntry {
        name: "b00t_task_list",
        description: "List all tasks",
        subcommand: "task list",
    },
    ToolCatalogEntry {
        name: "b00t_task_next",
        description: "Show next pending task",
        subcommand: "task next",
    },
    ToolCatalogEntry {
        name: "b00t_task_add",
        description: "Add a new task",
        subcommand: "task add",
    },
    ToolCatalogEntry {
        name: "b00t_task_done",
        description: "Mark task as done",
        subcommand: "task done",
    },
    ToolCatalogEntry {
        name: "b00t_task_update",
        description: "Update task status/title/notes",
        subcommand: "task update",
    },
    ToolCatalogEntry {
        name: "b00t_up",
        description: "Launch ralph agent REPL outer-loop",
        subcommand: "up",
    },
    ToolCatalogEntry {
        name: "b00t_ontology_query",
        description: "Query live capability ontology",
        subcommand: "ontology query",
    },
    ToolCatalogEntry {
        name: "b00t_skill_list",
        description: "List available skills",
        subcommand: "skill list",
    },
    ToolCatalogEntry {
        name: "b00t_skill_activate",
        description: "Activate a skill",
        subcommand: "skill activate",
    },
    ToolCatalogEntry {
        name: "b00t_app_vscode_mcp_install",
        description: "Install MCP in VSCode",
        subcommand: "app vscode mcp install",
    },
    ToolCatalogEntry {
        name: "b00t_app_claudecode_mcp_install",
        description: "Install MCP in Claude Code",
        subcommand: "app claudecode mcp install",
    },
    ToolCatalogEntry {
        name: "b00t_pipeline",
        description: "Manage pipeline lifecycle — list, run, validate, inspect, cost",
        subcommand: "pipeline",
    },
    ToolCatalogEntry {
        name: "b00t_just",
        description: "Run a registered just recipe through a typed recipe-and-args contract",
        subcommand: "just",
    },
    ToolCatalogEntry {
        name: "b00t_store_init",
        description: "Initialise the knowledge store",
        subcommand: "store init",
    },
    ToolCatalogEntry {
        name: "b00t_store_status",
        description: "Show store status (backend, objects, bytes)",
        subcommand: "store status",
    },
    ToolCatalogEntry {
        name: "b00t_store_validate",
        description: "Cross-engine consistency check",
        subcommand: "store validate",
    },
];

/// Search TOOL_CATALOG by keyword (case-insensitive substring match on name + description)
pub fn discover_tools(query: &str) -> Vec<&'static ToolCatalogEntry> {
    let q = query.to_lowercase();
    TOOL_CATALOG
        .iter()
        .filter(|e| {
            e.name.contains(&*q)
                || e.description.to_lowercase().contains(&*q)
                || e.subcommand.contains(&*q)
        })
        .collect()
}

// ── Surface Tool: b00t_exec ──────────────────────────────────────────────────
/// Execute ANY b00t-cli command by argv string.
/// Replaces 50+ individual MCP tools — use b00t_discover first to find the right command.
///
/// # Examples
/// - b00t_exec("task list") → lists tasks
/// - b00t_exec("cli check jq") → checks jq version
/// - b00t_exec("grok ask 'how to use just'") → RAG query
#[derive(Parser, Clone)]
pub struct BExecCommand {
    #[arg(help = "b00t-cli argv string, e.g. 'task list' or 'cli check jq'")]
    pub argv: String,
}
// 🤓 BExecCommand: custom McpReflection+McpExecutor — argv must be shell-split,
// not treated as a single positional string; macro would mangle it.
impl crate::clap_reflection::McpReflection for BExecCommand {
    fn mcp_tool_name() -> String {
        "b00t_exec".to_string()
    }
    fn command_path() -> Vec<String> {
        vec![]
    }
}
impl crate::clap_reflection::McpExecutor for BExecCommand {
    fn execute_mcp_call(
        params: &std::collections::HashMap<String, serde_json::Value>,
    ) -> anyhow::Result<String> {
        let argv = params
            .get("argv")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow::anyhow!("b00t_exec requires argv: string"))?;
        let parts: Vec<String> = shlex::split(argv).ok_or_else(|| {
            anyhow::anyhow!(
                "b00t_exec: invalid argv quoting (unterminated quote or trailing backslash) in: {argv}"
            )
        })?;
        if parts.is_empty() {
            anyhow::bail!("b00t_exec: argv must not be empty");
        }
        let output = std::process::Command::new("b00t-cli")
            .args(&parts)
            .output()
            .map_err(|e| anyhow::anyhow!("b00t-cli exec failed: {}", e))?;
        if output.status.success() {
            Ok(String::from_utf8_lossy(&output.stdout).to_string())
        } else {
            anyhow::bail!("{}", String::from_utf8_lossy(&output.stderr))
        }
    }
}

// ── Surface Tool: b00t_just ──────────────────────────────────────────────────
/// Execute a registered just recipe without routing through generic argv or a shell.
#[derive(Parser, Clone)]
pub struct BJustCommand {
    #[arg(help = "Registered just recipe name, e.g. 'game-play::test'")]
    pub just: String,
    #[arg(long, help = "Optional recipe arguments, passed without shell parsing")]
    pub args: Vec<String>,
}
impl crate::clap_reflection::McpReflection for BJustCommand {
    fn mcp_tool_name() -> String {
        "b00t_just".to_string()
    }
    fn command_path() -> Vec<String> {
        vec![]
    }
}
impl crate::clap_reflection::McpExecutor for BJustCommand {
    fn execute_mcp_call(
        params: &std::collections::HashMap<String, serde_json::Value>,
    ) -> anyhow::Result<String> {
        let recipe = params
            .get("just")
            .and_then(|value| value.as_str())
            .ok_or_else(|| anyhow::anyhow!("b00t_just requires just: string"))?;
        let args: Vec<&str> = params
            .get("args")
            .and_then(|value| value.as_array())
            .into_iter()
            .flatten()
            .map(|value| {
                value
                    .as_str()
                    .ok_or_else(|| anyhow::anyhow!("b00t_just args must be strings"))
            })
            .collect::<anyhow::Result<_>>()?;
        let output = std::process::Command::new("just")
            .arg(recipe)
            .args(args)
            .output()
            .map_err(|error| anyhow::anyhow!("just exec failed: {}", error))?;

        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);
        if output.status.success() {
            Ok(format!("{stdout}{stderr}"))
        } else {
            anyhow::bail!("{stdout}{stderr}")
        }
    }
}

// BVerifyCommand: verify a Z3/SMT2 assertion by shelling directly to `z3 -in`.
// 🤓 Was `b00t-cli admin verify --assertion ...` — that subcommand never
//    existed (no "admin" command in b00t-cli's Commands enum at all), so this
//    always failed and every caller (grammar-shape audit, verify tool loop)
//    silently got back {"result":"error"} regardless of the assertion.
//    Matches the working `z3 -in` piped-stdin pattern already used by
//    b00t-c0re-lib::z3_examples and b00t-datum-core::edl.
#[derive(Parser, Clone)]
pub struct BVerifyCommand {
    #[arg(help = "Z3 or formal assertion string")]
    pub assertion: String,
}
impl crate::clap_reflection::McpReflection for BVerifyCommand {
    fn mcp_tool_name() -> String {
        "b00t_verify".to_string()
    }
    fn command_path() -> Vec<String> {
        vec![]
    }
}
impl crate::clap_reflection::McpExecutor for BVerifyCommand {
    fn execute_mcp_call(
        params: &std::collections::HashMap<String, serde_json::Value>,
    ) -> anyhow::Result<String> {
        use std::io::Write;
        use std::process::{Command, Stdio};

        let assertion = params
            .get("assertion")
            .and_then(|v| v.as_str())
            .unwrap_or("true");
        let mut child = Command::new("z3")
            .arg("-in")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|e| anyhow::anyhow!("z3 not found in PATH: {e}"))?;
        child
            .stdin
            .as_mut()
            .expect("stdin was piped")
            .write_all(assertion.as_bytes())?;
        let output = child.wait_with_output()?;
        let stdout = String::from_utf8_lossy(&output.stdout).trim().to_lowercase();
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
        let result = if stdout == "unsat" {
            "unsat"
        } else if stdout == "sat" {
            "sat"
        } else {
            "unknown"
        };
        Ok(serde_json::json!({
            "result": result,
            "verified": result != "unknown",
            "raw_stdout": stdout,
            "raw_stderr": stderr,
        })
        .to_string())
    }
}

// BDiscoverCommand: custom executor — searches TOOL_CATALOG and returns JSON matches
impl crate::clap_reflection::McpReflection for BDiscoverCommand {
    fn mcp_tool_name() -> String {
        "b00t_discover".to_string()
    }
    fn command_path() -> Vec<String> {
        vec![]
    }
}
impl crate::clap_reflection::McpExecutor for BDiscoverCommand {
    fn execute_mcp_call(
        params: &std::collections::HashMap<String, serde_json::Value>,
    ) -> anyhow::Result<String> {
        let query = params.get("query").and_then(|v| v.as_str()).unwrap_or("");
        let limit = params.get("limit").and_then(|v| v.as_u64()).unwrap_or(8) as usize;
        let results: Vec<serde_json::Value> = discover_tools(query)
            .into_iter()
            .take(limit)
            .map(|e| {
                serde_json::json!({
                    "name": e.name,
                    "description": e.description,
                    "usage": format!("b00t_exec(\"{} <args>\")", e.subcommand),
                    "direct_cli": format!("b00t-cli {} <args>", e.subcommand),
                })
            })
            .collect();
        let out = serde_json::json!({
            "query": query,
            "count": results.len(),
            "tools": results,
            "tip": "Call b00t_exec(argv) with chosen subcommand, e.g. argv=\"task list\""
        });
        Ok(serde_json::to_string_pretty(&out)?)
    }
}

// ── Surface Tool: b00t_discover ──────────────────────────────────────────────
/// Discover b00t capabilities by keyword.
/// Returns matching tool names + descriptions from the catalog.
/// Then call b00t_exec to run the chosen command.
///
/// # Example workflow
/// 1. b00t_discover("install cli tool") → [{name:"b00t_cli_install", subcommand:"cli install"}]
/// 2. b00t_exec("cli install jq")
#[derive(Parser, Clone)]
pub struct BDiscoverCommand {
    #[arg(help = "Keyword to search (name, description, or subcommand)")]
    pub query: String,
    #[arg(long, help = "Max results to return", default_value = "8")]
    pub limit: Option<usize>,
}

/// Create SLIM surface registry — 6 core tools visible to agents.
/// Sub-agents call b00t_discover(query) to find tools, b00t_exec(argv) to run them.
/// Use create_full_mcp_registry() for debug/migration compatibility.
pub fn create_mcp_registry() -> McpCommandRegistry {
    create_mcp_registry_with_notify(std::sync::Arc::new(|| {}))
}

/// Same as create_mcp_registry but fires  after stack load/unload.
///
/// The notify closure is injected by B00tMcpServerRusty::new() with the peer handle
/// captured at connection time — this completes the dynamic tool-list-changed loop.
pub fn create_mcp_registry_with_notify(
    notify_fn: std::sync::Arc<dyn Fn() + Send + Sync>,
) -> McpCommandRegistry {
    let mut builder = McpCommandRegistry::builder();
    // Surface: learn + whoami + status + exec + discover + pipeline + viz + log + verify + stack_load + stack_unload + DataFramerr (22 tools)
    builder
        .register::<LearnCommand>()
        .register::<WhoamiCommand>()
        .register::<StatusCommand>()
        .register::<BExecCommand>()
        .register::<BJustCommand>()
        .register::<BDiscoverCommand>()
        .register::<BPipelineCommand>()
        .add_post_hook("b00t_mcp_stack_load", std::sync::Arc::clone(&notify_fn))
        .add_post_hook("b00t_mcp_stack_unload", notify_fn);
    crate::soul_dataframerr_tools::register_dataframerr_tools(&mut builder);
    builder.build()
}

/// Create FULL registry with all 56+ tools (debug / backward-compat only).
/// 🤓 DO NOT use this for sub-agents — context cost is too high.
pub fn create_full_mcp_registry() -> McpCommandRegistry {
    let mut builder = McpCommandRegistry::builder();

    // Register all MCP tools
    builder
        .register::<McpListCommand>()
        .register::<McpAddCommand>()
        .register::<McpInstallCommand>()
        .register::<McpOutputCommand>()
        .register::<CliDetectCommand>()
        .register::<CliDesiresCommand>()
        .register::<CliCheckCommand>()
        .register::<CliInstallCommand>()
        .register::<CliUpdateCommand>()
        .register::<CliUpCommand>()
        .register::<UpgradeCommand>()
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
        .register::<AgentVoteTallyCommand>()
        .register::<DelegateDatumCommand>()
        .register::<AgentWaitCommand>()
        .register::<AgentNotifyCommand>()
        .register::<AgentCapabilityCommand>()
        // Skill discovery tools
        .register::<SkillListCommand>()
        // Grok knowledgebase tools
        .register::<GrokDigestCommand>()
        .register::<GrokAskCommand>()
        .register::<GrokLearnCommand>()
        .register::<GrokStatusCommand>()
        .register::<GrokWanderCommand>()
        // Tutorial progression tools
        .register::<TutorialStatusCommand>()
        .register::<TutorialNextCommand>()
        // Ontology query tool
        .register::<OntologyQueryCommand>()
        // Up command — ralph REPL outer-loop
        .register::<UpCommand>()
        // b00t task — native task management (replaces taskmaster-ai)
        .register::<TaskListCommand>()
        .register::<TaskNextCommand>()
        .register::<TaskAddCommand>()
        .register::<TaskDoneCommand>()
        .register::<TaskUpdateCommand>()
        // Justfile capability discovery — wraps b00t-cli justfile registry/query
        .register::<JustfileListCommand>()
        .register::<JustfileQueryCommand>()
        .register::<JustfileValidateCommand>()
        .register::<JustfileRegistryCommand>()
        .register::<JustfileRunCommand>()
        // Pipeline lifecycle tool — GH #717/#736
        .register::<BPipelineCommand>();
    // ACP Hive coordination tools
    // 🤓 Disabled - acp_hive uses full NATS Agent from old ACP; chat refactor simplified to stubs
    // .register::<AcpHiveJoinCommand>()
    // .register::<AcpHiveCreateCommand>()
    // .register::<AcpHiveStatusCommand>()
    // .register::<AcpHiveProposeCommand>()
    // .register::<AcpHiveSyncCommand>()
    // .register::<AcpHiveReadyCommand>()
    // .register::<AcpHiveShowCommand>()
    // .register::<AcpHiveLeaveCommand>();
    crate::soul_dataframerr_tools::register_dataframerr_tools(&mut builder);

    builder.build()
}

// ┌──────────────────────────────────────────────────────────────────────────────┐
// │ Code Mode: search() + execute() consolidation (SDD-007)                  │
// └──────────────────────────────────────────────────────────────────────────────┘

/// Global full registry for Code Mode search/execute to dispatch against.
/// Contains all ~40 b00t tools. SearchCommand and ExecuteCommand use this
/// internally while only exposing two tools via MCP.
use std::sync::Mutex;

lazy_static::lazy_static! {
    static ref FULL_REGISTRY: Mutex<McpCommandRegistry> = Mutex::new(create_mcp_registry());
}

lazy_static::lazy_static! {
    static ref NATS_CLIENT: tokio::sync::Mutex<Option<b00t_chat::ChatClient>> = tokio::sync::Mutex::new(None);
}

// ┌──────────────────────────────────────────────────────────────────────────────┐
// │ b00t-council wiring: durable/observable player messages + real vote tally     │
// └──────────────────────────────────────────────────────────────────────────────┘

/// Durable message/vote log, shared by the agent_message/notify/vote MCP
/// tools. NATS delivery is unchanged (still fire-and-forget) — this adds the
/// durable record NATS never provided. Path overridable via
/// `B00T_MESSAGE_LOG_PATH` (used by tests to avoid writing into a real
/// `~/.local/share/b00t/messages.jsonl`).
fn message_sink() -> b00t_council::JsonlSink {
    match std::env::var_os("B00T_MESSAGE_LOG_PATH") {
        Some(path) => b00t_council::JsonlSink::at(path),
        None => b00t_council::JsonlSink::default_location(),
    }
}

/// Best-effort identity of whichever process is calling these MCP tools.
/// None of the agent_message/notify/vote tool params carry an explicit
/// sender id today, so this is the fallback used to attribute recorded
/// envelopes and to look up `sender_is_player`.
fn caller_agent_id() -> String {
    std::env::var("B00T_AGENT_ID")
        .or_else(|_| std::env::var("USER"))
        .unwrap_or_else(|_| "mcp-anonymous".to_string())
}

/// Record one [`b00t_council::Envelope`] to the durable log. Failures are
/// observability-only and never fail the caller's MCP tool invocation —
/// mirrors `b00t-ipc::MessageBus::send`'s non-blocking sink write.
fn record_message<T: serde::Serialize>(from: &str, to: b00t_council::Recipient, body: T) {
    let sender_is_player = b00t_cli::commands::crew_handler::is_player(from);
    let envelope = b00t_council::Envelope::new(from, to, sender_is_player, body);
    match envelope.to_value_envelope() {
        Ok(erased) => {
            if let Err(e) = message_sink().record(&erased) {
                eprintln!("b00t-mcp: message log record failed: {e:#}");
            }
        }
        Err(e) => eprintln!("b00t-mcp: message log serialize failed: {e:#}"),
    }
}

/// `vote_type` (from `AgentVoteCreateCommand`) -> `b00t_council::Quorum`.
/// `"veto_capable"` matches `b00t_c0re_lib::agent_coordination::VotingType::
/// VetoCapable`'s naming. Unrecognized/absent types fall back to
/// `AtLeast(2)`, matching `b00t-ipc::Proposal`'s original default.
fn quorum_for_vote_type(vote_type: &str) -> b00t_council::Quorum {
    match vote_type {
        "unanimous" => b00t_council::Quorum::Unanimous,
        "majority" | "single_choice" | "ranked_choice" => b00t_council::Quorum::Majority,
        "veto_capable" => b00t_council::Quorum::LiberumVeto,
        _ => b00t_council::Quorum::AtLeast(2),
    }
}

async fn get_nats_client() -> anyhow::Result<b00t_chat::ChatClient> {
    let mut guard = NATS_CLIENT.lock().await;
    if let Some(ref client) = *guard {
        return Ok(client.clone());
    }
    let client = b00t_chat::ChatClient::nats(None, None, None)
        .map_err(|e| anyhow::anyhow!("Failed to create NATS client: {}", e))?;
    *guard = Some(client.clone());
    Ok(client)
}

/// Search the b00t command registry
#[derive(Parser, Clone)]
pub struct SearchCommand {
    #[arg(help = "Search keyword across names, descriptions, tags")]
    pub query: String,

    #[arg(long, help = "Filter by category")]
    pub category: Option<String>,

    #[arg(long, default_value = "10", help = "Max results")]
    pub limit: Option<usize>,
}

impl McpReflection for SearchCommand {
    fn mcp_tool_name() -> String {
        "search".to_string()
    }

    fn command_path() -> Vec<String> {
        vec!["search".to_string()]
    }
}

impl McpExecutor for SearchCommand {
    fn execute_mcp_call(params: &HashMap<String, Value>) -> Result<String> {
        let query = params.get("query").and_then(|v| v.as_str()).unwrap_or("");
        let category = params.get("category").and_then(|v| v.as_str());
        let limit = params.get("limit").and_then(|v| v.as_u64()).unwrap_or(10) as usize;

        let registry = FULL_REGISTRY
            .lock()
            .map_err(|e| anyhow::anyhow!("Registry lock poisoned: {}", e))?;
        // Filter tools by query string (name/description) and optional category
        let all_tools = registry.get_tools();
        let query_lower = query.to_lowercase();
        let results: Vec<_> = all_tools
            .into_iter()
            .filter(|t| {
                let name_match = t.name.to_lowercase().contains(&query_lower);
                let desc_match = t
                    .description
                    .as_ref()
                    .map(|d| d.to_lowercase().contains(&query_lower))
                    .unwrap_or(false);
                if !(name_match || desc_match) {
                    return false;
                }
                if let Some(cat) = category {
                    // Category encoded as name prefix up to first underscore
                    let cat_prefix = t.name.split('_').next().unwrap_or("").to_lowercase();
                    cat_prefix == cat.to_lowercase()
                } else {
                    true
                }
            })
            .take(limit)
            .collect();

        let json_results: Vec<Value> = results
            .into_iter()
            .map(|r| {
                json!({
                    "name": r.name,
                    "description": r.description.as_deref().unwrap_or(""),
                    "category": r.name.split('_').next().unwrap_or(""),
                    "schema": null,
                })
            })
            .collect();

        let response = json!({
            "success": true,
            "query": query,
            "results": json_results,
            "total": json_results.len(),
        });

        Ok(response.to_string())
    }
}

/// MCP command for executing any b00t command by name
#[derive(Parser, Clone)]
pub struct ExecuteCommand {
    #[arg(help = "Exact command name from b00t_search")]
    pub command: String,

    #[arg(help = "Command parameters as a JSON object string")]
    pub params: Option<String>,
}

impl McpReflection for ExecuteCommand {
    fn mcp_tool_name() -> String {
        "execute".to_string()
    }

    fn command_path() -> Vec<String> {
        vec!["execute".to_string()]
    }

    /// Override schema to advertise `params` as an object, not a string
    fn generate_json_schema() -> Map<String, Value> {
        let mut schema = Map::new();
        let mut properties = Map::new();

        schema.insert("type".to_string(), json!("object"));

        let mut cmd_schema = Map::new();
        cmd_schema.insert("type".to_string(), json!("string"));
        cmd_schema.insert(
            "description".to_string(),
            json!("Exact command name from b00t_search (e.g., 'b00t_grok_ask')"),
        );
        properties.insert("command".to_string(), Value::Object(cmd_schema));

        let mut params_schema = Map::new();
        params_schema.insert("type".to_string(), json!("object"));
        params_schema.insert(
            "description".to_string(),
            json!("Command parameters as a JSON object matching the command's schema"),
        );
        properties.insert("params".to_string(), Value::Object(params_schema));

        schema.insert("properties".to_string(), Value::Object(properties));
        schema.insert("required".to_string(), json!(["command"]));

        schema
    }
}

impl McpExecutor for ExecuteCommand {
    fn execute_mcp_call(params: &HashMap<String, Value>) -> Result<String> {
        let command_name = params
            .get("command")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow::anyhow!("Missing required 'command' parameter"))?;

        // Parse the params value: may be a JSON object or a JSON string
        let inner_params: HashMap<String, Value> = match params.get("params") {
            Some(Value::Object(obj)) => obj.clone().into_iter().collect(),
            Some(Value::String(s)) => serde_json::from_str(s)
                .map_err(|e| anyhow::anyhow!("Failed to parse params JSON string: {}", e))?,
            Some(other) => {
                return Err(anyhow::anyhow!(
                    "params must be a JSON object or JSON string, got: {}",
                    other
                ));
            }
            None => HashMap::new(),
        };

        let registry = FULL_REGISTRY
            .lock()
            .map_err(|e| anyhow::anyhow!("Registry lock poisoned: {}", e))?;

        match registry.execute(command_name, &inner_params) {
            Ok(output) => Ok(output),
            Err(e) => {
                // Generate Levenshtein-like suggestions from available tool names
                let available: Vec<String> = registry
                    .get_tools()
                    .into_iter()
                    .map(|t| t.name.as_ref().to_string())
                    .filter(|n| {
                        // Simple substring similarity
                        let lower_cmd = command_name.to_lowercase();
                        let lower_name = n.to_lowercase();
                        lower_name.contains(&lower_cmd) || lower_cmd.contains(&lower_name)
                    })
                    .take(3)
                    .collect();

                let err_response = json!({
                    "success": false,
                    "error": {
                        "type": "unknown_command",
                        "message": format!("Command '{}' not found: {}", command_name, e),
                    },
                    "suggestions": available,
                });

                Ok(err_response.to_string())
            }
        }
    }
}

/// Create a Code Mode registry containing only search() + execute()
pub fn create_code_mode_registry() -> McpCommandRegistry {
    let mut builder = McpCommandRegistry::builder();
    builder
        .register::<B00tTimerCommand>()
        .register::<SearchCommand>()
        .register::<ExecuteCommand>();
    builder.build()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::clap_reflection::McpExecutor;
    use std::collections::HashMap;

    #[test]
    fn test_registry_creation() {
        // Slim surface registry: 6 core + DataFramerr tools
        let surface = create_mcp_registry();
        let surface_tools = surface.get_tools();
        assert!(!surface_tools.is_empty());
        let surface_names: Vec<&str> = surface_tools.iter().map(|t| t.name.as_ref()).collect();
        assert!(
            surface_names.contains(&"b00t_whoami"),
            "whoami must be in surface"
        );
        assert!(
            surface_names.contains(&"b00t_status"),
            "status must be in surface"
        );
        assert!(
            surface_names.contains(&"b00t_learn"),
            "learn must be in surface"
        );
        assert!(
            surface_names.contains(&"b00t_exec"),
            "exec must be in surface"
        );
        assert!(
            surface_names.contains(&"b00t_just"),
            "typed just execution must be in surface"
        );
        assert!(
            surface_names.contains(&"b00t_discover"),
            "discover must be in surface"
        );
        assert!(
            surface_names.contains(&"b00t_pipeline"),
            "pipeline must be in surface"
        );
        assert!(
            surface_tools.len() >= 6,
            "surface registry must have at least 6 tools"
        );

        // Full registry: all tools for debug/migration
        let full = create_full_mcp_registry();
        let full_tools = full.get_tools();
        let full_names: Vec<&str> = full_tools.iter().map(|t| t.name.as_ref()).collect();
        assert!(full_names.contains(&"b00t_mcp_list"));
        assert!(full_names.contains(&"b00t_cli_detect"));
    }

    #[test]
    fn test_just_tool_schema_uses_just_parameter() {
        let tool = BJustCommand::to_mcp_tool();
        let properties = tool.input_schema["properties"].as_object().unwrap();

        assert!(properties.contains_key("just"));
        assert!(!properties.contains_key("argv"));
    }

    #[test]
    fn test_tool_schema_generation() {
        let tool = McpListCommand::to_mcp_tool();
        // #827 — was "mcp_list" (unprefixed), contradicting test_registry_creation's
        // "b00t_mcp_list" expectation; the documented, external tool-name contract
        // (b00t_quick_reference.md, b00t-mcp-cloudflare/README.md) uses the prefix.
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

    #[test]
    fn test_grok_digest_content_is_positional() {
        // Regression test for #339/#330: content must be emitted positionally, not as --content
        let mut params = HashMap::new();
        params.insert("topic".to_string(), serde_json::json!("rust"));
        params.insert(
            "content".to_string(),
            serde_json::json!("ownership prevents data races"),
        );

        let args = GrokDigestCommand::params_to_args(&params);
        // content must appear as a positional value
        assert!(
            args.contains(&"ownership prevents data races".to_string()),
            "content should be positional"
        );
        // must NOT appear as a flag
        assert!(
            !args.contains(&"--content".to_string()),
            "--content flag must not be emitted"
        );
        // topic is a named flag
        assert!(
            args.contains(&"--topic".to_string()),
            "--topic flag must be present"
        );
    }

    #[test]
    fn test_grok_ask_query_is_positional() {
        // Regression test for #339/#330: query must be emitted positionally, not as --query
        let mut params = HashMap::new();
        params.insert(
            "query".to_string(),
            serde_json::json!("memory safety patterns"),
        );

        let args = GrokAskCommand::params_to_args(&params);
        // query must appear as a positional value
        assert!(
            args.contains(&"memory safety patterns".to_string()),
            "query should be positional"
        );
        // must NOT appear as a flag
        assert!(
            !args.contains(&"--query".to_string()),
            "--query flag must not be emitted"
        );
    }

    #[test]
    fn test_grok_learn_content_is_positional() {
        // Regression test for #339/#330: content must be emitted positionally, not as --content
        let mut params = HashMap::new();
        params.insert(
            "content".to_string(),
            serde_json::json!("Rust is a systems language"),
        );
        params.insert("topic".to_string(), serde_json::json!("rust"));

        let args = GrokLearnCommand::params_to_args(&params);
        // content must appear as a positional value
        assert!(
            args.contains(&"Rust is a systems language".to_string()),
            "content should be positional"
        );
        // must NOT appear as a flag
        assert!(
            !args.contains(&"--content".to_string()),
            "--content flag must not be emitted"
        );
    }

    #[test]
    fn test_pipeline_tool_registered() {
        use crate::clap_reflection::McpReflection;
        use crate::tools::pipeline::BPipelineCommand;

        let tool = BPipelineCommand::to_mcp_tool();
        assert_eq!(tool.name.as_ref(), "b00t_pipeline");
        assert_eq!(
            tool.description.as_deref().unwrap_or(""),
            "Manage pipeline lifecycle — create, validate, execute, inspect"
        );

        // Schema must contain action (required) + pipeline/params (optional)
        let schema = tool.input_schema.as_ref();
        let props = schema["properties"].as_object().unwrap();
        assert!(props.contains_key("action"), "schema must have 'action'");
        assert!(
            props.contains_key("pipeline"),
            "schema must have 'pipeline'"
        );
        assert!(props.contains_key("params"), "schema must have 'params'");
        assert!(
            schema["required"]
                .as_array()
                .unwrap()
                .contains(&serde_json::json!("action")),
            "action must be required"
        );
    }

    #[test]
    fn test_pipeline_tool_in_catalog() {
        let has_entry = TOOL_CATALOG.iter().any(|e| e.name == "b00t_pipeline");
        assert!(has_entry, "b00t_pipeline must be in TOOL_CATALOG");
    }

    #[test]
    fn test_justfile_tools_registered_and_discoverable() {
        let full = create_full_mcp_registry();
        let tools = full.get_tools();
        let names: Vec<&str> = tools.iter().map(|t| t.name.as_ref()).collect();
        assert!(names.contains(&"justfile_list"));
        assert!(names.contains(&"justfile_query"));
        assert!(names.contains(&"justfile_validate"));
        assert!(names.contains(&"justfile_registry"));
        assert!(names.contains(&"justfile_run"));

        assert!(
            TOOL_CATALOG.iter().any(|e| e.name == "justfile_query"),
            "justfile_query must be discoverable through b00t_discover"
        );
        assert!(
            TOOL_CATALOG.iter().any(|e| e.name == "justfile_run"),
            "justfile_run must be discoverable through b00t_discover"
        );
    }

    #[test]
    fn test_bexec_shlex_tokenizes_quoted_multiword_arg() {
        let argv = r#"task add "some multi word description""#;
        let parts = shlex::split(argv).expect("valid shell quoting must tokenize");
        assert_eq!(parts, vec!["task", "add", "some multi word description"]);
    }

    #[test]
    fn test_bexec_shlex_rejects_unterminated_quote() {
        let argv = r#"task add "unterminated"#;
        assert!(
            shlex::split(argv).is_none(),
            "unterminated quote must fail tokenization, not panic"
        );
    }

    #[test]
    fn test_store_subcommands_discoverable() {
        // #708: store subcommands (init/status/validate) must be discoverable
        // via b00t_discover("store") — the CLI commands already exist and are
        // reachable through b00t_exec, but were missing from TOOL_CATALOG.
        let matches = discover_tools("store");
        assert!(
            matches.len() >= 3,
            "expected at least 3 store entries in discover_tools(\"store\"), got {}",
            matches.len()
        );
        for expected in ["b00t_store_init", "b00t_store_status", "b00t_store_validate"] {
            assert!(
                matches.iter().any(|e| e.name == expected),
                "discover_tools(\"store\") missing entry {}",
                expected
            );
        }
    }

    #[test]
    fn test_grok_rag_defaults_match_cli() {
        // Verify the MCP schema advertises 'both' as the default rag backend,
        // matching b00t-cli grok digest/ask/learn (not the legacy 'raglight' or 'qdrant').
        let digest_tool = GrokDigestCommand::to_mcp_tool();
        let ask_tool = GrokAskCommand::to_mcp_tool();
        let learn_tool = GrokLearnCommand::to_mcp_tool();

        for tool in [&digest_tool, &ask_tool, &learn_tool] {
            let props = tool.input_schema["properties"].as_object().unwrap();
            if let Some(rag_prop) = props.get("rag") {
                let description = rag_prop["description"].as_str().unwrap_or("");
                assert!(
                    description.contains("raglite"),
                    "rag description should mention 'raglite' for tool {}",
                    tool.name
                );
                assert!(
                    description.contains("irontology"),
                    "rag description should mention 'irontology' for tool {}",
                    tool.name
                );
                assert!(
                    description.contains("both"),
                    "rag description should mention 'both' for tool {}",
                    tool.name
                );
                assert!(
                    !description.contains("qdrant"),
                    "rag description must not mention 'qdrant' for tool {}",
                    tool.name
                );
                assert!(
                    !description.contains("raglight"),
                    "rag description must not mention legacy 'raglight' for tool {}",
                    tool.name
                );
            }
        }
    }
}
