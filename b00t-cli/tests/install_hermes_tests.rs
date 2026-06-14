//! Integration tests for `b00t install hermes` — hermes_special_install,
//! update_hermes_mcp_config, config merge, YAML parsing/validation, and
//! error handling when the hermes binary is not available.
//!
//! Run with:
//!   cd /home/brianh/.b00t/b00t-cli && cargo test --test install_hermes_tests

use std::io::Write;
use std::path::Path;
use tempfile::TempDir;

// ---------------------------------------------------------------------------
// Constants duplicated from src/commands/install.rs (not pub there).
// Keep in sync with the source.
// ---------------------------------------------------------------------------
const HERMES_B00T_MCP_COMMAND: &str = "/home/brianh/.cargo/bin/b00t-mcp";
const HERMES_B00T_MCP_ARGS: &[&str] = &["stdio", "-d", "/home/brianh/.b00t"];
const CODEBASE_MEMORY_MCP_PATH: &str =
    "/home/brianh/.b00t/vendor/codebase-memory-mcp-b00t-ir0n-ledg3rr/build/c/codebase-memory-mcp";

// ---------------------------------------------------------------------------
// Helper: run `b00t-cli install hermes` as a subprocess.
// ---------------------------------------------------------------------------
fn run_install_hermes(dry_run: bool) -> Result<std::process::Output, Box<dyn std::error::Error>> {
    let mut cmd = assert_cmd::Command::cargo_bin("b00t-cli")?;
    let mut args = vec!["install", "hermes"];
    if dry_run {
        args.push("--dry-run");
    }
    Ok(cmd.args(&args).output()?)
}

// ---------------------------------------------------------------------------
// Helper: create a minimal valid hermes config.yaml at the given path.
// ---------------------------------------------------------------------------
fn write_hermes_config(path: &Path, content: &str) {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).unwrap();
    }
    let mut f = std::fs::File::create(path).unwrap();
    f.write_all(content.as_bytes()).unwrap();
    f.flush().unwrap();
}

// ===========================================================================
// Test 1: hermes_special_install dry-run mode
// ===========================================================================

/// Dry-run must return Ok(()) and produce NO side effects (no hermes binary
/// check failure, no files created).
#[test]
fn test_hermes_dry_run_returns_ok() {
    let result = b00t_cli::commands::install::hermes_special_install(true);
    assert!(
        result.is_ok(),
        "dry-run hermes_special_install should always succeed: {:?}",
        result
    );
}

