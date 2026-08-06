//! `b00t maintenance` — start/status/stop the b00t maintenance daemon.
//!
//! The daemon:
//!   1. Initializes GovernanceRuntime (background tokio loop, hook ring, expired-hook recovery)
//!   2. Seeds the exercise reminder schedule if not present (interval=15, prompt="⏰ Time to move!")
//!   3. Writes a PID file to ~/.local/share/b00t/maintenance.pid
//!   4. Prints status to stdout (machine-parseable JSON for /b00t slash command)
//!   5. On stop: emits stop event via GovernanceRuntime::emit_stop_event()

use anyhow::{Context, Result};
use serde_json::json;

#[derive(Debug, clap::Args)]
pub struct MaintenanceArgs {
    #[command(subcommand)]
    pub cmd: MaintenanceCmd,
}

#[derive(Debug, clap::Subcommand)]
pub enum MaintenanceCmd {
    /// Start the maintenance daemon (foreground; use systemd/nohup for background)
    Start {
        /// Override exercise reminder interval in minutes (default: 15)
        #[arg(long, default_value_t = 15)]
        interval_mins: i64,
    },
    /// Print daemon status as JSON
    Status,
    /// Stop the daemon (sends SIGTERM to PID file process)
    Stop,
}

fn pid_path() -> std::path::PathBuf {
    dirs::data_dir()
        .unwrap_or_default()
        .join("b00t/maintenance.pid")
}

fn read_pid() -> Option<u32> {
    let path = pid_path();
    let content = std::fs::read_to_string(&path).ok()?;
    content.trim().parse::<u32>().ok()
}

pub async fn handle_maintenance_command(args: &MaintenanceArgs) -> Result<()> {
    match &args.cmd {
        MaintenanceCmd::Status => cmd_status(),
        MaintenanceCmd::Start { interval_mins } => cmd_start(*interval_mins).await,
        MaintenanceCmd::Stop => cmd_stop(),
    }
}

fn cmd_status() -> Result<()> {
    let pid_file = pid_path();

    match read_pid() {
        None => {
            println!("{}", json!({"running": false}));
        }
        Some(pid) => {
            let proc_exists = std::path::Path::new(&format!("/proc/{pid}")).exists();
            if proc_exists {
                // Try to compute uptime from PID file mtime
                let uptime_secs: Option<u64> = pid_file
                    .metadata()
                    .ok()
                    .and_then(|m| m.modified().ok())
                    .and_then(|mtime| {
                        std::time::SystemTime::now()
                            .duration_since(mtime)
                            .ok()
                            .map(|d| d.as_secs())
                    });

                // Count exercise reminder jobs in scheduler
                let reminder_count = crate::commands::scheduler::SchedulerDb::init()
                    .ok()
                    .and_then(|db| db.list_jobs(true).ok())
                    .map(|jobs| {
                        jobs.iter()
                            .filter(|j| j.name == "b00t-exercise-reminder")
                            .count()
                    })
                    .unwrap_or(0);

                println!(
                    "{}",
                    json!({
                        "running": true,
                        "pid": pid,
                        "uptime_secs": uptime_secs,
                        "exercise_reminder_jobs": reminder_count,
                    })
                );
            } else {
                // Stale PID file — clean up silently
                let _ = std::fs::remove_file(&pid_file);
                println!("{}", json!({"running": false, "stale_pid": pid}));
            }
        }
    }
    Ok(())
}

async fn cmd_start(interval_mins: i64) -> Result<()> {
    // 1. Write PID file
    let pid = std::process::id();
    let pid_file = pid_path();
    if let Some(parent) = pid_file.parent() {
        std::fs::create_dir_all(parent).context("create b00t data dir")?;
    }
    std::fs::write(&pid_file, pid.to_string()).context("write maintenance.pid")?;

    // 2. Init GovernanceRuntime (starts background tokio loop, hook ring, expired-hook recovery)
    let gov = crate::governance::GovernanceRuntime::init()
        .await
        .context("init GovernanceRuntime")?;

    // 3. Seed exercise reminder if not present
    let db = crate::commands::scheduler::SchedulerDb::init().context("init SchedulerDb")?;
    let jobs = db.list_jobs(false).context("list jobs")?;
    if !jobs.iter().any(|j| j.name == "b00t-exercise-reminder") {
        db.create_job(
            "b00t-exercise-reminder",
            "15-min exercise reminder — confirm with <|👍🏻|> to earn cake lottery ticket",
            "interval",
            Some(interval_mins),
            None,
            None,
            -1,
            None,
            None,
            "llm",
            None,
            "⏰ Time to move! Stand up, stretch, or do 10 jumping jacks.\nRespond <|👍🏻|> to confirm (earns a cake lottery ticket) or <|👎🏻|> to skip.",
            None,
            None,
        )
        .context("seed exercise reminder")?;
        println!("✅ Seeded exercise reminder every {interval_mins} min");
    }

    // 4. Print status JSON
    println!(
        "{}",
        json!({"running": true, "pid": pid, "interval_mins": interval_mins})
    );

    // 5. Wait for SIGTERM / Ctrl-C
    tokio::signal::ctrl_c().await.context("wait for ctrl-c")?;

    gov.emit_stop_event();
    let _ = std::fs::remove_file(&pid_file);
    println!("🛑 Maintenance daemon stopped");

    Ok(())
}

fn cmd_stop() -> Result<()> {
    let pid = read_pid()
        .ok_or_else(|| anyhow::anyhow!("No maintenance.pid found — daemon not running"))?;

    // Send SIGTERM using the `kill` command (avoids nix dependency)
    let status = std::process::Command::new("kill")
        .args(["-TERM", &pid.to_string()])
        .status()
        .context("send SIGTERM")?;

    if status.success() {
        println!("{}", json!({"stopped": true, "pid": pid}));
    } else {
        anyhow::bail!("kill -TERM {pid} failed: {status}");
    }

    Ok(())
}
