//! b00t scheduler — SQLite-backed job scheduling with agent dispatch
//!
//! Storage: `~/.local/share/b00t/scheduler/scheduler.db` (Linux), override with
//! `B00T_SCHEDULER_DB=<path>` (used by tests to avoid touching the real DB).
//! Schema: schedules, runs, agents tables (see _b00t_/datums/SCHEDULER-SCHEMA.tomllmd)
//!
//! 🤓 `create`/`list`/`status`/`show` are definition-only CRUD. `run` claims one due
//!    schedule via `crate::scheduler::claim::try_claim` and, for `agent_type = "shell"`,
//!    executes it synchronously and closes out the run. Other agent types (llm/mcp/
//!    webhook/b00t) are claimed but left for the caller to close via `finish` — no
//!    background daemon or distributed tick loop exists yet (see DESIGN-SCHEDULER.md
//!    for the target distributed-claim model this is a single-shot subset of).

use crate::scheduler::{try_claim, ClaimResult};
use anyhow::{Context, Result};
use clap::Subcommand;
use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;
use uuid::Uuid;

// ── Data types ─────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Schedule {
    pub id: String,
    pub name: String,
    pub description: String,
    pub schedule_kind: String,
    pub interval_mins: Option<i64>,
    pub cron_expr: Option<String>,
    pub oneshot_at: Option<String>,
    pub max_runs: i64,
    pub run_count: i64,
    pub required_capabilities: Option<String>,
    pub required_agent: Option<String>,
    pub agent_type: String,
    pub agent_config: Option<String>,
    pub prompt: String,
    pub command: Option<String>,
    pub workdir: Option<String>,
    pub enabled: bool,
    pub created_at: String,
    pub updated_at: Option<String>,
}

// ── Database connection ────────────────────────────────────────────────────────

pub struct SchedulerDb {
    conn: Connection,
}

impl SchedulerDb {
    /// Resolve the scheduler database path.
    ///
    /// Uses `dirs::data_dir()` which resolves to:
    ///   - Linux:   ~/.local/share/b00t/scheduler/scheduler.db
    ///   - macOS:   ~/Library/Application Support/b00t/scheduler/scheduler.db
    ///   - Windows: {FOLDERID_LocalAppData}/b00t/scheduler/scheduler.db
    pub fn db_path() -> PathBuf {
        // Test/operator seam: point the scheduler at an isolated DB file
        // instead of the real one. Unset in normal operation.
        if let Ok(path) = std::env::var("B00T_SCHEDULER_DB") {
            return PathBuf::from(path);
        }

        let base = dirs::data_dir().unwrap_or_else(|| {
            // Fallback to ~/.local/share/ if dirs fails
            dirs::home_dir()
                .unwrap_or_else(|| PathBuf::from("/tmp"))
                .join(".local/share")
        });
        base.join("b00t").join("scheduler").join("scheduler.db")
    }

