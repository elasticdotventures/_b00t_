use assert_cmd::prelude::*;
use std::process::Command;
use tempfile::tempdir;

/// Verify that `b00t-cli install --help` exposes all new flag surface area.
#[test]
fn test_install_help_shows_new_flags() -> Result<(), Box<dyn std::error::Error>> {
    let mut cmd = Command::cargo_bin("b00t-cli")?;
    cmd.args(["install", "--help"]);
    let output = cmd.output()?;

    assert!(
        output.status.success(),
        "install --help failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let stdout = String::from_utf8(output.stdout)?;
    assert!(stdout.contains("--interactive"), "missing --interactive flag");
    assert!(stdout.contains("--runtimes"), "missing --runtimes flag");
    assert!(stdout.contains("--scope"), "missing --scope flag");
    assert!(
        stdout.contains("--yes") || stdout.contains("-y"),
        "missing --yes/-y flag"
    );
    Ok(())
}

/// An unknown runtime name must cause the process to exit with a non-zero status
/// and print a recognisable error message to stderr.
#[test]
fn test_install_unknown_runtime_exits_nonzero() -> Result<(), Box<dyn std::error::Error>> {
    let dir = tempdir()?;

    let mut cmd = Command::cargo_bin("b00t-cli")?;
    cmd.args([
        "--path",
        dir.path().to_str().unwrap(),
        "install",
        "--runtimes",
        "bogus_runtime",
        "--yes",
    ]);

    let output = cmd.output()?;

    assert!(
        !output.status.success(),
        "expected non-zero exit for unknown runtime, got success"
    );
    assert_eq!(
        output.status.code(),
        Some(1),
        "expected exit code 1 for unknown runtime"
    );

    let stderr = String::from_utf8(output.stderr)?;
    assert!(
        stderr.contains("bogus_runtime") || stderr.contains("unknown runtime"),
        "error message did not mention the bad runtime name; stderr: {stderr}"
    );
    Ok(())
}

/// A comma-separated list containing an unknown runtime must also exit non-zero.
#[test]
fn test_install_mixed_runtimes_with_unknown_exits_nonzero() -> Result<(), Box<dyn std::error::Error>> {
    let dir = tempdir()?;

    let mut cmd = Command::cargo_bin("b00t-cli")?;
    cmd.args([
        "--path",
        dir.path().to_str().unwrap(),
        "install",
        "--runtimes",
        "claude,not_a_real_runtime",
        "--yes",
    ]);

    let output = cmd.output()?;

    assert!(
        !output.status.success(),
        "expected non-zero exit for mixed runtimes containing unknown name"
    );
    assert_eq!(output.status.code(), Some(1));
    Ok(())
}

/// `--runtimes claude --scope local --yes` with an empty B00T_ROOT should exit 0.
/// Source files are absent so FileCopyPack silently skips, but argument parsing
/// and the runtime dispatch path must succeed end-to-end.
#[test]
fn test_install_valid_runtime_local_scope_exits_zero() -> Result<(), Box<dyn std::error::Error>> {
    let b00t_root = tempdir()?;
    let work_dir = tempdir()?;

    let mut cmd = Command::cargo_bin("b00t-cli")?;
    cmd.current_dir(work_dir.path())
        // B00T_ROOT controls runtimes_source_root(); empty dir → silent skip for all packs
        .env("B00T_ROOT", b00t_root.path())
        .args([
            "--path",
            b00t_root.path().to_str().unwrap(),
            "install",
            "--runtimes",
            "claude",
            "--scope",
            "local",
            "--yes",
        ]);

    let output = cmd.output()?;

    assert!(
        output.status.success(),
        "install --runtimes claude --scope local --yes failed unexpectedly.\nstdout: {}\nstderr: {}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr),
    );
    Ok(())
}

/// Comma-separated `--runtimes claude,gemini` must be *parsed* correctly (both IDs recognised).
/// GeminiAdapter is a work-in-progress stub that returns an error, so the command may fail,
/// but the error must be the stub's "not yet implemented" — not an "unknown runtime" parse error.
#[test]
fn test_install_comma_separated_runtimes_parsed() -> Result<(), Box<dyn std::error::Error>> {
    let b00t_root = tempdir()?;
    let work_dir = tempdir()?;

    let mut cmd = Command::cargo_bin("b00t-cli")?;
    cmd.current_dir(work_dir.path())
        .env("B00T_ROOT", b00t_root.path())
        .args([
            "--path",
            b00t_root.path().to_str().unwrap(),
            "install",
            "--runtimes",
            "claude,gemini",
            "--scope",
            "local",
            "--yes",
        ]);

    let output = cmd.output()?;

    // Both IDs are valid so the error must NOT be an "unknown runtime" parse error.
    let stderr = String::from_utf8(output.stderr)?;
    assert!(
        !stderr.to_lowercase().contains("unknown runtime"),
        "comma-separated runtimes were not parsed correctly; stderr: {stderr}"
    );
    // Verify that at least claude (the fully-implemented adapter) was started
    let stdout = String::from_utf8(output.stdout)?;
    assert!(
        stdout.contains("Claude Code"),
        "expected Claude Code to appear in output; stdout: {stdout}"
    );
    Ok(())
}

/// `--scope global` is the default; passing it explicitly must not cause an error.
#[test]
fn test_install_scope_global_explicit_accepted() -> Result<(), Box<dyn std::error::Error>> {
    let b00t_root = tempdir()?;
    // Redirect global target to a temp ~/.claude equivalent via HOME override
    let fake_home = tempdir()?;

    let mut cmd = Command::cargo_bin("b00t-cli")?;
    cmd.env("B00T_ROOT", b00t_root.path())
        .env("HOME", fake_home.path())
        .args([
            "--path",
            b00t_root.path().to_str().unwrap(),
            "install",
            "--runtimes",
            "claude",
            "--scope",
            "global",
            "--yes",
        ]);

    let output = cmd.output()?;

    assert!(
        output.status.success(),
        "install --scope global failed.\nstdout: {}\nstderr: {}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr),
    );
    Ok(())
}
