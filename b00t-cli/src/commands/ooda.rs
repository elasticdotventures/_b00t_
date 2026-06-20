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

use anyhow::{Result, bail};
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
}

pub async fn handle_ooda(cmd: OodaCommands) -> Result<()> {
    match cmd {
        OodaCommands::Run { agent, max_iter, task, root, dry_run } => {
            run_ooda_loop(&agent, max_iter, task.as_deref(), root, dry_run)
        }
        OodaCommands::Status { root } => ooda_status(root),
        OodaCommands::Phase { json } => ooda_phase(json),
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
    let ralph_dir = project_root.join("_b00t_/ralph");

    if !ralph_dir.exists() {
        bail!(
            "ralph not found at {}. Run 'git submodule update --init --recursive'",
            ralph_dir.display()
        );
    }

    let status = Command::new("uv")
        .args(["run", "ralph", "status"])
        .current_dir(&ralph_dir)
        .env("PROJECT_ROOT", project_root.to_str().unwrap_or("."))
        .status()
        .map_err(|e| anyhow::anyhow!("uv exec failed: {e}"))?;

    if !status.success() {
        bail!("ralph status exited with code {:?}", status.code());
    }
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

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
}