    /// Open (or create) the database and run the schema DDL.
    pub fn init() -> Result<Self> {
        let path = Self::db_path();

        // Ensure parent directory exists
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)
                .with_context(|| format!("create scheduler db dir {}", parent.display()))?;
        }

        let conn = Connection::open(&path)
            .with_context(|| format!("open scheduler db {}", path.display()))?;

        // Enable WAL mode for better concurrent access
        conn.execute_batch("PRAGMA journal_mode=WAL;")?;

        // Create tables — schema mirrors _b00t_/datums/SCHEDULER-SCHEMA.tomllmd
        conn.execute_batch(
            "
            CREATE TABLE IF NOT EXISTS schedules (
                id                    TEXT PRIMARY KEY,
                name                  TEXT NOT NULL,
                description           TEXT DEFAULT '',
                schedule_kind         TEXT NOT NULL CHECK(schedule_kind IN ('interval','cron','oneshot')),
                interval_mins         INTEGER,
                cron_expr             TEXT,
                oneshot_at            TEXT,
                max_runs              INTEGER DEFAULT -1,
                run_count             INTEGER DEFAULT 0,
                required_capabilities TEXT,
                required_agent        TEXT,
                agent_type            TEXT DEFAULT 'llm',
                agent_config          TEXT,
                prompt                TEXT NOT NULL,
                command               TEXT,
                workdir               TEXT,
                enabled               INTEGER DEFAULT 1,
                created_at            TEXT NOT NULL,
                updated_at            TEXT
            );

            CREATE TABLE IF NOT EXISTS runs (
                id            TEXT PRIMARY KEY,
                schedule_id   TEXT NOT NULL REFERENCES schedules(id),
                claimed_by    TEXT NOT NULL,
                status        TEXT NOT NULL CHECK(status IN ('claimed','running','success','failed','timed_out','cancelled')),
                started_at    TEXT,
                finished_at   TEXT,
                exit_code     INTEGER,
                output_path   TEXT,
                summary       TEXT,
                error         TEXT
            );

            CREATE TABLE IF NOT EXISTS agents (
                id              TEXT PRIMARY KEY,
                agent_type      TEXT,
                status          TEXT DEFAULT 'offline' CHECK(status IN ('online','offline','busy','error')),
                capabilities    TEXT DEFAULT '[]',
                label           TEXT,
                last_heartbeat  TEXT,
                current_job_id  TEXT,
                metadata        TEXT
            );

            CREATE INDEX IF NOT EXISTS idx_runs_schedule_id ON runs(schedule_id);
            CREATE INDEX IF NOT EXISTS idx_runs_status ON runs(status);
            CREATE INDEX IF NOT EXISTS idx_agents_status ON agents(status);
            ",
        )
        .context("create scheduler schema tables")?;

        Ok(Self { conn })
    }

    /// Insert a new schedule into the database.
    pub fn create_job(
        &self,
        name: &str,
        description: &str,
        schedule_kind: &str,
        interval_mins: Option<i64>,
        cron_expr: Option<&str>,
        oneshot_at: Option<&str>,
        max_runs: i64,
        required_capabilities: Option<&str>,
        required_agent: Option<&str>,
        agent_type: &str,
        agent_config: Option<&str>,
        prompt: &str,
        command: Option<&str>,
        workdir: Option<&str>,
    ) -> Result<Schedule> {
        let id = format!("sched_{}", Uuid::new_v4());
        let now = chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Secs, true);

        self.conn.execute(
            "INSERT INTO schedules (id, name, description, schedule_kind, interval_mins, cron_expr,
             oneshot_at, max_runs, run_count, required_capabilities, required_agent, agent_type,
             agent_config, prompt, command, workdir, enabled, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, 0, ?9, ?10, ?11, ?12, ?13, ?14, ?15, 1, ?16, NULL)",
            params![
                id,
                name,
                description,
                schedule_kind,
                interval_mins,
                cron_expr,
                oneshot_at,
                max_runs,
                required_capabilities,
                required_agent,
                agent_type,
                agent_config,
                prompt,
                command,
                workdir,
                now,
            ],
        ).context("insert schedule")?;

        Ok(Schedule {
            id,
            name: name.to_string(),
            description: description.to_string(),
            schedule_kind: schedule_kind.to_string(),
            interval_mins,
            cron_expr: cron_expr.map(String::from),
            oneshot_at: oneshot_at.map(String::from),
            max_runs,
            run_count: 0,
            required_capabilities: required_capabilities.map(String::from),
            required_agent: required_agent.map(String::from),
            agent_type: agent_type.to_string(),
            agent_config: agent_config.map(String::from),
            prompt: prompt.to_string(),
            command: command.map(String::from),
            workdir: workdir.map(String::from),
            enabled: true,
            created_at: now,
            updated_at: None,
        })
    }

    /// List all schedules, optionally filtered by enabled status.
    pub fn list_jobs(&self, enabled_only: bool) -> Result<Vec<Schedule>> {
        let sql = if enabled_only {
            "SELECT id, name, description, schedule_kind, interval_mins, cron_expr, oneshot_at,
                    max_runs, run_count, required_capabilities, required_agent, agent_type,
                    agent_config, prompt, command, workdir, enabled, created_at, updated_at
             FROM schedules WHERE enabled = 1 ORDER BY created_at DESC"
        } else {
            "SELECT id, name, description, schedule_kind, interval_mins, cron_expr, oneshot_at,
                    max_runs, run_count, required_capabilities, required_agent, agent_type,
                    agent_config, prompt, command, workdir, enabled, created_at, updated_at
             FROM schedules ORDER BY created_at DESC"
        };

        let mut stmt = self.conn.prepare(sql).context("prepare list schedules")?;

        let rows = stmt
            .query_map([], |row| {
                Ok(Schedule {
                    id: row.get(0)?,
                    name: row.get(1)?,
                    description: row.get(2)?,
                    schedule_kind: row.get(3)?,
                    interval_mins: row.get(4)?,
                    cron_expr: row.get(5)?,
                    oneshot_at: row.get(6)?,
                    max_runs: row.get(7)?,
                    run_count: row.get(8)?,
                    required_capabilities: row.get(9)?,
                    required_agent: row.get(10)?,
                    agent_type: row.get(11)?,
                    agent_config: row.get(12)?,
                    prompt: row.get(13)?,
                    command: row.get(14)?,
                    workdir: row.get(15)?,
                    enabled: row.get::<_, i64>(16)? != 0,
                    created_at: row.get(17)?,
                    updated_at: row.get(18)?,
                })
            })
            .context("query schedules")?;

        let mut results = Vec::new();
        for row in rows {
            results.push(row.context("read schedule row")?);
        }
        Ok(results)
    }

    /// Get a single schedule by ID.
    pub fn get_job(&self, schedule_id: &str) -> Result<Schedule> {
        self.conn
            .query_row(
                "SELECT id, name, description, schedule_kind, interval_mins, cron_expr, oneshot_at,
                        max_runs, run_count, required_capabilities, required_agent, agent_type,
                        agent_config, prompt, command, workdir, enabled, created_at, updated_at
                 FROM schedules WHERE id = ?1",
                params![schedule_id],
                |row| {
                    Ok(Schedule {
                        id: row.get(0)?,
                        name: row.get(1)?,
                        description: row.get(2)?,
                        schedule_kind: row.get(3)?,
                        interval_mins: row.get(4)?,
                        cron_expr: row.get(5)?,
                        oneshot_at: row.get(6)?,
                        max_runs: row.get(7)?,
                        run_count: row.get(8)?,
                        required_capabilities: row.get(9)?,
                        required_agent: row.get(10)?,
                        agent_type: row.get(11)?,
                        agent_config: row.get(12)?,
                        prompt: row.get(13)?,
                        command: row.get(14)?,
                        workdir: row.get(15)?,
                        enabled: row.get::<_, i64>(16)? != 0,
                        created_at: row.get(17)?,
                        updated_at: row.get(18)?,
                    })
                },
            )
            .with_context(|| format!("schedule '{schedule_id}' not found"))
    }

    /// Show scheduler status — table row counts.
    pub fn status(&self) -> Result<SchedulerStatus> {
        let schedule_count: i64 =
            self.conn
                .query_row("SELECT COUNT(*) FROM schedules", [], |r| r.get(0))?;
        let enabled_count: i64 = self.conn.query_row(
            "SELECT COUNT(*) FROM schedules WHERE enabled = 1",
            [],
            |r| r.get(0),
        )?;
        let run_count: i64 = self
            .conn
            .query_row("SELECT COUNT(*) FROM runs", [], |r| r.get(0))?;
        let agent_count: i64 = self
            .conn
            .query_row("SELECT COUNT(*) FROM agents", [], |r| r.get(0))?;
        let online_agents: i64 = self.conn.query_row(
            "SELECT COUNT(*) FROM agents WHERE status = 'online'",
            [],
            |r| r.get(0),
        )?;

        Ok(SchedulerStatus {
            schedules_total: schedule_count,
            schedules_enabled: enabled_count,
            runs_total: run_count,
            agents_total: agent_count,
            agents_online: online_agents,
            db_path: Self::db_path().to_string_lossy().to_string(),
        })
    }

    /// Transition a claimed run to `running` (best-effort progress marker;
    /// terminal state is set separately via [`SchedulerDb::update_run`]).
    pub fn set_run_running(&self, run_id: &str) -> Result<()> {
        self.conn
            .execute(
                "UPDATE runs SET status = 'running' WHERE id = ?1",
                params![run_id],
            )
            .context("mark run as running")?;
        Ok(())
    }

    /// Close out a run with a terminal status (success/failed/timed_out/cancelled).
    ///
    /// Sets `finished_at` to now and records exit code / summary / error.
    pub fn update_run(
        &self,
        run_id: &str,
        status: &str,
        exit_code: Option<i32>,
        summary: Option<&str>,
        error: Option<&str>,
    ) -> Result<()> {
        let finished_at = chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Secs, true);
        let rows = self
            .conn
            .execute(
                "UPDATE runs SET status = ?1, finished_at = ?2, exit_code = ?3, summary = ?4, error = ?5 \
                 WHERE id = ?6",
                params![status, finished_at, exit_code, summary, error, run_id],
            )
            .context("update run terminal status")?;
        if rows == 0 {
            anyhow::bail!("run '{run_id}' not found");
        }
        Ok(())
    }
}

