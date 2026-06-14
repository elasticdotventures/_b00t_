//! Agent coordination commands for b00t-cli.
//!
//! Implements all MCP agent coordination commands using the b00t-c0re-lib
//! agent coordination infrastructure.

use anyhow::Result;
use b00t_c0re_lib::AgentManager;
use b00t_c0re_lib::agent_coordination::{
    AgentCoordinator, AgentMetadata, MessageFilter, RequestUrgency, TaskCompletionStatus,
    TaskPriority,
};
use b00t_c0re_lib::redis::{AgentStatus, RedisComms, RedisConfig};
use clap::Parser;
use std::collections::{BTreeMap, HashMap};
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

/// Agent management and coordination commands
#[derive(Parser, Clone)]
pub enum AgentCommands {
    #[clap(about = "Discover agents on the network")]
    Discover {
        #[arg(long, help = "Filter by agent role")]
        role: Option<String>,

        #[arg(long, help = "Filter by crew membership")]
        crew: Option<String>,

        #[arg(long, help = "Required capabilities (comma-separated)")]
        capabilities: Option<String>,

        #[arg(long, help = "Output in JSON format")]
        json: bool,
    },

    #[clap(about = "Show agent quota/status/infraction dashboard in TOON format")]
    Dashboard {
        #[arg(long, help = "Output in JSON format instead of TOON")]
        json: bool,

        #[arg(long, help = "Maximum rows to show", default_value_t = 20)]
        limit: usize,
    },

    #[clap(about = "Send a direct message to an agent")]
    Message {
        #[arg(help = "Target agent ID")]
        to_agent: String,

        #[arg(help = "Message subject")]
        subject: String,

        #[arg(help = "Message content")]
        content: String,

        #[arg(long, help = "Require acknowledgment")]
        ack: bool,
    },

    #[clap(about = "Delegate a task to a worker agent")]
    Delegate {
        #[arg(help = "Worker agent ID")]
        worker: String,

        #[arg(help = "Task ID")]
        task_id: String,

        #[arg(help = "Task description")]
        description: String,

        #[arg(
            long,
            help = "Priority level (low, normal, high, critical)",
            default_value = "normal"
        )]
        priority: String,

        #[arg(long, help = "Deadline in minutes")]
        deadline: Option<u64>,

        #[arg(long, help = "Required capabilities (comma-separated)")]
        capabilities: Option<String>,

        #[arg(long, help = "Block until completion")]
        blocking: bool,

        #[arg(
            long,
            help = "Inject skill instructions into task context (skill name)"
        )]
        skill: Option<String>,

        #[arg(
            long,
            help = "Inject role constraints into task context (role datum name)"
        )]
        role: Option<String>,

        #[arg(
            long,
            help = "Expected output contract enforced at completion (e.g. 'PASS|FAIL:<5lines>')"
        )]
        output_contract: Option<String>,
    },

    #[clap(about = "Report task completion")]
    Complete {
        #[arg(help = "Captain agent ID")]
        captain: String,

        #[arg(help = "Task ID")]
        task_id: String,

        #[arg(long, help = "Completion status", default_value = "success")]
        status: String,

        #[arg(long, help = "Result description")]
        result: Option<String>,

        #[arg(long, help = "Output artifacts (comma-separated paths)")]
        artifacts: Option<String>,
    },

    #[clap(about = "Report task progress")]
    Progress {
        #[arg(help = "Task ID")]
        task_id: String,

        #[arg(help = "Progress percentage (0-100)")]
        progress: f32,

        #[arg(help = "Status message")]
        message: String,

        #[arg(long, help = "Estimated completion in minutes")]
        eta: Option<u64>,
    },

    #[clap(about = "Start an agent from config file")]
    Start {
        #[arg(help = "Path to .agent.toml config file")]
        config: PathBuf,
    },

    #[clap(about = "Start all agents in a directory")]
    StartAll {
        #[arg(
            help = "Directory containing .agent.toml files",
            default_value = "_b00t_"
        )]
        dir: PathBuf,
    },

    #[clap(about = "Report this agent's capabilities and request capable agents for a task")]
    Capability {
        #[arg(help = "Required capabilities (comma-separated)")]
        capabilities: String,

        #[arg(help = "Task description")]
        description: String,

        #[arg(
            long,
            help = "Request urgency (low, normal, high, emergency)",
            default_value = "normal"
        )]
        urgency: String,
    },

    #[clap(about = "Send a notification event to the b00t IPC channel")]
    Notify {
        #[arg(help = "Event type (e.g., 'file_created', 'pr_opened')")]
        event_type: String,

        #[arg(help = "Event source")]
        source: String,

        #[arg(help = "Event details (JSON)")]
        details: String,

        #[arg(long, help = "Target specific agents (comma-separated)")]
        agents: Option<String>,
    },

    #[clap(about = "Wait for an agent message or event")]
    Wait {
        #[arg(long, help = "Timeout in seconds", default_value = "30")]
        timeout: u64,

        #[arg(long, help = "Filter by message type")]
        message_type: Option<String>,

        #[arg(long, help = "Filter by sender agent")]
        from_agent: Option<String>,

        #[arg(long, help = "Filter by task ID")]
        task_id: Option<String>,

        #[arg(long, help = "Filter by subject")]
        subject: Option<String>,
    },

    #[clap(about = "Invoke an agent executor directly (deterministic tool-call loop, no Redis)")]
    Invoke {
        #[arg(help = "Agent name matching a _b00t_/<name>.agent.toml")]
        agent: String,

        #[arg(help = "Prompt to send to the agent")]
        prompt: String,

        #[arg(long, help = "Path to agent TOML (overrides auto-discovery)")]
        config: Option<PathBuf>,
    },

    #[clap(about = "Run ralph autonomous agent for hive maintenance/validation")]
    Ralph {
        #[arg(
            long,
            help = "Executor tool (codex, claude, amp, opencode, mistralrs, pi)",
            default_value = "codex"
        )]
        tool: String,

        #[arg(
            long,
            help = "Task filter (pending, hive-validate, maintenance)",
            default_value = "hive-validate"
        )]
        task: String,

        #[arg(long, help = "Maximum iterations", default_value = "5")]
        max_iterations: u32,

        #[arg(long, help = "Project root path")]
        project_root: Option<PathBuf>,
    },
}

