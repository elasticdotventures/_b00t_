# Plan: SqlProvider + DuckDB — Verify, Harden, and Integrate

## Context
The SqlProvider trait + DuckDB backend has been built and compiles cleanly. The toon pgwire adapter has been partially wired. Need to verify correctness, fix any issues, and fully integrate.

## Tasks

### Task 1: Verify DuckDbProvider tests pass
- Run `cargo test -p b00t-c0re-lib --lib sql::` with extended timeout
- Fix any test failures
- Files: `b00t-c0re-lib/src/sql/duckdb.rs`

### Task 2: Wire DuckDbProvider into pgwire mock server
- The `handle_query()` function in `toon.rs` currently uses hardcoded `send_plan_phases_result()` etc.
- Replace with dynamic DuckDbProvider queries
- Files: `b00t-cli/src/commands/toon.rs`

### Task 3: Build release binary + verify
- `cargo build --release -p b00t-cli -p b00t-c0re-lib`
- Test `b00t toon query "SELECT * FROM plan_phases"`
- Test `b00t toon serve --mock` with psql