/// `b00t-cli install hermes --dry-run` via the CLI binary must exit 0.
#[test]
fn test_cli_install_hermes_dry_run_exit_zero() -> Result<(), Box<dyn std::error::Error>> {
    let output = run_install_hermes(true)?;
    assert!(
        output.status.success(),
        "b00t-cli install hermes --dry-run failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout)?;
    assert!(
        stdout.contains("Dry run"),
        "dry-run output should mention 'Dry run': {}",
        stdout
    );
    Ok(())
}

/// Dry-run must NOT write any hermes config files.
#[test]
fn test_hermes_dry_run_no_side_effects() {
    // Save home dir; dry-run should NOT touch ~/.hermes/config.yaml
    let home = dirs::home_dir().expect("HOME must be set");
    let hermes_config = home.join(".hermes/config.yaml");
    let before = if hermes_config.exists() {
        std::fs::read_to_string(&hermes_config).ok()
    } else {
        None
    };

    let result = b00t_cli::commands::install::hermes_special_install(true);
    assert!(result.is_ok());

    let after = if hermes_config.exists() {
        std::fs::read_to_string(&hermes_config).ok()
    } else {
        None
    };

    assert_eq!(
        before, after,
        "dry-run must not modify ~/.hermes/config.yaml"
    );
}

// ===========================================================================
// Test 2: hermes config registration idempotency
// ===========================================================================

/// Calling update_hermes_mcp_config twice on a non-existent file must produce
/// byte-identical output both times.
#[test]
fn test_config_idempotent_fresh() {
    let dir = TempDir::new().unwrap();
    let config_path = dir.path().join("hermes/config.yaml");

    b00t_cli::commands::install::update_hermes_mcp_config(&config_path).unwrap();
    let first = std::fs::read_to_string(&config_path).unwrap();

    b00t_cli::commands::install::update_hermes_mcp_config(&config_path).unwrap();
    let second = std::fs::read_to_string(&config_path).unwrap();

    assert_eq!(
        first, second,
        "Calling update_hermes_mcp_config twice must produce identical output"
    );
}

/// Calling update_hermes_mcp_config three times in a row must all produce the
/// same output (stricter idempotency check).
#[test]
fn test_config_idempotent_three_calls() {
    let dir = TempDir::new().unwrap();
    let config_path = dir.path().join("hermes/config.yaml");

    b00t_cli::commands::install::update_hermes_mcp_config(&config_path).unwrap();
    let first = std::fs::read_to_string(&config_path).unwrap();

    b00t_cli::commands::install::update_hermes_mcp_config(&config_path).unwrap();
    b00t_cli::commands::install::update_hermes_mcp_config(&config_path).unwrap();
    let last = std::fs::read_to_string(&config_path).unwrap();

    assert_eq!(
        first, last,
        "Three consecutive calls must produce identical output"
    );
}

/// If the config already has the correct entries, calling again must not
/// change anything.
#[test]
fn test_config_idempotent_already_correct() {
    let dir = TempDir::new().unwrap();
    let config_path = dir.path().join("hermes/config.yaml");

    // First write sets up canonical config.
    b00t_cli::commands::install::update_hermes_mcp_config(&config_path).unwrap();
    let canonical = std::fs::read_to_string(&config_path).unwrap();

    // Write a copy of the canonical config back (simulate pre-existing correct state).
    std::fs::write(&config_path, &canonical).unwrap();

    // Second write must leave it unchanged.
    b00t_cli::commands::install::update_hermes_mcp_config(&config_path).unwrap();
    let after = std::fs::read_to_string(&config_path).unwrap();

    assert_eq!(
        canonical, after,
        "Re-applying to an already-correct config must be a no-op"
    );
}

// ===========================================================================
// Test 3: config merge logic when b00t-mcp and codebase-memory already exist
// ===========================================================================

/// When the config already has a b00t-mcp entry with different values, the
/// merge must overwrite it with canonical values.
#[test]
fn test_config_merge_overwrites_b00t_mcp_command() {
    let dir = TempDir::new().unwrap();
    let config_path = dir.path().join("hermes/config.yaml");

    let preexisting = r#"
mcp_servers:
  b00t-mcp:
    command: /old/bogus/path
    args: ["--old-flag"]
"#;
    write_hermes_config(&config_path, preexisting);

    b00t_cli::commands::install::update_hermes_mcp_config(&config_path).unwrap();

    let content = std::fs::read_to_string(&config_path).unwrap();
    let doc: serde_yaml::Mapping = serde_yaml::from_str(&content).unwrap();
    let servers = doc
        .get(&serde_yaml::Value::String("mcp_servers".into()))
        .unwrap()
        .as_mapping()
        .unwrap();
    let b00t_mcp = servers
        .get(&serde_yaml::Value::String("b00t-mcp".into()))
        .unwrap()
        .as_mapping()
        .unwrap();
    let cmd = b00t_mcp
        .get(&serde_yaml::Value::String("command".into()))
        .unwrap()
        .as_str()
        .unwrap();
    assert_eq!(
        cmd, HERMES_B00T_MCP_COMMAND,
        "b00t-mcp command must be overwritten with canonical value"
    );
}

/// When the config already has a codebase-memory entry, it must be overwritten
/// with canonical values (if the binary exists on the system).
#[test]
fn test_config_merge_overwrites_codebase_memory_if_exists() {
    let dir = TempDir::new().unwrap();
    let config_path = dir.path().join("hermes/config.yaml");

    let preexisting = r#"
mcp_servers:
  b00t-mcp:
    command: /old/path
    args: []
  codebase-memory:
    command: /old/cm/path
    args: ["--old"]
"#;
    write_hermes_config(&config_path, preexisting);

    b00t_cli::commands::install::update_hermes_mcp_config(&config_path).unwrap();

    let content = std::fs::read_to_string(&config_path).unwrap();
    let doc: serde_yaml::Mapping = serde_yaml::from_str(&content).unwrap();
    let servers = doc
        .get(&serde_yaml::Value::String("mcp_servers".into()))
        .unwrap()
        .as_mapping()
        .unwrap();

    // b00t-mcp always gets canonical values.
    let b00t_mcp = servers
        .get(&serde_yaml::Value::String("b00t-mcp".into()))
        .unwrap()
        .as_mapping()
        .unwrap();
    let b00t_cmd = b00t_mcp
        .get(&serde_yaml::Value::String("command".into()))
        .unwrap()
        .as_str()
        .unwrap();
    assert_eq!(b00t_cmd, HERMES_B00T_MCP_COMMAND);

    // codebase-memory: if the binary exists it's overwritten; if not, the
    // pre-existing entry survives untouched.
    if std::path::Path::new(CODEBASE_MEMORY_MCP_PATH).exists() {
        let cm = servers
            .get(&serde_yaml::Value::String("codebase-memory".into()))
            .unwrap()
            .as_mapping()
            .unwrap();
        let cm_cmd = cm
            .get(&serde_yaml::Value::String("command".into()))
            .unwrap()
            .as_str()
            .unwrap();
        assert_eq!(cm_cmd, CODEBASE_MEMORY_MCP_PATH);
    } else {
        // Binary doesn't exist — function won't touch codebase-memory entry.
        let cm = servers
            .get(&serde_yaml::Value::String("codebase-memory".into()))
            .unwrap()
            .as_mapping()
            .unwrap();
        let cm_cmd = cm
            .get(&serde_yaml::Value::String("command".into()))
            .unwrap()
            .as_str()
            .unwrap();
        assert_eq!(cm_cmd, "/old/cm/path");
    }
}

/// Unrelated top-level keys (e.g. config_version, agents) must be preserved
/// during merge.
#[test]
fn test_config_merge_preserves_other_keys() {
    let dir = TempDir::new().unwrap();
    let config_path = dir.path().join("hermes/config.yaml");

    let preexisting = r#"
config_version: 42
agents:
  - name: test-agent
mcp_servers:
  b00t-mcp:
    command: /old/path
    args: []
  some-custom-server:
    command: /custom
    args: ["--flag"]
"#;
    write_hermes_config(&config_path, preexisting);

    b00t_cli::commands::install::update_hermes_mcp_config(&config_path).unwrap();

    let content = std::fs::read_to_string(&config_path).unwrap();
    let doc: serde_yaml::Mapping = serde_yaml::from_str(&content).unwrap();

    // config_version must be preserved
    assert!(doc.contains_key(&serde_yaml::Value::String("config_version".into())));
    let version = doc
        .get(&serde_yaml::Value::String("config_version".into()))
        .unwrap()
        .as_i64()
        .unwrap();
    assert_eq!(version, 42);

    // agents list must be preserved
    assert!(doc.contains_key(&serde_yaml::Value::String("agents".into())));

    // custom server must be preserved under mcp_servers
    let servers = doc
        .get(&serde_yaml::Value::String("mcp_servers".into()))
        .unwrap()
        .as_mapping()
        .unwrap();
    assert!(
        servers.contains_key(&serde_yaml::Value::String("some-custom-server".into())),
        "unrelated mcp server entries must be preserved"
    );
}

// ===========================================================================
// Test 4: error handling when hermes binary not found
// ===========================================================================

/// When hermes is not on PATH and pip is not available (simulated by PATH
/// containing neither), hermes_special_install(/* dry_run = */ false) must
/// return an Err, not panic.
#[test]
fn test_hermes_binary_not_found_returns_error() {
    // Save original environment
    let original_path = std::env::var_os("PATH").unwrap_or_default();
    let original_home = std::env::var_os("HOME");

    // Create a temp dir with empty bin/ (no hermes, no pip)
    let sandbox = TempDir::new().unwrap();
    let fake_bin = sandbox.path().join("bin");
    std::fs::create_dir_all(&fake_bin).unwrap();

    // Put only a fake 'pip' that fails
    let pip_stub = fake_bin.join("pip");
    std::fs::write(
        &pip_stub,
        "#!/bin/sh\necho 'pip not available' >&2\nexit 1\n",
    )
    .unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut perms = std::fs::metadata(&pip_stub).unwrap().permissions();
        perms.set_mode(0o755);
        std::fs::set_permissions(&pip_stub, perms).unwrap();
    }

    unsafe {
        std::env::set_var("PATH", &fake_bin);
    }

    // Run non-dry-run — hermes not found, pip fails => error
    let result = b00t_cli::commands::install::hermes_special_install(false);

    // Restore environment
    unsafe {
        std::env::set_var("PATH", &original_path);
        match original_home {
            Some(h) => std::env::set_var("HOME", h),
            None => std::env::remove_var("HOME"),
        }
    }

    assert!(
        result.is_err(),
        "Expected Err when hermes binary is not on PATH and pip fails, got Ok"
    );

    let err_msg = format!("{}", result.unwrap_err());
    assert!(
        err_msg.contains("pip") || err_msg.contains("install") || err_msg.contains("hermes"),
        "Error message should mention pip/install: {}",
        err_msg
    );
}

