// b00t-cli/src/commands/context.rs
// Agent context snapshot & resume — "eureka" moment capture for AI agent tooling.
// 🤓 Uses b00t-c0re-gov::ContextStore for atomic crash-safe persistence.
//    Contexts are stored in ~/.local/share/b00t/hooks/ alongside hook-triggered snapshots,
//    distinguished by gate value "context:manual" and hook_type TimerMs(0).

use anyhow::{Context as AnyhowContext, Result};
use b00t_c0re_gov::store::ContextStore;
use b00t_c0re_gov::types::{AgentContext, HookToken, HookType};
use chrono::Utc;
use clap::Parser;
use uuid::Uuid;

/// Gate value that distinguishes manual context saves from hook-triggered snapshots.
const CONTEXT_GATE: &str = "context:manual";

#[derive(Parser, Debug)]
pub enum ContextCommands {
    /// Save agent context snapshot for later resumption (eureka moment capture).
    Save {
        #[clap(long, help = "Agent identifier (default: $USER)")]
        agent_id: Option<String>,
        #[clap(long, help = "Task description")]
        task: Option<String>,
        #[clap(long, short, help = "Message / insight (e.g. 'found the bug in X')")]
        message: Option<String>,
        #[clap(long, help = "Resume instruction for agent")]
        continuation: Option<String>,
        #[clap(long, help = "TTL in seconds before auto-expiry")]
        ttl: Option<u64>,
    },
    /// List all saved agent contexts.
    List {
        #[clap(long, help = "Show full details (default: summary)")]
        verbose: bool,
    },
    /// Show full details of a specific saved context.
    Show {
        #[clap(help = "Context ID (UUID)")]
        id: String,
    },
    /// Output resume instructions for a saved context.
    Resume {
        #[clap(help = "Context ID (UUID)")]
        id: String,
        #[clap(long, help = "Delete context after resuming")]
        delete: bool,
    },
    /// Compact: save context and emit /compact instruction for context window reset.
    Compact {
        #[clap(long, short, help = "Message / insight for resumption")]
        message: Option<String>,
    },
    /// Delete a saved context.
    Delete {
        #[clap(help = "Context ID (UUID)")]
        id: String,
    },
}

pub fn handle_context_command(cmd: &ContextCommands) -> Result<()> {
    match cmd {
        ContextCommands::Save {
            agent_id,
            task,
            message,
            continuation,
            ttl,
        } => handle_save(
            agent_id.as_deref(),
            task.as_deref(),
            message.as_deref(),
            continuation.as_deref(),
            *ttl,
        ),
        ContextCommands::List { verbose } => handle_list(*verbose),
        ContextCommands::Show { id } => handle_show(id),
        ContextCommands::Resume { id, delete } => handle_resume(id, *delete),
        ContextCommands::Compact { message } => handle_compact(message.as_deref()),
        ContextCommands::Delete { id } => handle_delete(id),
    }
}

fn store() -> Result<ContextStore> {
    ContextStore::new().context("failed to open context store")
}

fn agent_id() -> String {
    std::env::var("USER")
        .or_else(|_| std::env::var("USERNAME"))
        .unwrap_or_else(|_| "b00t-agent".to_string())
}

/// Save current agent context to the store.
fn handle_save(
    agent_id_arg: Option<&str>,
    task: Option<&str>,
    message: Option<&str>,
    continuation: Option<&str>,
    ttl_secs: Option<u64>,
) -> Result<()> {
    let store = store()?;
    let id = Uuid::new_v4();
    let agent = agent_id_arg.unwrap_or(&agent_id()).to_string();

    let token = HookToken {
        id,
        hook_type: HookType::TimerMs(0), // marker: manual context save
        created_at: Utc::now(),
        ttl_ms: ttl_secs.map(|s| s * 1000),
        description: message.unwrap_or("manual context snapshot").to_string(),
    };

    let ctx = AgentContext {
        agent_id: agent,
        task: task.unwrap_or("context-save").to_string(),
        gate: CONTEXT_GATE.to_string(),
        result_so_far: serde_json::json!({
            "message": message.unwrap_or(""),
            "branch": current_branch().unwrap_or_default(),
            "cwd": std::env::current_dir().map(|p| p.display().to_string()).unwrap_or_default(),
        }),
        reasoning: message.unwrap_or("").to_string(),
        created_at: Utc::now(),
        hook_token: token.clone(),
        continuation: continuation
            .unwrap_or("resume from context snapshot")
            .to_string(),
    };

    store.save(&token, &ctx).context("failed to save context")?;

    println!("context saved: {}", id);
    println!("  agent:    {}", ctx.agent_id);
    println!("  task:     {}", ctx.task);
    if let Some(msg) = message {
        println!("  message:  {}", msg);
    }
    println!("  resume:   {}", ctx.continuation);
    if let Some(ttl) = ttl_secs {
        println!("  ttl:      {}s", ttl);
    }

    Ok(())
}

