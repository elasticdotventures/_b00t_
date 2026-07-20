//! Conversion between typed Rust structs and rusqlite `Row`s.
//!
//! The `schedules`, `runs`, and `agents` tables store data in SQLite
//! columns (TEXT, INTEGER, etc.).  This module provides `FromRow` trait
//! impls that map raw rows into the typed structs from [`schema`].
//!
//! # Column mapping
//!
//! * **JSON columns** (e.g. `required_capabilities`, `agent_config`, `metadata`)
//!   are stored as SQLite `TEXT` and parsed via `serde_json`.
//! * **Enum columns** (e.g. `schedule_kind`, `status`) are stored as SQLite
//!   `TEXT` and matched against enum variant names.
//! * **DateTime columns** are stored as ISO-8601 RFC 3339 strings and parsed
//!   via `chrono::DateTime::parse_from_rfc3339`.

use crate::scheduler::schema::{
    AgentRegistration, AgentStatus, RunRecord, RunStatus, ScheduleDef, ScheduleKind,
};
use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use rusqlite::{Row, types::ValueRef};
use serde_json::Value;

// ── Helper: parse a JSON array TEXT column into Vec<String> ────────────────────

/// Parse a JSON array string column (or `NULL`) into `Vec<String>`.
/// Returns an empty vec if the column is `None` or empty.
fn parse_json_string_array(val: Option<&str>) -> Vec<String> {
    match val {
        None | Some("") | Some("[]") => Vec::new(),
        Some(json) => serde_json::from_str(json).unwrap_or_else(|_| {
            // Fallback: split by comma (legacy format)
            json.split(',')
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty())
                .collect()
        }),
    }
}

/// Parse a JSON TEXT column into `Option<Value>`.
fn parse_json_value(val: Option<&str>) -> Option<Value> {
    match val {
        None | Some("") => None,
        Some(json) => serde_json::from_str(json).ok(),
    }
}

/// Parse an ISO-8601 TEXT column into `DateTime<Utc>`.
fn parse_datetime(val: Option<&str>, context: &str) -> Result<DateTime<Utc>> {
    let s = val
        .filter(|s| !s.is_empty())
        .with_context(|| format!("missing or empty datetime for {}", context))?;
    DateTime::parse_from_rfc3339(s)
        .map(|dt| dt.with_timezone(&Utc))
        .with_context(|| format!("invalid RFC 3339 datetime '{}' for {}", s, context))
}

fn parse_datetime_opt(val: Option<&str>) -> Option<DateTime<Utc>> {
    val.filter(|s| !s.is_empty())
        .and_then(|s| DateTime::parse_from_rfc3339(s).ok())
        .map(|dt| dt.with_timezone(&Utc))
}

// ── Helper: extract an optional string from a ValueRef ─────────────────────────

#[allow(dead_code)]
fn opt_str<'a>(row: &'a Row, idx: usize) -> Result<Option<&'a str>> {
    match row.get_ref(idx)? {
        ValueRef::Null => Ok(None),
        ValueRef::Text(t) => {
            let s = std::str::from_utf8(t)
                .with_context(|| format!("non-utf8 text in column {}", idx))?;
            if s.is_empty() { Ok(None) } else { Ok(Some(s)) }
        }
        _ => Ok(None),
    }
}

// ── ScheduleKind parsing ───────────────────────────────────────────────────────

/// Parse a `ScheduleKind` from the raw DDL columns.
///
/// `schedule_kind` determines which timing column to read:
///
/// | DDL value   | Schema variant            | Active column       |
/// |-------------|---------------------------|---------------------|
/// | `interval`  | `Interval { interval_mins }` | `interval_mins`   |
/// | `cron`      | `Cron { cron_expr }`      | `cron_expr`         |
/// | `oneshot`   | `Oneshot { run_at }`      | `oneshot_at`        |
pub fn parse_schedule_kind(
    kind_str: &str,
    interval_mins: Option<i64>,
    cron_expr: Option<&str>,
    oneshot_at: Option<&str>,
) -> Result<ScheduleKind> {
    match kind_str {
        "interval" => {
            let mins = interval_mins
                .map(|m| m as u32)
                .with_context(|| "interval_mins is required for interval schedule kind")?;
            Ok(ScheduleKind::Interval {
                interval_mins: mins,
            })
        }
        "cron" => {
            let expr = cron_expr
                .filter(|s| !s.is_empty())
                .with_context(|| "cron_expr is required for cron schedule kind")?;
            Ok(ScheduleKind::Cron {
                cron_expr: expr.to_string(),
            })
        }
        "oneshot" => {
            let raw = oneshot_at
                .filter(|s| !s.is_empty())
                .with_context(|| "oneshot_at is required for oneshot schedule kind")?;
            let run_at = DateTime::parse_from_rfc3339(raw)
                .with_context(|| format!("invalid oneshot_at '{}'", raw))?
                .with_timezone(&Utc);
            Ok(ScheduleKind::Oneshot { run_at })
        }
        other => anyhow::bail!(
            "invalid schedule_kind '{}'; expected 'interval', 'cron', or 'oneshot'",
            other
        ),
    }
}

