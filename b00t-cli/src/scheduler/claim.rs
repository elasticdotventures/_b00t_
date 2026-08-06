//! Claim protocol — agents claim due schedules from the database.
//!
//! An agent calls [`try_claim`] to atomically:
//!
//! 1. Find an eligible (enabled, due, not exceeded max_runs) schedule.
//! 2. Create a `runs` row with status `claimed`.
//! 3. Return the schedule to the caller.
//!
//! All steps execute inside an SQLite `IMMEDIATE` transaction over a
//! private connection to the shared scheduler database file to prevent
//! double-claiming under concurrent agent access.

use crate::commands::scheduler::SchedulerDb;
use crate::scheduler::convert::row_to_schedule_def_direct;
use crate::scheduler::schema::{ScheduleDef, ScheduleKind};
use anyhow::{Context, Result};
use chrono::Utc;
use rusqlite::{Connection, params};
use uuid::Uuid;

// ── Claim result ───────────────────────────────────────────────────────────────

/// Outcome of a `try_claim` call.
#[derive(Debug, Clone, PartialEq)]
pub enum ClaimResult {
    /// A schedule was successfully claimed. `run_id` identifies the `runs`
    /// row `try_claim` already inserted (status `claimed`) — callers use it
    /// to transition the run to `running` and later to a terminal status via
    /// `SchedulerDb::set_run_running` / `SchedulerDb::update_run`.
    Claimed { schedule: ScheduleDef, run_id: String },
    /// No schedule is currently due (or all schedules exhausted).
    NotDue,
    /// The agent's capabilities do not match any eligible schedule.
    CapabilityMismatch,
    /// Another agent already claimed the only eligible schedule.
    AlreadyClaimed,
}

// ── Public API ─────────────────────────────────────────────────────────────────

/// Attempt to claim a due schedule for the given agent.
///
/// Opens a private SQLite connection in `IMMEDIATE` transaction mode so
/// that concurrent callers (from separate processes or threads) are
/// serialized at the database level.
///
/// # Parameters
///
/// * `_db` — The established `SchedulerDb` handle (used only to resolve
///   the DB path via `SchedulerDb::db_path()`).
/// * `agent_id` — The claiming agent's identifier.
/// * `capabilities` — The agent's capability list for matching against
///   schedule `required_capabilities`.
///
/// # Returns
///
/// One of the [`ClaimResult`] variants.  Internal SQL errors are returned
/// as `Err`.
pub fn try_claim(
    _db: &SchedulerDb,
    agent_id: &str,
    capabilities: &[String],
) -> Result<ClaimResult> {
    let path = SchedulerDb::db_path();

    // Open a fresh connection with WAL and IMMEDIATE transaction
    let conn = Connection::open(&path)
        .with_context(|| format!("open scheduler db for claim at {}", path.display()))?;
    conn.execute_batch("PRAGMA journal_mode=WAL;")
        .context("set WAL mode on claim connection")?;

    // Acquire an IMMEDIATE transaction to serialize concurrent claims.
    conn.execute_batch("BEGIN IMMEDIATE")
        .context("begin immediate transaction in try_claim")?;

    let claim_result = try_claim_inner(&conn, agent_id, capabilities);

    match &claim_result {
        Ok(ClaimResult::Claimed { .. })
        | Ok(ClaimResult::NotDue)
        | Ok(ClaimResult::CapabilityMismatch)
        | Ok(ClaimResult::AlreadyClaimed) => {
            conn.execute_batch("COMMIT")
                .context("commit claim transaction")?;
        }
        Err(_) => {
            let _ = conn.execute_batch("ROLLBACK");
        }
    }

    claim_result
}

// ── Inner logic ────────────────────────────────────────────────────────────────

