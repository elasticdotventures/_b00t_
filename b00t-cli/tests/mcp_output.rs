use assert_cmd::prelude::*;
use std::fs::File;
use std::io::Write;
use std::process::Command;
use tempfile::tempdir;

#[test]
fn test_mcp_output() -> Result<(), Box<dyn std::error::Error>> {
    let dir = tempdir()?;
    let b00t_path = dir.path();

    // Create a dummy mcp.toml file
    let mcp_toml_path = b00t_path.join("test-server.mcp.toml");
    let mut file = File::create(mcp_toml_path)?;
    writeln!(
        file,
        r#"[b00t]
name = "test-server"
command = "echo"
args = ["hello"]
hint = "a test server"
"#
    )?;

    let mut cmd = Command::cargo_bin("b00t-cli")?;
    cmd.arg("--path")
        .arg(b00t_path.to_str().unwrap())
        .arg("mcp")
        .arg("output")
        .arg("test-server");

    let output = cmd.output()?;
    let stdout_str = String::from_utf8(output.stdout)?;

    let expected_json = serde_json::json!({
        "mcpServers": {
            "test-server": {
                "command": "echo",
                "args": ["hello"]
            }
        }
    });
    let expected_str = serde_json::to_string_pretty(&expected_json)?;

    assert_eq!(stdout_str.trim(), expected_str.trim());

    Ok(())
}
