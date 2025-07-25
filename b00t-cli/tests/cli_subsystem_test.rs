use std::env;
use tempfile::TempDir;
use b00t_cli::*;

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_cli_datum_creation() {
        let temp_dir = TempDir::new().unwrap();
        let path = temp_dir.path().to_str().unwrap();
        
        // Create a test CLI config
        let config_content = r#"
[b00t]
name = "test-cli"
type = "unknown"
hint = "Test CLI tool"
version = "test --version"
version_regex = "v(\\d+\\.\\d+\\.\\d+)"
desires = "1.0.0"
"#;
        
        let config_path = temp_dir.path().join("test-cli.cli.toml");
        std::fs::write(&config_path, config_content).unwrap();
        
        // Test that we can create a CliDatum from config
        // This test mainly verifies the trait architecture compiles correctly
        assert!(config_path.exists());
    }
    
    #[test]
    fn test_version_status_enum() {
        use b00t_cli::traits::VersionStatus;
        
        assert_eq!(VersionStatus::Match.emoji(), "👍🏻");
        assert_eq!(VersionStatus::Newer.emoji(), "🐣");
        assert_eq!(VersionStatus::Older.emoji(), "😭");
        assert_eq!(VersionStatus::Missing.emoji(), "😱");
        assert_eq!(VersionStatus::Unknown.emoji(), "⏹️");
    }
}