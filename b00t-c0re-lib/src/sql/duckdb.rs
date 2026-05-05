//! # DuckDbProvider — columnar SQL backend via DuckDB
//!
//! Reference implementation of `SqlProvider` using the embeddable DuckDB
//! engine.  DuckDB is columnar, fast, and compiles from source via the
//! `bundled` feature — no system dependency needed.
//!
//! # Seed tables
//!
//! On first load, the provider creates and seeds these tables:
//!
//! | Table          | Rows | Purpose                              |
//! |----------------|------|--------------------------------------|
//! | `plan_phases`  | 4    | Project lifecycle phases             |
//! | `env_vars`     | 2    | Environment variable descriptions    |
//! | `cloud_services` | 2  | Cloud service inventory              |
//!
//! These match the original mock data in the toon pgwire adapter so that
//! the transition is seamless.

use crate::sql::{QueryResult, Row, SqlProvider};
use anyhow::{Context, Result};
use duckdb::Connection;
use serde_json::json;
use std::sync::Mutex;

/// DuckDB-backed SQL provider.
pub struct DuckDbProvider {
    conn: Mutex<Connection>,
    name: String,
}

impl DuckDbProvider {
    /// Create a new DuckDB provider.
    ///
    /// If `path` is `":memory:"` or empty, an in-memory database is used.
    /// Otherwise, data is persisted to the given `.duckdb` file.
    ///
    /// On first load, seed tables are created automatically.
    pub fn new(path: &str) -> Result<Self> {
        let conn = if path == ":memory:" || path.is_empty() {
            Connection::open_in_memory()
                .context("failed to open DuckDB in-memory database")?
        } else {
            Connection::open(path)
                .with_context(|| format!("failed to open DuckDB database at {path}"))?
        };
        let provider = Self {
            conn: Mutex::new(conn),
            name: format!("duckdb:{path}"),
        };
        provider.seed().context("failed to seed DuckDB provider")?;
        Ok(provider)
    }

    /// Create seed tables if they don't exist.
    fn seed(&self) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute_batch(
            "
            CREATE TABLE IF NOT EXISTS plan_phases (
                id INTEGER PRIMARY KEY,
                name TEXT NOT NULL,
                description TEXT,
                status TEXT CHECK(status IN ('pending','in-progress','done','cancelled'))
            );

            CREATE TABLE IF NOT EXISTS env_vars (
                name TEXT PRIMARY KEY,
                detected BOOLEAN,
                hint TEXT
            );

            CREATE TABLE IF NOT EXISTS cloud_services (
                name TEXT PRIMARY KEY,
                kind TEXT,
                status TEXT,
                plan TEXT
            );
            ",
        )
        .context("failed to create seed tables")?;

        // Insert seed data only if tables are empty
        let count: i64 = conn
            .query_row("SELECT COUNT(*) FROM plan_phases", [], |row| row.get(0))
            .unwrap_or(0);
        if count == 0 {
            conn.execute_batch(
                "
                INSERT INTO plan_phases VALUES (1, 'research', 'Research phase', 'done');
                INSERT INTO plan_phases VALUES (2, 'implement', 'Implementation', 'in-progress');
                INSERT INTO plan_phases VALUES (3, 'test', 'Testing', 'pending');
                INSERT INTO plan_phases VALUES (4, 'deploy', 'Deployment', 'pending');

                INSERT INTO env_vars VALUES ('CLOUDFLARE_API_TOKEN', true, 'CF API Token');
                INSERT INTO env_vars VALUES ('CLOUDFLARE_ACCOUNT_ID', true, 'Account ID');

                INSERT INTO cloud_services VALUES ('workers-ai', 'inference', 'active', 'paid');
                INSERT INTO cloud_services VALUES ('d1', 'database', 'active', 'paid');
                ",
            )
            .context("failed to insert seed data")?;
        }

        Ok(())
    }

    /// Convert a DuckDB `Value` to a `serde_json::Value`.
    fn duck_value_to_json(val: &duckdb::types::Value) -> serde_json::Value {
        match val {
            duckdb::types::Value::Null => serde_json::Value::Null,
            duckdb::types::Value::Boolean(b) => json!(b),
            duckdb::types::Value::TinyInt(i) => json!(i),
            duckdb::types::Value::SmallInt(i) => json!(i),
            duckdb::types::Value::Int(i) => json!(i),
            duckdb::types::Value::BigInt(i) => json!(i),
            duckdb::types::Value::Float(f) => json!(f),
            duckdb::types::Value::Double(f) => json!(f),
            duckdb::types::Value::Text(s) => json!(s),
            _ => serde_json::Value::Null,
        }
    }
}

impl SqlProvider for DuckDbProvider {
    fn name(&self) -> &str {
        &self.name
    }

