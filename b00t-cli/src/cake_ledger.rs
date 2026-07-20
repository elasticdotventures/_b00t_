//! CakeLedger — probabilistic reward accounting for b00t hive agents.
//!
//! Cake tokens: `<|👍🏻|>` = VoteChoice::Yes (thumbs up), `<|👎🏻|>` = VoteChoice::No (thumbs down).
//! Lottery math keeps agents honest about time estimates while rewarding useful work.

use anyhow::{Context, Result};
use rusqlite::{Connection, params};
use std::path::{Path, PathBuf};

// ---------------------------------------------------------------------------
// Public types
// ---------------------------------------------------------------------------

/// Parsed form of a cake vote token embedded in agent output.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum VoteTokenKind {
    ThumbsUp,
    ThumbsDown,
}

/// Input for creating a pending cake ticket.
pub struct CakeTicketRequest {
    pub agent: String,
    pub thumbs: VoteTokenKind,
    pub task_id: Option<String>,
    pub git_ref: Option<String>,
    pub estimate_mins: Option<i64>,
    pub actual_mins: Option<i64>,
    pub justification: Option<String>,
}

/// A row from the `cake_tickets` table.
#[derive(Debug)]
pub struct CakeTicket {
    pub id: String,
    pub agent: String,
    pub task_id: Option<String>,
    pub git_ref: Option<String>,
    pub thumbs: String,
    pub estimate_mins: Option<i64>,
    pub actual_mins: Option<i64>,
    pub justification: Option<String>,
    pub useful_work_score: f64,
    pub time_accuracy: f64,
    pub p_cake: Option<f64>,
    pub luck_roll: Option<f64>,
    pub amount: Option<i64>,
    pub reviewer_verdict: Option<String>,
    pub reviewer_output: Option<String>,
    pub resolved_at: Option<String>,
    pub created_at: String,
}

/// Outcome returned from `resolve_ticket`.
#[derive(Debug)]
pub struct CakeOutcome {
    pub ticket_id: String,
    pub amount: i64,
    pub p_cake: f64,
    pub luck_roll: f64,
    pub won: bool,
}

// ---------------------------------------------------------------------------
// DDL
// ---------------------------------------------------------------------------

const DDL: &str = "
CREATE TABLE IF NOT EXISTS cake_tickets (
    id                TEXT PRIMARY KEY,
    agent             TEXT NOT NULL,
    task_id           TEXT,
    git_ref           TEXT,
    thumbs            TEXT NOT NULL CHECK (thumbs IN ('up','down')),
    estimate_mins     INTEGER,
    actual_mins       INTEGER,
    justification     TEXT,
    useful_work_score REAL DEFAULT 1.0,
    time_accuracy     REAL DEFAULT 1.0,
    p_cake            REAL,
    luck_roll         REAL,
    amount            INTEGER,
    reviewer_verdict  TEXT CHECK (reviewer_verdict IN ('APPROVE','REQUEST_CHANGES','REJECT',NULL)),
    reviewer_output   TEXT,
    resolved_at       TEXT,
    created_at        TEXT NOT NULL DEFAULT (datetime('now'))
);
CREATE INDEX IF NOT EXISTS idx_cake_agent ON cake_tickets(agent, created_at DESC);

CREATE TABLE IF NOT EXISTS cake_balance (
    agent      TEXT PRIMARY KEY,
    balance    INTEGER NOT NULL DEFAULT 0,
    updated_at TEXT NOT NULL DEFAULT (datetime('now'))
);
";

// ---------------------------------------------------------------------------
// CakeLedger
// ---------------------------------------------------------------------------

pub struct CakeLedger {
    db_path: PathBuf,
}

impl CakeLedger {
    /// Open using the same path as `SchedulerDb::db_path()`.
    pub fn open() -> Result<Self> {
        let base = dirs::data_dir().unwrap_or_else(|| {
            dirs::home_dir()
                .unwrap_or_else(|| PathBuf::from("/tmp"))
                .join(".local/share")
        });
        let path = base.join("b00t").join("scheduler").join("scheduler.db");
        Self::open_at(&path)
    }

