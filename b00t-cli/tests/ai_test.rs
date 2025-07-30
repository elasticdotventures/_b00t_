use b00t_cli::datum_ai::*;
use b00t_cli::*;
use std::env;
use tempfile::TempDir;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ai_datum_creation() {
        let temp_dir = TempDir::new().unwrap();
        let path = temp_dir.path().to_str().unwrap();

        // Create OpenAI AI config
        let config_content = r#"
[b00t]
name = "openai"
type = "ai"
hint = "OpenAI GPT models via official API"

[models.gpt-4]
capabilities = "text,chat,vision"
context_length = 8192
cost_per_1k_tokens = 0.03
max_tokens = 4096

[env]
OPENAI_API_BASE = "https://api.openai.com/v1"
OPENAI_API_KEY = "${OPENAI_API_KEY}"
"#;

        let config_path = temp_dir.path().join("openai.ai.toml");
        std::fs::write(&config_path, config_content).unwrap();

        // Test AI datum creation
        let ai_datum = AiDatum::from_config("openai", path).unwrap();
        assert_eq!(StatusProvider::name(&ai_datum), "openai");
        assert_eq!(StatusProvider::subsystem(&ai_datum), "ai");
        assert_eq!(
            StatusProvider::hint(&ai_datum),
            "OpenAI GPT models via official API"
        );
        assert_eq!(
            DatumProvider::datum(&ai_datum).datum_type,
            Some(DatumType::Ai)
        );
    }

    #[test]
    fn test_ai_constraint_evaluation_without_env() {
        let temp_dir = TempDir::new().unwrap();
        let path = temp_dir.path().to_str().unwrap();

        // Create AI config without setting environment variables
        let config_content = r#"
[b00t]
name = "test-ai"
type = "ai"
hint = "Test AI provider"

[env]
TEST_API_KEY = ""
"#;

        let config_path = temp_dir.path().join("test-ai.ai.toml");
        std::fs::write(&config_path, config_content).unwrap();

        let ai_datum = AiDatum::from_config("test-ai", path).unwrap();

        // Should not be installed (no env vars set)
        assert!(!DatumChecker::is_installed(&ai_datum));
        assert_eq!(
            DatumChecker::version_status(&ai_datum),
            VersionStatus::Missing
        );
        assert!(ai_datum.is_disabled());
    }

    #[test]
    fn test_ai_constraint_evaluation_with_env() {
        let temp_dir = TempDir::new().unwrap();
        let path = temp_dir.path().to_str().unwrap();

        // Set a test environment variable
        unsafe {
            env::set_var("TEST_AI_KEY", "test-value");
        }

        // Create AI config
        let config_content = r#"
[b00t]
name = "test-ai-enabled"
type = "ai"
hint = "Test AI provider with env"

[env]
TEST_AI_KEY = ""
"#;

        let config_path = temp_dir.path().join("test-ai-enabled.ai.toml");
        std::fs::write(&config_path, config_content).unwrap();

        let ai_datum = AiDatum::from_config("test-ai-enabled", path).unwrap();

        // Should be installed (env var is set)
        assert!(DatumChecker::is_installed(&ai_datum));
        assert_eq!(
            DatumChecker::version_status(&ai_datum),
            VersionStatus::Unknown
        );
        assert!(!ai_datum.is_disabled());

        // Clean up
        unsafe {
            env::remove_var("TEST_AI_KEY");
        }
    }

    #[test]
    fn test_ai_tools_status_collection() {
        let temp_dir = TempDir::new().unwrap();
        let path = temp_dir.path().to_str().unwrap();

        // Create multiple AI configs
        let openai_config = r#"
[b00t]
name = "openai"
type = "ai"
hint = "OpenAI provider"

[env]
OPENAI_API_KEY = ""
"#;

        let anthropic_config = r#"
[b00t]
name = "anthropic"
type = "ai"
hint = "Anthropic provider"

[env]
ANTHROPIC_API_KEY = ""
"#;

        std::fs::write(temp_dir.path().join("openai.ai.toml"), openai_config).unwrap();
        std::fs::write(temp_dir.path().join("anthropic.ai.toml"), anthropic_config).unwrap();

        let tools = b00t_cli::get_ai_tools_status(path).unwrap();

        // Should collect all AI tools (even if disabled due to missing env vars)
        // The filtering happens at a higher level in the display logic
        assert!(tools.len() >= 2); // Should have openai and anthropic

        // Test with environment variable set
        unsafe {
            env::set_var("OPENAI_API_KEY", "test-key");
        }
        let tools_with_env = b00t_cli::get_ai_tools_status(path).unwrap();

        // Should include at least the OpenAI provider now
        let openai_tools: Vec<_> = tools_with_env
            .iter()
            .filter(|tool| StatusProvider::name(tool.as_ref()) == "openai")
            .collect();
        assert!(openai_tools.len() > 0);

        unsafe {
            env::remove_var("OPENAI_API_KEY");
        }
    }
}