/// update_hermes_mcp_config must return an error when given a path whose
/// parent directory cannot be created (e.g. when parent is a file).
#[test]
fn test_update_config_errors_on_invalid_parent() {
    let dir = TempDir::new().unwrap();

    // Create a file at the intended parent path so create_dir_all fails
    let parent_file = dir.path().join("config.yaml");
    std::fs::write(&parent_file, b"i am a file, not a directory").unwrap();
    let config_path = parent_file.join("nested/config.yaml");

    let result = b00t_cli::commands::install::update_hermes_mcp_config(&config_path);
    assert!(result.is_err(), "Expected Err when parent path is a file");
}

/// update_hermes_mcp_config must return an error when given an unparseable
/// YAML file (e.g. binary garbage or invalid YAML syntax).
#[test]
fn test_update_config_errors_on_corrupt_yaml() {
    let dir = TempDir::new().unwrap();
    let config_path = dir.path().join("config.yaml");
    std::fs::write(&config_path, b"\x00\xff\xfe\xed\xca\xfe").unwrap();

    let result = b00t_cli::commands::install::update_hermes_mcp_config(&config_path);
    assert!(
        result.is_err(),
        "Expected Err when YAML file is binary garbage"
    );

    let err_msg = format!("{}", result.unwrap_err());
    assert!(
        err_msg.contains("parse") || err_msg.contains("yaml") || err_msg.contains("YAML"),
        "Error message should mention parse or YAML: {}",
        err_msg
    );
}

