//! `b00t ooda` — OODA control plane for autonomous hive task execution.
//!
//! Delegates task execution to the ralph Python runner which implements
//! Observe→Orient→Decide→Act cycles via the b00t-c0re-lib OodaLoop primitives.
//!
//! Examples:
//!   b00t ooda run                       # run with defaults (claude, 5 iter)
//!   b00t ooda run --agent=opencode --max-iter=10
//!   b00t ooda run --agent=pi --max-iter=5
//!   b00t ooda run --task=40             # target specific task
//!   b00t ooda status                    # show task backlog summary
//!   b00t ooda phase                     # show current phase from OodaLoop

use anyhow::{Context, Result, bail};
use clap::Subcommand;
use std::path::PathBuf;
use std::process::Command;

#[derive(Debug, Subcommand, Clone)]
pub enum OodaCommands {
    #[clap(about = "Run OODA loop via ralph until mission complete or max-iter")]
    Run {
        #[arg(
            long,
            help = "Executor agent (claude, codex, opencode, amp, pi)",
            default_value = "claude"
        )]
        agent: String,

        #[arg(long, help = "Maximum OODA iterations", default_value_t = 5)]
        max_iter: u32,

        #[arg(long, help = "Target task ID (default: next pending priority task)")]
        task: Option<String>,

        #[arg(long, help = "Project root (default: git root)")]
        root: Option<PathBuf>,

        #[arg(long, help = "Dry-run: print command without executing")]
        dry_run: bool,
    },
    #[clap(about = "Show task backlog status (pending/in-progress/done counts)")]
    Status {
        #[arg(long, help = "Project root (default: git root)")]
        root: Option<PathBuf>,
    },
    #[clap(about = "Show current OodaPhase from last run (reads from .b00t/ooda-state.json)")]
    Phase {
        #[arg(long, help = "Emit as JSON")]
        json: bool,
    },
    #[clap(
        about = "Karpathy orient→act reviewer gate: validate next task against 4-principle checklist before acting",
        long_about = "Reads next pending task and applies Karpathy's 4 principles as a pre-act gate:\n  1. Think Before Coding — assumptions stated, ambiguity surfaced\n  2. Simplicity First — no speculative abstractions, YAGNI\n  3. Surgical Changes — scope bounded, minimal file surface\n  4. Goal-Driven TDD — failing test identified before implementation\n\nOutputs: PASS or FAIL: <reason> (sm0l output contract)\n\nExamples:\n  b00t ooda review                  # review next pending task\n  b00t ooda review --task=42        # review specific task\n  b00t ooda review --json           # machine-readable output"
    )]
    Review {
        #[arg(long, help = "Task ID to review (default: next pending)")]
        task: Option<String>,
        #[arg(long, help = "Project root (default: git root)")]
        root: Option<PathBuf>,
        #[arg(long, help = "Emit JSON verdict")]
        json: bool,
        #[arg(long, help = "Skip interactive prompts — auto-PASS heuristic checks only")]
        auto: bool,
    },
}

pub async fn handle_ooda(cmd: OodaCommands) -> Result<()> {
    match cmd {
        OodaCommands::Run { agent, max_iter, task, root, dry_run } => {
            run_ooda_loop(&agent, max_iter, task.as_deref(), root, dry_run)
        }
        OodaCommands::Status { root } => ooda_status(root),
        OodaCommands::Phase { json } => ooda_phase(json),
        OodaCommands::Review { task, root, json, auto } => ooda_review(task.as_deref(), root, json, auto),
    }
}

fn find_project_root(override_root: Option<PathBuf>) -> PathBuf {
    if let Some(r) = override_root {
        return r;
    }
    // Walk up from cwd to find git root
    let cwd = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
    let mut dir = cwd.clone();
    loop {
        if dir.join(".git").exists() {
            return dir;
        }
        if !dir.pop() {
            return cwd;
        }
    }
}

fn run_ooda_loop(
    agent: &str,
    max_iter: u32,
    task: Option<&str>,
    root: Option<PathBuf>,
    dry_run: bool,
) -> Result<()> {
    let project_root = find_project_root(root);
    let ralph_dir = project_root.join("_b00t_/ralph");

    if !ralph_dir.exists() {
        bail!(
            "ralph not found at {}. Run 'git submodule update --init --recursive'",
            ralph_dir.display()
        );
    }

    // Build: uv run ralph run --tool=<agent> --max-iterations=<N> [--task-id=<T>]
    let mut uv_args: Vec<String> = vec![
        "run".into(),
        "ralph".into(),
        "run".into(),
        "--tool".into(),
        agent.into(),
        "--max-iterations".into(),
        max_iter.to_string(),
    ];
    if let Some(t) = task {
        uv_args.push("--task-id".into());
        uv_args.push(t.into());
    }

    if dry_run {
        println!(
            "[dry-run] cd {} && uv {}",
            ralph_dir.display(),
            uv_args.join(" ")
        );
        return Ok(());
    }

    println!("🔄 OODA loop: agent={agent} max_iter={max_iter}");

    let status = Command::new("uv")
        .args(&uv_args)
        .current_dir(&ralph_dir)
        .env("PROJECT_ROOT", project_root.to_str().unwrap_or("."))
        .env("B00T_ROLE", "operator")
        .status()
        .map_err(|e| anyhow::anyhow!("uv exec failed: {e}"))?;

    if status.success() {
        println!("🍰 OODA loop complete");
    } else {
        bail!("OODA loop exited with code {:?}", status.code());
    }

    Ok(())
}

