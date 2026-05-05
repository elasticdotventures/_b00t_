//! # SQL module — Generic SQL provider abstraction
//!
//! Provides a trait-based abstraction over SQL backends so that the toon
//! pgwire adapter (and other consumers) can serve queries from any backend
//! without coupling to a specific implementation.
//!
//! ## Backends
//!
//! | Backend | Module | Status |
//! |---------|--------|--------|
//! | DuckDB  | `duckdb` | ✅ Reference implementation |
//! | SQLite  | —        | Planned |
//! | PostgreSQL | —     | Planned |
//!
//! ## Usage
//!
//! ```rust,ignore
//! use b00t_c0re_lib::sql::{SqlProvider, DuckDbProvider};
//!
//! let provider = DuckDbProvider::new(":memory:")?;
//! let result = provider.query("SELECT * FROM plan_phases")?;
//! for row in &result.rows {
//!     for (col, val) in row {
//!         println!("{col} = {val}");
//!     }
//! }
//! ```

mod duckdb;
mod provider;

pub use duckdb::DuckDbProvider;
pub use provider::{QueryResult, Row, SqlProvider};
