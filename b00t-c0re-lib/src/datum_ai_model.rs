//! # AI Model Datum - b00t-c0re-lib
//!
//! Abstract schema for AI model management within the b00t ecosystem.
//! Data lives in TOML files; this provides the Rust type schema and behaviors.
//! Based on berriai/litellm configuration patterns.

use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Model size classification for resource planning and capability routing  
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "lowercase")]
pub enum ModelSize {
    /// Small models (<8B parameters) - fast, efficient, local deployment friendly
    #[serde(alias = "sm0l")]
    Small,
    /// Large models (>=8B parameters) - powerful, resource-intensive  
    #[serde(alias = "ch0nky")]
    Large,
}

/// AI model capabilities for task routing
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "snake_case")]
pub enum ModelCapability {
    /// Text completion and conversation
    Chat,
    /// Text embeddings for semantic search
    Embeddings,
    /// Document reranking for search relevance
    Rerank,
    /// Code generation and analysis
    Code,
    /// Image analysis and vision tasks
    Vision,
    /// Function/tool calling
    Tools,
    /// JSON structured output
    JsonMode,
    /// Web search integration
    WebSearch,
    /// Reasoning/thinking mode
    Reasoning,
    /// Batch processing support
    Batch,
}

/// LLM Provider enumeration mapping to litellm prefixes
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "snake_case")]
pub enum ModelProvider {
    #[serde(rename = "openai")]
    OpenAI,
    #[serde(rename = "anthropic")]
    Anthropic,
    #[serde(rename = "azure")]
    AzureOpenAI,
    #[serde(rename = "vertex_ai")]
    VertexAI,
    #[serde(rename = "gemini")]
    GoogleAI,
    #[serde(rename = "bedrock")]
    Bedrock,
    #[serde(rename = "cohere")]
    Cohere,
    #[serde(rename = "huggingface")]
    HuggingFace,
    #[serde(rename = "fireworks_ai")]
    FireworksAI,
    #[serde(rename = "groq")]
    Groq,
    #[serde(rename = "replicate")]
    Replicate,
    #[serde(rename = "ollama")]
    Ollama,
    #[serde(rename = "xai")]
    XAI,
    /// Generic OpenAI-compatible endpoint
    #[serde(rename = "openai_compatible")]
    OpenAICompatible,
    /// Catch-all for new/unknown providers stored as raw string
    #[serde(untagged)]
    Other(String),
}

impl ModelProvider {
    /// Get the litellm prefix for this provider
    pub fn litellm_prefix(&self) -> &str {
        match self {
            Self::OpenAI => "openai/",
            Self::Anthropic => "anthropic/",
            Self::AzureOpenAI => "azure/",
            Self::VertexAI => "vertex_ai/",
            Self::GoogleAI => "gemini/",
            Self::Bedrock => "bedrock/",
            Self::Cohere => "cohere/",
            Self::HuggingFace => "huggingface/",
            Self::FireworksAI => "fireworks_ai/",
            Self::Groq => "groq/",
            Self::Replicate => "replicate/",
            Self::Ollama => "ollama/",
            Self::XAI => "xai/",
            Self::OpenAICompatible => "openai/",
            Self::Other(prefix) => prefix,
        }
    }
}