/// List saved contexts, filtering to manual saves.
fn handle_list(verbose: bool) -> Result<()> {
    let store = store()?;
    let pending = store.list_pending().context("failed to list contexts")?;

    let manuals: Vec<_> = pending
        .into_iter()
        .filter(|t| matches!(t.hook_type, HookType::TimerMs(0)))
        .collect();

    if manuals.is_empty() {
        println!("no saved contexts");
        return Ok(());
    }

    for token in &manuals {
        if verbose {
            if let Ok(Some(ctx)) = store.load_by_id(&token.id) {
                println!("── {}", token.id);
                println!("  agent:    {}", ctx.agent_id);
                println!("  task:     {}", ctx.task);
                println!("  message:  {}", token.description);
                println!("  resume:   {}", ctx.continuation);
                println!(
                    "  created:  {}",
                    token.created_at.format("%Y-%m-%d %H:%M:%S")
                );
                if let Some(ttl) = token.ttl_ms {
                    let age = (Utc::now() - token.created_at).num_seconds() as u64;
                    let remaining = ttl.saturating_sub(age * 1000) / 1000;
                    println!("  ttl:      {}s remaining", remaining);
                }
                println!();
            }
        } else {
            let age = Utc::now().signed_duration_since(token.created_at);
            let age_str = if age.num_hours() > 0 {
                format!("{}h", age.num_hours())
            } else if age.num_minutes() > 0 {
                format!("{}m", age.num_minutes())
            } else {
                format!("{}s", age.num_seconds())
            };
            println!("{}  {:>6}  {}", token.id, age_str, token.description,);
        }
    }

    Ok(())
}

/// Show full details of a context.
fn handle_show(id: &str) -> Result<()> {
    let store = store()?;
    let uuid = Uuid::parse_str(id).context("invalid UUID")?;
    let ctx = store.load_by_id(&uuid)?.context("context not found")?;

    println!("id:           {}", uuid);
    println!("agent:        {}", ctx.agent_id);
    println!("task:         {}", ctx.task);
    println!("gate:         {}", ctx.gate);
    println!("continuation: {}", ctx.continuation);
    println!("reasoning:    {}", ctx.reasoning);
    println!(
        "result:       {}",
        serde_json::to_string_pretty(&ctx.result_so_far)?
    );
    println!(
        "created:      {}",
        ctx.created_at.format("%Y-%m-%d %H:%M:%S")
    );
    if let Some(ttl) = ctx.hook_token.ttl_ms {
        let age = (Utc::now() - ctx.created_at).num_seconds() as u64;
        let remaining = (ttl.saturating_sub(age * 1000)) / 1000;
        println!("ttl:          {}s remaining", remaining);
    }

    Ok(())
}

/// Output resume instructions.
fn handle_resume(id: &str, delete: bool) -> Result<()> {
    let store = store()?;
    let uuid = Uuid::parse_str(id).context("invalid UUID")?;
    let ctx = store.load_by_id(&uuid)?.context("context not found")?;

    println!("/resume {}", uuid);
    println!();
    println!("agent:        {}", ctx.agent_id);
    println!("task:         {}", ctx.task);
    println!("continuation: {}", ctx.continuation);
    println!();
    println!("reasoning:");
    println!("  {}", ctx.reasoning.replace('\n', "\n  "));
    println!();
    println!("context:");
    println!(
        "  {}",
        serde_json::to_string_pretty(&ctx.result_so_far)?.replace('\n', "\n  ")
    );

    if delete {
        store.delete(&uuid).context("failed to delete context")?;
        println!();
        println!("context deleted after resume");
    }

    Ok(())
}

/// Compact: save context + emit /compact instruction.
fn handle_compact(message: Option<&str>) -> Result<()> {
    let store = store()?;
    let id = Uuid::new_v4();
    let agent = agent_id();

    let token = HookToken {
        id,
        hook_type: HookType::TimerMs(0),
        created_at: Utc::now(),
        ttl_ms: None, // no expiry for compacted contexts
        description: message.unwrap_or("compacted context").to_string(),
    };

    let ctx = AgentContext {
        agent_id: agent,
        task: "compacted-session".to_string(),
        gate: CONTEXT_GATE.to_string(),
        result_so_far: serde_json::json!({
            "message": message.unwrap_or(""),
            "branch": current_branch().unwrap_or_default(),
            "compacted_at": Utc::now().to_rfc3339(),
        }),
        reasoning: message.unwrap_or("resume compacted session").to_string(),
        created_at: Utc::now(),
        hook_token: token.clone(),
        continuation: "restore compacted context and continue".to_string(),
    };

    store
        .save(&token, &ctx)
        .context("failed to save compacted context")?;

    println!("/compact");
    println!();
    println!("saved context: {}", id);
    println!("resume with:   b00t context resume {}", id);
    println!();
    println!("context window compacted. session state preserved.");
    println!("agent MUST issue /compact to clear context window.");
    println!(
        "on resume, restore with: b00t context resume {} --delete",
        id
    );

    Ok(())
}