#[derive(Debug, Serialize)]
pub struct SchedulerStatus {
    pub schedules_total: i64,
    pub schedules_enabled: i64,
    pub runs_total: i64,
    pub agents_total: i64,
    pub agents_online: i64,
    pub db_path: String,
}

// ── CLI definition ────────────────────────────────────────────────────────────

#[derive(Subcommand, Clone)]
pub enum SchedulerCommands {
    /// Create a new scheduled job
    ///
    /// Examples:
    ///   b00t scheduler create --name 'daily-digest' --schedule cron --cron '0 8 * * *' --prompt 'Generate daily summary'
    ///   b00t scheduler create --name 'health-check' --schedule interval --interval 30 --prompt 'Run health checks'
    ///   b00t scheduler create --name 'deploy' --schedule oneshot --at '2026-06-01T00:00:00Z' --prompt 'Deploy release v2.0'
    Create {
        /// Name for this schedule (required)
        #[arg(long)]
        name: String,

        /// Schedule kind: interval, cron, or oneshot (required)
        #[arg(long, value_name = "KIND")]
        schedule: String,

        /// Interval in minutes (required for schedule=interval)
        #[arg(long)]
        interval: Option<i64>,

        /// Cron expression (required for schedule=cron, e.g. '0 8 * * *')
        #[arg(long)]
        cron: Option<String>,

        /// ISO-8601 timestamp (required for schedule=oneshot, e.g. '2026-06-01T00:00:00Z')
        #[arg(long)]
        at: Option<String>,

        /// Prompt sent to the agent (required)
        #[arg(long)]
        prompt: String,

        /// Optional description
        #[arg(long)]
        description: Option<String>,

        /// Optional shell command to execute instead of agent prompt
        #[arg(long)]
        command: Option<String>,

        /// Working directory for the job
        #[arg(long)]
        workdir: Option<String>,

        /// Maximum number of runs (-1 = unlimited)
        #[arg(long, default_value = "-1")]
        max_runs: i64,

        /// Required agent capabilities (comma-separated or JSON array)
        #[arg(long)]
        capabilities: Option<String>,

        /// Specific agent ID to target
        #[arg(long)]
        agent: Option<String>,

        /// Agent type (default: llm)
        #[arg(long, default_value = "llm")]
        agent_type: String,
    },