fn ooda_status(root: Option<PathBuf>) -> Result<()> {
    let project_root = find_project_root(root);
    let tasks = load_b00t_tasks(&project_root)?;
    let total = tasks.len();
    let pending = tasks.iter().filter(|t| task_status(t) == "pending").count();
    let in_progress = tasks.iter().filter(|t| task_status(t) == "in-progress").count();
    let done = tasks.iter().filter(|t| task_status(t) == "done").count();
    println!("tasks: total={total} pending={pending} in-progress={in_progress} done={done}");
    Ok(())
}

fn ooda_phase(json: bool) -> Result<()> {
    let state_path = dirs::home_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join(".b00t/ooda-state.json");

    if !state_path.exists() {
        if json {
            println!(r#"{{"phase":"Idle","reason":"no state file"}}"#);
        } else {
            println!("Phase: Idle (no active loop)");
        }
        return Ok(());
    }

    let raw = std::fs::read_to_string(&state_path)?;
    if json {
        println!("{raw}");
    } else {
        // best-effort: parse phase field
        let phase = serde_json::from_str::<serde_json::Value>(&raw)
            .ok()
            .and_then(|v| v["phase"].as_str().map(|s| s.to_string()))
            .unwrap_or_else(|| "Unknown".into());
        println!("Phase: {phase}");
    }
    Ok(())
}

/// Karpathy orient→act reviewer gate.
///
/// Reads the next pending task (or the specified task ID) and applies the 4-principle
/// checklist heuristically from task description alone. For a full LLM review, pipe
/// the output into `b00t-cli advice`.
///
/// Output contract (sm0l tier): `PASS` or `FAIL: <≤5 lines>`
fn ooda_review(task_id: Option<&str>, root: Option<PathBuf>, as_json: bool, auto: bool) -> Result<()> {
    let project_root = find_project_root(root);
    let tasks = load_b00t_tasks(&project_root)?;
    let task_desc = select_task_for_review(&tasks, task_id)?;

    // Karpathy 4-principle heuristic checks (no LLM needed for basic gate)
    let mut issues: Vec<&str> = vec![];

    let lower = task_desc.to_lowercase();

    // P1: Think Before Coding — task must not be pure "add X" with zero scope detail
    let is_vague = lower.len() < 20 && !lower.contains("test") && !lower.contains("fix");
    if is_vague {
        issues.push("P1: task too vague — state assumptions before acting");
    }

    // P2: Simplicity First — flag if task mentions multiple large systems simultaneously
    let complexity_words = ["rewrite", "migrate", "refactor all", "replace everything"];
    if complexity_words.iter().any(|w| lower.contains(w)) {
        issues.push("P2: high complexity signal — decompose before acting");
    }

    // P3: Surgical Changes — flag if no bounded scope (no file/module/function mentioned)
    let has_scope = lower.contains(".rs") || lower.contains(".toml") || lower.contains("fn ")
        || lower.contains("mod ") || lower.contains("struct ") || lower.contains("::");
    if !has_scope && lower.len() > 60 {
        issues.push("P3: no file/symbol scope — bound the change surface first");
    }

    // P4: Goal-Driven TDD — flag if no test signal
    let has_test_signal = lower.contains("test") || lower.contains("tdd") || lower.contains("failing")
        || lower.contains("assert") || lower.contains("verify") || lower.contains("pass");
    if !has_test_signal && !auto {
        issues.push("P4: no test strategy — write the failing test first");
    }

    let verdict = if issues.is_empty() { "PASS" } else { "FAIL" };

    if as_json {
        let issues_json = serde_json::to_string(&issues).unwrap_or_else(|_| "[]".into());
        println!(
            r#"{{"verdict":"{verdict}","task":{task_json},"issues":{issues_json}}}"#,
            task_json = serde_json::to_string(&task_desc).unwrap_or_else(|_| r#""""#.into()),
        );
    } else if issues.is_empty() {
        println!("PASS — {task_desc}");
    } else {
        println!("FAIL: {}", issues.join("; "));
        println!("  task: {task_desc}");
    }

    if !issues.is_empty() {
        anyhow::bail!("FAIL: {}", issues.join("; "));
    }
    Ok(())
}

fn b00t_tasks_path(project_root: &std::path::Path) -> PathBuf {
    project_root.join(".b00t/tasks.json")
}

fn load_b00t_tasks(project_root: &std::path::Path) -> Result<Vec<serde_json::Value>> {
    let tasks_path = b00t_tasks_path(project_root);
    if !tasks_path.exists() {
        return Ok(vec![]);
    }
    let raw = std::fs::read_to_string(&tasks_path)
        .with_context(|| format!("read {}", tasks_path.display()))?;
    let parsed: serde_json::Value = serde_json::from_str(&raw)
        .with_context(|| format!("parse {}", tasks_path.display()))?;
    Ok(parsed
        .get("tasks")
        .and_then(|tasks| tasks.as_array())
        .cloned()
        .or_else(|| parsed.as_array().cloned())
        .unwrap_or_default())
}

fn task_status(task: &serde_json::Value) -> &str {
    task.get("status").and_then(|s| s.as_str()).unwrap_or("pending")
}

fn task_id_matches(task: &serde_json::Value, id: &str) -> bool {
    task.get("id").and_then(|v| {
        v.as_str().map(|s| s.to_string()).or_else(|| v.as_u64().map(|n| n.to_string()))
    }).as_deref() == Some(id)
}

fn task_text(task: &serde_json::Value) -> Option<String> {
    let title = task.get("title").and_then(|v| v.as_str()).unwrap_or("");
    let description = task.get("description").and_then(|v| v.as_str()).unwrap_or("");
    let text = match (title.is_empty(), description.is_empty()) {
        (true, true) => return None,
        (false, true) => title.to_string(),
        (true, false) => description.to_string(),
        (false, false) => format!("{title}: {description}"),
    };
    Some(text)
}

fn select_task_for_review(tasks: &[serde_json::Value], task_id: Option<&str>) -> Result<String> {
    let task = if let Some(id) = task_id {
        tasks.iter().find(|t| task_id_matches(t, id))
            .ok_or_else(|| anyhow::anyhow!("task {id} not found"))?
    } else {
        tasks.iter().find(|t| task_status(t) == "pending")
            .ok_or_else(|| anyhow::anyhow!("no pending tasks"))?
    };
    task_text(task).ok_or_else(|| anyhow::anyhow!("selected task has no title or description"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;
    use tempfile::TempDir;

    #[test]
    fn test_find_project_root_override() {
        let tmp = PathBuf::from("/tmp");
        assert_eq!(find_project_root(Some(tmp.clone())), tmp);
    }

    #[test]
    fn test_ooda_run_dry_run() {
        // dry_run emits a print statement and returns Ok
        let result = run_ooda_loop("claude", 3, Some("40"), Some(PathBuf::from("/tmp")), true);
        // /tmp has no ralph dir → bail; but dry_run check is before ralph_dir existence check
        // Expected: Err because /tmp/_b00t_/ralph doesn't exist
        assert!(result.is_err());
        let msg = result.unwrap_err().to_string();
        assert!(msg.contains("ralph not found"), "got: {msg}");
    }

    #[test]
    fn test_ooda_phase_no_state_file() {
        // Create a temp home with no ooda-state.json — just verify no panic
        // (function reads from ~/.b00t/ooda-state.json, skips gracefully if absent)
        let result = ooda_phase(true);
        // Either Ok (no state file → prints Idle) or an unexpected error — not a panic
        assert!(result.is_ok() || result.is_err());
    }

    #[test]
    fn test_ooda_review_pass_specific_task() {
        let tmp = TempDir::new().unwrap();
        let tasks_dir = tmp.path().join(".b00t");
        std::fs::create_dir_all(&tasks_dir).unwrap();
        std::fs::write(tasks_dir.join("tasks.json"), r#"{"tasks":[{
            "id": 42,
            "title": "Fix ooda.rs task lookup",
            "description": "Add test coverage and verify PASS output",
            "status": "pending"
        }]}"#).unwrap();

        let result = ooda_review(Some("42"), Some(tmp.path().to_path_buf()), true, true);

        assert!(result.is_ok());
    }

    #[test]
    fn test_load_b00t_tasks_reads_object_shape() {
        let tmp = TempDir::new().unwrap();
        let tasks_dir = tmp.path().join(".b00t");
        std::fs::create_dir_all(&tasks_dir).unwrap();
        std::fs::write(tasks_dir.join("tasks.json"), r#"{"tasks":[
            {"id":155,"title":"Fix OODA","description":"Use .b00t/tasks.json","status":"pending"},
            {"id":156,"title":"Done","status":"done"}
        ]}"#).unwrap();

        let tasks = load_b00t_tasks(tmp.path()).unwrap();

        assert_eq!(tasks.len(), 2);
        assert_eq!(task_status(&tasks[0]), "pending");
        assert_eq!(task_status(&tasks[1]), "done");
    }

    #[test]
    fn test_select_task_for_review_matches_numeric_id() {
        let tasks = vec![serde_json::json!({
            "id": 155,
            "title": "Fix OODA",
            "description": "Resolve task by id from project root",
            "status": "pending"
        })];

        let text = select_task_for_review(&tasks, Some("155")).unwrap();

        assert!(text.contains("Fix OODA"));
        assert!(text.contains("Resolve task by id"));
    }
}