// ── RunStatus parsing ──────────────────────────────────────────────────────────

/// Parse a `RunStatus` from its DDL text representation.
pub fn parse_run_status(s: &str) -> Result<RunStatus> {
    match s {
        "claimed" => Ok(RunStatus::Claimed),
        "running" => Ok(RunStatus::Running),
        "success" => Ok(RunStatus::Success),
        "failed" => Ok(RunStatus::Failed),
        "timed_out" => Ok(RunStatus::TimedOut),
        "cancelled" => Ok(RunStatus::Cancelled),
        other => anyhow::bail!(
            "invalid run status '{}'; expected one of: claimed, running, success, \
             failed, timed_out, cancelled",
            other
        ),
    }
}

// ── AgentStatus parsing ────────────────────────────────────────────────────────

/// Parse an `AgentStatus` from its DDL text representation.
pub fn parse_agent_status(s: &str) -> Result<AgentStatus> {
    match s {
        "online" => Ok(AgentStatus::Online),
        "busy" => Ok(AgentStatus::Busy),
        "offline" => Ok(AgentStatus::Offline),
        "error" => Ok(AgentStatus::Error),
        other => anyhow::bail!(
            "invalid agent status '{}'; expected one of: online, busy, offline, error",
            other
        ),
    }
}

// ── FromRow trait ──────────────────────────────────────────────────────────────

/// Conversion from a rusqlite `&Row` into a typed struct.
///
/// Implemented for [`ScheduleDef`], [`RunRecord`], and [`AgentRegistration`].
pub trait FromRow: Sized {
    /// Build `Self` from a database row.
    fn from_row(row: &Row) -> Result<Self>;
}

// ── ScheduleDef row extraction ─────────────────────────────────────────────────

impl FromRow for ScheduleDef {
    fn from_row(row: &Row) -> Result<Self> {
        // Columns: 0=id, 1=name, 2=description, 3=schedule_kind,
        // 4=interval_mins, 5=cron_expr, 6=oneshot_at, 7=max_runs,
        // 8=run_count, 9=required_capabilities, 10=required_agent,
        // 11=agent_type, 12=agent_config, 13=prompt, 14=command,
        // 15=workdir, 16=enabled, 17=created_at, 18=updated_at
        let id: String = row.get(0)?;
        let name: String = row.get(1)?;
        let description: String = row.get(2)?;
        let schedule_kind_str: String = row.get(3)?;
        let interval_mins: Option<i64> = row.get(4)?;
        let cron_expr: Option<String> = row.get(5)?;
        let oneshot_at: Option<String> = row.get(6)?;
        let max_runs_raw: i64 = row.get(7)?;
        let run_count_raw: i64 = row.get(8)?;
        let caps_raw: Option<String> = row.get(9)?;
        let required_agent: Option<String> = row.get(10)?;
        let agent_type: String = row.get(11)?;
        let config_raw: Option<String> = row.get(12)?;
        let prompt: Option<String> = row.get(13)?;
        let command: Option<String> = row.get(14)?;
        let workdir: Option<String> = row.get(15)?;
        let enabled_raw: i64 = row.get(16)?;
        let created_at_raw: String = row.get(17)?;
        let updated_at_raw: Option<String> = row.get(18)?;

        // Parse nested fields
        let schedule_kind = parse_schedule_kind(
            &schedule_kind_str,
            interval_mins,
            cron_expr.as_deref(),
            oneshot_at.as_deref(),
        )?;
        let max_runs = if max_runs_raw < 0 {
            None
        } else {
            Some(max_runs_raw as u32)
        };
        let run_count = run_count_raw as u32;
        let required_capabilities = parse_json_string_array(caps_raw.as_deref());
        let agent_config = parse_json_value(config_raw.as_deref());
        let enabled = enabled_raw != 0;
        let created_at = parse_datetime(Some(&created_at_raw), "ScheduleDef.created_at")?;
        let updated_at = parse_datetime_opt(updated_at_raw.as_deref());

        Ok(ScheduleDef {
            id,
            name,
            description,
            schedule_kind,
            max_runs,
            run_count,
            required_capabilities,
            required_agent,
            agent_type,
            agent_config,
            prompt,
            command,
            workdir,
            enabled,
            created_at,
            updated_at,
        })
    }
}

/// Direct extraction from a `&Row` — identical to `FromRow::from_row`.
/// Exists so that `claim.rs` can call it without importing the trait.
pub fn row_to_schedule_def(row: &Row) -> rusqlite::Result<ScheduleDef> {
    FromRow::from_row(row)
        .map_err(|_e| rusqlite::Error::ToSqlConversionFailure("anyhow error".into()))
}