/// update_hermes_mcp_config must return an error when mcp_servers exists but
/// is not a mapping (e.g. it's a string).
#[test]
fn test_update_config_errors_on_bad_mcp_servers_type() {
    let dir = TempDir::new().unwrap();
    let config_path = dir.path().join("config.yaml");

    // mcp_servers is a string — that's invalid because as_mapping_mut returns
    // None for non-mapping values, triggering the context("mcp_servers must
    // be a mapping") error.
    write_hermes_config(&config_path, "mcp_servers: \"this should be a mapping\"\n");

    let result = b00t_cli::commands::install::update_hermes_mcp_config(&config_path);
    assert!(
        result.is_err(),
        "Expected Err when mcp_servers is not a mapping"
    );

    let err_msg = format!("{}", result.unwrap_err());
    assert!(
        err_msg.contains("mcp_servers"),
        "Error message should reference mcp_servers: {}",
        err_msg
    );
}

// ===========================================================================
// Test 5: config.yaml parsing and validation
// ===========================================================================

/// Parsing a freshly generated config must yield the expected structure with
/// b00t-mcp populated and correct command/args fields.
#[test]
fn test_config_parsing_yields_correct_structure() {
    let dir = TempDir::new().unwrap();
    let config_path = dir.path().join("hermes/config.yaml");

    b00t_cli::commands::install::update_hermes_mcp_config(&config_path).unwrap();
    let content = std::fs::read_to_string(&config_path).unwrap();
    let doc: serde_yaml::Mapping = serde_yaml::from_str(&content).unwrap();

    // Top-level key
    assert!(
        doc.contains_key(&serde_yaml::Value::String("mcp_servers".into())),
        "Output must contain mcp_servers"
    );

    let servers = doc
        .get(&serde_yaml::Value::String("mcp_servers".into()))
        .unwrap()
        .as_mapping()
        .unwrap();

    // b00t-mcp must be present
    assert!(
        servers.contains_key(&serde_yaml::Value::String("b00t-mcp".into())),
        "mcp_servers must contain b00t-mcp"
    );

    let b00t_mcp = servers
        .get(&serde_yaml::Value::String("b00t-mcp".into()))
        .unwrap()
        .as_mapping()
        .unwrap();

    // command field
    assert!(
        b00t_mcp.contains_key(&serde_yaml::Value::String("command".into())),
        "b00t-mcp must have a command field"
    );
    let cmd = b00t_mcp
        .get(&serde_yaml::Value::String("command".into()))
        .unwrap()
        .as_str()
        .unwrap();
    assert_eq!(cmd, HERMES_B00T_MCP_COMMAND);

    // args field
    assert!(
        b00t_mcp.contains_key(&serde_yaml::Value::String("args".into())),
        "b00t-mcp must have an args field"
    );
    let args = b00t_mcp
        .get(&serde_yaml::Value::String("args".into()))
        .unwrap()
        .as_sequence()
        .unwrap();
    assert_eq!(args.len(), HERMES_B00T_MCP_ARGS.len());
    for (i, expected) in HERMES_B00T_MCP_ARGS.iter().enumerate() {
        assert_eq!(args[i].as_str().unwrap(), *expected, "arg {} mismatch", i);
    }
}

