//! Central B00tConfig datum - the single source of truth for all b00t configuration
//!
//! This module defines the unified configuration schema that spans across:
//! - Cloud services (Cloudflare, AWS, etc.)
//! - AI providers and models
//! - User preferences and settings
//! - Service credentials and validation
//!
//! TypeScript bindings are automatically generated for the website UI.

use anyhow::{Result, anyhow};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use ts_rs::TS;

/// Central configuration datum for the entire b00t ecosystem
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, TS)]
#[ts(
    export,
    export_to = "/home/brianh/promptexecution/infrastructure/b00t-website/dashboard/src/types/"
)]
#[serde(rename_all = "camelCase")]
pub struct B00tUnifiedConfig {
    /// User identification and preferences
    pub user: UserConfig,
    /// Cloud service configurations
    pub cloud: CloudServicesConfig,
    /// AI provider and model settings
    pub ai: AiConfiguration,
    /// Development and deployment preferences
    pub development: DevelopmentConfig,
    /// Security and authentication settings
    pub security: SecurityConfig,
    /// Configuration metadata
    pub metadata: ConfigMetadata,
}

/// User identification and preferences
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, TS)]
#[ts(
    export,
    export_to = "/home/brianh/promptexecution/infrastructure/b00t-website/dashboard/src/types/"
)]
#[serde(rename_all = "camelCase")]
pub struct UserConfig {
    /// GitHub username
    pub username: String,
    /// User's email address
    pub email: Option<String>,
    /// Avatar URL
    pub avatar_url: Option<String>,
    /// Preferred language/locale
    pub language: String,
    /// Timezone preference
    pub timezone: Option<String>,
    /// User's preferred name for display
    pub display_name: Option<String>,
}

/// Cloud service configurations and credentials
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, TS)]
#[ts(
    export,
    export_to = "/home/brianh/promptexecution/infrastructure/b00t-website/dashboard/src/types/"
)]
#[serde(rename_all = "camelCase")]
pub struct CloudServicesConfig {
    /// Cloudflare configuration
    pub cloudflare: Option<CloudflareConfig>,
    /// AWS configuration
    pub aws: Option<AwsConfig>,
    /// Qdrant vector database configuration
    pub qdrant: Option<QdrantConfig>,
    /// Additional cloud services
    pub additional: HashMap<String, CloudServiceConfig>,
}

/// Cloudflare service configuration
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, TS)]
#[ts(
    export,
    export_to = "/home/brianh/promptexecution/infrastructure/b00t-website/dashboard/src/types/"
)]
#[serde(rename_all = "camelCase")]
pub struct CloudflareConfig {
    /// Account ID
    pub account_id: String,
    /// API token (stored separately in keyring)
    pub api_token_keyring_key: String,
    /// Zone ID for DNS management
    pub zone_id: Option<String>,
    /// Workers subdomain
    pub workers_subdomain: Option<String>,
    /// Pages project name
    pub pages_project: Option<String>,
    /// Enabled services
    pub enabled_services: Vec<CloudflareService>,
}

/// AWS service configuration
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, TS)]
#[ts(
    export,
    export_to = "/home/brianh/promptexecution/infrastructure/b00t-website/dashboard/src/types/"
)]
#[serde(rename_all = "camelCase")]
pub struct AwsConfig {
    /// AWS region
    pub region: String,
    /// Access key ID (stored separately in keyring)
    pub access_key_keyring_key: String,
    /// Secret key (stored separately in keyring)
    pub secret_key_keyring_key: String,
    /// Enabled services
    pub enabled_services: Vec<AwsService>,
    /// S3 bucket configurations
    pub s3_buckets: Vec<S3BucketConfig>,
}

/// Qdrant vector database configuration
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, TS)]
#[ts(
    export,
    export_to = "/home/brianh/promptexecution/infrastructure/b00t-website/dashboard/src/types/"
)]
#[serde(rename_all = "camelCase")]
pub struct QdrantConfig {
    /// Qdrant endpoint URL
    pub endpoint: String,
    /// API key (stored separately in keyring)
    pub api_key_keyring_key: String,
    /// Collection configurations
    pub collections: Vec<QdrantCollectionConfig>,
}

/// Generic cloud service configuration
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, TS)]
#[ts(
    export,
    export_to = "/home/brianh/promptexecution/infrastructure/b00t-website/dashboard/src/types/"
)]
#[serde(rename_all = "camelCase")]
pub struct CloudServiceConfig {
    /// Service name
    pub name: String,
    /// Service endpoint
    pub endpoint: String,
    /// Authentication method
    pub auth_method: AuthMethod,
    /// Service-specific configuration
    #[ts(type = "Record<string, any>")]
    #[serde(skip_serializing_if = "HashMap::is_empty")]
    pub config: HashMap<String, serde_json::Value>,
}