/// Direct extraction from a `&Row` — returns `rusqlite::Result<ScheduleDef>`.
pub fn row_to_schedule_def_direct(row: &Row) -> rusqlite::Result<ScheduleDef> {
    FromRow::from_row(row).map_err(|e| {
        rusqlite::Error::ToSqlConversionFailure(format!("mapping error: {}", e).into())
    })
}

// ── RunRecord row extraction ───────────────────────────────────────────────────

impl FromRow for RunRecord {
    fn from_row(row: &Row) -> Result<Self> {
        // Columns: 0=id, 1=schedule_id, 2=claimed_by, 3=status,
        // 4=started_at, 5=finished_at, 6=exit_code, 7=output_path,
        // 8=summary, 9=error
        let id: String = row.get(0)?;
        let schedule_id: String = row.get(1)?;
        let claimed_by: String = row.get(2)?;
        let status_str: String = row.get(3)?;
        let started_at_raw: Option<String> = row.get(4)?;
        let finished_at_raw: Option<String> = row.get(5)?;
        let exit_code: Option<i32> = row.get(6)?;
        let output_path: Option<String> = row.get(7)?;
        let summary: Option<String> = row.get(8)?;
        let error: Option<String> = row.get(9)?;

        let status = parse_run_status(&status_str)?;
        let started_at = parse_datetime(started_at_raw.as_deref(), "RunRecord.started_at")?;
        let finished_at = parse_datetime_opt(finished_at_raw.as_deref());

        Ok(RunRecord {
            id,
            schedule_id,
            claimed_by,
            claimed_capability: None,
            status,
            started_at,
            finished_at,
            exit_code,
            output_path,
            summary,
            error,
        })
    }
}

/// Direct extraction — convenience wrapper.
pub fn row_to_run_record(row: &Row) -> rusqlite::Result<RunRecord> {
    FromRow::from_row(row).map_err(|e| {
        rusqlite::Error::ToSqlConversionFailure(format!("mapping error: {}", e).into())
    })
}

// ── AgentRegistration row extraction ───────────────────────────────────────────

impl FromRow for AgentRegistration {
    fn from_row(row: &Row) -> Result<Self> {
        // Columns: 0=id, 1=agent_type, 2=status, 3=capabilities,
        // 4=label, 5=last_heartbeat, 6=current_job_id, 7=metadata.
        let id: String = row.get(0)?;
        let agent_type: String = row.get(1)?;
        let status_str: String = row.get(2)?;
        let caps_raw: Option<String> = row.get(3)?;
        let label: Option<String> = row.get(4)?;
        let heartbeat_raw: Option<String> = row.get(5)?;
        let current_job_id: Option<String> = row.get(6)?;
        let metadata_raw: Option<String> = row.get(7)?;

        let status = parse_agent_status(&status_str)?;
        let capabilities = parse_json_string_array(caps_raw.as_deref());
        let last_heartbeat = parse_datetime_opt(heartbeat_raw.as_deref());
        let metadata = parse_json_value(metadata_raw.as_deref());

        Ok(AgentRegistration {
            id,
            agent_type,
            status,
            capabilities,
            label,
            last_heartbeat,
            current_job_id,
            current_capability: None,
            metadata,
        })
    }
}

/// Direct extraction — convenience wrapper.
pub fn row_to_agent_registration(row: &Row) -> rusqlite::Result<AgentRegistration> {
    FromRow::from_row(row).map_err(|e| {
        rusqlite::Error::ToSqlConversionFailure(format!("mapping error: {}", e).into())
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use rusqlite::Connection;

    #[test]
    fn agent_registration_maps_metadata_without_current_capability_column() {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(
            "
            CREATE TABLE agents (
                id              TEXT PRIMARY KEY,
                agent_type      TEXT,
                status          TEXT DEFAULT 'offline',
                capabilities    TEXT DEFAULT '[]',
                label           TEXT,
                last_heartbeat  TEXT,
                current_job_id  TEXT,
                metadata        TEXT
            );
            INSERT INTO agents (
                id, agent_type, status, capabilities, label, last_heartbeat,
                current_job_id, metadata
            ) VALUES (
                'agent_1', 'llm', 'online', '[\"rust\"]', 'builder',
                '2026-05-09T22:00:00Z', 'run_1', '{\"zone\":\"local\"}'
            );
            ",
        )
        .unwrap();

        let agent = conn
            .query_row(
                "SELECT id, agent_type, status, capabilities, label, last_heartbeat,
                        current_job_id, metadata
                 FROM agents WHERE id = 'agent_1'",
                [],
                row_to_agent_registration,
            )
            .unwrap();

        assert_eq!(agent.current_capability, None);
        assert_eq!(agent.metadata.unwrap()["zone"], "local");
    }
}
