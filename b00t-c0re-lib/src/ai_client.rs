//! AI client abstraction using Rig.rs
//! 
//! Provides unified LLM interface with cloud-managed provider configuration.
//! Supports dynamic provider switching based on b00t-dashboard selections.

use anyhow::anyhow;
use rig::client::CompletionClient;
use rig::completion::Prompt;
use rig::providers::{anthropic, openai};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use crate::B00tResult;

/// AI provider configuration from cloud dashboard
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AiProviderConfig {
    pub provider: String,
    pub model: String,
    pub api_key: Option<String>,
    pub endpoint: Option<String>,
    pub enabled: bool,
    pub priority: u8,
}

/// AI client configuration with multiple providers
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AiClientConfig {
    pub providers: HashMap<String, AiProviderConfig>,
    pub default_provider: String,
    pub fallback_provider: Option<String>,
    pub last_updated: Option<String>,
}

impl Default for AiClientConfig {
    fn default() -> Self {
        let mut providers = HashMap::new();
        
        // Default OpenAI configuration
        providers.insert("openai".to_string(), AiProviderConfig {
            provider: "openai".to_string(),
            model: "gpt-4o-mini".to_string(),
            api_key: None,
            endpoint: None,
            enabled: false,
            priority: 0,
        });
        
        // Default Anthropic configuration
        providers.insert("anthropic".to_string(), AiProviderConfig {
            provider: "anthropic".to_string(),
            model: "claude-3-haiku-20240307".to_string(),
            api_key: None,
            endpoint: None,
            enabled: false,
            priority: 1,
        });

        Self {
            providers,
            default_provider: "openai".to_string(),
            fallback_provider: Some("anthropic".to_string()),
            last_updated: None,
        }
    }
}

/// Unified AI client using Rig.rs for multiple providers
pub struct B00tAiClient {
    config: AiClientConfig,
    // 🤓 Store pre-configured clients to avoid recreating on each request
    _clients: HashMap<String, Box<dyn std::any::Any + Send + Sync>>,
}

impl B00tAiClient {
    /// Create new AI client with configuration
    pub fn new(config: AiClientConfig) -> B00tResult<Self> {
        Ok(Self {
            config,
            _clients: HashMap::new(),
        })
    }

    /// Create AI client with default configuration
    pub fn default() -> B00tResult<Self> {
        Self::new(AiClientConfig::default())
    }

    /// Update configuration from cloud dashboard
    pub fn update_config(&mut self, config: AiClientConfig) -> B00tResult<()> {
        self.config = config;
        self._clients.clear(); // Clear cached clients to force recreation
        Ok(())
    }

    /// Get current configuration
    pub fn config(&self) -> &AiClientConfig {
        &self.config
    }

    /// Get available providers
    pub fn available_providers(&self) -> Vec<String> {
        self.config.providers.keys().cloned().collect()
    }

    /// Check if provider is enabled and has API key
    pub fn is_provider_ready(&self, provider: &str) -> bool {
        self.config.providers.get(provider)
            .map(|p| p.enabled && p.api_key.is_some())
            .unwrap_or(false)
    }

    /// Get the best available provider (prioritized + enabled)
    pub fn get_best_provider(&self) -> Option<String> {
        let mut providers: Vec<_> = self.config.providers.values()
            .filter(|p| p.enabled && p.api_key.is_some())
            .collect();
        
        providers.sort_by_key(|p| p.priority);
        providers.first().map(|p| p.provider.clone())
    }

    /// Create simple completion (non-chat)
    pub async fn complete(&self, prompt: &str) -> B00tResult<String> {
        self.complete_with_provider(prompt, None).await
    }

    /// Create completion with specific provider
    pub async fn complete_with_provider(&self, prompt: &str, provider: Option<&str>) -> B00tResult<String> {
        let provider_name = provider.unwrap_or(&self.config.default_provider);
        let provider_config = self.config.providers.get(provider_name)
            .ok_or_else(|| anyhow!("Provider '{}' not found", provider_name))?;

        if !provider_config.enabled {
            return Err(anyhow!("Provider '{}' is disabled", provider_name).into());
        }

        let api_key = provider_config.api_key.as_ref()
            .ok_or_else(|| anyhow!("No API key configured for provider '{}'", provider_name))?;

        match provider_config.provider.as_str() {
            "openai" => {
                let client = openai::Client::new(api_key);
                let agent = client.agent(&provider_config.model).build();
                let response = agent.prompt(prompt).await
                    .map_err(|e| anyhow!("OpenAI completion failed: {}", e))?;
                Ok(response)
            }
            "anthropic" => {
                let client = anthropic::Client::new(api_key);
                let agent = client.agent(&provider_config.model).build();
                let response = agent.prompt(prompt).await
                    .map_err(|e| anyhow!("Anthropic completion failed: {}", e))?;
                Ok(response)
            }
            _ => Err(anyhow!("Unsupported provider: {}", provider_config.provider).into())
        }
    }