/// AI configuration extending the existing AiClientConfig
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, TS)]
#[ts(
    export,
    export_to = "/home/brianh/promptexecution/infrastructure/b00t-website/dashboard/src/types/"
)]
#[serde(rename_all = "camelCase")]
pub struct AiConfiguration {
    /// AI provider configurations
    pub providers: HashMap<String, AiProviderConfig>,
    /// Default provider
    pub default_provider: String,
    /// Fallback provider
    pub fallback_provider: Option<String>,
    /// Global AI preferences
    pub preferences: AiPreferences,
    /// Last configuration update
    pub last_updated: Option<String>,
}

/// AI provider configuration
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, TS)]
#[ts(
    export,
    export_to = "/home/brianh/promptexecution/infrastructure/b00t-website/dashboard/src/types/"
)]
#[serde(rename_all = "camelCase")]
pub struct AiProviderConfig {
    /// Provider name (openai, anthropic, etc.)
    pub provider: String,
    /// Model name
    pub model: String,
    /// API key keyring reference
    pub api_key_keyring_key: Option<String>,
    /// Custom endpoint URL
    pub endpoint: Option<String>,
    /// Whether this provider is enabled
    pub enabled: bool,
    /// Provider priority (higher = preferred)
    pub priority: u8,
    /// Provider-specific settings
    #[ts(type = "Record<string, any>")]
    #[serde(skip_serializing_if = "HashMap::is_empty")]
    pub settings: HashMap<String, serde_json::Value>,
}

/// AI preferences and behavior settings
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, TS)]
#[ts(
    export,
    export_to = "/home/brianh/promptexecution/infrastructure/b00t-website/dashboard/src/types/"
)]
#[serde(rename_all = "camelCase")]
pub struct AiPreferences {
    /// Maximum tokens per request
    pub max_tokens: Option<u32>,
    /// Temperature setting
    pub temperature: Option<f32>,
    /// Enable streaming responses
    pub streaming: bool,
    /// Enable function calling
    pub function_calling: bool,
    /// Retry attempts on failure
    pub retry_attempts: u32,
}

/// Development and deployment configuration
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, TS)]
#[ts(
    export,
    export_to = "/home/brianh/promptexecution/infrastructure/b00t-website/dashboard/src/types/"
)]
#[serde(rename_all = "camelCase")]
pub struct DevelopmentConfig {
    /// Preferred programming languages
    pub languages: Vec<String>,
    /// Development environment preferences
    pub environment: EnvironmentPreferences,
    /// Git configuration
    pub git: GitConfig,
    /// Deployment targets
    pub deployment: DeploymentConfig,
}

/// Environment preferences
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, TS)]
#[ts(
    export,
    export_to = "/home/brianh/promptexecution/infrastructure/b00t-website/dashboard/src/types/"
)]
#[serde(rename_all = "camelCase")]
pub struct EnvironmentPreferences {
    /// Preferred shell
    pub shell: String,
    /// Preferred editor/IDE
    pub editor: String,
    /// Terminal preferences
    pub terminal: TerminalConfig,
    /// Package manager preferences
    pub package_managers: HashMap<String, String>,
}

/// Git configuration preferences
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, TS)]
#[ts(
    export,
    export_to = "/home/brianh/promptexecution/infrastructure/b00t-website/dashboard/src/types/"
)]
#[serde(rename_all = "camelCase")]
pub struct GitConfig {
    /// Default branch name
    pub default_branch: String,
    /// Commit message template
    pub commit_template: Option<String>,
    /// Auto-push preference
    pub auto_push: bool,
    /// GPG signing preference
    pub gpg_sign: bool,
}

/// Deployment configuration
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, TS)]
#[ts(
    export,
    export_to = "/home/brianh/promptexecution/infrastructure/b00t-website/dashboard/src/types/"
)]
#[serde(rename_all = "camelCase")]
pub struct DeploymentConfig {
    /// Preferred deployment targets
    pub targets: Vec<DeploymentTarget>,
    /// CI/CD preferences
    pub cicd: CicdPreferences,
}

/// Security and authentication settings
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, TS)]
#[ts(
    export,
    export_to = "/home/brianh/promptexecution/infrastructure/b00t-website/dashboard/src/types/"
)]
#[serde(rename_all = "camelCase")]
pub struct SecurityConfig {
    /// Keyring backend preference
    pub keyring_backend: KeyringBackend,
    /// Session timeout in minutes
    pub session_timeout: u32,
    /// Enable two-factor authentication
    pub enable_2fa: bool,
    /// Allowed IP ranges (CIDR format)
    pub allowed_ip_ranges: Vec<String>,
}