/// AI Model Datum - abstract schema for TOML-stored model configurations
///
/// This struct defines the schema for AI model datums stored in TOML files.
/// Fields are flexible to allow configuration-driven model registration.
///
/// # TOML Example
///
/// ```toml
/// # ~/.dotfiles/_b00t_/claude-3-5-sonnet.ai_model.toml
/// [b00t]
/// name = "claude-3-5-sonnet"
/// type = "ai_model"
/// hint = "Anthropic's latest flagship model"
///
/// [ai_model]
/// provider = "anthropic"
/// size = "large"
/// capabilities = ["chat", "code", "tools", "reasoning"]
/// litellm_model = "anthropic/claude-3-5-sonnet-20241022"
/// api_key_env = "ANTHROPIC_API_KEY"
/// rpm_limit = 60
/// context_window = 200000
/// access_groups = ["beta-models"]
///
/// [ai_model.parameters]
/// max_tokens = 4096
/// temperature = 0.7
///
/// [ai_model.metadata]
/// family = "claude-3.5"
/// training_cutoff = "2024-04"
/// ```
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AiModelDatum {
    /// Primary provider for this model
    pub provider: ModelProvider,

    /// Model size classification
    pub size: ModelSize,

    /// Capabilities this model supports
    #[serde(default)]
    pub capabilities: Vec<ModelCapability>,

    /// Provider-specific model identifier for litellm
    pub litellm_model: String,

    /// Optional API endpoint override
    pub api_base: Option<String>,

    /// Environment variable name containing API key
    pub api_key_env: Option<String>,

    /// Additional provider-specific parameters
    #[serde(default)]
    pub parameters: HashMap<String, serde_json::Value>,

    /// Model metadata and tags  
    #[serde(default)]
    pub metadata: HashMap<String, String>,

    /// Rate limiting (requests per minute)
    pub rpm_limit: Option<u32>,

    /// Token context window size
    pub context_window: Option<u32>,

    /// Whether model is currently available/enabled
    #[serde(default = "default_true")]
    pub enabled: bool,

    /// Access control groups
    #[serde(default)]
    pub access_groups: Vec<String>,
}

fn default_true() -> bool {
    true
}

impl AiModelDatum {
    /// Generate litellm model list entry for proxy configuration
    pub fn to_litellm_config(&self, model_name: &str) -> serde_json::Value {
        let mut config = serde_json::json!({
            "model_name": model_name,
            "litellm_params": {
                "model": self.litellm_model,
            }
        });

        // Add API key if specified
        if let Some(ref api_key_env) = self.api_key_env {
            config["litellm_params"]["api_key"] =
                serde_json::json!(format!("os.environ/{}", api_key_env));
        }

        // Add API base if specified
        if let Some(ref api_base) = self.api_base {
            config["litellm_params"]["api_base"] = serde_json::json!(api_base);
        }

        // Add RPM limit if specified
        if let Some(rpm) = self.rpm_limit {
            config["litellm_params"]["rpm"] = serde_json::json!(rpm);
        }

        // Add additional parameters
        for (key, value) in &self.parameters {
            config["litellm_params"][key] = value.clone();
        }

        // Add model_info section
        let mut model_info = serde_json::Map::new();

        // Add access groups
        if !self.access_groups.is_empty() {
            model_info.insert(
                "access_groups".to_string(),
                serde_json::json!(self.access_groups),
            );
        }

        // Add capabilities as metadata
        if !self.capabilities.is_empty() {
            let capabilities: Vec<String> = self
                .capabilities
                .iter()
                .map(|c| format!("{:?}", c).to_lowercase())
                .collect();
            model_info.insert("capabilities".to_string(), serde_json::json!(capabilities));
        }

        model_info.insert("size".to_string(), serde_json::json!(self.size));
        model_info.insert("provider".to_string(), serde_json::json!(self.provider));

        // Add context window if specified
        if let Some(context) = self.context_window {
            model_info.insert("context_window".to_string(), serde_json::json!(context));
        }

        // Add custom metadata
        for (key, value) in &self.metadata {
            model_info.insert(key.clone(), serde_json::json!(value));
        }

        if !model_info.is_empty() {
            config["model_info"] = serde_json::json!(model_info);
        }

        config
    }

    /// Check if model has specific capability
    pub fn has_capability(&self, capability: &ModelCapability) -> bool {
        self.capabilities.contains(capability)
    }

    /// Check if model matches size classification
    pub fn is_size(&self, size: &ModelSize) -> bool {
        &self.size == size
    }

    /// Check if model is from specific provider
    pub fn is_provider(&self, provider: &ModelProvider) -> bool {
        &self.provider == provider
    }

    /// Generate full litellm model identifier
    pub fn full_litellm_id(&self) -> String {
        self.litellm_model.clone()
    }
}