    /// Create chat session
    pub async fn chat(&self, messages: &[ChatMessage]) -> B00tResult<String> {
        self.chat_with_provider(messages, None).await
    }

    /// Create chat with specific provider
    pub async fn chat_with_provider(&self, messages: &[ChatMessage], provider: Option<&str>) -> B00tResult<String> {
        let provider_name = provider.unwrap_or(&self.config.default_provider);
        let provider_config = self.config.providers.get(provider_name)
            .ok_or_else(|| anyhow!("Provider '{}' not found", provider_name))?;

        if !provider_config.enabled {
            return Err(anyhow!("Provider '{}' is disabled", provider_name).into());
        }

        let api_key = provider_config.api_key.as_ref()
            .ok_or_else(|| anyhow!("No API key configured for provider '{}'", provider_name))?;

        // For now, convert chat to a simple prompt (Rig.rs chat API requires more research)
        let prompt = messages.iter()
            .map(|msg| format!("{}: {}", msg.role, msg.content))
            .collect::<Vec<_>>()
            .join("\n");

        match provider_config.provider.as_str() {
            "openai" => {
                let client = openai::Client::new(api_key);
                let agent = client.agent(&provider_config.model).build();
                let response = agent.prompt(&prompt).await
                    .map_err(|e| anyhow!("OpenAI chat failed: {}", e))?;
                Ok(response)
            }
            "anthropic" => {
                let client = anthropic::Client::new(api_key);
                let agent = client.agent(&provider_config.model).build();
                let response = agent.prompt(&prompt).await
                    .map_err(|e| anyhow!("Anthropic chat failed: {}", e))?;
                Ok(response)
            }
            _ => Err(anyhow!("Unsupported provider: {}", provider_config.provider).into())
        }
    }

    /// Try completion with fallback to secondary provider
    pub async fn complete_with_fallback(&self, prompt: &str) -> B00tResult<String> {
        // Try primary provider
        match self.complete(prompt).await {
            Ok(response) => Ok(response),
            Err(primary_error) => {
                // Try fallback provider if configured
                if let Some(fallback) = &self.config.fallback_provider {
                    match self.complete_with_provider(prompt, Some(fallback)).await {
                        Ok(response) => {
                            eprintln!("⚠️ Primary provider failed, used fallback: {}", fallback);
                            Ok(response)
                        }
                        Err(fallback_error) => {
                            Err(anyhow!(
                                "Both providers failed. Primary: {}. Fallback: {}", 
                                primary_error, fallback_error
                            ).into())
                        }
                    }
                } else {
                    Err(primary_error)
                }
            }
        }
    }
}

/// Simple chat message structure
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatMessage {
    pub role: String,
    pub content: String,
}

impl ChatMessage {
    pub fn user(content: impl Into<String>) -> Self {
        Self {
            role: "user".to_string(),
            content: content.into(),
        }
    }

    pub fn assistant(content: impl Into<String>) -> Self {
        Self {
            role: "assistant".to_string(),
            content: content.into(),
        }
    }

    pub fn system(content: impl Into<String>) -> Self {
        Self {
            role: "system".to_string(),
            content: content.into(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let config = AiClientConfig::default();
        assert_eq!(config.default_provider, "openai");
        assert!(config.providers.contains_key("openai"));
        assert!(config.providers.contains_key("anthropic"));
    }

    #[test]
    fn test_client_creation() {
        let client = B00tAiClient::default().unwrap();
        assert_eq!(client.config().default_provider, "openai");
        assert!(!client.is_provider_ready("openai")); // No API key configured
    }

    #[test]
    fn test_chat_message_creation() {
        let user_msg = ChatMessage::user("Hello");
        assert_eq!(user_msg.role, "user");
        assert_eq!(user_msg.content, "Hello");

        let assistant_msg = ChatMessage::assistant("Hi there");
        assert_eq!(assistant_msg.role, "assistant");
        assert_eq!(assistant_msg.content, "Hi there");

        let system_msg = ChatMessage::system("You are a helpful assistant");
        assert_eq!(system_msg.role, "system");
        assert_eq!(system_msg.content, "You are a helpful assistant");
    }
}