pub async fn handle_agent_command(cmd: AgentCommands) -> Result<()> {
    match cmd {
        AgentCommands::Discover {
            role,
            crew,
            capabilities,
            json,
        } => handle_discover(role, crew, capabilities, json).await,

        AgentCommands::Dashboard { json, limit } => handle_dashboard(json, limit),

        AgentCommands::Message {
            to_agent,
            subject,
            content,
            ack,
        } => handle_message(&to_agent, &subject, &content, ack).await,

        AgentCommands::Delegate {
            worker,
            task_id,
            description,
            priority,
            deadline,
            capabilities,
            blocking,
            skill,
            role,
            output_contract,
        } => {
            handle_delegate(
                &worker,
                &task_id,
                &description,
                &priority,
                deadline,
                capabilities,
                blocking,
                skill.as_deref(),
                role.as_deref(),
                output_contract.as_deref(),
            )
            .await
        }

        AgentCommands::Complete {
            captain,
            task_id,
            status,
            result,
            artifacts,
        } => handle_complete(&captain, &task_id, &status, result, artifacts).await,

        AgentCommands::Progress {
            task_id,
            progress,
            message,
            eta,
        } => handle_progress(&task_id, progress, &message, eta).await,

        AgentCommands::Capability {
            capabilities,
            description,
            urgency,
        } => handle_capability(&capabilities, &description, &urgency).await,

        AgentCommands::Notify {
            event_type,
            source,
            details,
            agents,
        } => handle_notify(&event_type, &source, &details, agents).await,

        AgentCommands::Wait {
            timeout,
            message_type,
            from_agent,
            task_id,
            subject,
        } => handle_wait(timeout, message_type, from_agent, task_id, subject).await,

        AgentCommands::Start { config } => handle_start(&config).await,

        AgentCommands::StartAll { dir } => handle_start_all(&dir).await,

        AgentCommands::Invoke {
            agent,
            prompt,
            config,
        } => handle_invoke(&agent, &prompt, config.as_deref()).await,

        AgentCommands::Ralph {
            tool,
            task,
            max_iterations,
            project_root,
        } => handle_ralph(&tool, &task, max_iterations, project_root.as_deref()).await,
    }
}

#[derive(Debug, Clone, serde::Serialize)]
struct AgentDashboardRow {
    agent: String,
    kind: String,
    role: String,
    model: String,
    status: String,
    quota_daily: String,
    used_today: u32,
    remaining: String,
    last_used: String,
    infractions: u32,
    score: i32,
    evidence: String,
}

#[derive(Debug, Default, serde::Serialize)]
struct AgentDashboardReport {
    rows: Vec<AgentDashboardRow>,
    global_infractions: u32,
    malformed_agent_datums: u32,
    malformed_exec_log_lines: u32,
    malformed_guard_lines: u32,
}

#[derive(Debug, Clone)]
struct AgentDatumSummary {
    name: String,
    kind: String,
    role: String,
    model: String,
    socket: Option<String>,
    quota_daily: Option<f64>,
    skills_count: usize,
}

#[derive(Debug, Clone, Default)]
struct AgentUsageStats {
    used_today: u32,
    last_used: Option<String>,
}

#[derive(Debug, Clone, Default)]
struct GuardInfractionStats {
    global_infractions: u32,
    by_agent: BTreeMap<String, u32>,
    malformed_lines: u32,
}

fn handle_dashboard(json: bool, limit: usize) -> Result<()> {
    let report = build_agent_dashboard(limit)?;
    if json {
        println!("{}", serde_json::to_string_pretty(&report)?);
    } else {
        print_agent_dashboard_toon(&report);
    }
    Ok(())
}

fn build_agent_dashboard(limit: usize) -> Result<AgentDashboardReport> {
    let agent_dir = resolve_agent_datum_dir()?;
    let (agents, malformed_agent_datums) = load_agent_datum_summaries(&agent_dir);
    let exec_log = home_b00t_path("exec-log.jsonl");
    let guard_log = home_b00t_path("guard-violations.jsonl");
    let (usage, malformed_exec_log_lines) = load_agent_usage_stats(&exec_log, &agents);
    let guard_stats = load_guard_infraction_stats(&guard_log, &agents);

    let today = chrono::Utc::now().date_naive().to_string();
    let mut rows: Vec<AgentDashboardRow> = agents
        .iter()
        .map(|agent| {
            let agent_usage = usage.get(&agent.name).cloned().unwrap_or_default();
            let infractions = guard_stats.by_agent.get(&agent.name).copied().unwrap_or(0);
            let quota_daily = agent
                .quota_daily
                .map(format_quota)
                .unwrap_or_else(|| "untracked".to_string());
            let remaining = agent
                .quota_daily
                .map(|quota| format_quota((quota - f64::from(agent_usage.used_today)).max(0.0)))
                .unwrap_or_else(|| "untracked".to_string());
            let status = agent_status(agent);
            let mut score = 100 - (infractions as i32 * 10);
            if status == "offline" {
                score -= 5;
            }
            let evidence = format!(
                "kind:{};skills:{};socket:{};date:{}",
                agent.kind,
                agent.skills_count,
                agent.socket.as_deref().unwrap_or("none"),
                today
            );
            AgentDashboardRow {
                agent: agent.name.clone(),
                kind: agent.kind.clone(),
                role: agent.role.clone(),
                model: agent.model.clone(),
                status,
                quota_daily,
                used_today: agent_usage.used_today,
                remaining,
                last_used: agent_usage.last_used.unwrap_or_else(|| "never".to_string()),
                infractions,
                score: score.max(0),
                evidence,
            }
        })
        .collect();

    rows.sort_by(|a, b| b.score.cmp(&a.score).then_with(|| a.agent.cmp(&b.agent)));
    rows.truncate(limit);

    Ok(AgentDashboardReport {
        rows,
        global_infractions: guard_stats.global_infractions,
        malformed_agent_datums,
        malformed_exec_log_lines,
        malformed_guard_lines: guard_stats.malformed_lines,
    })
}

fn resolve_agent_datum_dir() -> Result<PathBuf> {
    let cwd_dir = std::env::current_dir()?.join("_b00t_");
    if cwd_dir.is_dir() {
        return Ok(cwd_dir);
    }
    Ok(dirs::home_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join(".b00t")
        .join("_b00t_"))
}

fn home_b00t_path(file: &str) -> PathBuf {
    dirs::home_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join(".b00t")
        .join(file)
}