    /// List all scheduled jobs
    List {
        /// Only show enabled schedules
        #[arg(long)]
        enabled: bool,

        /// Output as JSON
        #[arg(long)]
        json: bool,
    },

    /// Show scheduler system status
    Status {
        /// Output as JSON
        #[arg(long)]
        json: bool,
    },

    /// Show details for a specific schedule
    Show {
        /// Schedule ID to display
        schedule_id: String,
    },

    /// Claim and (for shell jobs) execute one due schedule
    ///
    /// Atomically claims the oldest due, enabled schedule via the claim
    /// protocol (see `scheduler::claim::try_claim`). `agent_type = "shell"`
    /// jobs run synchronously and the run is closed out automatically.
    /// Other agent types are left `running` — close them with `finish`.
    ///
    /// Examples:
    ///   b00t scheduler run
    ///   b00t scheduler run --agent-id my-agent --capabilities grok,hive
    Run {
        /// Agent ID claiming the job (default: "b00t-cli-<pid>")
        #[arg(long)]
        agent_id: Option<String>,

        /// Capabilities this agent offers (comma-separated)
        #[arg(long)]
        capabilities: Option<String>,

        /// Output as JSON
        #[arg(long)]
        json: bool,
    },

    /// Manually close out a claimed/running run (for non-shell agent types)
    ///
    /// Example:
    ///   b00t scheduler finish run_abc123 --status success --summary "handled by hermes"
    Finish {
        /// Run ID to close out (printed by `scheduler run`)
        run_id: String,

        /// Terminal status: success, failed, timed_out, or cancelled
        #[arg(long)]
        status: String,

        /// Process exit code, if applicable
        #[arg(long)]
        exit_code: Option<i32>,

        /// Short summary for the dashboard
        #[arg(long)]
        summary: Option<String>,

        /// Error message, if status is failed/timed_out
        #[arg(long)]
        error: Option<String>,
    },
}