    fn query(&self, sql: &str) -> Result<QueryResult> {
        let conn = self.conn.lock().unwrap();
        let trimmed = sql.trim();

        // Handle DDL/DML statements that don't return rows
        let upper = trimmed.to_uppercase();
        let is_query = upper.starts_with("SELECT")
            || upper.starts_with("WITH")
            || upper.starts_with("PRAGMA")
            || upper.starts_with("DESCRIBE")
            || upper.starts_with("EXPLAIN")
            || upper.starts_with("SHOW");

        if !is_query {
            // Execute non-query statement and return affected rows
            let affected = conn
                .execute(trimmed, [])
                .with_context(|| format!("failed to execute non-query SQL: {trimmed}"))?;
            return Ok(QueryResult {
                columns: vec![],
                rows: vec![],
                affected_rows: affected as u64,
            });
        }

        // Prepare, execute via query(), collect rows, then access metadata
        let mut stmt = conn
            .prepare(trimmed)
            .with_context(|| format!("failed to prepare SQL: {trimmed}"))?;

        // Execute query — this sets internal result for column metadata access
        let mut rows_iter = stmt
            .query([])
            .with_context(|| format!("failed to execute query: {trimmed}"))?;

        // Collect raw values first (as Vec<Vec<Value>>)
        let mut raw_values: Vec<Vec<duckdb::types::Value>> = Vec::new();
        while let Some(row) = rows_iter
            .next()
            .with_context(|| "failed to read row".to_string())?
        {
            let mut r = Vec::new();
            // Read row without knowing column count yet
            let mut i = 0;
            loop {
                match row.get::<_, duckdb::types::Value>(i) {
                    Ok(val) => r.push(val),
                    Err(_) => break,
                }
                i += 1;
            }
            if i > 0 {
                raw_values.push(r);
            }
        }

        // Drop rows_iter so we can access stmt
        drop(rows_iter);

        // Now column metadata is available from the executed statement
        let col_count = stmt.column_count();
        let col_names: Vec<String> = (0..col_count)
            .map(|i| stmt.column_name(i).map_or("?", |s| s.as_str()).to_string())
            .collect();

        // Build rows from raw values + column names
        let rows: Vec<Row> = raw_values
            .into_iter()
            .map(|vals| {
                col_names
                    .iter()
                    .zip(vals.into_iter().map(|v| Self::duck_value_to_json(&v)))
                    .map(|(c, v)| (c.clone(), v))
                    .collect()
            })
            .collect();
        let affected = rows.len() as u64;
        Ok(QueryResult {
            columns: col_names,
            rows,
            affected_rows: affected,
        })
    }
}