fn find_agent_config_path(agent_dir: &Path, agent: &str) -> Option<PathBuf> {
    let preferred = [
        format!("{agent}.agent.toml"),
        format!("{agent}.agent.cli.toml"),
        format!("{agent}.agent.sdk.toml"),
        format!("{agent}.agent.ide.vsix.toml"),
        format!("{agent}.agent.gui.toml"),
        format!("{agent}.agent.tomllm"),
        format!("{agent}.agent.cli.tomllm"),
        format!("{agent}.agent.sdk.tomllm"),
        format!("{agent}.agent.ide.vsix.tomllm"),
        format!("{agent}.agent.gui.tomllm"),
        format!("{agent}.agent.tomllmd"),
        format!("{agent}.agent.cli.tomllmd"),
        format!("{agent}.agent.sdk.tomllmd"),
        format!("{agent}.agent.ide.vsix.tomllmd"),
        format!("{agent}.agent.gui.tomllmd"),
    ];
    preferred
        .iter()
        .map(|name| agent_dir.join(name))
        .find(|path| path.exists())
}

fn load_agent_datum_summaries(agent_dir: &Path) -> (Vec<AgentDatumSummary>, u32) {
    let mut agents = Vec::new();
    let mut malformed = 0;
    let entries = match std::fs::read_dir(agent_dir) {
        Ok(entries) => entries,
        Err(_) => return (agents, 0),
    };

    for entry in entries.flatten() {
        let path = entry.path();
        let Some(file_name) = path.file_name().and_then(|n| n.to_str()) else {
            continue;
        };
        if !file_name.contains(".agent.") || file_name.starts_with("++abstract") {
            continue;
        }
        let Ok(content) = std::fs::read_to_string(&path) else {
            malformed += 1;
            continue;
        };
        let Ok(value) = toml::from_str::<toml::Value>(&content) else {
            malformed += 1;
            continue;
        };
        if !is_agent_datum(&value) {
            continue;
        }
        if let Some(agent) = agent_summary_from_value(&value, file_name) {
            agents.push(agent);
        } else {
            malformed += 1;
        }
    }
    agents.sort_by(|a, b| a.name.cmp(&b.name));
    (agents, malformed)
}

fn is_agent_datum(value: &toml::Value) -> bool {
    value
        .get("b00t")
        .and_then(|b| b.get("type"))
        .and_then(|v| v.as_str())
        .is_some_and(|t| t == "agent" || t.starts_with("agent."))
}

fn agent_summary_from_value(
    value: &toml::Value,
    fallback_file_name: &str,
) -> Option<AgentDatumSummary> {
    let b00t = value.get("b00t")?;
    let agent = b00t.get("agent");
    let name = b00t
        .get("name")
        .and_then(|v| v.as_str())
        .map(str::to_string)
        .unwrap_or_else(|| {
            fallback_file_name
                .split('.')
                .next()
                .unwrap_or("agent")
                .to_string()
        });
    if name == "agent-name" {
        return None;
    }
    let kind = b00t
        .get("type")
        .and_then(|v| v.as_str())
        .map(normalize_agent_kind)
        .unwrap_or_else(|| "agent".to_string());
    let role = agent
        .and_then(|a| a.get("role"))
        .or_else(|| {
            agent
                .and_then(|a| a.get("crew"))
                .and_then(|c| c.get("role"))
        })
        .and_then(|v| v.as_str())
        .unwrap_or("specialist")
        .to_string();
    let model = agent
        .and_then(|a| a.get("model"))
        .and_then(|v| v.as_str())
        .unwrap_or("unknown")
        .to_string();
    let socket = agent
        .and_then(|a| a.get("ipc"))
        .and_then(|ipc| ipc.get("socket"))
        .and_then(|v| v.as_str())
        .filter(|s| !s.is_empty())
        .map(str::to_string);
    let quota_daily = agent
        .and_then(|a| a.get("quota"))
        .and_then(|q| q.get("daily"))
        .and_then(toml_number_to_f64)
        .or_else(|| {
            agent
                .and_then(|a| a.get("daily_limit"))
                .and_then(toml_number_to_f64)
        })
        .or_else(|| b00t.get("daily_limit").and_then(toml_number_to_f64));
    let skills_count = agent
        .and_then(|a| a.get("skills"))
        .and_then(|v| v.as_array())
        .map(|skills| skills.len())
        .unwrap_or(0);

    Some(AgentDatumSummary {
        name,
        kind,
        role,
        model,
        socket,
        quota_daily,
        skills_count,
    })
}

fn normalize_agent_kind(kind: &str) -> String {
    match kind {
        "agent.cli" | "agent.sdk" | "agent.ide.vsix" | "agent.gui" => kind.to_string(),
        "agent" => "agent".to_string(),
        other if other.starts_with("agent.") => other.to_string(),
        _ => "agent".to_string(),
    }
}

fn toml_number_to_f64(value: &toml::Value) -> Option<f64> {
    value
        .as_float()
        .or_else(|| value.as_integer().map(|n| n as f64))
}

fn agent_status(agent: &AgentDatumSummary) -> String {
    match agent.socket.as_deref() {
        Some(socket) if Path::new(socket).exists() => "online".to_string(),
        Some(_) => "offline".to_string(),
        None => "configured".to_string(),
    }
}

fn load_agent_usage_stats(
    exec_log_path: &Path,
    agents: &[AgentDatumSummary],
) -> (BTreeMap<String, AgentUsageStats>, u32) {
    let mut stats = BTreeMap::new();
    let mut malformed = 0;
    let Ok(content) = std::fs::read_to_string(exec_log_path) else {
        return (stats, 0);
    };
    let today = chrono::Utc::now().date_naive().to_string();
    for line in content.lines() {
        let Ok(value) = serde_json::from_str::<serde_json::Value>(line) else {
            malformed += 1;
            continue;
        };
        let cmd = value.get("cmd").and_then(|v| v.as_str()).unwrap_or("");
        let ts = value.get("ts").and_then(|v| v.as_str()).unwrap_or("");
        for agent in agents {
            if command_mentions_agent(cmd, &agent.name) {
                let entry = stats
                    .entry(agent.name.clone())
                    .or_insert_with(AgentUsageStats::default);
                if ts.starts_with(&today) {
                    entry.used_today += 1;
                }
                if entry
                    .last_used
                    .as_deref()
                    .map(|last| ts > last)
                    .unwrap_or(true)
                {
                    entry.last_used = Some(ts.to_string());
                }
            }
        }
    }
    (stats, malformed)
}