    /// Open at an explicit path (used in tests).
    pub fn open_at(path: &Path) -> Result<Self> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)
                .with_context(|| format!("create cake ledger db dir {}", parent.display()))?;
        }
        let ledger = CakeLedger {
            db_path: path.to_path_buf(),
        };
        // Run migrations immediately on open.
        let conn = ledger.connect()?;
        Self::migrate(&conn)?;
        Ok(ledger)
    }

    /// Apply the cake schema to an existing connection (idempotent).
    pub fn migrate(conn: &Connection) -> Result<()> {
        conn.execute_batch(DDL)
            .context("apply cake_ledger schema")?;
        Ok(())
    }

    // -----------------------------------------------------------------------
    // Internal helpers
    // -----------------------------------------------------------------------

    fn connect(&self) -> Result<Connection> {
        Connection::open(&self.db_path)
            .with_context(|| format!("open cake ledger db {}", self.db_path.display()))
    }

    // -----------------------------------------------------------------------
    // Token parsing
    // -----------------------------------------------------------------------

    /// Parse `<|👍🏻|>` → `Some(ThumbsUp)`, `<|👎🏻|>` → `Some(ThumbsDown)`, else `None`.
    pub fn parse_vote_token(s: &str) -> Option<VoteTokenKind> {
        if s.contains("<|👍🏻|>") {
            Some(VoteTokenKind::ThumbsUp)
        } else if s.contains("<|👎🏻|>") {
            Some(VoteTokenKind::ThumbsDown)
        } else {
            None
        }
    }

    // -----------------------------------------------------------------------
    // Ticket lifecycle
    // -----------------------------------------------------------------------

    /// Create a pending ticket. Returns the new ticket id.
    pub fn create_ticket(&self, req: CakeTicketRequest) -> Result<String> {
        let id = format!("cake_{}", uuid::Uuid::new_v4().to_string().replace('-', ""));
        let thumbs = match req.thumbs {
            VoteTokenKind::ThumbsUp => "up",
            VoteTokenKind::ThumbsDown => "down",
        };

        let conn = self.connect()?;
        conn.execute(
            "INSERT INTO cake_tickets
                (id, agent, task_id, git_ref, thumbs, estimate_mins, actual_mins, justification)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            params![
                id,
                req.agent,
                req.task_id,
                req.git_ref,
                thumbs,
                req.estimate_mins,
                req.actual_mins,
                req.justification,
            ],
        )
        .context("insert cake ticket")?;

        Ok(id)
    }

    /// Resolve a pending ticket: run lottery, update balance if won.
    ///
    /// Only tickets with `verdict == "APPROVE"` can win cake.
    pub fn resolve_ticket(
        &self,
        ticket_id: &str,
        verdict: &str,
        reviewer_output: &str,
        useful_work_score: f64,
    ) -> Result<CakeOutcome> {
        let conn = self.connect()?;

        // Fetch the ticket row we need for lottery math.
        let (agent, estimate_mins, actual_mins, justification): (
            String,
            Option<i64>,
            Option<i64>,
            Option<String>,
        ) = conn
            .query_row(
                "SELECT agent, estimate_mins, actual_mins, justification
                 FROM cake_tickets WHERE id = ?1",
                params![ticket_id],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
            )
            .with_context(|| format!("fetch ticket {ticket_id}"))?;

        // --- time_accuracy ---
        let time_accuracy = match (estimate_mins, actual_mins) {
            (Some(est), Some(act)) if est > 0 => {
                let ratio = act as f64 / est as f64;
                if ratio <= 1.0 {
                    1.0_f64
                } else if ratio <= 1.5 {
                    0.7_f64
                } else {
                    // >1.5× over estimate: justification earns partial credit
                    if justification.is_some() {
                        0.7_f64
                    } else {
                        0.3_f64
                    }
                }
            }
            // No timing data → neutral
            _ => 1.0_f64,
        };

        // --- p_cake ---
        let clamped_uws = useful_work_score.clamp(0.5, 2.0);
        let p_cake = 0.6 * time_accuracy * clamped_uws;

        // --- lottery ---
        let luck_roll: f64 = rand::random();
        let amount = if verdict == "APPROVE" && luck_roll < p_cake {
            // Exponential payout: rarer rolls → bigger cake
            let raw = (-luck_roll.ln()).ceil() as i64;
            raw.max(1)
        } else {
            0
        };
        let won = amount > 0;

        // --- persist results ---
        let now = chrono_now_utc();
        conn.execute(
            "UPDATE cake_tickets SET
                useful_work_score = ?1,
                time_accuracy     = ?2,
                p_cake            = ?3,
                luck_roll         = ?4,
                amount            = ?5,
                reviewer_verdict  = ?6,
                reviewer_output   = ?7,
                resolved_at       = ?8
             WHERE id = ?9",
            params![
                useful_work_score,
                time_accuracy,
                p_cake,
                luck_roll,
                amount,
                verdict,
                reviewer_output,
                now,
                ticket_id,
            ],
        )
        .context("update cake ticket resolution")?;

        // Update balance if cake was won.
        if won {
            conn.execute(
                "INSERT INTO cake_balance (agent, balance, updated_at)
                 VALUES (?1, ?2, ?3)
                 ON CONFLICT(agent) DO UPDATE SET
                     balance    = balance + excluded.balance,
                     updated_at = excluded.updated_at",
                params![agent, amount, now],
            )
            .context("update cake balance")?;
        }

        Ok(CakeOutcome {
            ticket_id: ticket_id.to_string(),
            amount,
            p_cake,
            luck_roll,
            won,
        })
    }

    // -----------------------------------------------------------------------
    // Queries
    // -----------------------------------------------------------------------

    /// Current cake balance for an agent (0 if never recorded).
    pub fn balance(&self, agent: &str) -> Result<i64> {
        let conn = self.connect()?;
        let result: rusqlite::Result<i64> = conn.query_row(
            "SELECT balance FROM cake_balance WHERE agent = ?1",
            params![agent],
            |row| row.get(0),
        );
        match result {
            Ok(b) => Ok(b),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(0),
            Err(e) => Err(e).context("query cake balance"),
        }
    }

    /// Recent ticket history for an agent, newest first.
    pub fn history(&self, agent: &str, limit: usize) -> Result<Vec<CakeTicket>> {
        let conn = self.connect()?;
        let mut stmt = conn
            .prepare(
                "SELECT id, agent, task_id, git_ref, thumbs,
                        estimate_mins, actual_mins, justification,
                        COALESCE(useful_work_score, 1.0),
                        COALESCE(time_accuracy, 1.0),
                        p_cake, luck_roll, amount,
                        reviewer_verdict, reviewer_output, resolved_at, created_at
                 FROM cake_tickets
                 WHERE agent = ?1
                 ORDER BY created_at DESC
                 LIMIT ?2",
            )
            .context("prepare history query")?;

        let rows = stmt
            .query_map(params![agent, limit as i64], map_row)
            .context("execute history query")?;

        rows.map(|r| r.context("map cake ticket row")).collect()
    }

    /// Full-text search across agent, task_id, reviewer_output, and justification.
    pub fn search(&self, query: &str, limit: usize) -> Result<Vec<CakeTicket>> {
        let pattern = format!("%{}%", query);
        let conn = self.connect()?;
        let mut stmt = conn
            .prepare(
                "SELECT id, agent, task_id, git_ref, thumbs,
                        estimate_mins, actual_mins, justification,
                        COALESCE(useful_work_score, 1.0),
                        COALESCE(time_accuracy, 1.0),
                        p_cake, luck_roll, amount,
                        reviewer_verdict, reviewer_output, resolved_at, created_at
                 FROM cake_tickets
                 WHERE agent LIKE ?1
                    OR task_id LIKE ?1
                    OR reviewer_output LIKE ?1
                    OR justification LIKE ?1
                 ORDER BY created_at DESC
                 LIMIT ?2",
            )
            .context("prepare search query")?;

        let rows = stmt
            .query_map(params![pattern, limit as i64], map_row)
            .context("execute search query")?;

        rows.map(|r| r.context("map cake ticket row")).collect()
    }
}