// ── Handlers ──────────────────────────────────────────────────────────────────

pub fn handle_scheduler_command(cmd: &SchedulerCommands) -> Result<()> {
    match cmd {
        SchedulerCommands::Create {
            name,
            schedule,
            interval,
            cron,
            at,
            prompt,
            description,
            command,
            workdir,
            max_runs,
            capabilities,
            agent,
            agent_type,
        } => cmd_create(
            name,
            schedule,
            *interval,
            cron.as_deref(),
            at.as_deref(),
            prompt,
            description.as_deref(),
            command.as_deref(),
            workdir.as_deref(),
            *max_runs,
            capabilities.as_deref(),
            agent.as_deref(),
            agent_type,
        ),
        SchedulerCommands::List { enabled, json } => cmd_list(*enabled, *json),
        SchedulerCommands::Status { json } => cmd_status(*json),
        SchedulerCommands::Show { schedule_id } => cmd_show(schedule_id),
        SchedulerCommands::Run {
            agent_id,
            capabilities,
            json,
        } => cmd_run(agent_id.as_deref(), capabilities.as_deref(), *json),
        SchedulerCommands::Finish {
            run_id,
            status,
            exit_code,
            summary,
            error,
        } => cmd_finish(
            run_id,
            status,
            *exit_code,
            summary.as_deref(),
            error.as_deref(),
        ),
    }
}

fn cmd_create(
    name: &str,
    schedule_kind: &str,
    interval_mins: Option<i64>,
    cron_expr: Option<&str>,
    oneshot_at: Option<&str>,
    prompt: &str,
    description: Option<&str>,
    command: Option<&str>,
    workdir: Option<&str>,
    max_runs: i64,
    capabilities: Option<&str>,
    required_agent: Option<&str>,
    agent_type: &str,
) -> Result<()> {
    // Validate schedule_kind
    let kind = schedule_kind.to_lowercase();
    if !["interval", "cron", "oneshot"].contains(&kind.as_str()) {
        anyhow::bail!(
            "invalid schedule kind '{}'; must be 'interval', 'cron', or 'oneshot'",
            schedule_kind
        );
    }

    // Validate required fields per kind
    match kind.as_str() {
        "interval" => {
            if interval_mins.is_none() || interval_mins.unwrap() <= 0 {
                anyhow::bail!("--interval is required and must be > 0 for schedule=interval");
            }
        }
        "cron" => {
            if cron_expr.is_none() || cron_expr.unwrap().trim().is_empty() {
                anyhow::bail!("--cron is required for schedule=cron");
            }
        }
        "oneshot" => {
            if oneshot_at.is_none() || oneshot_at.unwrap().trim().is_empty() {
                anyhow::bail!("--at is required for schedule=oneshot");
            }
        }
        _ => unreachable!(),
    }

    let db = SchedulerDb::init().context("initialize scheduler database")?;

    let schedule = db.create_job(
        name,
        description.unwrap_or(""),
        &kind,
        interval_mins,
        cron_expr,
        oneshot_at,
        max_runs,
        capabilities,
        required_agent,
        agent_type,
        None, // agent_config — reserved for future use
        prompt,
        command,
        workdir,
    )?;

    println!("Created schedule:");
    println!("  ID:      {}", schedule.id);
    println!("  Name:    {}", schedule.name);
    println!("  Kind:    {}", schedule.schedule_kind);
    println!("  Prompt:  {}", schedule.prompt);
    println!("  Enabled: {}", if schedule.enabled { "yes" } else { "no" });
    println!("  DB:      {}", SchedulerDb::db_path().display());

    Ok(())
}

