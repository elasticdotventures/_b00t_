//! Cloud configuration synchronization for b00t-cli
//! 
//! Syncs AI provider configurations from b00t-website cloud service
//! to local b00t-cli sessions for unified model management.

use anyhow::{anyhow, Result};
use b00t_c0re_lib::AiClientConfig;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::time::{Duration, SystemTime};

use crate::session_memory::SessionMemory;

/// Cloud sync configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CloudSyncConfig {
    pub cloud_endpoint: String,
    pub sync_interval_seconds: u64,
    pub last_sync: Option<SystemTime>,
    pub enabled: bool,
    pub auth_token: Option<String>,
}

impl Default for CloudSyncConfig {
    fn default() -> Self {
        Self {
            cloud_endpoint: "https://b00t.promptexecution.com/worker".to_string(),
            sync_interval_seconds: 300, // 5 minutes
            last_sync: None,
            enabled: true,
            auth_token: None,
        }
    }
}

/// Cloud sync client for fetching AI configurations
pub struct CloudSyncClient {
    client: Client,
    config: CloudSyncConfig,
}

impl CloudSyncClient {
    /// Create new cloud sync client
    pub fn new(config: CloudSyncConfig) -> Result<Self> {
        let client = Client::builder()
            .timeout(Duration::from_secs(30))
            .build()
            .map_err(|e| anyhow!("Failed to create HTTP client: {}", e))?;

        Ok(Self { client, config })
    }

    /// Create cloud sync client with default configuration
    pub fn default() -> Result<Self> {
        Self::new(CloudSyncConfig::default())
    }

    /// Check if sync is needed based on interval
    pub fn should_sync(&self) -> bool {
        if !self.config.enabled {
            return false;
        }

        match self.config.last_sync {
            Some(last_sync) => {
                let elapsed = SystemTime::now()
                    .duration_since(last_sync)
                    .unwrap_or(Duration::from_secs(0));
                elapsed.as_secs() >= self.config.sync_interval_seconds
            }
            None => true, // Never synced before
        }
    }

    /// Fetch AI configuration from cloud website
    pub async fn fetch_ai_config(&self) -> Result<AiClientConfig> {
        if let Some(token) = &self.config.auth_token {
            self.fetch_ai_config_authenticated(token).await
        } else {
            // Try to get auth token from session memory
            match SessionMemory::load() {
                Ok(memory) => {
                    if let Some(github_user) = memory.get("github_user") {
                        self.fetch_ai_config_for_user(&github_user).await
                    } else {
                        Err(anyhow!("No authentication available for cloud sync"))
                    }
                }
                Err(_) => Err(anyhow!("No session available for cloud sync")),
            }
        }
    }

    /// Fetch AI config with authentication token
    async fn fetch_ai_config_authenticated(&self, token: &str) -> Result<AiClientConfig> {
        let url = format!("{}/ai-config", self.config.cloud_endpoint);
        
        let response = self
            .client
            .get(&url)
            .header("Authorization", format!("Bearer {}", token))
            .send()
            .await
            .map_err(|e| anyhow!("Failed to fetch AI config: {}", e))?;

        if !response.status().is_success() {
            return Err(anyhow!(
                "Failed to fetch AI config: HTTP {}",
                response.status()
            ));
        }

        let cloud_config: CloudAiConfig = response
            .json()
            .await
            .map_err(|e| anyhow!("Failed to parse AI config response: {}", e))?;

        Ok(self.convert_cloud_config_to_client_config(cloud_config))
    }

    /// Fetch AI config for specific GitHub user (fallback method)
    async fn fetch_ai_config_for_user(&self, username: &str) -> Result<AiClientConfig> {
        // This would require implementing a public endpoint for user configs
        // For now, return a default configuration
        eprintln!("⚠️ Cloud sync not fully authenticated, using default AI config");
        Ok(AiClientConfig::default())
    }

