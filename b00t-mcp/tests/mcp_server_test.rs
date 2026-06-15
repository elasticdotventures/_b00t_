use b00t_mcp::{B00tMcpServerRusty, DetectParams, StatusParams};
use rmcp::handler::server::ServerHandler;

fn run_in_tokio_runtime<T>(f: impl FnOnce() -> T) -> T {
    let runtime = tokio::runtime::Runtime::new().expect("tokio runtime");
    let guard = runtime.enter();
    let result = f();
    drop(guard);
    runtime.shutdown_background();
    result
}

#[test]
fn test_server_creation() {
    run_in_tokio_runtime(|| {
        // Create a temporary ACL config file for testing
        let temp_dir = std::env::temp_dir();
        let config_path = temp_dir.join("test_acl.toml");

        // Create a minimal ACL config
        std::fs::write(
            &config_path,
            r#"default_policy = "allow"

[commands]
detect = { policy = "allow" }
status = { policy = "allow" }
"#,
        )
        .expect("Failed to write test config");

        let config_path_str = config_path.to_str().unwrap();

        // Test server creation with new rusty server
        let server = B00tMcpServerRusty::new_flat(".", config_path_str);
        match &server {
            Ok(_) => {}
            Err(e) => {
                println!("Server creation failed with error: {:?}", e);
            }
        }
        assert!(
            server.is_ok(),
            "Server creation should succeed: {:?}",
            server.err()
        );

        // Test server info
        let server = server.unwrap();
        let info = server.get_info();
        assert_eq!(
            info.protocol_version,
            rmcp::model::ProtocolVersion::default()
        );
        assert!(info.capabilities.tools.is_some());
        // 🦀 Test resources support
        assert!(info.capabilities.resources.is_some());

        // Clean up
        std::fs::remove_file(&config_path).ok();
    });
}

#[test]
fn test_parameter_struct_creation() {
    // Test DetectParams
    let detect_params = DetectParams {
        tool: "git".to_string(),
        verbose: true,
    };
    assert_eq!(detect_params.tool, "git");
    assert!(detect_params.verbose);

    // Test StatusParams
    let status_params = StatusParams {
        verbose: false,
        detailed: true,
    };
    assert!(!status_params.verbose);
    assert!(status_params.detailed);
}

#[test]
fn test_lfmf_command_creates_lesson() {
    use b00t_c0re_lib::learn::record_lesson;
    use std::fs;
    let temp_dir = tempfile::tempdir().unwrap();
    let temp_path = temp_dir.path();
    let tool = "mcp_testtool";
    let lesson = "Lesson from MCP.";
    let _ = record_lesson(temp_path.to_str().unwrap(), tool, lesson);
    let file_path = temp_path.join("learn").join(format!("{}.md", tool));
    assert!(file_path.exists());
    let content = fs::read_to_string(&file_path).unwrap();
    assert!(content.contains(lesson));
}

#[test]
fn test_json_schema_generation() {
    use schemars::schema_for;

    // Test that our parameter structs can generate JSON schemas
    let detect_schema = schema_for!(DetectParams);
    // Check if schema has an object representation
    assert!(detect_schema.as_object().is_some());

    let status_schema = schema_for!(StatusParams);
    assert!(status_schema.as_object().is_some());
}

/// Parity: b00t_session_init MCP struct must agree with `b00t-cli session init` CLI flags.
///
/// This guards against argument drift (e.g. MCP passing --time-limit that CLI doesn't accept).
/// When CLI gains a new flag, add it to SessionInitCommand and update this test.
#[test]
fn test_session_init_mcp_cli_parity() {
    use b00t_mcp::mcp_tools::SessionInitCommand;
    use clap::CommandFactory;

    // Collect the --long flag names accepted by the CLI struct
    let cmd = SessionInitCommand::command();
    let mcp_flags: std::collections::HashSet<String> = cmd
        .get_arguments()
        .filter_map(|a| a.get_long())
        .map(|s| s.to_string())
        .collect();

    // Known accepted flags — must match `b00t-cli session init --help`
    let expected: std::collections::HashSet<String> =
        ["budget", "name"].iter().map(|s| s.to_string()).collect();

    // Any flag in MCP struct but not in expected → potential unsupported arg passed to CLI
    let unexpected: Vec<_> = mcp_flags.difference(&expected).collect();
    assert!(
        unexpected.is_empty(),
        "SessionInitCommand has flags not accepted by b00t-cli session init: {:?}\n\
         Remove them or add support in b00t-cli/src/commands/session.rs",
        unexpected
    );

    // Any expected flag missing from MCP struct → MCP schema is stale
    let missing: Vec<_> = expected.difference(&mcp_flags).collect();
    assert!(
        missing.is_empty(),
        "b00t-cli session init supports flags not exposed by SessionInitCommand: {:?}\n\
         Add them to b00t-mcp/src/mcp_tools.rs SessionInitCommand",
        missing
    );
}