/// Inner claim logic — runs inside the IMMEDIATE transaction.
fn try_claim_inner(
    conn: &Connection,
    agent_id: &str,
    capabilities: &[String],
) -> Result<ClaimResult> {
    // 1. Find eligible schedules (enabled, not exceeding max_runs).
    let sql = "SELECT id, name, description, schedule_kind, interval_mins, cron_expr, \
               oneshot_at, max_runs, run_count, required_capabilities, required_agent, \
               agent_type, agent_config, prompt, command, workdir, enabled, created_at, updated_at \
               FROM schedules \
               WHERE enabled = 1 \
                 AND (max_runs = -1 OR run_count < max_runs) \
               ORDER BY created_at ASC";

    let mut stmt = conn
        .prepare(sql)
        .context("prepare eligible schedules query in try_claim")?;

    let rows = stmt
        .query_map([], |row| row_to_schedule_def_direct(row))
        .context("query eligible schedules in try_claim")?;

    let mut candidates: Vec<ScheduleDef> = Vec::new();
    for row in rows {
        candidates.push(row.context("read schedule row in try_claim")?);
    }

    if candidates.is_empty() {
        return Ok(ClaimResult::NotDue);
    }

    // 2. Filter by capability match and schedule kind due-ness.
    let now = Utc::now();

    for candidate in &candidates {
        // Capability check: if the schedule requires capabilities, the agent
        // must have at least one matching capability.
        if !candidate.required_capabilities.is_empty() {
            let has_capability = capabilities
                .iter()
                .any(|cap| candidate.required_capabilities.contains(cap));
            if !has_capability {
                continue;
            }
        }

        // Schedule kind due check
        let is_due = match &candidate.schedule_kind {
            ScheduleKind::Interval { interval_mins } => {
                let last_run = get_last_run_time(conn, &candidate.id)?;
                match last_run {
                    Some(last) => {
                        let elapsed = (now - last).num_minutes() as u32;
                        elapsed >= *interval_mins
                    }
                    None => true, // never run — due
                }
            }
            ScheduleKind::Cron { .. } => {
                // Cron: always consider due for now.
                // A full cron expression parser should be added later.
                true
            }
            ScheduleKind::Oneshot { run_at } => {
                if candidate.run_count > 0 {
                    false // already ran
                } else {
                    now >= *run_at
                }
            }
        };

        if !is_due {
            continue;
        }

        // 3. Claim this schedule: insert a runs row, increment run_count.
        let run_id = format!("run_{}", Uuid::new_v4());
        let started_at = now.to_rfc3339_opts(chrono::SecondsFormat::Secs, true);

        conn.execute(
            "INSERT INTO runs (id, schedule_id, claimed_by, status, started_at) \
             VALUES (?1, ?2, ?3, 'claimed', ?4)",
            params![run_id, candidate.id, agent_id, started_at],
        )
        .context("insert run record in try_claim")?;

        conn.execute(
            "UPDATE schedules SET run_count = run_count + 1, updated_at = ?1 WHERE id = ?2",
            params![started_at, candidate.id],
        )
        .context("increment run_count in try_claim")?;

        let mut claimed = candidate.clone();
        claimed.run_count += 1;

        return Ok(ClaimResult::Claimed {
            schedule: claimed,
            run_id,
        });
    }

    // Determine why we didn't claim.
    let has_capability_match = candidates.iter().any(|c| {
        if c.required_capabilities.is_empty() {
            return true;
        }
        capabilities
            .iter()
            .any(|cap| c.required_capabilities.contains(cap))
    });

    if !has_capability_match {
        Ok(ClaimResult::CapabilityMismatch)
    } else {
        Ok(ClaimResult::NotDue)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Duration;
    use rusqlite::Connection;

    fn create_test_schema(conn: &Connection) {
        conn.execute_batch(
            "
            CREATE TABLE schedules (
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

            CREATE TABLE runs (
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
            ",
        )
        .unwrap();
    }

    #[test]
    fn interval_schedule_not_due_returns_not_due() {
        let conn = Connection::open_in_memory().unwrap();
        create_test_schema(&conn);
        let now = Utc::now();
        let created_at =
            (now - Duration::minutes(10)).to_rfc3339_opts(chrono::SecondsFormat::Secs, true);
        let started_at = now.to_rfc3339_opts(chrono::SecondsFormat::Secs, true);

        conn.execute(
            "INSERT INTO schedules (id, name, schedule_kind, interval_mins, max_runs, run_count, prompt, created_at)
             VALUES ('sched_1', 'recent interval', 'interval', 60, -1, 1, 'do work', ?1)",
            params![created_at],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO runs (id, schedule_id, claimed_by, status, started_at)
             VALUES ('run_1', 'sched_1', 'agent_1', 'success', ?1)",
            params![started_at],
        )
        .unwrap();

        let result = try_claim_inner(&conn, "agent_2", &[]).unwrap();
        assert_eq!(result, ClaimResult::NotDue);
    }

    #[test]
    fn never_run_interval_schedule_is_claimed_and_returns_run_id() {
        let conn = Connection::open_in_memory().unwrap();
        create_test_schema(&conn);
        let created_at = (Utc::now() - Duration::minutes(10))
            .to_rfc3339_opts(chrono::SecondsFormat::Secs, true);

        conn.execute(
            "INSERT INTO schedules (id, name, schedule_kind, interval_mins, max_runs, run_count, prompt, created_at)
             VALUES ('sched_2', 'never run', 'interval', 60, -1, 0, 'do work', ?1)",
            params![created_at],
        )
        .unwrap();

        let result = try_claim_inner(&conn, "agent_1", &[]).unwrap();
        match result {
            ClaimResult::Claimed { schedule, run_id } => {
                assert_eq!(schedule.id, "sched_2");
                assert_eq!(schedule.run_count, 1);
                assert!(run_id.starts_with("run_"), "run_id was {run_id:?}");

                // The runs row try_claim inserted must be readable back by that id.
                let status: String = conn
                    .query_row(
                        "SELECT status FROM runs WHERE id = ?1",
                        params![run_id],
                        |r| r.get(0),
                    )
                    .unwrap();
                assert_eq!(status, "claimed");
            }
            other => panic!("expected Claimed, got {other:?}"),
        }

        // A second claim attempt immediately after must not double-claim —
        // the schedule is no longer due (interval hasn't elapsed since the
        // run we just recorded).
        let second = try_claim_inner(&conn, "agent_2", &[]).unwrap();
        assert_eq!(second, ClaimResult::NotDue);
    }
}

/// Get the most recent run's `started_at` for a schedule, if any.
fn get_last_run_time(
    conn: &Connection,
    schedule_id: &str,
) -> Result<Option<chrono::DateTime<Utc>>> {
    let result: Result<Option<String>, _> = conn.query_row(
        "SELECT started_at FROM runs WHERE schedule_id = ?1 \
         ORDER BY started_at DESC LIMIT 1",
        params![schedule_id],
        |row| row.get::<_, Option<String>>(0),
    );

    match result {
        Ok(Some(ts)) => {
            let dt = chrono::DateTime::parse_from_rfc3339(&ts)
                .map(|d| d.with_timezone(&Utc))
                .ok();
            Ok(dt)
        }
        Ok(None) => Ok(None),
        Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
        Err(e) => Err(e.into()),
    }
}
