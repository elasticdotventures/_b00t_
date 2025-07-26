#[cfg(test)]
mod integration_tests {
    use std::fs;
    use std::path::PathBuf;
    use tempfile::TempDir;
    use crate::{mcp_add_json, get_mcp_config};

    fn setup_temp_dir() -> TempDir {
        tempfile::tempdir().expect("Failed to create temp directory")
    }

    #[test]
    fn test_mcp_add_and_get_workflow() {
        let temp_dir = setup_temp_dir();
        let temp_path = temp_dir.path().to_str().unwrap();

        // Test adding an MCP server
        let json = r#"{"playwright": {"command": "npx", "args": ["-y", "@executeautomation/playwright-mcp-server"]}}"#;
        
        let result = mcp_add_json(json, false, temp_path);
        assert!(result.is_ok());

        // Verify the TOML file was created
        let toml_path = temp_dir.path().join("playwright.mcp-json.toml");
        assert!(toml_path.exists());

        // Test reading the config back
        let server = get_mcp_config("playwright", temp_path).unwrap();
        assert_eq!(server.name, "playwright");
        assert_eq!(server.command, Some("npx".to_string()));
        assert_eq!(server.args, Some(vec!["-y".to_string(), "@executeautomation/playwright-mcp-server".to_string()]));
    }

    #[test]
    fn test_get_mcp_config_not_found() {
        let temp_dir = setup_temp_dir();
        let temp_path = temp_dir.path().to_str().unwrap();

        let result = get_mcp_config("nonexistent", temp_path);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("not found"));
    }

    #[test]
    fn test_mcp_add_with_dwiw() {
        let temp_dir = setup_temp_dir();
        let temp_path = temp_dir.path().to_str().unwrap();

        let json_with_comments = r#"// This is a comment
{
  "github": {
    // Another comment
    "command": "npx",
    "args": ["-y", "@modelcontextprotocol/server-github"]
  }
}"#;
        
        let result = mcp_add_json(json_with_comments, true, temp_path);
        assert!(result.is_ok());

        let server = get_mcp_config("github", temp_path).unwrap();
        assert_eq!(server.name, "github");
        assert_eq!(server.command, Some("npx".to_string()));
        assert_eq!(server.args, Some(vec!["-y".to_string(), "@modelcontextprotocol/server-github".to_string()]));
    }

    #[test]
    fn test_mcp_list_empty_directory() {
        let temp_dir = setup_temp_dir();
        let temp_path = temp_dir.path().to_str().unwrap();

        // mcp_list should not error on empty directory
        let result = crate::mcp_list(temp_path, false);
        assert!(result.is_ok());
    }

    #[test]
    fn test_mcp_list_with_servers() {
        let temp_dir = setup_temp_dir();
        let temp_path = temp_dir.path().to_str().unwrap();

        // Add a couple of servers
        let json1 = r#"{"playwright": {"command": "npx", "args": ["-y", "@executeautomation/playwright-mcp-server"]}}"#;
        let json2 = r#"{"filesystem": {"command": "npx", "args": ["-y", "@modelcontextprotocol/server-filesystem"]}}"#;
        
        mcp_add(json1, false, temp_path).unwrap();
        mcp_add(json2, false, temp_path).unwrap();

        // List should work without error (both text and JSON)
        let result = crate::mcp_list(temp_path, false);
        assert!(result.is_ok());
        
        let result_json = crate::mcp_list(temp_path, true);
        assert!(result_json.is_ok());
    }
}