fn command_mentions_agent(cmd: &str, agent: &str) -> bool {
    cmd.split(|c: char| !c.is_ascii_alphanumeric() && c != '-' && c != '_')
        .any(|token| token == agent || token == format!("{agent}-agent"))
}

fn load_guard_infraction_stats(path: &Path, agents: &[AgentDatumSummary]) -> GuardInfractionStats {
    let mut stats = GuardInfractionStats::default();
    let Ok(content) = std::fs::read_to_string(path) else {
        return stats;
    };
    for line in content.lines() {
        let Ok(value) = serde_json::from_str::<serde_json::Value>(line) else {
            stats.malformed_lines += 1;
            continue;
        };
        stats.global_infractions += 1;
        let haystack = value.to_string();
        for agent in agents {
            if haystack.contains(&agent.name) {
                *stats.by_agent.entry(agent.name.clone()).or_insert(0) += 1;
            }
        }
    }
    stats
}

fn format_quota(value: f64) -> String {
    if (value.fract()).abs() < f64::EPSILON {
        format!("{}", value as i64)
    } else {
        format!("{value:.2}")
    }
}

fn print_agent_dashboard_toon(report: &AgentDashboardReport) {
    println!(
        "agent_dashboard[{}]{{agent,kind,role,model,status,quota_daily,used_today,remaining,last_used,infractions,score,evidence}}:",
        report.rows.len()
    );
    for row in &report.rows {
        println!(
            "{},{},{},{},{},{},{},{},{},{},{},{}",
            toon_cell(&row.agent),
            toon_cell(&row.kind),
            toon_cell(&row.role),
            toon_cell(&row.model),
            toon_cell(&row.status),
            toon_cell(&row.quota_daily),
            row.used_today,
            toon_cell(&row.remaining),
            toon_cell(&row.last_used),
            row.infractions,
            row.score,
            toon_cell(&row.evidence),
        );
    }
    println!(
        "dashboard_evidence{{global_infractions,malformed_agent_datums,malformed_exec_log_lines,malformed_guard_lines}}: {},{},{},{}",
        report.global_infractions,
        report.malformed_agent_datums,
        report.malformed_exec_log_lines,
        report.malformed_guard_lines,
    );
}

fn toon_cell(input: &str) -> String {
    input
        .replace(['\n', '\r', '\t'], " ")
        .replace(',', ";")
        .trim()
        .to_string()
}

async fn handle_discover(
    role: Option<String>,
    crew: Option<String>,
    capabilities: Option<String>,
    json: bool,
) -> Result<()> {
    let config = RedisConfig::default();
    let redis = RedisComms::new(config, "cli-discover".into())?;

    let metadata = AgentMetadata {
        agent_id: "cli-discover".to_string(),
        agent_role: "cli".to_string(),
        capabilities: vec![],
        crew: None,
        status: AgentStatus::Online,
        last_seen: SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs(),
        load: 0.0,
        specializations: HashMap::new(),
    };

    let coordinator = AgentCoordinator::new(redis, metadata);
    let mut agents = coordinator.discover_agents().await?;

    // Apply filters
    if let Some(role_filter) = role {
        agents.retain(|a| a.agent_role == role_filter);
    }

    if let Some(crew_filter) = crew {
        agents.retain(|a| a.crew.as_ref() == Some(&crew_filter));
    }

    if let Some(caps) = capabilities {
        let required: Vec<_> = caps.split(',').map(|s| s.trim()).collect();
        agents.retain(|a| {
            required
                .iter()
                .all(|c| a.capabilities.contains(&c.to_string()))
        });
    }

    if json {
        println!("{}", serde_json::to_string_pretty(&agents)?);
    } else {
        println!("📡 Discovered {} agents:\n", agents.len());
        for agent in agents {
            println!("🤖 {} ({})", agent.agent_id, agent.agent_role);
            println!("   Skills: {}", agent.capabilities.join(", "));
            if let Some(crew) = agent.crew {
                println!("   Crew: {}", crew);
            }
            println!("   Status: {:?}", agent.status);
            println!();
        }
    }

    Ok(())
}

async fn handle_message(to_agent: &str, subject: &str, content: &str, ack: bool) -> Result<()> {
    let config = RedisConfig::default();
    let redis = RedisComms::new(config, "cli-message".into())?;

    let metadata = AgentMetadata {
        agent_id: "cli-sender".to_string(),
        agent_role: "cli".to_string(),
        capabilities: vec![],
        crew: None,
        status: AgentStatus::Online,
        last_seen: SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs(),
        load: 0.0,
        specializations: HashMap::new(),
    };

    let coordinator = AgentCoordinator::new(redis, metadata);
    let message_id = coordinator
        .send_message(to_agent, subject, content, ack)
        .await?;

    println!("✅ Message sent to {}: {}", to_agent, message_id);

    Ok(())
}

async fn handle_delegate(
    worker: &str,
    task_id: &str,
    description: &str,
    priority_str: &str,
    deadline: Option<u64>,
    capabilities: Option<String>,
    blocking: bool,
    skill: Option<&str>,
    role: Option<&str>,
    output_contract: Option<&str>,
) -> Result<()> {
    let config = RedisConfig::default();
    let redis = RedisComms::new(config, "cli-captain".into())?;

    let metadata = AgentMetadata {
        agent_id: "cli-captain".to_string(),
        agent_role: "captain".to_string(),
        capabilities: vec![],
        crew: None,
        status: AgentStatus::Online,
        last_seen: SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs(),
        load: 0.0,
        specializations: HashMap::new(),
    };

    let mut coordinator = AgentCoordinator::new(redis, metadata);

    // Parse priority
    let priority = match priority_str.to_lowercase().as_str() {
        "low" => TaskPriority::Low,
        "normal" => TaskPriority::Normal,
        "high" => TaskPriority::High,
        "critical" => TaskPriority::Critical,
        _ => TaskPriority::Normal,
    };

    // Parse capabilities
    let required_caps = capabilities
        .map(|s| s.split(',').map(|c| c.trim().to_string()).collect())
        .unwrap_or_default();

    // Parse deadline
    let deadline_duration = deadline.map(|mins| Duration::from_secs(mins * 60));

    // Build enriched task description with skill/role context injection
    let enriched_description =
        build_enriched_description(description, skill, role, output_contract);

    println!("📋 Delegating task {} to {}", task_id, worker);
    if skill.is_some() || role.is_some() {
        println!("   🧠 Context: skill={:?} role={:?}", skill, role);
    }
    if let Some(contract) = output_contract {
        println!("   📐 Output contract: {}", contract);
    }

    let result = coordinator
        .delegate_task(
            worker,
            task_id,
            &enriched_description,
            priority,
            deadline_duration,
            required_caps,
            blocking,
        )
        .await?;

    if let Some(completion) = result {
        println!("✅ Task completed: {:?}", completion.status);
        if let Some(res) = completion.result {
            println!("   Result: {}", res);
        }
    } else {
        println!("✅ Task delegated (non-blocking)");
    }

    Ok(())
}

