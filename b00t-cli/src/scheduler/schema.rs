//! Core typed structs matching the SCHEDULER-SCHEMA DDL
//!
//! These types provide strongly-typed Rust representations of the
//! `schedules`, `runs`, and `agents` tables defined in
//! `_b00t_/schema/SCHEDULER-SCHEMA.tomllmd`.
//!
//! # Enums vs raw strings
//!
//! The DDL stores enums as `TEXT CHECK(...)` columns.  This module maps
//! them to proper Rust enums for compile-time safety.  Use the
//! [`convert`](crate::scheduler::convert) module to translate between
//! raw DB rows and these typed structs.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;

// ── Schedule kind ──────────────────────────────────────────────────────────────

/// Timing strategy for a scheduled job.
///
/// Mirrors the DDL `CHECK(schedule_kind IN ('interval','cron','oneshot'))`.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum ScheduleKind {
    /// Runs every `interval_mins` minutes.
    Interval { interval_mins: u32 },
    /// Runs on a 5-field cron expression.
    Cron { cron_expr: String },
    /// Runs exactly once at the given timestamp.
    Oneshot { run_at: DateTime<Utc> },
}

// ── Schedule definition ────────────────────────────────────────────────────────

/// A declarative job definition matching the `schedules` table.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ScheduleDef {
    pub id: String,
    pub name: String,
    pub description: String,
    pub schedule_kind: ScheduleKind,
    pub max_runs: Option<u32>,
    pub run_count: u32,
    pub required_capabilities: Vec<String>,
    pub required_agent: Option<String>,
    pub agent_type: String,
    pub agent_config: Option<Value>,
    pub prompt: Option<String>,
    pub command: Option<String>,
    pub workdir: Option<String>,
    pub enabled: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: Option<DateTime<Utc>>,
}

// ── Run status ─────────────────────────────────────────────────────────────────

/// Execution status of a single run attempt.
///
/// Mirrors the DDL `CHECK(status IN ('claimed','running','success','failed','timed_out','cancelled'))`.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum RunStatus {
    Claimed,
    Running,
    Success,
    Failed,
    TimedOut,
    Cancelled,
}

// ── Run record ─────────────────────────────────────────────────────────────────

/// A single execution of a schedule (row in the `runs` table).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RunRecord {
    pub id: String,
    pub schedule_id: String,
    pub claimed_by: String,
    pub claimed_capability: Option<String>,
    pub status: RunStatus,
    pub started_at: DateTime<Utc>,
    pub finished_at: Option<DateTime<Utc>>,
    pub exit_code: Option<i32>,
    pub output_path: Option<String>,
    pub summary: Option<String>,
    pub error: Option<String>,
}

// ── Agent status ───────────────────────────────────────────────────────────────

/// Agent availability status.
///
/// Mirrors the DDL `CHECK(status IN ('online','offline','busy','error'))`.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum AgentStatus {
    Online,
    Busy,
    Offline,
    Error,
}

// ── Agent registration ─────────────────────────────────────────────────────────

/// Agent registration from the `agents` table.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentRegistration {
    pub id: String,
    pub agent_type: String,
    pub status: AgentStatus,
    pub capabilities: Vec<String>,
    pub label: Option<String>,
    pub last_heartbeat: Option<DateTime<Utc>>,
    pub current_job_id: Option<String>,
    pub current_capability: Option<String>,
    pub metadata: Option<Value>,
}

// ── Display impls ──────────────────────────────────────────────────────────────

use std::fmt;

impl fmt::Display for ScheduleKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ScheduleKind::Interval { interval_mins } => {
                write!(f, "interval ({}m)", interval_mins)
            }
            ScheduleKind::Cron { cron_expr } => write!(f, "cron '{}'", cron_expr),
            ScheduleKind::Oneshot { run_at } => write!(f, "oneshot at {}", run_at),
        }
    }
}

impl fmt::Display for RunStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            RunStatus::Claimed => write!(f, "claimed"),
            RunStatus::Running => write!(f, "running"),
            RunStatus::Success => write!(f, "success"),
            RunStatus::Failed => write!(f, "failed"),
            RunStatus::TimedOut => write!(f, "timed_out"),
            RunStatus::Cancelled => write!(f, "cancelled"),
        }
    }
}

impl fmt::Display for AgentStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            AgentStatus::Online => write!(f, "online"),
            AgentStatus::Busy => write!(f, "busy"),
            AgentStatus::Offline => write!(f, "offline"),
            AgentStatus::Error => write!(f, "error"),
        }
    }
}
