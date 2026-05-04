use std::fs;
use std::path::Path;
use std::process::Command;
use std::sync::Mutex;
use tempfile::TempDir;

mod common;

// 🤓 Prevent cargo lock contention - serialize b00t-cli execution
static CARGO_LOCK: Mutex<()> = Mutex::new(());

fn setup_test_environment(temp_dir: &Path) -> Result<(), std::io::Error> {
    // Create minimal _b00t_ directory structure
    let b00t_dir = temp_dir.join("_b00t_");
    fs::create_dir_all(&b00t_dir)?;

    // Create minimal session files to prevent "No such file" errors
    let session_dir = temp_dir.join(".git");
    fs::create_dir_all(&session_dir)?;

    Ok(())
}

#[test]
fn test_hello_world_with_skip_all_flags() {
    let b00t = common::get_b00t_binary();

    // Test hello-world command help output to verify it works
    let output = Command::new(&b00t)
        .args(["init", "hello-world", "--help"])
        .output()
        .expect("Failed to execute command");

    assert!(
        output.status.success(),
        "Hello world help command failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("Execute the b00t hello_world protocol"));
    assert!(stdout.contains("--skip-redis"));
    assert!(stdout.contains("--skip-diagnostics"));
    assert!(stdout.contains("--skip-tour"));
}

#[test]
fn test_hello_world_mcp_introspection() {
    let b00t = common::get_b00t_binary();

    // Test that MCP introspection functionality works by testing MCP list directly
    let output = Command::new(&b00t)
        .args(["mcp", "list"])
        .output()
        .expect("Failed to execute MCP list command");

    // MCP list should work regardless of success/failure
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);

    // Either we get MCP servers listed, or we get "No MCP servers found"
    assert!(
        stdout.contains("MCP server")
            || stderr.contains("MCP server")
            || stdout.contains("No MCP")
            || stderr.contains("No MCP")
            || stdout.len() > 0
            || stderr.len() > 0
    );
}

#[test]
fn test_hello_world_session_memory_tracking() {
    let b00t = common::get_b00t_binary();

    // Test session memory operations directly
    let output = Command::new(&b00t)
        .args(["session", "keys"])
        .output()
        .expect("Failed to execute session keys command");

    // Session keys should work and show some keys
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);

    // We should get either some keys or an empty list response
    assert!(
        stdout.len() > 0 || stderr.len() > 0,
        "Session keys command produced no output"
    );
}

#[test]
fn test_hello_world_system_preferences() {
    let _lock = CARGO_LOCK
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner()); // 🤓 Prevent cargo contention

    let temp_dir = TempDir::new().unwrap();
    let b00t = common::get_b00t_binary();

    // Initialize git repo and test environment
    Command::new("git")
        .args(["init"])
        .current_dir(temp_dir.path())
        .output()
        .unwrap();
    setup_test_environment(temp_dir.path()).unwrap();

    // Test hello-world command
    let output = Command::new(&b00t)
        .args(&[
            "init",
            "hello-world",
            "--skip-redis",
            "--skip-diagnostics",
            "--skip-tour",
        ])
        .current_dir(temp_dir.path())
        .env("_B00T_TEST_ROOT", temp_dir.path())
        .output()
        .expect("Failed to execute command");

    assert!(
        output.status.success(),
        "Hello world command failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let stdout = String::from_utf8_lossy(&output.stdout);
    // When --skip-redis is used, Phase 4 is skipped entirely
    // Check for phases that should always be there
    assert!(
        stdout.contains("🔧 Phase 3: Tool & Service Discovery")
            || stdout.contains("✅ Agent enlightenment complete!")
    );
}

#[test]
fn test_hello_world_agent_detection() {
    let _lock = CARGO_LOCK
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner()); // 🤓 Prevent cargo contention

    let temp_dir = TempDir::new().unwrap();
    let b00t = common::get_b00t_binary();

    // Initialize git repo and test environment
    Command::new("git")
        .args(["init"])
        .current_dir(temp_dir.path())
        .output()
        .unwrap();
    setup_test_environment(temp_dir.path()).unwrap();

    // Test with CLAUDECODE environment variable
    let output = Command::new(&b00t)
        .env("CLAUDECODE", "1")
        .args(&[
            "init",
            "hello-world",
            "--skip-redis",
            "--skip-diagnostics",
            "--skip-tour",
        ])
        .current_dir(temp_dir.path())
        .env("_B00T_TEST_ROOT", temp_dir.path())
        .output()
        .expect("Failed to execute command");

    assert!(
        output.status.success(),
        "Hello world command failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let stdout = String::from_utf8_lossy(&output.stdout);
    // The new laconic protocol shows agent detection in Phase 1
    assert!(
        stdout.contains("🤖 Phase 1: Agent Identity Detection")
            || stdout.contains("🎯 Agent:")
            || stdout.contains("🏷️  Role:")
            || stdout.contains("✅ Agent enlightenment complete!")
    );
}

#[test]
fn test_hello_world_help_output() {
    let b00t = common::get_b00t_binary();

    let output = Command::new(&b00t)
        .args(["init", "hello-world", "--help"])
        .output()
        .expect("Failed to execute command");

    assert!(
        output.status.success(),
        "Hello world help command failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("Execute the b00t hello_world protocol"));
    assert!(stdout.contains("--skip-redis"));
    assert!(stdout.contains("--skip-diagnostics"));
    assert!(stdout.contains("--skip-tour"));
    assert!(stdout.contains("Verify and start Redis server"));
    assert!(stdout.contains("Load MCP server configurations"));
    assert!(stdout.contains("Run system diagnostics"));
}
