#[cfg(test)]
mod integration_tests {
    use crate::{UnifiedConfig, get_mcp_config, mcp_add_json};
    use b00t_cli::datum_mcp::McpDatum;
    use serde_json::Value;
    use std::path::{Path, PathBuf};
    use tempfile::TempDir;

    fn setup_temp_dir() -> TempDir {
        tempfile::tempdir().expect("Failed to create temp directory")
    }

    fn repo_root() -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .expect("workspace root")
            .to_path_buf()
    }

    fn repo_b00t_dir() -> PathBuf {
        repo_root().join("_b00t_")
    }

    fn load_repo_mcp_toml(name: &str) -> String {
        let path = repo_b00t_dir().join(format!("{name}.mcp.toml"));
        std::fs::read_to_string(&path)
            .unwrap_or_else(|e| panic!("failed to read {}: {e}", path.display()))
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
                assert!(
                    !stdio_methods.is_empty(),
                    "Should have at least one stdio method"
                );
                let first_method = &stdio_methods[0];
                let command = first_method.get("command").and_then(|v| v.as_str());
                let args = first_method
                    .get("args")
                    .and_then(|v| v.as_array())
                    .map(|arr| {
                        arr.iter()
                            .filter_map(|v| v.as_str())
                            .map(|s| s.to_string())
                            .collect::<Vec<_>>()
                    });

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

    #[tokio::test]
    async fn test_lfmf_creates_and_appends_lesson() {
        use b00t_cli::commands::lfmf::handle_lfmf;
        let temp_dir = setup_temp_dir();
        let temp_path = temp_dir.path().to_str().unwrap();
        let tool = "testtool";
        let lesson1 = "First: lesson learned.";
        let lesson2 = "Second: lesson learned.";
        // First call: should create file
        let result1 = handle_lfmf(temp_path, tool, lesson1, "repo");
        assert!(result1.await.is_ok());
        let file_path = temp_dir.path().join("learn").join(format!("{}.md", tool));
        assert!(file_path.exists());
        let content1 = std::fs::read_to_string(&file_path).unwrap();
        assert!(content1.contains(lesson1));
        // Second call: should append
        let result2 = handle_lfmf(temp_path, tool, lesson2, "repo");
        assert!(result2.await.is_ok());
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
                assert!(
                    !stdio_methods.is_empty(),
                    "Should have at least one stdio method"
                );
                let first_method = &stdio_methods[0];
                let command = first_method.get("command").and_then(|v| v.as_str());
                let args = first_method
                    .get("args")
                    .and_then(|v| v.as_array())
                    .map(|arr| {
                        arr.iter()
                            .filter_map(|v| v.as_str())
                            .map(|s| s.to_string())
                            .collect::<Vec<_>>()
                    });

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

    #[test]
    fn test_repo_mcp_datums_parse_with_runtime_loader() {
        for name in ["github", "gemini-mcp-tool", "azure-ai-foundry", "ralph"] {
            let content = load_repo_mcp_toml(name);
            let config: UnifiedConfig =
                toml::from_str(&content).unwrap_or_else(|e| panic!("{name} should parse: {e}"));
            assert_eq!(config.b00t.name, name);
            assert_eq!(
                config.b00t.datum_type,
                Some(crate::DatumType::Mcp),
                "{name} should remain an MCP datum"
            );
        }

        let b00t_path = repo_b00t_dir();
        let b00t_path_str = b00t_path.to_str().expect("utf8 path");

        let github = McpDatum::from_config("github", b00t_path_str).expect("load github datum");
        let github_stdio = github.parse_stdio_methods();
        assert!(
            !github_stdio.is_empty(),
            "github datum should expose at least one stdio method"
        );
        assert_eq!(github_stdio[0].command, "npx");

        let gemini = get_mcp_config("gemini-mcp-tool", b00t_path_str).expect("load gemini datum");
        assert_eq!(
            gemini.entangled_cli,
            Some(vec!["geminicli".to_string()]),
            "gemini datum should keep top-level entangled CLI metadata"
        );

        let azure =
            get_mcp_config("azure-ai-foundry", b00t_path_str).expect("load azure foundry datum");
        assert_eq!(
            azure.lfmf_category.as_deref(),
            Some("azure-ai-foundry"),
            "azure datum should keep top-level lfmf_category metadata"
        );
    }

    #[test]
    fn test_mcp_schema_covers_runtime_metadata_fields() {
        let schema_path = repo_b00t_dir().join("schema-资源").join("mcp.json");
        let schema: Value = serde_json::from_str(
            &std::fs::read_to_string(&schema_path)
                .unwrap_or_else(|e| panic!("failed to read {}: {e}", schema_path.display())),
        )
        .expect("schema json should parse");

        let b00t_props = &schema["properties"]["b00t"]["properties"];
        assert!(
            b00t_props.get("lfmf_category").is_some(),
            "schema must cover BootDatum.lfmf_category"
        );
        assert!(
            b00t_props.get("entangled_cli").is_some(),
            "schema must cover BootDatum.entangled_cli"
        );

        let stdio_props = &schema["definitions"]["stdioMethod"]["properties"];
        assert!(
            stdio_props.get("pre_start").is_some(),
            "schema must cover method-level pre_start used by github.mcp.toml"
        );
        assert!(
            stdio_props.get("entangled_cli").is_some(),
            "schema must cover method-level entangled_cli when attached to a stdio method"
        );

        let install_schema = &b00t_props["install"];
        let install_variants = install_schema["oneOf"]
            .as_array()
            .expect("schema install should allow multiple representations");
        assert!(
            install_variants
                .iter()
                .any(|variant| variant.get("type").and_then(Value::as_str) == Some("string")),
            "schema must allow script-style install strings"
        );
        assert!(
            install_variants.iter().any(|variant| {
                variant["properties"]["requires"]["type"].as_str() == Some("array")
            }),
            "schema must allow install.requires arrays used by richer MCP datums"
        );

        let learn_props = &schema["definitions"]["learnBlock"]["properties"];
        assert!(
            learn_props.get("inline").is_some(),
            "schema must cover inline learn blocks used by just-mcp"
        );
    }
}

#[cfg(test)]
mod uninstall_integration {
    use assert_cmd::Command;
    use std::fs;
    use tempfile::TempDir;

    fn write_uninstall_datum(dir: &TempDir, name: &str, script: &str) {
        let content = format!(
            "[b00t]\nname = {:?}\ntype = \"cli\"\nhint = \"test\"\nuninstall = {:?}\n",
            name, script
        );
        fs::write(dir.path().join(format!("{}.cli.toml", name)), content).unwrap();
    }

    #[test]
    fn test_uninstall_command_not_found() {
        let dir = TempDir::new().unwrap();
        let mut cmd = Command::cargo_bin("b00t-cli").unwrap();
        cmd.args([
            "--path",
            dir.path().to_str().unwrap(),
            "uninstall",
            "--yes",
            "nonexistent",
        ]);
        let output = cmd.output().unwrap();
        assert!(!output.status.success());
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert!(stderr.contains("not found"), "got: {}", stderr);
    }

    #[test]
    fn test_uninstall_command_executes() {
        let dir = TempDir::new().unwrap();
        let marker = dir.path().join("removed.txt");
        write_uninstall_datum(&dir, "mytool", &format!("touch {}", marker.display()));
        let mut cmd = Command::cargo_bin("b00t-cli").unwrap();
        cmd.args([
            "--path",
            dir.path().to_str().unwrap(),
            "uninstall",
            "--yes",
            "mytool",
        ]);
        cmd.assert().success();
        assert!(marker.exists());
    }
}
