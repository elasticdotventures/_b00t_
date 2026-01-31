//! Thin wrapper binary that delegates to b00t-cli
//!
//! This allows `cargo install` to create both `b00t` and `b00t-cli` binaries.
//! Agents/subshells can use `b00t` without relying on bash aliases.

use std::env;
use std::process::{Command, exit};

fn main() {
    // Get the directory where this binary is located
    let current_exe = env::current_exe().expect("Failed to determine current executable path");

    let bin_dir = current_exe
        .parent()
        .expect("Failed to get parent directory");

    // Path to b00t-cli in the same directory
    let b00t_cli_path = bin_dir.join("b00t-cli");

    // Collect all arguments (skip argv[0] which is "b00t")
    let args: Vec<String> = env::args().skip(1).collect();

    // Execute b00t-cli with all arguments
    let status = Command::new(&b00t_cli_path)
        .args(&args)
        .status()
        .unwrap_or_else(|e| {
            eprintln!("Failed to execute b00t-cli: {}", e);
            eprintln!("Expected at: {}", b00t_cli_path.display());
            exit(1);
        });

    // Exit with the same code as b00t-cli
    exit(status.code().unwrap_or(1));
}