    /// Convert cloud configuration format to client configuration
    fn convert_cloud_config_to_client_config(&self, cloud_config: CloudAiConfig) -> AiClientConfig {
        let mut providers = HashMap::new();

        for (provider_name, cloud_provider) in cloud_config.providers {
            providers.insert(provider_name.clone(), b00t_c0re_lib::AiProviderConfig {
                provider: provider_name,
                model: cloud_provider.model,
                api_key: None, // API keys are stored separately in local keyring
                endpoint: None,
                enabled: cloud_provider.enabled,
                priority: cloud_provider.priority,
            });
        }

        AiClientConfig {
            providers,
            default_provider: cloud_config.default_provider,
            fallback_provider: cloud_config.fallback_provider,
            last_updated: Some(cloud_config.last_updated.unwrap_or_else(|| {
                chrono::Utc::now().to_rfc3339()
            })),
        }
    }

    /// Sync configuration with cloud dashboard
    pub async fn sync_config(&mut self, memory: &mut SessionMemory) -> Result<bool> {
        if !self.should_sync() {
            return Ok(false);
        }

        match self.fetch_ai_config().await {
            Ok(ai_config) => {
                // Store the synced configuration in session memory
                memory.set(
                    "ai_config_json",
                    &serde_json::to_string(&ai_config)
                        .map_err(|e| anyhow!("Failed to serialize AI config: {}", e))?,
                )?;

                // Update last sync timestamp
                self.config.last_sync = Some(SystemTime::now());
                
                // Store updated sync config in session
                memory.set(
                    "cloud_sync_config",
                    &serde_json::to_string(&self.config)
                        .map_err(|e| anyhow!("Failed to serialize sync config: {}", e))?,
                )?;

                memory.save()?;

                eprintln!("✅ Synced AI configuration from cloud dashboard");
                Ok(true)
            }
            Err(e) => {
                eprintln!("⚠️ Cloud sync failed: {}", e);
                // Don't treat sync failures as critical errors
                Ok(false)
            }
        }
    }

    /// Get AI configuration from session memory (cached)
    pub fn get_cached_ai_config(memory: &SessionMemory) -> Option<AiClientConfig> {
        memory.get("ai_config_json")
            .and_then(|json| serde_json::from_str(&json).ok())
    }
}

/// Cloud AI configuration format (matches worker API response)
#[derive(Debug, Serialize, Deserialize)]
struct CloudAiConfig {
    providers: HashMap<String, CloudProviderConfig>,
    default_provider: String,
    fallback_provider: Option<String>,
    last_updated: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
struct CloudProviderConfig {
    enabled: bool,
    model: String,
    priority: u8,
}

/// Initialize cloud sync for session
pub fn init_cloud_sync(memory: &mut SessionMemory) -> Result<CloudSyncClient> {
    // Try to load existing sync config from session
    let sync_config = if let Some(config_json) = memory.get("cloud_sync_config") {
        serde_json::from_str(&config_json).unwrap_or_default()
    } else {
        CloudSyncConfig::default()
    };

    CloudSyncClient::new(sync_config)
}

/// Periodic cloud sync task
pub async fn periodic_cloud_sync() -> Result<()> {
    let mut memory = SessionMemory::load().map_err(|e| {
        anyhow!("Failed to load session memory for cloud sync: {}", e)
    })?;

    let mut sync_client = init_cloud_sync(&mut memory)?;

    // Perform sync if needed
    match sync_client.sync_config(&mut memory).await {
        Ok(true) => {
            eprintln!("🔄 Cloud configuration sync completed");
        }
        Ok(false) => {
            // No sync needed or sync failed non-critically
        }
        Err(e) => {
            eprintln!("❌ Critical cloud sync error: {}", e);
            return Err(e);
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_sync_config() {
        let config = CloudSyncConfig::default();
        assert!(config.enabled);
        assert_eq!(config.sync_interval_seconds, 300);
        assert!(config.last_sync.is_none());
    }

    #[test]
    fn test_should_sync_never_synced() {
        let config = CloudSyncConfig::default();
        let client = CloudSyncClient::new(config).unwrap();
        assert!(client.should_sync());
    }

    #[test]
    fn test_should_sync_recent() {
        let mut config = CloudSyncConfig::default();
        config.last_sync = Some(SystemTime::now());
        let client = CloudSyncClient::new(config).unwrap();
        assert!(!client.should_sync());
    }

    #[test]
    fn test_disabled_sync() {
        let mut config = CloudSyncConfig::default();
        config.enabled = false;
        let client = CloudSyncClient::new(config).unwrap();
        assert!(!client.should_sync());
    }
}