// ---------------------------------------------------------------------------
// Row mapping helper
// ---------------------------------------------------------------------------

fn map_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<CakeTicket> {
    Ok(CakeTicket {
        id: row.get(0)?,
        agent: row.get(1)?,
        task_id: row.get(2)?,
        git_ref: row.get(3)?,
        thumbs: row.get(4)?,
        estimate_mins: row.get(5)?,
        actual_mins: row.get(6)?,
        justification: row.get(7)?,
        useful_work_score: row.get(8)?,
        time_accuracy: row.get(9)?,
        p_cake: row.get(10)?,
        luck_roll: row.get(11)?,
        amount: row.get(12)?,
        reviewer_verdict: row.get(13)?,
        reviewer_output: row.get(14)?,
        resolved_at: row.get(15)?,
        created_at: row.get(16)?,
    })
}

// ---------------------------------------------------------------------------
// Time helper
// ---------------------------------------------------------------------------

fn chrono_now_utc() -> String {
    chrono::Utc::now().format("%Y-%m-%d %H:%M:%S").to_string()
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn temp_ledger() -> (CakeLedger, TempDir) {
        let dir = tempfile::tempdir().expect("create temp dir");
        let db_path = dir.path().join("test_cake.db");
        let ledger = CakeLedger::open_at(&db_path).expect("open_at temp db");
        (ledger, dir)
    }

    #[test]
    fn test_parse_vote_token() {
        assert_eq!(
            CakeLedger::parse_vote_token("great job <|👍🏻|> well done"),
            Some(VoteTokenKind::ThumbsUp)
        );
        assert_eq!(
            CakeLedger::parse_vote_token("needs work <|👎🏻|>"),
            Some(VoteTokenKind::ThumbsDown)
        );
        assert_eq!(CakeLedger::parse_vote_token("no token here"), None);
        assert_eq!(CakeLedger::parse_vote_token(""), None);
    }

    #[test]
    fn test_create_and_resolve_ticket_round_trip() {
        let (ledger, _dir) = temp_ledger();

        // Create a ticket with timing data on-budget.
        let req = CakeTicketRequest {
            agent: "test-agent".to_string(),
            thumbs: VoteTokenKind::ThumbsUp,
            task_id: Some("task-42".to_string()),
            git_ref: Some("abc1234".to_string()),
            estimate_mins: Some(30),
            actual_mins: Some(25), // under estimate → time_accuracy = 1.0
            justification: None,
        };
        let ticket_id = ledger.create_ticket(req).expect("create_ticket");
        assert!(!ticket_id.is_empty());

        // Resolve with APPROVE verdict.
        let outcome = ledger
            .resolve_ticket(&ticket_id, "APPROVE", "looks good", 1.0)
            .expect("resolve_ticket");

        assert_eq!(outcome.ticket_id, ticket_id);
        // p_cake = 0.6 * 1.0 * 1.0 = 0.6; luck_roll is random but outcome is deterministic given it.
        assert!((outcome.p_cake - 0.6).abs() < 1e-9, "p_cake should be 0.6");
        assert!(outcome.luck_roll >= 0.0 && outcome.luck_roll <= 1.0);
        if outcome.luck_roll < 0.6 {
            assert!(outcome.won);
            assert!(outcome.amount >= 1);
        } else {
            assert!(!outcome.won);
            assert_eq!(outcome.amount, 0);
        }

        // Balance reflects the outcome.
        let balance = ledger.balance("test-agent").expect("balance");
        assert_eq!(balance, outcome.amount);

        // History returns the ticket.
        let hist = ledger.history("test-agent", 10).expect("history");
        assert_eq!(hist.len(), 1);
        assert_eq!(hist[0].id, ticket_id);
        assert_eq!(hist[0].task_id.as_deref(), Some("task-42"));

        // Search finds it by task_id fragment.
        let results = ledger.search("task-42", 10).expect("search");
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].id, ticket_id);
    }

    #[test]
    fn test_non_approve_verdict_wins_nothing() {
        let (ledger, _dir) = temp_ledger();
        let req = CakeTicketRequest {
            agent: "agent-b".to_string(),
            thumbs: VoteTokenKind::ThumbsUp,
            task_id: None,
            git_ref: None,
            estimate_mins: Some(10),
            actual_mins: Some(10),
            justification: None,
        };
        let id = ledger.create_ticket(req).expect("create");
        let outcome = ledger
            .resolve_ticket(&id, "REQUEST_CHANGES", "not yet", 1.5)
            .expect("resolve");
        assert_eq!(outcome.amount, 0);
        assert!(!outcome.won);
        assert_eq!(ledger.balance("agent-b").expect("balance"), 0);
    }

    #[test]
    fn test_time_accuracy_tiers() {
        let (ledger, _dir) = temp_ledger();

        // Over 1.5× without justification → time_accuracy = 0.3
        let req = CakeTicketRequest {
            agent: "agent-c".to_string(),
            thumbs: VoteTokenKind::ThumbsUp,
            task_id: None,
            git_ref: None,
            estimate_mins: Some(10),
            actual_mins: Some(20), // 2× over
            justification: None,
        };
        let id = ledger.create_ticket(req).expect("create");
        let outcome = ledger
            .resolve_ticket(&id, "APPROVE", "ok", 1.0)
            .expect("resolve");
        // p_cake = 0.6 * 0.3 * 1.0 = 0.18
        assert!(
            (outcome.p_cake - 0.18).abs() < 1e-9,
            "p_cake={}",
            outcome.p_cake
        );
    }
}