async fn handle_complete(
    captain: &str,
    task_id: &str,
    status_str: &str,
    result: Option<String>,
    artifacts: Option<String>,
) -> Result<()> {
    let config = RedisConfig::default();
    let redis = RedisComms::new(config, "cli-worker".into())?;

    let metadata = AgentMetadata {
        agent_id: "cli-worker".to_string(),
        agent_role: "worker".to_string(),
        capabilities: vec![],
        crew: None,
        status: AgentStatus::Online,
        last_seen: SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs(),
        load: 0.0,
        specializations: HashMap::new(),
    };

    let coordinator = AgentCoordinator::new(redis, metadata);

    // Parse status
    let status = match status_str.to_lowercase().as_str() {
        "success" => TaskCompletionStatus::Success,
        "failed" => TaskCompletionStatus::Failed("Task failed".to_string()),
        "partial" => TaskCompletionStatus::PartialSuccess("Partially completed".to_string()),
        "cancelled" => TaskCompletionStatus::Cancelled,
        _ => TaskCompletionStatus::Success,
    };

    // Parse artifacts
    let artifact_list = artifacts
        .map(|s| s.split(',').map(|a| a.trim().to_string()).collect())
        .unwrap_or_default();

    coordinator
        .complete_task(captain, task_id, status, result, artifact_list)
        .await?;

    println!("✅ Task completion reported to {}", captain);

    Ok(())
}

async fn handle_progress(
    task_id: &str,
    progress: f32,
    message: &str,
    eta: Option<u64>,
) -> Result<()> {
    let config = RedisConfig::default();
    let redis = RedisComms::new(config, "cli-worker".into())?;

    let metadata = AgentMetadata {
        agent_id: "cli-worker".to_string(),
        agent_role: "worker".to_string(),
        capabilities: vec![],
        crew: None,
        status: AgentStatus::Online,
        last_seen: SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs(),
        load: 0.0,
        specializations: HashMap::new(),
    };

    let coordinator = AgentCoordinator::new(redis, metadata);

    let eta_duration = eta.map(|mins| Duration::from_secs(mins * 60));

    coordinator
        .report_progress(task_id, progress, message, eta_duration)
        .await?;

    println!("📊 Progress reported: {}% - {}", progress, message);

    Ok(())
}

async fn handle_capability(capabilities: &str, description: &str, urgency_str: &str) -> Result<()> {
    let config = RedisConfig::default();
    let redis = RedisComms::new(config, "cli-capability".into())?;

    let metadata = AgentMetadata {
        agent_id: "cli-capability".to_string(),
        agent_role: "cli".to_string(),
        capabilities: vec![],
        crew: None,
        status: AgentStatus::Online,
        last_seen: SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs(),
        load: 0.0,
        specializations: HashMap::new(),
    };

    let coordinator = AgentCoordinator::new(redis, metadata);

    let required_caps: Vec<String> = capabilities
        .split(',')
        .map(|s| s.trim().to_string())
        .collect();

    let urgency = match urgency_str.to_lowercase().as_str() {
        "low" => RequestUrgency::Low,
        "high" => RequestUrgency::High,
        "emergency" => RequestUrgency::Emergency,
        _ => RequestUrgency::Normal,
    };

    println!("Requesting agents with capabilities: {}", capabilities);
    println!("Task: {}", description);

    let agents = coordinator
        .request_capability(required_caps, description, urgency)
        .await?;

    if agents.is_empty() {
        println!("No agents responded (Redis may be unavailable)");
    } else {
        println!("Capable agents found: {}", agents.len());
        for (agent_id, skills) in &agents {
            println!("  {} - {:?}", agent_id, skills);
        }
    }

    Ok(())
}

async fn handle_notify(
    event_type: &str,
    source: &str,
    details_str: &str,
    agents: Option<String>,
) -> Result<()> {
    let config = RedisConfig::default();
    let redis = RedisComms::new(config, "cli-notify".into())?;

    let metadata = AgentMetadata {
        agent_id: "cli-notify".to_string(),
        agent_role: "cli".to_string(),
        capabilities: vec![],
        crew: None,
        status: AgentStatus::Online,
        last_seen: SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs(),
        load: 0.0,
        specializations: HashMap::new(),
    };

    let coordinator = AgentCoordinator::new(redis, metadata);

    let details: serde_json::Value = serde_json::from_str(details_str)
        .unwrap_or_else(|_| serde_json::json!({"message": details_str}));

    let affected_agents = agents.map(|s| {
        s.split(',')
            .map(|a| a.trim().to_string())
            .collect::<Vec<_>>()
    });

    coordinator
        .notify_event(event_type, source, details, affected_agents)
        .await?;

    println!("Notification sent: [{}] from {}", event_type, source);

    Ok(())
}

async fn handle_wait(
    timeout_secs: u64,
    message_type: Option<String>,
    from_agent: Option<String>,
    task_id: Option<String>,
    subject: Option<String>,
) -> Result<()> {
    let config = RedisConfig::default();
    let redis = RedisComms::new(config, "cli-wait".into())?;

    let metadata = AgentMetadata {
        agent_id: "cli-wait".to_string(),
        agent_role: "cli".to_string(),
        capabilities: vec![],
        crew: None,
        status: AgentStatus::Online,
        last_seen: SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs(),
        load: 0.0,
        specializations: HashMap::new(),
    };

    let coordinator = AgentCoordinator::new(redis, metadata);

    let filter = MessageFilter {
        message_types: message_type.map(|t| vec![t]),
        from_agents: from_agent.map(|a| vec![a]),
        task_ids: task_id.map(|t| vec![t]),
        subjects: subject.map(|s| vec![s]),
    };

    println!("Waiting for message (timeout: {}s)...", timeout_secs);

    let timeout_duration = Duration::from_secs(timeout_secs);
    match coordinator.wait_for_message(timeout_duration, filter).await {
        Ok(msg) => {
            println!("Message received: {:?}", msg);
        }
        Err(e) => {
            // Timeout or channel closed is expected non-fatal outcome
            println!("Wait ended: {}", e);
        }
    }

    Ok(())
}

