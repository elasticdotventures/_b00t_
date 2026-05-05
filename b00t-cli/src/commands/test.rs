//! `b00t test --fast` — compile test binary once, run directly for subsequent runs.
//!
//! Usage:
//!   b00t test fast <crate_name> <filter>
//!
//! Example:
//!   b00t test fast b00t-c0re-lib sql::
//!
//! On first invocation, compiles the test binary via `cargo test --no-run`.
//! On subsequent invocations (if source unchanged), cargo's build cache skips
//! recompilation and only the binary is re-run — saving ~5min of DuckDB
//! relinking per iteration.

use anyhow::{Result, bail};
use clap::Parser;

#[derive(Parser, Debug, Clone)]
pub enum TestCommands {
    #[clap(about = "Run tests with fast binary reuse — compile once, run many")]
    Fast {
        #[arg(help = "Crate name (e.g. b00t-c0re-lib)")]
        crate_name: String,
        #[arg(help = "Test filter (e.g. sql::)")]
        filter: String,
    },
}

pub fn handle_test_fast(crate_name: &str, filter: &str) -> Result<()> {
    println!("test --fast: {} {}", crate_name, filter);

    // Determine workspace root from manifest dir or env
    let workspace_root = std::env::var("CARGO_MANIFEST_DIR")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|_| {
            std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from("."))
        });

    // Step 1: Compile test binary (cached by cargo on subsequent runs)
    let output = duct::cmd!(
        "cargo",
        "test",
        "--no-run",
        "-p",
        crate_name,
        "--lib",
        filter
    )
    .dir(&workspace_root)
    .stderr_capture()
    .unchecked()
    .run()?;

    // Step 2: Parse "Executable unittests src/lib.rs (target/debug/deps/...)" line
    let stderr = std::str::from_utf8(&output.stderr)?;
    let bin_path = stderr
        .lines()
        .find(|l| l.contains("Executable"))
        .and_then(|l| {
            l.split('(')
                .nth(1)
                .and_then(|s| s.split(')').next())
        })
        .map(|s| s.trim().to_string());

    match bin_path {
        Some(path) => {
            println!("  Binary: {}", path);
            let result = duct::cmd!(&path, filter)
                .dir(&workspace_root)
                .unchecked()
                .run()?;
            // Forward test output to stdout/stderr
            println!("{}", String::from_utf8_lossy(&result.stdout));
            let stderr_out = String::from_utf8_lossy(&result.stderr);
            if !stderr_out.is_empty() {
                eprintln!("{}", stderr_out);
            }
            if !result.status.success() {
                std::process::exit(result.status.code().unwrap_or(1));
            }
        }
        None => {
            // Fallback: parse error or unexpected output — run full cargo test
            eprintln!("  Could not detect binary path — falling back to cargo test full");
            let full = duct::cmd!("cargo", "test", "-p", crate_name, "--lib", filter)
                .dir(&workspace_root)
                .unchecked()
                .run()?;
            println!("{}", String::from_utf8_lossy(&full.stdout));
            let full_stderr = String::from_utf8_lossy(&full.stderr);
            if !full_stderr.is_empty() {
                eprintln!("{}", full_stderr);
            }
            if !full.status.success() {
                std::process::exit(full.status.code().unwrap_or(1));
            }
        }
    }

    Ok(())
}
