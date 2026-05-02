use assert_cmd::prelude::*;
use std::fs;
use std::process::Command;
use tempfile::tempdir;

fn write_claude_runtime_fixture(root: &std::path::Path) -> std::io::Result<()> {
    let claude_root = root.join("_b00t_/runtimes/claude");
    fs::create_dir_all(claude_root.join("skills"))?;
    fs::create_dir_all(claude_root.join("agents"))?;
    fs::create_dir_all(claude_root.join("hooks"))?;
    fs::write(claude_root.join("skills/test.md"), "# test skill\n")?;
    fs::write(claude_root.join("agents/test.md"), "# test agent\n")?;
    fs::write(claude_root.join("hooks/test.js"), "console.log('test');\n")?;
    fs::write(claude_root.join("settings_fragment.json"), "{}\n")?;
    Ok(())
}

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
    assert!(
        stdout.contains("--interactive"),
        "missing --interactive flag"
    );
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
fn test_install_mixed_runtimes_with_unknown_exits_nonzero() -> Result<(), Box<dyn std::error::Error>>
{
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

/// `--runtimes claude --scope local --yes` with runtime-source fixtures should exit 0.
/// Content packs are required, so the fixture must include minimal source dirs.
#[test]
fn test_install_valid_runtime_local_scope_exits_zero() -> Result<(), Box<dyn std::error::Error>> {
    let b00t_root = tempdir()?;
    let work_dir = tempdir()?;
    write_claude_runtime_fixture(b00t_root.path())?;
    let runtimes_source_root = b00t_root.path().join("_b00t_/runtimes");

    let mut cmd = Command::cargo_bin("b00t-cli")?;
    cmd.current_dir(work_dir.path())
        .env("B00T_RUNTIMES_SOURCE_ROOT", &runtimes_source_root)
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
    write_claude_runtime_fixture(b00t_root.path())?;
    let runtimes_source_root = b00t_root.path().join("_b00t_/runtimes");

    let mut cmd = Command::cargo_bin("b00t-cli")?;
    cmd.current_dir(work_dir.path())
        .env("B00T_RUNTIMES_SOURCE_ROOT", &runtimes_source_root)
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
    write_claude_runtime_fixture(b00t_root.path())?;
    let runtimes_source_root = b00t_root.path().join("_b00t_/runtimes");

    let mut cmd = Command::cargo_bin("b00t-cli")?;
    cmd.env("B00T_RUNTIMES_SOURCE_ROOT", &runtimes_source_root)
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
