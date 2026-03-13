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
use std::collections::HashMap;
use std::path::PathBuf;
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

        #[arg(long, help = "Request urgency (low, normal, high, emergency)", default_value = "normal")]
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

    #[clap(about = "Run ralph autonomous agent for hive maintenance/validation")]
    Ralph {
        #[arg(
            long,
            help = "Executor tool (codex, claude, amp, opencode, mistralrs)",
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
        } => {
            handle_delegate(
                &worker,
                &task_id,
                &description,
                &priority,
                deadline,
                capabilities,
                blocking,
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

        AgentCommands::Ralph {
            tool,
            task,
            max_iterations,
            project_root,
        } => handle_ralph(&tool, &task, max_iterations, project_root.as_deref()).await,
    }
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

    println!("📋 Delegating task {} to {}", task_id, worker);

    let result = coordinator
        .delegate_task(
            worker,
            task_id,
            description,
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

async fn handle_capability(
    capabilities: &str,
    description: &str,
    urgency_str: &str,
) -> Result<()> {
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

    // Check if taskmaster is initialized
    let taskmaster_path = root.join(".taskmaster");
    if !taskmaster_path.exists() {
        println!("⚠️  TaskMaster not initialized. Initializing now...");
        cmd!("taskmaster", "init")
            .dir(&root)
            .stdout_capture()
            .stderr_capture()
            .run()
            .map_err(|e| anyhow::anyhow!("Failed to initialize taskmaster: {}", e))?;
    }

    // Create hive validation task if needed
    if task == "hive-validate" {
        ensure_hive_validation_task(&root).await?;
    }

    // Run ralph
    println!("🚀 Starting ralph autonomous loop...");
    let ralph_args = build_ralph_command_args(tool, max_iterations, task);
    let ralph_cmd = cmd("uv", ralph_args)
        .dir(&ralph_path)
        .env("PROJECT_ROOT", root.to_str().unwrap_or("."));

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

fn resolve_ralph_task_id(task: &str) -> Option<String> {
    let normalized = task.trim().to_lowercase();
    if normalized.is_empty() || normalized == "pending" || normalized == "all" {
        return None;
    }

    match normalized.as_str() {
        "hive-validate" => Some("hive-001".to_string()),
        "maintenance" => Some("hive-002".to_string()),
        _ => Some(task.to_string()),
    }
}

async fn ensure_hive_validation_task(root: &std::path::Path) -> Result<()> {
    use std::fs;

    let tasks_file = root.join(".taskmaster/tasks/tasks.json");

    // Read existing tasks
    if !tasks_file.exists() {
        println!("📋 Creating hive validation tasks...");

        let tasks_dir = tasks_file.parent().unwrap();
        fs::create_dir_all(tasks_dir)?;

        let initial_tasks = serde_json::json!({
            "metadata": {
                "version": "1.0.0",
                "branchName": "main",
                "createdAt": chrono::Utc::now().to_rfc3339()
            },
            "tasks": [
                {
                    "id": "hive-001",
                    "title": "Hive Validation - b00t System Health",
                    "description": "As a system operator, I need to validate the b00t hive is healthy and all critical datums are properly configured, so that agents can operate reliably. This includes checking: 1) All submodules are initialized, 2) Critical CLI tools are installed (rust, uv, just, gh), 3) MCP servers are configured, 4) Agent coordination is functional.",
                    "status": "pending",
                    "priority": "high",
                    "tags": ["hive-validate", "system-health"],
                    "blockedBy": [],
                    "acceptanceCriteria": [
                        "Git submodules initialized and up to date",
                        "Rust toolchain installed and functional",
                        "UV package manager available",
                        "Just command runner available",
                        "GitHub CLI authenticated",
                        "At least 3 MCP servers configured",
                        "Redis available for agent coordination"
                    ]
                },
                {
                    "id": "hive-002",
                    "title": "Hive Maintenance - Datum Ontology Validation",
                    "description": "As a system operator, I need to validate that all datum files are properly structured and follow b00t conventions, so that the hive can discover and use capabilities correctly. Validate: 1) TOML syntax, 2) Required fields present, 3) Version detection works, 4) No duplicate datums, 5) Stack dependencies are valid.",
                    "status": "pending",
                    "priority": "normal",
                    "tags": ["hive-validate", "maintenance"],
                    "blockedBy": ["hive-001"],
                    "acceptanceCriteria": [
                        "All .toml files pass syntax validation",
                        "No missing required fields in datums",
                        "Version regex patterns are valid",
                        "Stack dependencies form valid DAG",
                        "No circular dependencies detected"
                    ]
                }
            ]
        });

        fs::write(&tasks_file, serde_json::to_string_pretty(&initial_tasks)?)?;
        println!("✅ Created hive validation tasks");
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{build_ralph_command_args, resolve_ralph_task_id};

    #[test]
    fn test_resolve_ralph_task_id_mappings() {
        assert_eq!(
            resolve_ralph_task_id("hive-validate"),
            Some("hive-001".to_string())
        );
        assert_eq!(
            resolve_ralph_task_id("maintenance"),
            Some("hive-002".to_string())
        );
        assert_eq!(resolve_ralph_task_id("pending"), None);
        assert_eq!(resolve_ralph_task_id("all"), None);
        assert_eq!(
            resolve_ralph_task_id("custom-123"),
            Some("custom-123".to_string())
        );
    }

    #[test]
    fn test_build_ralph_command_args_uses_task_id_not_filter() {
        let args = build_ralph_command_args("codex", 5, "hive-validate");
        assert!(args.contains(&"--task-id".to_string()));
        assert!(args.contains(&"hive-001".to_string()));
        assert!(!args.contains(&"--filter".to_string()));
    }

    #[test]
    fn test_build_ralph_command_args_omits_task_for_pending() {
        let args = build_ralph_command_args("codex", 5, "pending");
        assert!(!args.contains(&"--task-id".to_string()));
    }
}
