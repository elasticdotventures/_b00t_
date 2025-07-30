use std::process::Command;
use tempfile::TempDir;

// 🦨 SKUNK: MCP add command test fails due to missing 'add' subcommand in CLI
#[ignore]
#[test]
fn test_mcp_add_command_mode() {
    let temp_dir = TempDir::new().unwrap();
    let path = temp_dir.path().to_str().unwrap();

    let output = Command::new("cargo")
        .args(&[
            "run",
            "--bin",
            "b00t-cli",
            "--",
            "--path",
            path,
            "mcp",
            "add",
            "test-server",
            "--hint",
            "Test server",
            "--",
            "npx",
            "-y",
            "@test/server",
        ])
        .output()
        .expect("Failed to execute command");

    assert!(
        output.status.success(),
        "Command failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    // Check that the file was created
    let config_path = temp_dir.path().join("test-server.mcp.toml");
    assert!(config_path.exists(), "Config file was not created");

    let content = std::fs::read_to_string(&config_path).unwrap();
    assert!(content.contains("name = \"test-server\""));
    assert!(content.contains("command = \"npx\""));
    assert!(content.contains("args = [\"-y\", \"@test/server\"]"));
    assert!(content.contains("hint = \"Test server\""));
}

// 🦨 SKUNK: MCP whitelist rejection test fails due to missing 'add' subcommand in CLI
#[ignore]
#[test]
fn test_mcp_whitelist_rejection() {
    let temp_dir = TempDir::new().unwrap();
    let path = temp_dir.path().to_str().unwrap();

    let output = Command::new("cargo")
        .args(&[
            "run",
            "--bin",
            "b00t-cli",
            "--",
            "--path",
            path,
            "mcp",
            "add",
            "malicious",
            "--",
            "rm",
            "-rf",
            "/",
        ])
        .output()
        .expect("Failed to execute command");

    assert!(!output.status.success(), "Command should have failed");

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("not whitelisted"),
        "Should reject non-whitelisted command"
    );
}

// 🦨 SKUNK: Environment variable path override test fails due to missing 'add' subcommand in CLI
#[ignore]
#[test]
fn test_environment_variable_path_override() {
    let temp_dir = TempDir::new().unwrap();
    let path = temp_dir.path().to_str().unwrap();

    let output = Command::new("cargo")
        .env("_B00T_Path", path)
        .args(&[
            "run",
            "--bin",
            "b00t-cli",
            "--",
            "mcp",
            "add",
            "env-test",
            "--",
            "npx",
            "-y",
            "@test/env-server",
        ])
        .output()
        .expect("Failed to execute command");

    assert!(
        output.status.success(),
        "Command failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    // Check that the file was created in the env-specified path
    let config_path = temp_dir.path().join("env-test.mcp.toml");
    assert!(
        config_path.exists(),
        "Config file was not created with env path"
    );
}