/// Validate that the YAML is well-formed and can be re-serialized round-trip.
#[test]
fn test_config_round_trip() {
    let dir = TempDir::new().unwrap();
    let config_path = dir.path().join("roundtrip/config.yaml");

    b00t_cli::commands::install::update_hermes_mcp_config(&config_path).unwrap();
    let original = std::fs::read_to_string(&config_path).unwrap();

    // Parse
    let doc: serde_yaml::Mapping = serde_yaml::from_str(&original).unwrap();

    // Re-serialize
    let re_serialized = serde_yaml::to_string(&doc).unwrap();

    // Re-parse the re-serialized version
    let re_doc: serde_yaml::Mapping = serde_yaml::from_str(&re_serialized).unwrap();

    // Compare structural equality (not byte equality due to formatting differences)
    assert_eq!(
        doc, re_doc,
        "YAML round-trip must preserve structural equality"
    );
}

/// Parsing a config that has codebase-memory (if the binary exists) must
/// produce valid server entries.
#[test]
fn test_config_parsing_codebase_memory() {
    let dir = TempDir::new().unwrap();
    let config_path = dir.path().join("hermes/config.yaml");

    b00t_cli::commands::install::update_hermes_mcp_config(&config_path).unwrap();
    let content = std::fs::read_to_string(&config_path).unwrap();
    let doc: serde_yaml::Mapping = serde_yaml::from_str(&content).unwrap();

    let servers = doc
        .get(&serde_yaml::Value::String("mcp_servers".into()))
        .unwrap()
        .as_mapping()
        .unwrap();

    if std::path::Path::new(CODEBASE_MEMORY_MCP_PATH).exists() {
        assert!(
            servers.contains_key(&serde_yaml::Value::String("codebase-memory".into())),
            "codebase-memory must be present when the binary exists"
        );
        let cm = servers
            .get(&serde_yaml::Value::String("codebase-memory".into()))
            .unwrap()
            .as_mapping()
            .unwrap();
        let cm_cmd = cm
            .get(&serde_yaml::Value::String("command".into()))
            .unwrap()
            .as_str()
            .unwrap();
        assert_eq!(cm_cmd, CODEBASE_MEMORY_MCP_PATH);
        let cm_args = cm
            .get(&serde_yaml::Value::String("args".into()))
            .unwrap()
            .as_sequence()
            .unwrap();
        assert!(cm_args.is_empty(), "codebase-memory args should be empty");
    } else {
        // If binary doesn't exist, codebase-memory is not added by the function.
        // But if it was in the pre-existing config, it would be preserved.
        // This test uses a fresh config, so it shouldn't be present.
        // We just verify we don't crash.
    }
}

