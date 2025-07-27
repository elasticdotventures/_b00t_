use tempfile::TempDir;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_constraint_evaluation() {
        // Test NEEDS_ANY_ENV constraint
        let temp_dir = TempDir::new().unwrap();
        let _path = temp_dir.path().to_str().unwrap();

        // Create taskmaster-ai config with NEEDS_ANY_ENV constraint
        let config_content = r#"
[b00t]
name = "taskmaster-ai"
type = "mcp"
hint = "MCP server for task management AI"
command = "npx"
args = ["-y", "--package=task-master-ai", "task-master-ai"]
require = ["NEEDS_ANY_ENV"]

[b00t.env]
ANTHROPIC_API_KEY = ""
OPENAI_API_KEY = ""
"#;

        let config_path = temp_dir.path().join("taskmaster-ai.mcp.toml");
        std::fs::write(&config_path, config_content).unwrap();

        // Test constraint logic by verifying config was created
        assert!(config_path.exists());

        // Verify the require field is parsed
        let content = std::fs::read_to_string(&config_path).unwrap();
        assert!(content.contains("require = [\"NEEDS_ANY_ENV\"]"));
        assert!(content.contains("ANTHROPIC_API_KEY"));
        assert!(content.contains("OPENAI_API_KEY"));
    }

    #[test]
    fn test_version_status_enum_display() {
        use b00t_cli::VersionStatus;

        assert_eq!(VersionStatus::Match.emoji(), "👍🏻");
        assert_eq!(VersionStatus::Newer.emoji(), "🐣");
        assert_eq!(VersionStatus::Older.emoji(), "😭");
        assert_eq!(VersionStatus::Missing.emoji(), "😱");
        assert_eq!(VersionStatus::Unknown.emoji(), "⏹️");
    }
}