async fn handle_start(config_path: &PathBuf) -> Result<()> {
    let manager = AgentManager::default();
    let _handle = manager.spawn_agent(config_path).await?;

    println!("✅ Agent started from {}", config_path.display());
    println!("   Press Ctrl+C to stop");

    // Keep the agent running
    tokio::signal::ctrl_c().await?;

    Ok(())
}

async fn handle_start_all(dir: &PathBuf) -> Result<()> {
    let manager = AgentManager::default();
    let handles = manager.spawn_from_directory(dir).await?;

    println!("✅ Started {} agents from {}", handles.len(), dir.display());
    println!("   Press Ctrl+C to stop all agents");

    // Keep agents running
    tokio::signal::ctrl_c().await?;

    Ok(())
}

/// Direct deterministic agent invocation — no Redis, no Python, no ralph.
/// Loads [b00t.agent.executor] from <agent>.agent.toml, runs invoke_agent_executor() loop.
async fn handle_invoke(
    agent: &str,
    prompt: &str,
    config_override: Option<&std::path::Path>,
) -> Result<()> {
    use b00t_c0re_lib::agent_manager::{AgentManager, invoke_agent_executor};

    // Resolve config path: override → _b00t_/<agent>.agent.toml → cwd search
    let config_path = if let Some(p) = config_override {
        p.to_path_buf()
    } else {
        let root = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
        let agent_dir = root.join("_b00t_");
        find_agent_config_path(&agent_dir, agent).ok_or_else(|| {
            anyhow::anyhow!(
                "No config found for agent '{}' under {}",
                agent,
                agent_dir.display()
            )
        })?
    };

    let config = AgentManager::load_config(&config_path).await?;

    let executor = config.b00t.agent.executor.as_ref().ok_or_else(|| {
        anyhow::anyhow!(
            "Agent '{}' has no [b00t.agent.executor] section in {}",
            agent,
            config_path.display()
        )
    })?;

    // Merge agent env into process env
    let mut env: std::collections::HashMap<String, String> = std::env::vars().collect();
    if let Some(agent_env) = &config.b00t.env {
        env.extend(agent_env.clone());
    }

    println!(
        "🤖 Invoking {} (max {} iterations)",
        agent, executor.max_iterations
    );
    println!(
        "   cli: {} {}",
        executor.cli_path,
        executor.cli_args.join(" ")
    );

    let result = invoke_agent_executor(executor, &env, prompt)
        .map_err(|e| anyhow::anyhow!("Agent invocation failed: {}", e))?;

    println!("{}", result);
    Ok(())
}

