//! CLI-level integration tests for `b00t-cli scheduler run` / `scheduler finish`
//!
//! These exercise the real claim protocol (scheduler::claim::try_claim) end
//! to end through the compiled binary against an isolated SQLite DB (via
//! B00T_SCHEDULER_DB) so they never touch the operator's real scheduler.db.
//!
//! Written for gh task #14: the claim protocol existed but nothing in the
//! CLI ever called it — schedules just accumulated with no dispatch. These
//! tests pin down the new `run`/`finish` subcommands that close that gap.

use serde_json::Value;
use std::process::Command;
use tempfile::TempDir;

mod common;

fn isolated_db() -> (TempDir, String) {
    let dir = tempfile::tempdir().expect("tempdir");
    let db_path = dir.path().join("scheduler.db");
    let db_path_str = db_path.to_string_lossy().to_string();
    (dir, db_path_str)
}

fn run_cli(db: &str, args: &[&str]) -> std::process::Output {
    let b00t = common::get_b00t_binary();
    Command::new(&b00t)
        .args(args)
        .env("B00T_SCHEDULER_DB", db)
        .output()
        .expect("failed to execute b00t-cli")
}

#[test]
fn scheduler_run_executes_a_due_shell_job_and_closes_the_run() {
    let (_dir, db) = isolated_db();

    let create = run_cli(
        &db,
        &[
            "scheduler",
            "create",
            "--name",
            "smoke-shell-job",
            "--schedule",
            "interval",
            "--interval",
            "1440",
            "--agent-type",
            "shell",
            "--command",
            "echo hello-from-scheduler-run",
            "--prompt",
            "unused for shell jobs",
        ],
    );
    assert!(
        create.status.success(),
        "create failed: {}",
        String::from_utf8_lossy(&create.stderr)
    );

    // First run: the job was just created (never run) — it is due immediately.
    let run1 = run_cli(&db, &["scheduler", "run", "--json"]);
    assert!(
        run1.status.success(),
        "first run failed: stdout={} stderr={}",
        String::from_utf8_lossy(&run1.stdout),
        String::from_utf8_lossy(&run1.stderr)
    );
    let out1: Value = serde_json::from_slice(&run1.stdout).expect("run1 stdout is JSON");
    assert_eq!(out1["status"], "success");
    assert_eq!(out1["exit_code"], 0);
    assert!(
        out1["summary"]
            .as_str()
            .unwrap()
            .contains("hello-from-scheduler-run"),
        "summary was {out1:?}"
    );

    // Second run, immediately after: interval is 1440 minutes, so the job
    // is not due again yet — try_claim must report NotDue, not re-execute.
    let run2 = run_cli(&db, &["scheduler", "run", "--json"]);
    assert!(run2.status.success());
    let out2: Value = serde_json::from_slice(&run2.stdout).expect("run2 stdout is JSON");
    assert_eq!(out2["status"], "not_due");
}

#[test]
fn scheduler_run_reports_failure_for_nonzero_exit_shell_job() {
    let (_dir, db) = isolated_db();

    let create = run_cli(
        &db,
        &[
            "scheduler",
            "create",
            "--name",
            "smoke-fail-job",
            "--schedule",
            "interval",
            "--interval",
            "60",
            "--agent-type",
            "shell",
            "--command",
            "exit 7",
            "--prompt",
            "unused",
        ],
    );
    assert!(create.status.success());

    let run1 = run_cli(&db, &["scheduler", "run", "--json"]);
    // cmd_run bails (non-zero process exit) when the job itself failed.
    assert!(!run1.status.success(), "expected b00t-cli to exit non-zero on job failure");
    let out1: Value = serde_json::from_slice(&run1.stdout).expect("run1 stdout is JSON");
    assert_eq!(out1["status"], "failed");
    assert_eq!(out1["exit_code"], 7);
}

#[test]
fn scheduler_run_claims_non_shell_job_and_finish_closes_it() {
    let (_dir, db) = isolated_db();

    let create = run_cli(
        &db,
        &[
            "scheduler",
            "create",
            "--name",
            "smoke-llm-job",
            "--schedule",
            "interval",
            "--interval",
            "60",
            "--prompt",
            "summarize the hive status",
        ],
    );
    assert!(create.status.success());

    let run1 = run_cli(&db, &["scheduler", "run", "--json"]);
    assert!(run1.status.success());
    let out1: Value = serde_json::from_slice(&run1.stdout).expect("run1 stdout is JSON");
    assert_eq!(out1["status"], "claimed");
    let run_id = out1["run_id"].as_str().expect("run_id present").to_string();
    assert!(run_id.starts_with("run_"));

    let finish = run_cli(
        &db,
        &[
            "scheduler",
            "finish",
            &run_id,
            "--status",
            "success",
            "--summary",
            "closed manually by test",
        ],
    );
    assert!(
        finish.status.success(),
        "finish failed: {}",
        String::from_utf8_lossy(&finish.stderr)
    );
    assert!(String::from_utf8_lossy(&finish.stdout).contains("marked success"));
}

#[test]
fn scheduler_finish_rejects_invalid_status() {
    let (_dir, db) = isolated_db();
    let finish = run_cli(
        &db,
        &["scheduler", "finish", "run_doesnotexist", "--status", "bogus"],
    );
    assert!(!finish.status.success());
    assert!(
        String::from_utf8_lossy(&finish.stderr).contains("invalid --status"),
        "stderr was: {}",
        String::from_utf8_lossy(&finish.stderr)
    );
}
