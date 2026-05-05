//! # SqlProvider — Generic SQL provider trait
//!
//! Abstract over SQL backends (DuckDB, SQLite, etc.) so the toon pgwire
//! adapter can serve queries from any backend.  DuckDB is the reference
//! implementation — columnar, fast, embeddable.
//!
//! # Future backends
//! - SQLite (via rusqlite or sqlx)
//! - PostgreSQL (via pgwire protocol)
//! - In-memory mock for testing

use anyhow::Result;
use serde_json::Value;

/// A single row of query results as `column_name → value` mappings.
pub type Row = Vec<(String, Value)>;

/// Result of a SQL query execution.
#[derive(Debug, Clone)]
pub struct QueryResult {
    pub columns: Vec<String>,
    pub rows: Vec<Row>,
    pub affected_rows: u64,
}

/// Generic SQL provider — implement for any backend.
///
/// # Example
///
/// ```rust,ignore
/// use b00t_c0re_lib::sql::{SqlProvider, QueryResult};
///
/// struct MockProvider;
///
/// impl SqlProvider for MockProvider {
///     fn name(&self) -> &str { "mock" }
///     fn query(&self, _sql: &str) -> anyhow::Result<QueryResult> {
///         Ok(QueryResult {
///             columns: vec!["msg".into()],
///             rows: vec![vec![("msg".into(), serde_json::json!("hello"))]],
///             affected_rows: 1,
///         })
///     }
/// }
/// ```
pub trait SqlProvider: Send + Sync {
    /// Provider name for display/debug.
    fn name(&self) -> &str;

    /// Execute a SQL query and return results.
    ///
    /// For `SELECT` queries, `rows` contains the result set and `affected_rows`
    /// is the number of rows returned.
    ///
    /// For `INSERT`/`UPDATE`/`DELETE`, `affected_rows` is the number of rows
    /// modified, and `columns`/`rows` may be empty.
    fn query(&self, sql: &str) -> Result<QueryResult>;
}

// ──────────────────────────────────────────────
// Tests for the trait contract
// ──────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    /// A minimal mock provider to verify the trait is object-safe and works.
    struct TestMockProvider;

    impl SqlProvider for TestMockProvider {
        fn name(&self) -> &str {
            "test-mock"
        }

        fn query(&self, sql: &str) -> Result<QueryResult> {
            // Return the SQL as a single-row result for verification
            Ok(QueryResult {
                columns: vec!["sql".into(), "len".into()],
                rows: vec![vec![
                    ("sql".into(), Value::String(sql.to_string())),
                    ("len".into(), json!(sql.len())),
                ]],
                affected_rows: 1,
            })
        }
    }

    #[test]
    fn test_trait_object_safe() {
        // SqlProvider must be object-safe (Send + Sync + no Self: Sized bounds)
        let provider: Box<dyn SqlProvider> = Box::new(TestMockProvider);
        assert_eq!(provider.name(), "test-mock");
    }

    #[test]
    fn test_mock_query() {
        let provider = TestMockProvider;
        let result = provider.query("SELECT 1").unwrap();
        assert_eq!(result.columns.len(), 2);
        assert_eq!(result.rows.len(), 1);
        assert_eq!(result.affected_rows, 1);
        // Check that we can extract values from the row
        let row = &result.rows[0];
        let sql_val: Option<&Value> = row.iter().find(|(k, _)| k == "sql").map(|(_, v)| v);
        assert!(sql_val.is_some());
        assert_eq!(sql_val.unwrap().as_str(), Some("SELECT 1"));
    }

    #[test]
    fn test_query_result_debug_and_clone() {
        let qr = QueryResult {
            columns: vec!["a".into()],
            rows: vec![vec![("a".into(), json!(42))]],
            affected_rows: 1,
        };
        // Clone + Debug must work
        let _cloned = qr.clone();
        let _debug = format!("{:?}", qr);
    }
}