/// Model registry for aggregating multiple model datums
/// This would be used by b00t-cli to manage collections of models
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ModelRegistry {
    /// Registry metadata
    pub metadata: RegistryMetadata,

    /// Model configurations indexed by name
    pub models: HashMap<String, AiModelDatum>,

    /// Provider defaults and global settings
    #[serde(default)]
    pub provider_defaults: HashMap<ModelProvider, ProviderDefaults>,
}

/// Registry metadata
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RegistryMetadata {
    /// Registry version for compatibility tracking
    pub version: String,

    /// Description of this registry
    pub description: Option<String>,

    /// Last updated timestamp
    pub updated_at: Option<String>,
}

/// Provider-specific default configurations
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ProviderDefaults {
    /// Default API base URL
    pub api_base: Option<String>,

    /// Default API key environment variable
    pub api_key_env: String,

    /// Provider-specific default parameters
    #[serde(default)]
    pub defaults: HashMap<String, serde_json::Value>,

    /// Whether this provider is enabled by default
    #[serde(default = "default_true")]
    pub enabled: bool,
}

impl ModelRegistry {
    /// Create new empty registry
    pub fn new() -> Self {
        Self {
            metadata: RegistryMetadata {
                version: "1.0.0".to_string(),
                description: Some("b00t AI Model Registry".to_string()),
                updated_at: None,
            },
            models: HashMap::new(),
            provider_defaults: HashMap::new(),
        }
    }

    /// Generate complete litellm configuration YAML
    pub fn to_litellm_yaml(&self) -> Result<String> {
        let mut config = serde_json::json!({
            "model_list": []
        });

        // Generate model list from enabled models
        let model_list: Vec<serde_json::Value> = self
            .models
            .iter()
            .filter(|(_, datum)| datum.enabled)
            .map(|(name, datum)| datum.to_litellm_config(name))
            .collect();

        config["model_list"] = serde_json::json!(model_list);

        // Add general settings
        config["general_settings"] = serde_json::json!({
            "master_key": "os.environ/LITELLM_MASTER_KEY"
        });

        config["litellm_settings"] = serde_json::json!({
            "request_timeout": 600,
            "set_verbose": false,
            "json_logs": true
        });

        // Convert to YAML
        let yaml_str = serde_yaml::to_string(&config)
            .map_err(|e| anyhow::anyhow!("Failed to serialize to YAML: {}", e))?;
        Ok(yaml_str)
    }

    /// Filter models by capability
    pub fn models_with_capability(
        &self,
        capability: &ModelCapability,
    ) -> Vec<(&String, &AiModelDatum)> {
        self.models
            .iter()
            .filter(|(_, datum)| datum.enabled && datum.has_capability(capability))
            .collect()
    }

    /// Filter models by size
    pub fn models_by_size(&self, size: &ModelSize) -> Vec<(&String, &AiModelDatum)> {
        self.models
            .iter()
            .filter(|(_, datum)| datum.enabled && datum.is_size(size))
            .collect()
    }

    /// Get models by provider
    pub fn models_by_provider(&self, provider: &ModelProvider) -> Vec<(&String, &AiModelDatum)> {
        self.models
            .iter()
            .filter(|(_, datum)| datum.enabled && datum.is_provider(provider))
            .collect()
    }

    /// Add model to registry
    pub fn add_model(&mut self, name: String, datum: AiModelDatum) {
        self.models.insert(name, datum);
    }

    /// Remove model from registry
    pub fn remove_model(&mut self, name: &str) -> Option<AiModelDatum> {
        self.models.remove(name)
    }

    /// Get model by name
    pub fn get_model(&self, name: &str) -> Option<&AiModelDatum> {
        self.models.get(name)
    }
}

impl Default for ModelRegistry {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_model_datum_creation() {
        let datum = AiModelDatum {
            provider: ModelProvider::OpenAI,
            size: ModelSize::Large,
            capabilities: vec![
                ModelCapability::Chat,
                ModelCapability::Vision,
                ModelCapability::Tools,
            ],
            litellm_model: "openai/gpt-4o".to_string(),
            api_base: None,
            api_key_env: Some("OPENAI_API_KEY".to_string()),
            parameters: HashMap::new(),
            metadata: HashMap::new(),
            rpm_limit: Some(60),
            context_window: Some(128000),
            enabled: true,
            access_groups: vec![],
        };