/// Configuration metadata
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, TS)]
#[ts(
    export,
    export_to = "/home/brianh/promptexecution/infrastructure/b00t-website/dashboard/src/types/"
)]
#[serde(rename_all = "camelCase")]
pub struct ConfigMetadata {
    /// Configuration version
    pub version: String,
    /// Created timestamp
    pub created_at: String,
    /// Last updated timestamp
    pub updated_at: String,
    /// Configuration schema version
    pub schema_version: String,
    /// Source of the configuration (dashboard, cli, api)
    pub source: String,
    /// Checksum for integrity verification
    pub checksum: Option<String>,
}

// Enums for various configuration options

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, TS)]
#[ts(
    export,
    export_to = "/home/brianh/promptexecution/infrastructure/b00t-website/dashboard/src/types/"
)]
pub enum CloudflareService {
    Workers,
    Pages,
    DNS,
    KV,
    D1,
    R2,
    Vectorize,
    AI,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, TS)]
#[ts(
    export,
    export_to = "/home/brianh/promptexecution/infrastructure/b00t-website/dashboard/src/types/"
)]
pub enum AwsService {
    EC2,
    S3,
    Lambda,
    RDS,
    DynamoDB,
    SQS,
    SNS,
    Bedrock,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, TS)]
#[ts(
    export,
    export_to = "/home/brianh/promptexecution/infrastructure/b00t-website/dashboard/src/types/"
)]
pub enum AuthMethod {
    ApiKey,
    OAuth,
    BasicAuth,
    Bearer,
    Custom,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, TS)]
#[ts(
    export,
    export_to = "/home/brianh/promptexecution/infrastructure/b00t-website/dashboard/src/types/"
)]
pub enum KeyringBackend {
    System,
    File,
    Memory,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, TS)]
#[ts(
    export,
    export_to = "/home/brianh/promptexecution/infrastructure/b00t-website/dashboard/src/types/"
)]
#[serde(rename_all = "camelCase")]
pub struct S3BucketConfig {
    pub name: String,
    pub region: String,
    pub public_read: bool,
    pub versioning: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, TS)]
#[ts(
    export,
    export_to = "/home/brianh/promptexecution/infrastructure/b00t-website/dashboard/src/types/"
)]
#[serde(rename_all = "camelCase")]
pub struct QdrantCollectionConfig {
    pub name: String,
    pub vector_size: u64,
    pub distance_metric: String,
    pub replicas: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, TS)]
#[ts(
    export,
    export_to = "/home/brianh/promptexecution/infrastructure/b00t-website/dashboard/src/types/"
)]
#[serde(rename_all = "camelCase")]
pub struct TerminalConfig {
    pub font_family: String,
    pub font_size: u8,
    pub theme: String,
    pub transparency: Option<f32>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, TS)]
#[ts(
    export,
    export_to = "/home/brianh/promptexecution/infrastructure/b00t-website/dashboard/src/types/"
)]
#[serde(rename_all = "camelCase")]
pub struct DeploymentTarget {
    pub name: String,
    pub platform: String,
    pub environment: String,
    #[ts(type = "Record<string, any>")]
    #[serde(skip_serializing_if = "HashMap::is_empty")]
    pub config: HashMap<String, serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, TS)]
#[ts(
    export,
    export_to = "/home/brianh/promptexecution/infrastructure/b00t-website/dashboard/src/types/"
)]
#[serde(rename_all = "camelCase")]
pub struct CicdPreferences {
    pub platform: String,
    pub auto_deploy: bool,
    pub run_tests: bool,
    pub notify_on_failure: bool,
}