fn cmd_list(enabled_only: bool, json: bool) -> Result<()> {
    let db = SchedulerDb::init().context("initialize scheduler database")?;
    let schedules = db.list_jobs(enabled_only)?;

    if json {
        println!("{}", serde_json::to_string_pretty(&schedules)?);
        return Ok(());
    }

    if schedules.is_empty() {
        println!(
            "No schedules found{}",
            if enabled_only { " (enabled only)" } else { "" }
        );
        return Ok(());
    }

    for s in &schedules {
        let enabled_mark = if s.enabled { "active" } else { "paused" };
        let timing = match s.schedule_kind.as_str() {
            "interval" => format!("every {}m", s.interval_mins.unwrap_or(0)),
            "cron" => format!("cron '{}'", s.cron_expr.as_deref().unwrap_or("")),
            "oneshot" => format!("at {}", s.oneshot_at.as_deref().unwrap_or("")),
            _ => s.schedule_kind.clone(),
        };
        println!(
            "[{}] {} | {} | {} | runs: {}/{} | {}",
            enabled_mark,
            s.id,
            s.name,
            timing,
            s.run_count,
            if s.max_runs < 0 {
                "unlimited".into()
            } else {
                s.max_runs.to_string()
            },
            if s.prompt.len() > 60 {
                format!("{}...", &s.prompt[..57])
            } else {
                s.prompt.clone()
            },
        );
    }

    println!("\nTotal: {} schedule(s)", schedules.len());
    Ok(())
}

fn cmd_status(json: bool) -> Result<()> {
    let db = SchedulerDb::init().context("initialize scheduler database")?;
    let status = db.status()?;

    if json {
        println!("{}", serde_json::to_string_pretty(&status)?);
        return Ok(());
    }

    println!("Scheduler Status");
    println!("  DB path:       {}", status.db_path);
    println!(
        "  Schedules:     {} total, {} enabled",
        status.schedules_total, status.schedules_enabled
    );
    println!("  Runs:          {}", status.runs_total);
    println!(
        "  Agents:        {} total, {} online",
        status.agents_total, status.agents_online
    );

    Ok(())
}

fn cmd_show(schedule_id: &str) -> Result<()> {
    let db = SchedulerDb::init().context("initialize scheduler database")?;
    let s = db.get_job(schedule_id)?;

    println!("Schedule: {}", s.id);
    println!("  Name:        {}", s.name);
    println!("  Description: {}", s.description);
    println!("  Kind:        {}", s.schedule_kind);
    match s.schedule_kind.as_str() {
        "interval" => println!("  Interval:    {} minutes", s.interval_mins.unwrap_or(0)),
        "cron" => println!("  Cron:        {}", s.cron_expr.as_deref().unwrap_or("")),
        "oneshot" => println!("  At:          {}", s.oneshot_at.as_deref().unwrap_or("")),
        _ => {}
    }
    println!(
        "  Max runs:    {}",
        if s.max_runs < 0 {
            "unlimited".into()
        } else {
            s.max_runs.to_string()
        }
    );
    println!("  Run count:   {}", s.run_count);
    println!("  Enabled:     {}", if s.enabled { "yes" } else { "no" });
    println!("  Agent type:  {}", s.agent_type);
    if let Some(caps) = &s.required_capabilities {
        println!("  Capabilities: {}", caps);
    }
    if let Some(agent) = &s.required_agent {
        println!("  Agent:       {}", agent);
    }
    if let Some(cmd) = &s.command {
        println!("  Command:     {}", cmd);
    }
    if let Some(wd) = &s.workdir {
        println!("  Workdir:     {}", wd);
    }
    println!("  Prompt:");
    for line in s.prompt.lines() {
        println!("    {}", line);
    }
    println!("  Created:     {}", s.created_at);
    if let Some(upd) = &s.updated_at {
        println!("  Updated:     {}", upd);
    }

    Ok(())
}

/// Truncate a string to at most `max_chars` characters (char-boundary safe).
fn truncate_chars(s: &str, max_chars: usize) -> String {
    s.chars().take(max_chars).collect()
}