async fn handle_ralph(
    tool: &str,
    task: &str,
    max_iterations: u32,
    project_root: Option<&std::path::Path>,
) -> Result<()> {
    use duct::cmd;

    println!("🥾 Running ralph autonomous agent");
    println!("   Tool: {}", tool);
    println!("   Task: {}", task);
    println!("   Max iterations: {}", max_iterations);

    // Determine project root
    let root = project_root
        .map(|p| p.to_path_buf())
        .unwrap_or_else(|| std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")));

    println!("   Project: {}", root.display());

    // Check if ralph submodule exists
    let ralph_path = root.join("_b00t_/ralph");
    if !ralph_path.exists() {
        anyhow::bail!(
            "Ralph submodule not found at {}. Run 'git submodule update --init --recursive'",
            ralph_path.display()
        );
    }

    // TaskMaster was removed from b00t. Keep Ralph pointed at the live markdown backlog instead.
    if task == "hive-validate" || task == "maintenance" {
        ensure_todo_next_backlog(&root).await?;
    }

    // Use the shell Ralph loop for self-hosted local tools because it already
    // knows how to fall back between gateway and direct Gemma4 inference.
    let shell_ralph_script = root.join("b00t.sh");
    let (program, ralph_args, working_dir) =
        if uses_shell_ralph(tool) && shell_ralph_script.exists() {
            (
                "bash",
                build_shell_ralph_command_args(tool, max_iterations, task),
                root.clone(),
            )
        } else {
            (
                "uv",
                build_ralph_command_args(tool, max_iterations, task),
                ralph_path.clone(),
            )
        };

    println!("🚀 Starting ralph autonomous loop...");
    let ralph_cmd = cmd(program, ralph_args)
        .dir(working_dir)
        .env("PROJECT_ROOT", root.to_str().unwrap_or("."))
        .env("B00T_ROLE", "operator");

    let output = ralph_cmd
        .stdout_to_stderr()
        .run()
        .map_err(|e| anyhow::anyhow!("Ralph execution failed: {}", e))?;

    if output.status.success() {
        println!("✅ Ralph completed successfully");
    } else {
        anyhow::bail!("Ralph failed with exit code: {:?}", output.status.code());
    }

    Ok(())
}

fn uses_shell_ralph(tool: &str) -> bool {
    matches!(tool, "pi" | "opencode" | "mistralrs" | "gemma4")
}

fn build_ralph_command_args(tool: &str, max_iterations: u32, task: &str) -> Vec<String> {
    let mut args = vec![
        "run".to_string(),
        "ralph".to_string(),
        "run".to_string(),
        "--tool".to_string(),
        tool.to_string(),
        "--max-iterations".to_string(),
        max_iterations.to_string(),
    ];

    if let Some(task_id) = resolve_ralph_task_id(task) {
        args.push("--task-id".to_string());
        args.push(task_id);
    }

    args
}

fn build_shell_ralph_command_args(tool: &str, max_iterations: u32, task: &str) -> Vec<String> {
    let mut args = vec![
        "b00t.sh".to_string(),
        "--tool".to_string(),
        tool.to_string(),
        "--max-iterations".to_string(),
        max_iterations.to_string(),
        "--role".to_string(),
        "operator".to_string(),
    ];

    if let Some(task_id) = resolve_ralph_task_id(task) {
        args.push("--task-id".to_string());
        args.push(task_id);
    }

    args
}

fn resolve_ralph_task_id(task: &str) -> Option<String> {
    let normalized = task.trim().to_lowercase();
    if normalized.is_empty() || normalized == "pending" || normalized == "all" {
        return None;
    }

    match normalized.as_str() {
        "hive-validate" | "maintenance" => None,
        _ => Some(task.to_string()),
    }
}

async fn ensure_todo_next_backlog(root: &std::path::Path) -> Result<()> {
    use std::fs;

    let todo_path = root.join("TODO-next.md");

    if !todo_path.exists() {
        println!("📋 Creating TODO-next.md backlog...");
        fs::write(
            &todo_path,
            r#"# TODO-next

## Critical path
- [ ] Hive validate: submodules, MCPs, Redis, CLI health
- [ ] Maintenance: datum validation, dependency DAG, duplicate checks
"#,
        )?;
        println!("✅ Created TODO-next.md backlog");
    }

    Ok(())
}

/// Build enriched task description by prepending skill instructions and role context.
///
/// Skill/role context is injected as a structured preamble before the task description.
/// This ensures the receiving agent (any model) gets complete context without needing
/// out-of-band communication.
///
/// Format:
/// ```text
/// [SKILL: fast-rust]
/// <skill instructions body>
/// ---
/// [ROLE: executive]
/// Role `executive`: Hive captain — manages...
/// ---
/// [OUTPUT CONTRACT: PASS|FAIL:<5lines>]
/// ---
/// <original task description>
/// ```
fn build_enriched_description(
    description: &str,
    skill: Option<&str>,
    role: Option<&str>,
    output_contract: Option<&str>,
) -> String {
    let mut parts: Vec<String> = Vec::new();

    if let Some(skill_name) = skill {
        let resolver = crate::skill_resolver::SkillResolver::default();
        match resolver.load(skill_name) {
            Ok(content) => {
                parts.push(format!("[SKILL: {}]\n{}", skill_name, content.instructions));
            }
            Err(_) => {
                // Skill not found — note it but don't fail delegation
                parts.push(format!("[SKILL: {} — not resolved]", skill_name));
            }
        }
    }

    if let Some(role_name) = role {
        let role_summary = load_role_hint(role_name);
        parts.push(format!("[ROLE: {}]\n{}", role_name, role_summary));
    }

    if let Some(contract) = output_contract {
        parts.push(format!("[OUTPUT CONTRACT: {}]", contract));
    }

    if parts.is_empty() {
        return description.to_string();
    }

    format!("{}\n---\n{}", parts.join("\n---\n"), description)
}

/// Load a brief role hint string for delegation preamble
fn load_role_hint(role_name: &str) -> String {
    let b00t_dirs: Vec<_> = [
        std::env::current_dir().ok().map(|d| d.join("_b00t_")),
        dirs::home_dir().map(|d| d.join(".b00t").join("_b00t_")),
    ]
    .into_iter()
    .flatten()
    .filter(|p| p.is_dir())
    .collect();

    for dir in &b00t_dirs {
        for ext in &["role.tomllmd", "role.tomllm", "role.toml"] {
            let path = dir.join(format!("{}.{}", role_name, ext));
            if path.exists() {
                if let Ok(content) = std::fs::read_to_string(&path) {
                    if let Ok(value) = toml::from_str::<toml::Value>(&content) {
                        let hint = value
                            .get("b00t")
                            .and_then(|b| b.get("hint"))
                            .or_else(|| value.get("hint"))
                            .and_then(|h| h.as_str())
                            .unwrap_or(role_name);
                        return format!("Role `{}`: {}", role_name, hint);
                    }
                }
            }
        }
    }
    format!("Role: {}", role_name)
}

#[cfg(test)]
mod tests {
    use super::{
        AgentDatumSummary, agent_summary_from_value, build_enriched_description,
        build_ralph_command_args, build_shell_ralph_command_args, command_mentions_agent,
        find_agent_config_path, is_agent_datum, load_guard_infraction_stats, resolve_ralph_task_id,
        toon_cell, uses_shell_ralph,
    };

    #[test]
    fn test_agent_dashboard_toon_cell_sanitizes_delimiters() {
        assert_eq!(toon_cell("alpha,beta\nnext"), "alpha;beta next");
    }

    #[test]
    fn test_agent_dashboard_command_mentions_agent_token_boundaries() {
        assert!(command_mentions_agent(
            "b00t agent invoke opencode task",
            "opencode"
        ));
        assert!(command_mentions_agent(
            "systemctl start b00t@pi-agent.service",
            "pi"
        ));
        assert!(!command_mentions_agent("topic obscure", "pi"));
    }

    #[test]
    fn test_agent_dashboard_guard_infraction_stats_skip_malformed_lines() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("guard-violations.jsonl");
        std::fs::write(
            &path,
            "{\"pattern\":\"raw sed\",\"tags\":[\"guard\"]}\nnot-json\n{\"agent\":\"alpha\",\"count\":1}\n",
        )
        .unwrap();
        let agents = vec![AgentDatumSummary {
            name: "alpha".to_string(),
            kind: "agent".to_string(),
            role: "specialist".to_string(),
            model: "test".to_string(),
            socket: None,
            quota_daily: None,
            skills_count: 1,
        }];
        let stats = load_guard_infraction_stats(&path, &agents);
        assert_eq!(stats.global_infractions, 2);
        assert_eq!(stats.malformed_lines, 1);
        assert_eq!(stats.by_agent.get("alpha"), Some(&1));
    }

    #[test]
    fn test_agent_dashboard_accepts_cardinal_subtype() {
        let value: toml::Value = toml::from_str(
            r#"
[b00t]
name = "claude"
type = "agent.cli"
hint = "Claude CLI"

[b00t.agent]
pid = "claude-cli"
model = "claude"
skills = ["code"]
role = "specialist"

[b00t.agent.ipc]
socket = ""
"#,
        )
        .unwrap();
        assert!(is_agent_datum(&value));
        let summary = agent_summary_from_value(&value, "claude.agent.cli.toml").unwrap();
        assert_eq!(summary.name, "claude");
        assert_eq!(summary.kind, "agent.cli");
    }

    #[test]
    fn test_find_agent_config_path_accepts_cardinal_subtypes() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("claude.agent.cli.toml");
        std::fs::write(&path, "").unwrap();
        assert_eq!(find_agent_config_path(dir.path(), "claude"), Some(path));
    }

    #[test]
    fn test_resolve_ralph_task_id_mappings() {
        assert_eq!(resolve_ralph_task_id("hive-validate"), None);
        assert_eq!(resolve_ralph_task_id("maintenance"), None);
        assert_eq!(resolve_ralph_task_id("pending"), None);
        assert_eq!(resolve_ralph_task_id("all"), None);
        assert_eq!(
            resolve_ralph_task_id("custom-123"),
            Some("custom-123".to_string())
        );
    }

    #[test]
    fn test_build_ralph_command_args_omits_task_id_for_named_backlog_views() {
        let args = build_ralph_command_args("codex", 5, "hive-validate");
        assert!(!args.contains(&"--task-id".to_string()));
        assert!(!args.contains(&"--filter".to_string()));
    }

    #[test]
    fn test_build_ralph_command_args_omits_task_for_pending() {
        let args = build_ralph_command_args("codex", 5, "pending");
        assert!(!args.contains(&"--task-id".to_string()));
    }

    #[test]
    fn test_uses_shell_ralph_for_self_hosted_tools() {
        assert!(uses_shell_ralph("pi"));
        assert!(uses_shell_ralph("opencode"));
        assert!(uses_shell_ralph("mistralrs"));
        assert!(uses_shell_ralph("gemma4"));
        assert!(!uses_shell_ralph("codex"));
    }

    #[test]
    fn test_build_shell_ralph_command_args_includes_operator_role() {
        let args = build_shell_ralph_command_args("pi", 3, "");
        assert_eq!(args[0], "b00t.sh");
        assert!(args.contains(&"--tool".to_string()));
        assert!(args.contains(&"pi".to_string()));
        assert!(args.contains(&"--role".to_string()));
        assert!(args.contains(&"operator".to_string()));
    }

    #[test]
    fn test_build_shell_ralph_command_args_passes_task_id() {
        let args = build_shell_ralph_command_args("gemma4", 5, "42");
        assert!(args.contains(&"--task-id".to_string()));
        assert!(args.contains(&"42".to_string()));
    }

    #[test]
    fn test_build_shell_ralph_command_args_omits_empty_task() {
        let args = build_shell_ralph_command_args("pi", 3, "");
        assert!(!args.contains(&"--task-id".to_string()));
    }

    #[test]
    fn test_build_enriched_description_passthrough() {
        let result = build_enriched_description("do the thing", None, None, None);
        assert_eq!(result, "do the thing");
    }

    #[test]
    fn test_build_enriched_description_with_output_contract() {
        let result =
            build_enriched_description("run tests", None, None, Some("PASS|FAIL:<5lines>"));
        assert!(result.contains("[OUTPUT CONTRACT: PASS|FAIL:<5lines>]"));
        assert!(result.contains("run tests"));
        assert!(result.contains("---"));
    }

    #[test]
    fn test_build_enriched_description_with_missing_skill_graceful() {
        // Skill not found — should not panic, just note it
        let result =
            build_enriched_description("do work", Some("nonexistent-skill-xyz"), None, None);
        assert!(result.contains("nonexistent-skill-xyz"));
        assert!(result.contains("do work"));
    }
}

