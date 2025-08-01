use b00t_mcp::{mcp_server::B00tMcpServer, params::{DetectParams, StatusParams}};
use rmcp::handler::server::ServerHandler;

#[test]
fn test_server_creation() {
    // Create a temporary ACL config file for testing
    let temp_dir = std::env::temp_dir();
    let config_path = temp_dir.join("test_acl.toml");

    // Create a minimal ACL config
    std::fs::write(&config_path, r#"default_policy = "allow"

[commands]
detect = { policy = "allow" }
status = { policy = "allow" }
"#).expect("Failed to write test config");

    let config_path_str = config_path.to_str().unwrap();

    // Test server creation
    let server = B00tMcpServer::new(".", config_path_str);
    match &server {
        Ok(_) => {},
        Err(e) => {
            println!("Server creation failed with error: {:?}", e);
        }
    }
    assert!(server.is_ok(), "Server creation should succeed: {:?}", server.err());

    // Test server info
    let server = server.unwrap();
    let info = server.get_info();
    assert_eq!(info.protocol_version, rmcp::model::ProtocolVersion::V_2024_11_05);
    assert!(info.capabilities.tools.is_some());

    // Clean up
    std::fs::remove_file(&config_path).ok();
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
fn test_json_schema_generation() {
    use schemars::schema_for;

    // Test that our parameter structs can generate JSON schemas
    let detect_schema = schema_for!(DetectParams);
    // Check if schema has an object representation
    assert!(detect_schema.as_object().is_some());

    let status_schema = schema_for!(StatusParams);
    assert!(status_schema.as_object().is_some());
}