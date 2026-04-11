// Integration tests: b00t hive status, b00t whoami, b00t up --help
// H1 gap-fill (OpenHarness analysis) — elasticdotventures/_b00t_#343
use assert_cmd::prelude::*;
use std::process::Command;

// ── H1-a: b00t-cli --help exits 0 ─────────────────────────────────────────────
#[test]
fn test_b00t_cli_help_exits_zero() -> Result<(), Box<dyn std::error::Error>> {
    let mut cmd = Command::cargo_bin("b00t-cli")?;
    cmd.arg("--help");
    let output = cmd.output()?;

    assert!(
        output.status.success(),
        "b00t-cli --help non-zero exit: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let stdout = String::from_utf8_lossy(&output.stdout);
    // Verify core subcommands are advertised
    assert!(
        stdout.contains("hive") || stdout.contains("whoami") || stdout.contains("up"),
        "expected core subcommands in --help output, got: {stdout}"
    );
    Ok(())
}

// ── H1-b: b00t whoami subcommand runs without panic ────────────────────────────
#[test]
fn test_b00t_whoami_runs() -> Result<(), Box<dyn std::error::Error>> {
    let mut cmd = Command::cargo_bin("b00t-cli")?;
    cmd.arg("whoami");
    let output = cmd.output()?;

    // whoami may fail (missing config) but MUST NOT panic (exit 101 = rust panic)
    let exit_code = output.status.code().unwrap_or(0);
    assert_ne!(
        exit_code, 101,
        "b00t whoami panicked (exit 101): {}",
        String::from_utf8_lossy(&output.stderr)
    );

    // Either stdout or stderr should have content — no silent failures
    let has_output = !output.stdout.is_empty() || !output.stderr.is_empty();
    assert!(has_output, "b00t whoami produced no output");
    Ok(())
}

// ── H1-c: b00t hive status subcommand runs without panic ──────────────────────
#[test]
fn test_b00t_hive_status_runs() -> Result<(), Box<dyn std::error::Error>> {
    let mut cmd = Command::cargo_bin("b00t-cli")?;
    cmd.args(["hive", "status"]);
    let output = cmd.output()?;

    let exit_code = output.status.code().unwrap_or(0);
    assert_ne!(
        exit_code, 101,
        "b00t hive status panicked (exit 101): {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let has_output = !output.stdout.is_empty() || !output.stderr.is_empty();
    assert!(has_output, "b00t hive status produced no output");
    Ok(())
}

// ── H1-d: b00t up --help exits 0 ──────────────────────────────────────────────
#[test]
fn test_b00t_up_help_exits_zero() -> Result<(), Box<dyn std::error::Error>> {
    let mut cmd = Command::cargo_bin("b00t-cli")?;
    cmd.args(["up", "--help"]);
    let output = cmd.output()?;

    assert!(
        output.status.success(),
        "b00t up --help non-zero exit: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    Ok(())
}
