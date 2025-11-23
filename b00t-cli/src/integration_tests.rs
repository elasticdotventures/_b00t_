#[cfg(test)]
mod integration_tests {
    use crate::{get_mcp_config, mcp_add_json};
    use tempfile::TempDir;

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

        // Verify the TOML file was created (check actual filename from output)
        // 🦨 FIXED filename extension - output shows .mcp.toml not .mcp-json.toml
        let toml_path = temp_dir.path().join("playwright.mcp.toml");
        assert!(toml_path.exists());

        // Test reading the config back
        let server = get_mcp_config("playwright", temp_path).unwrap();
        assert_eq!(server.name, "playwright");
        
        // 🤓 Handle both legacy and new multi-source formats
        // Legacy format has direct command/args fields
        // New format has command/args in mcp.stdio[0]
        if let Some(ref mcp_methods) = server.mcp {
            if let Some(ref stdio_methods) = mcp_methods.stdio {
                assert!(!stdio_methods.is_empty(), "Should have at least one stdio method");
                let first_method = &stdio_methods[0];
                let command = first_method.get("command").and_then(|v| v.as_str());
                let args = first_method.get("args").and_then(|v| v.as_array())
                    .map(|arr| arr.iter().filter_map(|v| v.as_str()).map(|s| s.to_string()).collect::<Vec<_>>());
                    
                assert_eq!(command, Some("npx"));
                assert_eq!(
                    args,
                    Some(vec![
                        "-y".to_string(),
                        "@executeautomation/playwright-mcp-server".to_string()
                    ])
                );
            } else {
                panic!("Expected stdio methods in multi-source MCP config");
            }
        } else {
            // Legacy format
            assert_eq!(server.command, Some("npx".to_string()));
            assert_eq!(
                server.args,
                Some(vec![
                    "-y".to_string(),
                    "@executeautomation/playwright-mcp-server".to_string()
                ])
            );
        }
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
    fn test_lfmf_creates_and_appends_lesson() {
        use crate::commands::lfmf::handle_lfmf;
        let temp_dir = setup_temp_dir();
        let temp_path = temp_dir.path().to_str().unwrap();
        let tool = "testtool";
        let lesson1 = "First: lesson learned.";
        let lesson2 = "Second: lesson learned.";
        // First call: should create file
        let result1 = handle_lfmf(temp_path, tool, lesson1, "repo");
        assert!(result1.is_ok());
        let file_path = temp_dir.path().join("learn").join(format!("{}.md", tool));
        assert!(file_path.exists());
        let content1 = std::fs::read_to_string(&file_path).unwrap();
        assert!(content1.contains(lesson1));
        // Second call: should append
        let result2 = handle_lfmf(temp_path, tool, lesson2, "repo");
        assert!(result2.is_ok());
        let content2 = std::fs::read_to_string(&file_path).unwrap();
        assert!(content2.contains(lesson1));
        assert!(content2.contains(lesson2));
    }

    #[test]
    fn test_learn_lists_md_topics_without_toml() {
        use b00t_c0re_lib::learn::get_learn_topics;
        let temp_dir = setup_temp_dir();
        let temp_path = temp_dir.path().to_str().unwrap();
        let learn_dir = temp_dir.path().join("learn");
        std::fs::create_dir_all(&learn_dir).unwrap();
        let topic1 = learn_dir.join("foo.md");
        let topic2 = learn_dir.join("bar.md");
        std::fs::write(&topic1, "Foo lesson").unwrap();
        std::fs::write(&topic2, "Bar lesson").unwrap();
        let topics = get_learn_topics(temp_path).unwrap();
        assert!(topics.contains(&"foo".to_string()));
        assert!(topics.contains(&"bar".to_string()));
    }

    #[test]
    fn test_learn_returns_md_lesson_for_topic() {
        use b00t_c0re_lib::learn::get_learn_lesson;
        let temp_dir = setup_temp_dir();
        let temp_path = temp_dir.path().to_str().unwrap();
        let learn_dir = temp_dir.path().join("learn");
        std::fs::create_dir_all(&learn_dir).unwrap();
        let topic1 = learn_dir.join("foo.md");
        std::fs::write(&topic1, "Foo lesson content").unwrap();
        let lesson = get_learn_lesson(temp_path, "foo").unwrap();
        assert!(lesson.contains("Foo lesson content"));
    }

    #[test]
    fn test_learn_merges_toml_and_md_topics() {
        use b00t_c0re_lib::learn::get_learn_topics;
        let temp_dir = setup_temp_dir();
        let temp_path = temp_dir.path().to_str().unwrap();
        let learn_dir = temp_dir.path().join("learn");
        std::fs::create_dir_all(&learn_dir).unwrap();
        let topic1 = learn_dir.join("foo.md");
        let topic2 = learn_dir.join("bar.md");
        std::fs::write(&topic1, "Foo lesson").unwrap();
        std::fs::write(&topic2, "Bar lesson").unwrap();
        // Create learn.toml with one topic
        let toml_path = temp_dir.path().join("learn.toml");
        let toml_content = r#"[topics]
foo = "learn/foo.md"
baz = "learn/baz.md"
"#;
        std::fs::write(&toml_path, toml_content).unwrap();
        // Create baz.md
        let baz_md = learn_dir.join("baz.md");
        std::fs::write(&baz_md, "Baz lesson").unwrap();
        let topics = get_learn_topics(temp_path).unwrap();
        assert!(topics.contains(&"foo".to_string()));
        assert!(topics.contains(&"bar".to_string()));
        assert!(topics.contains(&"baz".to_string()));
    }


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
        
        // 🤓 Handle both legacy and new multi-source formats
        if let Some(ref mcp_methods) = server.mcp {
            if let Some(ref stdio_methods) = mcp_methods.stdio {
                assert!(!stdio_methods.is_empty(), "Should have at least one stdio method");
                let first_method = &stdio_methods[0];
                let command = first_method.get("command").and_then(|v| v.as_str());
                let args = first_method.get("args").and_then(|v| v.as_array())
                    .map(|arr| arr.iter().filter_map(|v| v.as_str()).map(|s| s.to_string()).collect::<Vec<_>>());
                    
                assert_eq!(command, Some("npx"));
                assert_eq!(
                    args,
                    Some(vec![
                        "-y".to_string(),
                        "@modelcontextprotocol/server-github".to_string()
                    ])
                );
            } else {
                panic!("Expected stdio methods in multi-source MCP config");
            }
        } else {
            // Legacy format
            assert_eq!(server.command, Some("npx".to_string()));
            assert_eq!(
                server.args,
                Some(vec![
                    "-y".to_string(),
                    "@modelcontextprotocol/server-github".to_string()
                ])
            );
        }
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

        // 🦨 REMOVED mcp_add - function not available, using mcp_add_json instead
        mcp_add_json(json1, false, temp_path).unwrap();
        mcp_add_json(json2, false, temp_path).unwrap();

        // List should work without error (both text and JSON)
        let result = crate::mcp_list(temp_path, false);
        assert!(result.is_ok());

        let result_json = crate::mcp_list(temp_path, true);
        assert!(result_json.is_ok());
    }
}