// ──────────────────────────────────────────────
// Tests — TDD: tests first, then implementation
// ──────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    // -----------------------------------------------------------------------
    // Seed data tests
    // -----------------------------------------------------------------------

    #[test]
    fn test_new_in_memory() {
        let provider = DuckDbProvider::new(":memory:").unwrap();
        assert!(provider.name().contains("duckdb"));
        assert!(provider.name().contains(":memory:"));
    }

    #[test]
    fn test_empty_path_creates_in_memory() {
        let provider = DuckDbProvider::new("").unwrap();
        // Empty path should create in-memory DB (name reflects this)
        assert!(!provider.name().is_empty());
        let result = provider.query("SELECT 1 as val").unwrap();
        assert_eq!(result.rows.len(), 1);
    }

    #[test]
    fn test_plan_phases_seeded() {
        let provider = DuckDbProvider::new(":memory:").unwrap();
        let result = provider.query("SELECT * FROM plan_phases ORDER BY id").unwrap();
        assert_eq!(result.columns.len(), 4);
        assert_eq!(result.rows.len(), 4, "expected 4 plan phases");
        // Verify first row
        let row0 = &result.rows[0];
        let id_val: Option<&serde_json::Value> =
            row0.iter().find(|(k, _)| k == "id").map(|(_, v)| v);
        let name_val: Option<&serde_json::Value> =
            row0.iter().find(|(k, _)| k == "name").map(|(_, v)| v);
        assert_eq!(id_val.and_then(|v| v.as_i64()), Some(1));
        assert_eq!(name_val.and_then(|v| v.as_str()), Some("research"));
    }

    #[test]
    fn test_env_vars_seeded() {
        let provider = DuckDbProvider::new(":memory:").unwrap();
        let result = provider.query("SELECT * FROM env_vars").unwrap();
        assert_eq!(result.columns.len(), 3);
        assert_eq!(result.rows.len(), 2, "expected 2 env vars");
    }

    #[test]
    fn test_cloud_services_seeded() {
        let provider = DuckDbProvider::new(":memory:").unwrap();
        let result = provider.query("SELECT * FROM cloud_services").unwrap();
        assert_eq!(result.columns.len(), 4);
        assert_eq!(result.rows.len(), 2, "expected 2 cloud services");
    }

    // -----------------------------------------------------------------------
    // Query tests
    // -----------------------------------------------------------------------

    #[test]
    fn test_select_count() {
        let provider = DuckDbProvider::new(":memory:").unwrap();
        let result = provider
            .query("SELECT COUNT(*) as cnt FROM plan_phases")
            .unwrap();
        assert_eq!(result.columns, vec!["cnt"]);
        assert_eq!(result.rows.len(), 1);
        let cnt = result.rows[0]
            .iter()
            .find(|(k, _)| k == "cnt")
            .and_then(|(_, v)| v.as_i64());
        assert_eq!(cnt, Some(4));
    }

    #[test]
    fn test_select_with_where() {
        let provider = DuckDbProvider::new(":memory:").unwrap();
        let result = provider
            .query("SELECT name, status FROM plan_phases WHERE status = 'done'")
            .unwrap();
        assert_eq!(result.rows.len(), 1);
        let status = result.rows[0]
            .iter()
            .find(|(k, _)| k == "status")
            .and_then(|(_, v)| v.as_str());
        assert_eq!(status, Some("done"));
    }

    #[test]
    fn test_insert_then_select() {
        let provider = DuckDbProvider::new(":memory:").unwrap();

        // Insert via non-query
        let insert_result = provider
            .query("INSERT INTO plan_phases (id, name, description, status) VALUES (99, 'custom', 'Custom phase', 'pending')")
            .unwrap();
        assert!(insert_result.columns.is_empty());
        assert_eq!(insert_result.affected_rows, 1);

        // Verify via query
        let select_result = provider
            .query("SELECT * FROM plan_phases WHERE id = 99")
            .unwrap();
        assert_eq!(select_result.rows.len(), 1);
        let name = select_result.rows[0]
            .iter()
            .find(|(k, _)| k == "name")
            .and_then(|(_, v)| v.as_str());
        assert_eq!(name, Some("custom"));
    }

    #[test]
    fn test_update_then_select() {
        let provider = DuckDbProvider::new(":memory:").unwrap();
        let update_result = provider
            .query("UPDATE plan_phases SET status = 'done' WHERE name = 'test'")
            .unwrap();
        assert_eq!(update_result.affected_rows, 1);

        let select_result = provider
            .query("SELECT status FROM plan_phases WHERE name = 'test'")
            .unwrap();
        let status = select_result.rows[0]
            .iter()
            .find(|(k, _)| k == "status")
            .and_then(|(_, v)| v.as_str());
        assert_eq!(status, Some("done"));
    }

    #[test]
    fn test_delete_then_select() {
        let provider = DuckDbProvider::new(":memory:").unwrap();
        let delete_result = provider
            .query("DELETE FROM env_vars WHERE name = 'CLOUDFLARE_API_TOKEN'")
            .unwrap();
        assert_eq!(delete_result.affected_rows, 1);

        let select_result = provider
            .query("SELECT COUNT(*) as cnt FROM env_vars")
            .unwrap();
        let cnt = select_result.rows[0]
            .iter()
            .find(|(k, _)| k == "cnt")
            .and_then(|(_, v)| v.as_i64());
        assert_eq!(cnt, Some(1));
    }

    // -----------------------------------------------------------------------
    // Edge cases
    // -----------------------------------------------------------------------

    #[test]
    fn test_select_from_empty_table() {
        let provider = DuckDbProvider::new(":memory:").unwrap();
        // Create a table with no data
        let _ = provider.query("CREATE TABLE IF NOT EXISTS empty_test (x INTEGER)");
        let result = provider.query("SELECT * FROM empty_test").unwrap();
        assert!(result.columns.len() == 1);
        assert!(result.rows.is_empty());
        assert_eq!(result.affected_rows, 0);
    }

    #[test]
    fn test_create_table() {
        let provider = DuckDbProvider::new(":memory:").unwrap();
        let result = provider
            .query("CREATE TABLE test_table (id INTEGER, val TEXT)")
            .unwrap();
        assert!(result.columns.is_empty());
        // Verify table exists
        let verify = provider
            .query("SELECT COUNT(*) as cnt FROM test_table")
            .unwrap();
        assert_eq!(verify.rows.len(), 1);
    }

    #[test]
    fn test_drop_table() {
        let provider = DuckDbProvider::new(":memory:").unwrap();
        let _ = provider.query("CREATE TABLE temp_table (x INTEGER)");
        let result = provider.query("DROP TABLE temp_table").unwrap();
        assert!(result.columns.is_empty());
        // Verify table is gone
        let tables = provider
            .query("SELECT name FROM sqlite_master WHERE type='table' AND name='temp_table'")
            .unwrap();
        assert_eq!(tables.rows.len(), 0);
    }

    #[test]
    fn test_provider_is_send_sync() {
        // Compile-time check: SqlProvider requires Send + Sync
        fn assert_send_sync<T: Send + Sync>() {}
        assert_send_sync::<DuckDbProvider>();
    }
}