        assert!(datum.has_capability(&ModelCapability::Vision));
        assert!(datum.is_size(&ModelSize::Large));
        assert!(datum.is_provider(&ModelProvider::OpenAI));
        assert_eq!(datum.full_litellm_id(), "openai/gpt-4o");
    }

    #[test]
    fn test_litellm_config_generation() {
        let datum = AiModelDatum {
            provider: ModelProvider::Anthropic,
            size: ModelSize::Large,
            capabilities: vec![ModelCapability::Chat, ModelCapability::Code],
            litellm_model: "anthropic/claude-3-5-sonnet".to_string(),
            api_base: None,
            api_key_env: Some("ANTHROPIC_API_KEY".to_string()),
            parameters: {
                let mut params = HashMap::new();
                params.insert("max_tokens".to_string(), serde_json::json!(4096));
                params
            },
            metadata: HashMap::new(),
            rpm_limit: Some(60),
            context_window: Some(200000),
            enabled: true,
            access_groups: vec!["beta-models".to_string()],
        };

        let config = datum.to_litellm_config("claude-3-5-sonnet");
        assert_eq!(config["model_name"], "claude-3-5-sonnet");
        assert_eq!(
            config["litellm_params"]["model"],
            "anthropic/claude-3-5-sonnet"
        );
        assert_eq!(
            config["litellm_params"]["api_key"],
            "os.environ/ANTHROPIC_API_KEY"
        );
        assert_eq!(config["litellm_params"]["max_tokens"], 4096);
        assert_eq!(config["litellm_params"]["rpm"], 60);
    }

    #[test]
    fn test_provider_litellm_prefix() {
        assert_eq!(ModelProvider::OpenAI.litellm_prefix(), "openai/");
        assert_eq!(ModelProvider::Anthropic.litellm_prefix(), "anthropic/");
        assert_eq!(ModelProvider::FireworksAI.litellm_prefix(), "fireworks_ai/");
    }

    #[test]
    fn test_model_registry() {
        let mut registry = ModelRegistry::new();

        let datum = AiModelDatum {
            provider: ModelProvider::OpenAI,
            size: ModelSize::Small,
            capabilities: vec![ModelCapability::Chat],
            litellm_model: "openai/gpt-3.5-turbo".to_string(),
            api_base: None,
            api_key_env: Some("OPENAI_API_KEY".to_string()),
            parameters: HashMap::new(),
            metadata: HashMap::new(),
            rpm_limit: None,
            context_window: Some(4096),
            enabled: true,
            access_groups: vec![],
        };

        registry.add_model("gpt-3.5-turbo".to_string(), datum);

        assert_eq!(registry.models.len(), 1);
        assert!(registry.get_model("gpt-3.5-turbo").is_some());

        let small_models = registry.models_by_size(&ModelSize::Small);
        assert_eq!(small_models.len(), 1);

        let chat_models = registry.models_with_capability(&ModelCapability::Chat);
        assert_eq!(chat_models.len(), 1);
    }

    #[test]
    fn test_serde_roundtrip() {
        let datum = AiModelDatum {
            provider: ModelProvider::FireworksAI,
            size: ModelSize::Small,
            capabilities: vec![ModelCapability::Chat, ModelCapability::Code],
            litellm_model: "fireworks_ai/llama-v3-8b-instruct".to_string(),
            api_base: Some("https://api.fireworks.ai/inference/v1".to_string()),
            api_key_env: Some("FIREWORKS_API_KEY".to_string()),
            parameters: HashMap::new(),
            metadata: HashMap::new(),
            rpm_limit: Some(600),
            context_window: Some(8192),
            enabled: true,
            access_groups: vec!["public".to_string()],
        };

        // Serialize to TOML
        let toml_str = toml::to_string(&datum).expect("Failed to serialize to TOML");

        // Deserialize back
        let deserialized: AiModelDatum =
            toml::from_str(&toml_str).expect("Failed to deserialize from TOML");

        assert_eq!(datum, deserialized);
    }
}