/// Establish Redis pub/sub channels for role-based delegation to entangled agents.
///
/// Creates channels in format: `agent:{role}:{agent_name}` for each entangled agent.
/// Enables captain→worker delegation through role-specific channels.
pub async fn setup_entangled_channels(
    role_datum: &crate::whoami::RoleDetails,
    path: &str,
) -> Result<BTreeMap<String, String>> {
    let mut channels = BTreeMap::new();
    let role = &role_datum.name;

    // Get channel_prefix from role datum, default to "agent:{role}:"
    let prefix = role_datum
        .channel_prefix
        .clone()
        .unwrap_or_else(|| format!("agent:{}:", role));

    for agent_name in &role_datum.entangled_agents {
        let channel = format!("{}{}", prefix, agent_name);
        println!("📡 Channel: {}", channel);
        channels.insert(agent_name.clone(), channel);
    }

    Ok(channels)
}

/// Get the delegation channel for a specific worker agent.
/// Falls back to default global channel if role not set or doesn't have entangled_agents.
fn get_delegation_channel(
    worker: &str,
    role: Option<&str>,
    entangled_channels: &BTreeMap<String, String>,
) -> String {
    // First check if we have a role-specific channel for this worker
    if let Some(channel) = entangled_channels.get(worker) {
        return channel.clone();
    }

    // Fallback to default global channel
    format!("agent:captain:{}", worker)
}

/// Resolve entangled channels for a role from the datum system.
async fn resolve_role_channels(
    role_name: Option<&str>,
    path: &str,
) -> Result<(Option<String>, BTreeMap<String, String>)> {
    let mut channels = BTreeMap::new();
    let channel_prefix = if let Some(role) = role_name {
        // Try to load role datum
        let b00t_dirs: Vec<_> = [
            std::env::current_dir().ok().map(|d| d.join("_b00t_")),
            dirs::home_dir().map(|d| d.join(".b00t").join("_b00t_")),
        ]
        .into_iter()
        .flatten()
        .filter(|p| p.is_dir())
        .collect();

        for dir in &b00t_dirs {
            let role_path = dir.join(format!("{}.role.toml", role));
            if role_path.exists() {
                // Try to load from file
                if let Ok(content) = std::fs::read_to_string(&role_path) {
                    if let Ok(value) = toml::from_str::<toml::Value>(&content) {
                        let datum = value.get("b00t");
                        let entangled = datum
                            .and_then(|d| d.get("entangled_agents"))
                            .and_then(|a| a.as_array())
                            .map(|arr| {
                                arr.iter()
                                    .filter_map(|v| v.as_str().map(|s| s.to_string()))
                                    .collect::<Vec<_>>()
                            });

                        let prefix = datum
                            .and_then(|d| d.get("channel_prefix"))
                            .and_then(|p| p.as_str())
                            .map(|s| s.to_string());

                        let prefix = prefix.clone(); // Clone for later use

                        if let Some(agents) = entangled {
                            let prefix = prefix
                                .as_ref()
                                .map(|p| p.to_string())
                                .unwrap_or_else(|| format!("agent:{}:", role));
                            for agent_name in agents {
                                let channel = format!("{}{}", prefix, agent_name);
                                channels.insert(agent_name.clone(), channel);
                            }
                        }

                        return Ok((prefix, channels));
                    }
                }
            }
        }
        None
    } else {
        None
    };

    Ok((channel_prefix, channels))
}