fn cmd_run(agent_id: Option<&str>, capabilities: Option<&str>, json: bool) -> Result<()> {
    let db = SchedulerDb::init().context("initialize scheduler database")?;

    let agent_id = agent_id
        .map(String::from)
        .unwrap_or_else(|| format!("b00t-cli-{}", std::process::id()));

    let caps: Vec<String> = capabilities
        .map(|c| {
            c.split(',')
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty())
                .collect()
        })
        .unwrap_or_default();

    let result = try_claim(&db, &agent_id, &caps).context("claim due schedule")?;

    match result {
        ClaimResult::NotDue => {
            if json {
                println!("{}", serde_json::json!({"status": "not_due"}));
            } else {
                println!("No due jobs.");
            }
            Ok(())
        }
        ClaimResult::CapabilityMismatch => {
            if json {
                println!("{}", serde_json::json!({"status": "capability_mismatch"}));
            } else {
                println!(
                    "Due job(s) exist but require capabilities agent '{agent_id}' doesn't offer."
                );
            }
            Ok(())
        }
        ClaimResult::AlreadyClaimed => {
            if json {
                println!("{}", serde_json::json!({"status": "already_claimed"}));
            } else {
                println!("Job was already claimed by another agent — nothing to do.");
            }
            Ok(())
        }
        ClaimResult::Claimed { schedule, run_id } => {
            db.set_run_running(&run_id)
                .context("mark run as running")?;

            if schedule.agent_type == "shell" {
                run_shell_job(&db, &schedule, &run_id, json)
            } else {
                if json {
                    println!(
                        "{}",
                        serde_json::json!({
                            "status": "claimed",
                            "run_id": run_id,
                            "schedule_id": schedule.id,
                            "agent_type": schedule.agent_type,
                            "prompt": schedule.prompt,
                        })
                    );
                } else {
                    println!(
                        "Claimed '{}' ({}) — agent_type '{}' has no inline executor in this CLI.",
                        schedule.name, schedule.id, schedule.agent_type
                    );
                    println!("  run_id: {run_id}");
                    if let Some(prompt) = &schedule.prompt {
                        println!("  prompt: {prompt}");
                    }
                    println!(
                        "  Close it with: b00t-cli scheduler finish {run_id} --status <success|failed> --summary '...'"
                    );
                }
                Ok(())
            }
        }
    }
}

/// Execute a claimed `agent_type = "shell"` job synchronously and close its run.
fn run_shell_job(
    db: &SchedulerDb,
    schedule: &crate::scheduler::ScheduleDef,
    run_id: &str,
    json: bool,
) -> Result<()> {
    let Some(cmd) = &schedule.command else {
        db.update_run(
            run_id,
            "failed",
            None,
            None,
            Some("agent_type=shell but no command set"),
        )?;
        anyhow::bail!(
            "schedule '{}' is agent_type=shell but has no command",
            schedule.name
        );
    };

    let mut command = std::process::Command::new("sh");
    command.arg("-c").arg(cmd);
    if let Some(wd) = &schedule.workdir {
        command.current_dir(wd);
    }

    let output = command
        .output()
        .with_context(|| format!("execute shell job command: {cmd}"))?;

    let exit_code = output.status.code().unwrap_or(-1);
    let mut combined = String::from_utf8_lossy(&output.stdout).to_string();
    combined.push_str(&String::from_utf8_lossy(&output.stderr));
    let summary = truncate_chars(&combined, 2000);
    let status = if output.status.success() {
        "success"
    } else {
        "failed"
    };
    let error = if output.status.success() {
        None
    } else {
        Some(summary.clone())
    };

    db.update_run(run_id, status, Some(exit_code), Some(&summary), error.as_deref())
        .context("close out shell run")?;

    if json {
        println!(
            "{}",
            serde_json::json!({
                "status": status,
                "run_id": run_id,
                "schedule_id": schedule.id,
                "exit_code": exit_code,
                "summary": summary,
            })
        );
    } else {
        println!("Ran '{}' ({})", schedule.name, schedule.id);
        println!("  run_id:    {run_id}");
        println!("  status:    {status}");
        println!("  exit_code: {exit_code}");
        println!("  output:    {summary}");
    }

    if status == "failed" {
        anyhow::bail!(
            "job '{}' failed with exit code {}",
            schedule.name,
            exit_code
        );
    }
    Ok(())
}

fn cmd_finish(
    run_id: &str,
    status: &str,
    exit_code: Option<i32>,
    summary: Option<&str>,
    error: Option<&str>,
) -> Result<()> {
    const VALID: [&str; 4] = ["success", "failed", "timed_out", "cancelled"];
    if !VALID.contains(&status) {
        anyhow::bail!("invalid --status '{status}'; must be one of {VALID:?}");
    }

    let db = SchedulerDb::init().context("initialize scheduler database")?;
    db.update_run(run_id, status, exit_code, summary, error)
        .context("finish run")?;

    println!("Run {run_id} marked {status}");
    Ok(())
}