impl Default for B00tUnifiedConfig {
    fn default() -> Self {
        Self {
            user: UserConfig {
                username: "unknown".to_string(),
                email: None,
                avatar_url: None,
                language: "en".to_string(),
                timezone: None,
                display_name: None,
            },
            cloud: CloudServicesConfig {
                cloudflare: None,
                aws: None,
                qdrant: None,
                additional: HashMap::new(),
            },
            ai: AiConfiguration {
                providers: HashMap::new(),
                default_provider: "openai".to_string(),
                fallback_provider: Some("anthropic".to_string()),
                preferences: AiPreferences {
                    max_tokens: Some(4096),
                    temperature: Some(0.7),
                    streaming: true,
                    function_calling: true,
                    retry_attempts: 3,
                },
                last_updated: None,
            },
            development: DevelopmentConfig {
                languages: vec!["rust".to_string(), "typescript".to_string()],
                environment: EnvironmentPreferences {
                    shell: "bash".to_string(),
                    editor: "vscode".to_string(),
                    terminal: TerminalConfig {
                        font_family: "JetBrains Mono".to_string(),
                        font_size: 14,
                        theme: "dark".to_string(),
                        transparency: None,
                    },
                    package_managers: HashMap::new(),
                },
                git: GitConfig {
                    default_branch: "main".to_string(),
                    commit_template: None,
                    auto_push: false,
                    gpg_sign: false,
                },
                deployment: DeploymentConfig {
                    targets: Vec::new(),
                    cicd: CicdPreferences {
                        platform: "github".to_string(),
                        auto_deploy: false,
                        run_tests: true,
                        notify_on_failure: true,
                    },
                },
            },
            security: SecurityConfig {
                keyring_backend: KeyringBackend::System,
                session_timeout: 60,
                enable_2fa: false,
                allowed_ip_ranges: Vec::new(),
            },
            metadata: ConfigMetadata {
                version: "1.0.0".to_string(),
                created_at: chrono::Utc::now().to_rfc3339(),
                updated_at: chrono::Utc::now().to_rfc3339(),
                schema_version: "1.0".to_string(),
                source: "default".to_string(),
                checksum: None,
            },
        }
    }
}

impl B00tUnifiedConfig {
    /// Create a new configuration with user information
    pub fn new_for_user(username: &str, email: Option<String>) -> Self {
        let mut config = Self::default();
        config.user.username = username.to_string();
        config.user.email = email;
        config.metadata.source = "user".to_string();
        config.metadata.created_at = chrono::Utc::now().to_rfc3339();
        config.metadata.updated_at = chrono::Utc::now().to_rfc3339();
        config
    }

    /// Update the configuration timestamp
    pub fn touch(&mut self) {
        self.metadata.updated_at = chrono::Utc::now().to_rfc3339();
    }

    /// Validate the configuration
    pub fn validate(&self) -> Result<()> {
        // Basic validation
        if self.user.username.is_empty() {
            return Err(anyhow!("Username cannot be empty"));
        }

        // Validate AI configuration
        if self.ai.providers.is_empty() {
            return Err(anyhow!("At least one AI provider must be configured"));
        }

        if !self.ai.providers.contains_key(&self.ai.default_provider) {
            return Err(anyhow!(
                "Default AI provider '{}' is not configured",
                self.ai.default_provider
            ));
        }

        // Validate cloud services if configured
        if let Some(ref cf) = self.cloud.cloudflare {
            if cf.account_id.is_empty() {
                return Err(anyhow!("Cloudflare account ID cannot be empty"));
            }
        }

        if let Some(ref aws) = self.cloud.aws {
            if aws.region.is_empty() {
                return Err(anyhow!("AWS region cannot be empty"));
            }
        }

        Ok(())
    }

    /// Get JSON schema for this configuration
    pub fn get_json_schema() -> schemars::schema::RootSchema {
        schemars::schema_for!(B00tUnifiedConfig)
    }

    /// Export TypeScript definitions
    pub fn export_typescript() -> Result<()> {
        use std::fs;

        // Create types directory if it doesn't exist
        let types_dir = "../../../b00t-dashboard/src/types";
        fs::create_dir_all(types_dir)
            .map_err(|e| anyhow!("Failed to create types directory: {}", e))?;

        // Export all types
        B00tUnifiedConfig::export()
            .map_err(|e| anyhow!("Failed to export B00tUnifiedConfig: {}", e))?;
        UserConfig::export().map_err(|e| anyhow!("Failed to export UserConfig: {}", e))?;
        CloudServicesConfig::export()
            .map_err(|e| anyhow!("Failed to export CloudServicesConfig: {}", e))?;
        AiConfiguration::export()
            .map_err(|e| anyhow!("Failed to export AiConfiguration: {}", e))?;

        println!("✅ TypeScript definitions exported to {}", types_dir);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config_validation() {
        let mut config = B00tUnifiedConfig::default();

        // Default config should be invalid (no providers)
        assert!(config.validate().is_err());

        // Add a provider to make it valid
        config.ai.providers.insert(
            "openai".to_string(),
            AiProviderConfig {
                provider: "openai".to_string(),
                model: "gpt-4".to_string(),
                api_key_keyring_key: Some("openai_api_key".to_string()),
                endpoint: None,
                enabled: true,
                priority: 10,
                settings: HashMap::new(),
            },
        );

        assert!(config.validate().is_ok());
    }

    #[test]
    fn test_user_config_creation() {
        let config =
            B00tUnifiedConfig::new_for_user("testuser", Some("test@example.com".to_string()));

        assert_eq!(config.user.username, "testuser");
        assert_eq!(config.user.email, Some("test@example.com".to_string()));
        assert_eq!(config.metadata.source, "user");
    }
}