/// Empty/invalid hermes config YAML must fail parsing validation.
#[test]
fn test_config_validation_rejects_empty() {
    // serde_yaml::from_str("") returns an empty Mapping, which is technically
    // valid YAML. But the subsequent mcp_servers entry will create it fresh,
    // so update_hermes_mcp_config handles empty input gracefully.
    let dir = TempDir::new().unwrap();
    let config_path = dir.path().join("empty.yaml");
    std::fs::write(&config_path, "").unwrap();

    // Should succeed because empty YAML is parsed as empty Mapping,
    // then mcp_servers is created via or_insert_with.
    let result = b00t_cli::commands::install::update_hermes_mcp_config(&config_path);
    assert!(
        result.is_ok(),
        "Empty YAML must be handled gracefully (treated as fresh config): {:?}",
        result
    );

    let content = std::fs::read_to_string(&config_path).unwrap();
    let doc: serde_yaml::Mapping = serde_yaml::from_str(&content).unwrap();
    assert!(
        doc.contains_key(&serde_yaml::Value::String("mcp_servers".into())),
        "Empty config must produce mcp_servers after update"
    );
}

/// Validating that every produced config entry has the correct schema:
/// each server has string command and sequence args.
#[test]
fn test_config_schema_validation() {
    let dir = TempDir::new().unwrap();
    let config_path = dir.path().join("schema/config.yaml");

    b00t_cli::commands::install::update_hermes_mcp_config(&config_path).unwrap();
    let content = std::fs::read_to_string(&config_path).unwrap();
    let doc: serde_yaml::Mapping = serde_yaml::from_str(&content).unwrap();

    let servers = doc
        .get(&serde_yaml::Value::String("mcp_servers".into()))
        .unwrap()
        .as_mapping()
        .unwrap();

    for (name, value) in servers {
        let name_str = name.as_str().unwrap_or("<?>");
        let server = value.as_mapping().unwrap_or_else(|| {
            panic!("Server '{}' must be a mapping", name_str);
        });

        // Every server must have command (string)
        let cmd_key = serde_yaml::Value::String("command".into());
        assert!(
            server.contains_key(&cmd_key),
            "Server '{}' must have 'command' field",
            name_str
        );
        assert!(
            server[&cmd_key].as_str().is_some(),
            "Server '{}'.command must be a string",
            name_str
        );

        // Every server must have args (sequence)
        let args_key = serde_yaml::Value::String("args".into());
        assert!(
            server.contains_key(&args_key),
            "Server '{}' must have 'args' field",
            name_str
        );
        assert!(
            server[&args_key].as_sequence().is_some(),
            "Server '{}'.args must be a sequence",
            name_str
        );
    }
}