/// Delete a saved context.
fn handle_delete(id: &str) -> Result<()> {
    let store = store()?;
    let uuid = Uuid::parse_str(id).context("invalid UUID")?;

    // Verify it exists before deleting
    if store.load_by_id(&uuid)?.is_none() {
        println!("context {} not found (already deleted?)", uuid);
        return Ok(());
    }

    store.delete(&uuid).context("failed to delete context")?;
    println!("deleted: {}", uuid);

    Ok(())
}

fn current_branch() -> Result<String> {
    std::process::Command::new("git")
        .args(["branch", "--show-current"])
        .output()
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
        .context("failed to get current git branch")
}

#[cfg(test)]
mod tests {
    use super::*;
    use b00t_c0re_gov::store::ContextStore;
    use tempfile::TempDir;

    fn test_store() -> (ContextStore, TempDir) {
        let tmp = TempDir::new().expect("tempdir");
        let path = tmp.path().join("hooks");
        std::fs::create_dir_all(&path).unwrap();
        (ContextStore::with_path(path), tmp)
    }

    fn test_token(id: Uuid, description: &str, ttl: Option<u64>) -> HookToken {
        HookToken {
            id,
            hook_type: HookType::TimerMs(0),
            created_at: Utc::now(),
            ttl_ms: ttl.map(|s| s * 1000),
            description: description.to_string(),
        }
    }

    fn test_context(
        token: &HookToken,
        agent: &str,
        task: &str,
        message: &str,
        continuation: &str,
    ) -> AgentContext {
        AgentContext {
            agent_id: agent.to_string(),
            task: task.to_string(),
            gate: "context:manual".to_string(),
            result_so_far: serde_json::json!({"message": message}),
            reasoning: message.to_string(),
            created_at: Utc::now(),
            hook_token: token.clone(),
            continuation: continuation.to_string(),
        }
    }

    #[test]
    fn test_save_and_load_context() {
        let (store, _tmp) = test_store();
        let id = Uuid::new_v4();
        let token = test_token(id, "test context", None);
        let ctx = test_context(
            &token,
            "test-agent",
            "test task",
            "hello world",
            "resume_here",
        );

        store.save(&token, &ctx).expect("save");
        let loaded = store.load_by_id(&id).expect("load").expect("found");
        assert_eq!(loaded.agent_id, "test-agent");
        assert_eq!(loaded.task, "test task");
        assert_eq!(loaded.reasoning, "hello world");
        assert_eq!(loaded.continuation, "resume_here");
    }

    #[test]
    fn test_list_contexts() {
        let (store, _tmp) = test_store();

        let id1 = Uuid::new_v4();
        let id2 = Uuid::new_v4();
        let token1 = test_token(id1, "context alpha", None);
        let token2 = test_token(id2, "context beta", Some(3600));
        let ctx1 = test_context(&token1, "agent-1", "task-1", "msg-1", "resume-1");
        let ctx2 = test_context(&token2, "agent-2", "task-2", "msg-2", "resume-2");

        store.save(&token1, &ctx1).expect("save 1");
        store.save(&token2, &ctx2).expect("save 2");

        let pending = store.list_pending().expect("list");
        let manuals: Vec<_> = pending
            .iter()
            .filter(|t| matches!(t.hook_type, HookType::TimerMs(0)))
            .collect();
        assert_eq!(manuals.len(), 2);
    }

    #[test]
    fn test_delete_context() {
        let (store, _tmp) = test_store();
        let id = Uuid::new_v4();
        let token = test_token(id, "to-delete", None);
        let ctx = test_context(&token, "agent", "task", "msg", "resume");
        store.save(&token, &ctx).expect("save");

        assert!(store.load_by_id(&id).expect("load").is_some());
        store.delete(&id).expect("delete");
        assert!(store.load_by_id(&id).expect("load").is_none());
    }

    #[test]
    fn test_context_with_ttl() {
        let (store, _tmp) = test_store();
        let id = Uuid::new_v4();
        let token = test_token(id, "expiring context", Some(60));
        assert_eq!(token.ttl_ms, Some(60_000));

        let ctx = test_context(&token, "agent", "task", "msg", "resume");
        store.save(&token, &ctx).expect("save");

        let loaded = store.load_by_id(&id).expect("load").expect("found");
        assert_eq!(loaded.hook_token.ttl_ms, Some(60_000));
    }

    #[test]
    fn test_atomic_save_no_partial_file() {
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join("hooks");
        std::fs::create_dir_all(&path).unwrap();

        // Simulate crash: write partial tmp file
        let id = Uuid::new_v4();
        std::fs::write(path.join(format!("{}.json.tmp", id)), b"garbage").unwrap();

        let store = ContextStore::with_path(path);
        // Store should still work — overwrites tmp
        let token = test_token(id, "crash recovery", None);
        let ctx = test_context(&token, "agent", "task", "msg", "resume");
        store.save(&token, &ctx).expect("save after crash");

        // tmp should be gone, json exists
        assert!(
            !tmp.path()
                .join("hooks")
                .join(format!("{}.json.tmp", id))
                .exists()
        );
        assert!(
            tmp.path()
                .join("hooks")
                .join(format!("{}.json", id))
                .exists()
        );
    }
}
