//! Internal KV store abstraction with auto-detection
//!
//! Provides transparent key-value storage for b00t internal use:
//! 1. Valkey (Redis-compatible, preferred)
//! 2. Redis (redis-cli available and responding)
//! 3. ForgeKV (RESP2-compatible, rust-native)
//! 4. File-based fallback (development without KV server)
//!
//! 🤓 This is INTERNAL ONLY - no CLI exposure, used by agent coordination

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use shellexpand::tilde;
use std::collections::HashMap;
use std::fs;
use std::path::Path;
use std::process::Command;

/// KV store backend type (internal)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum KvBackend {
    Valkey,  // Preferred - fully open source
    Redis,   // Original
    ForgeKV, // Rust-native alternative
    File,    // Fallback for development
}

impl std::fmt::Display for KvBackend {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            KvBackend::Valkey => write!(f, "Valkey"),
            KvBackend::Redis => write!(f, "Redis"),
            KvBackend::ForgeKV => write!(f, "ForgeKV"),
            KvBackend::File => write!(f, "File"),
        }
    }
}

/// Unified KV configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KvConfig {
    pub backend: KvBackend,
    pub host: String,
    pub port: u16,
    pub password: Option<String>,
    pub database: u8,
    pub file_path: Option<String>, // For file backend
}

impl Default for KvConfig {
    fn default() -> Self {
        Self {
            backend: KvBackend::File, // Default to file for safety
            host: "localhost".to_string(),
            port: 6379,
            password: None,
            database: 0,
            file_path: Some("~/.b00t/kv-store.json".to_string()),
        }
    }
}

impl KvConfig {
    /// Detect best available KV backend (priority: Valkey > Redis > ForgeKV > File)
    pub fn detect() -> Self {
        // Try Valkey first (check INFO server for valkey signature)
        if Self::check_valkey() {
            eprintln!("🔍 KV backend detected: Valkey (preferred)");
            return Self {
                backend: KvBackend::Valkey,
                ..Default::default()
            };
        }

        // Try Redis (redis-cli PONG response)
        if Self::check_redis_cli() {
            eprintln!("🔍 KV backend detected: Redis");
            return Self {
                backend: KvBackend::Redis,
                ..Default::default()
            };
        }

        // Try ForgeKV (same RESP2 protocol, check for forgekv signature)
        if Self::check_forgekv() {
            eprintln!("🔍 KV backend detected: ForgeKV");
            return Self {
                backend: KvBackend::ForgeKV,
                ..Default::default()
            };
        }

        // Fallback to file-based (silent, for development)
        Self::default()
    }

    /// Check if Valkey is available (Redis-compatible with different INFO)
    fn check_valkey() -> bool {
        Command::new("redis-cli")
            .args(["-p", "6379", "INFO", "server"])
            .output()
            .ok()
            .and_then(|out| String::from_utf8(out.stdout).ok())
            .map(|s| s.to_lowercase().contains("valkey"))
            .unwrap_or(false)
    }

    /// Check if Redis is available (redis-cli PONG response)
    fn check_redis_cli() -> bool {
        Command::new("redis-cli")
            .args(["-p", "6379", "PING"])
            .output()
            .ok()
            .and_then(|out| String::from_utf8(out.stdout).ok())
            .map(|s| s.trim() == "PONG")
            .unwrap_or(false)
    }

    /// Check if ForgeKV is available (RESP2 compatible, check signature)
    fn check_forgekv() -> bool {
        // ForgeKV is RESP2 compatible, so same check as Redis
        // Could differentiate by checking INFO or version
        Command::new("redis-cli")
            .args(["-p", "6379", "INFO", "server"])
            .output()
            .ok()
            .and_then(|out| String::from_utf8(out.stdout).ok())
            .map(|s| s.contains("forgekv") || s.contains("FORGEKV"))
            .unwrap_or(false)
    }

    /// Build connection URL
    pub fn connection_url(&self) -> String {
        match &self.password {
            Some(password) => format!(
                "redis://:{}@{}:{}/{}",
                password, self.host, self.port, self.database
            ),
            None => format!("redis://{}:{}/{}", self.host, self.port, self.database),
        }
    }
}

/// Unified KV store client
pub struct KvStore {
    config: KvConfig,
    // Could add connection pooling, caching, etc.
}

impl KvStore {
    pub fn new(config: KvConfig) -> Self {
        Self { config }
    }

    pub fn with_auto_detect() -> Self {
        Self::new(KvConfig::detect())
    }

    /// Read-only access to resolved KV configuration.
    pub fn config(&self) -> &KvConfig {
        &self.config
    }

    /// Get a value from the KV store
    pub fn get(&self, key: &str) -> Result<Option<String>> {
        match self.config.backend {
            KvBackend::Valkey | KvBackend::Redis | KvBackend::ForgeKV => self.get_redis(key),
            KvBackend::File => self.get_file(key),
        }
    }

    /// Set a value in the KV store
    pub fn set(&self, key: &str, value: &str, expire_secs: Option<u64>) -> Result<()> {
        match self.config.backend {
            KvBackend::Valkey | KvBackend::Redis | KvBackend::ForgeKV => self.set_redis(key, value, expire_secs),
            KvBackend::File => self.set_file(key, value),
        }
    }

    /// Delete a key from the KV store
    pub fn del(&self, key: &str) -> Result<usize> {
        match self.config.backend {
            KvBackend::Valkey | KvBackend::Redis | KvBackend::ForgeKV => self.del_redis(key),
            KvBackend::File => self.del_file(key),
        }
    }

    /// Check if key exists
    pub fn exists(&self, key: &str) -> Result<bool> {
        match self.config.backend {
            KvBackend::Valkey | KvBackend::Redis | KvBackend::ForgeKV => self.exists_redis(key),
            KvBackend::File => self.exists_file(key),
        }
    }

    /// Publish a message to a channel
    pub fn publish(&self, channel: &str, message: &str) -> Result<usize> {
        match self.config.backend {
            KvBackend::Valkey | KvBackend::Redis | KvBackend::ForgeKV => self.publish_redis(channel, message),
            KvBackend::File => Ok(0), // File backend doesn't support pub/sub
        }
    }

    // Redis/ForgeKV implementations using redis-cli
    fn get_redis(&self, key: &str) -> Result<Option<String>> {
        let output = Command::new("redis-cli")
            .args(["-p", &self.config.port.to_string(), "GET", key])
            .output()
            .context("Failed to execute redis-cli GET")?;

        let result = String::from_utf8_lossy(&output.stdout);
        let trimmed = result.trim();
        if trimmed == "(nil)" || trimmed.is_empty() {
            Ok(None)
        } else {
            Ok(Some(trimmed.to_string()))
        }
    }

    fn set_redis(&self, key: &str, value: &str, expire_secs: Option<u64>) -> Result<()> {
        let port = self.config.port.to_string();
        if let Some(expire) = expire_secs {
            Command::new("redis-cli")
                .args(["-p", &port, "SET", key, value, "EX", &expire.to_string()])
                .output()
                .context("Failed to execute redis-cli SETEX")?;
        } else {
            Command::new("redis-cli")
                .args(["-p", &port, "SET", key, value])
                .output()
                .context("Failed to execute redis-cli SET")?;
        }
        Ok(())
    }

    fn del_redis(&self, key: &str) -> Result<usize> {
        let output = Command::new("redis-cli")
            .args(["-p", &self.config.port.to_string(), "DEL", key])
            .output()
            .context("Failed to execute redis-cli DEL")?;

        let result = String::from_utf8_lossy(&output.stdout);
        result.trim().parse::<usize>().context("Failed to parse DEL result")
    }

    fn exists_redis(&self, key: &str) -> Result<bool> {
        let output = Command::new("redis-cli")
            .args(["-p", &self.config.port.to_string(), "EXISTS", key])
            .output()
            .context("Failed to execute redis-cli EXISTS")?;

        let result = String::from_utf8_lossy(&output.stdout);
        Ok(result.trim() == "1")
    }

    fn publish_redis(&self, channel: &str, message: &str) -> Result<usize> {
        let output = Command::new("redis-cli")
            .args(["-p", &self.config.port.to_string(), "PUBLISH", channel, message])
            .output()
            .context("Failed to execute redis-cli PUBLISH")?;

        let result = String::from_utf8_lossy(&output.stdout);
        result.trim().parse::<usize>().context("Failed to parse PUBLISH result")
    }

    // File backend implementations
    fn get_file(&self, key: &str) -> Result<Option<String>> {
        let file_path = self.config.file_path.as_deref().unwrap_or("~/.b00t/kv-store.json");
        let expanded = shellexpand::tilde(file_path);

        if !Path::new(expanded.as_ref()).exists() {
            return Ok(None);
        }

        let content = fs::read_to_string(expanded.as_ref())
            .context("Failed to read KV store file")?;
        
        let data: HashMap<String, String> = serde_json::from_str(&content).unwrap_or_default();
        Ok(data.get(key).cloned())
    }

    fn set_file(&self, key: &str, value: &str) -> Result<()> {
        let file_path = self.config.file_path.as_deref().unwrap_or("~/.b00t/kv-store.json");
        let expanded = shellexpand::tilde(file_path);
        let path = Path::new(expanded.as_ref());

        // Create directory if needed
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).ok();
        }

        // Load existing data
        let mut data: HashMap<String, String> = if path.exists() {
            let content = fs::read_to_string(path)?;
            serde_json::from_str(&content).unwrap_or_default()
        } else {
            HashMap::new()
        };

        // Update and save
        data.insert(key.to_string(), value.to_string());
        let json = serde_json::to_string_pretty(&data)?;
        fs::write(path, json)?;
        
        Ok(())
    }

    fn del_file(&self, key: &str) -> Result<usize> {
        let file_path = self.config.file_path.as_deref().unwrap_or("~/.b00t/kv-store.json");
        let expanded = shellexpand::tilde(file_path);
        let path = Path::new(expanded.as_ref());

        if !path.exists() {
            return Ok(0);
        }

        let content = fs::read_to_string(path)?;
        let mut data: HashMap<String, String> = serde_json::from_str(&content).unwrap_or_default();
        
        let existed = data.remove(key).is_some();
        
        if existed {
            let json = serde_json::to_string_pretty(&data)?;
            fs::write(path, json)?;
            Ok(1)
        } else {
            Ok(0)
        }
    }

    fn exists_file(&self, key: &str) -> Result<bool> {
        let file_path = self.config.file_path.as_deref().unwrap_or("~/.b00t/kv-store.json");
        let expanded = shellexpand::tilde(file_path);
        
        if !std::path::Path::new(expanded.as_ref()).exists() {
            return Ok(false);
        }

        let content = fs::read_to_string(expanded.as_ref())?;
        let data: HashMap<String, String> = serde_json::from_str(&content).unwrap_or_default();
        Ok(data.contains_key(key))
    }

    /// Get backend type
    pub fn backend(&self) -> KvBackend {
        self.config.backend
    }

    /// Ping the KV store
    pub fn ping(&self) -> Result<bool> {
        match self.config.backend {
            KvBackend::Valkey | KvBackend::Redis | KvBackend::ForgeKV => {
                let output = Command::new("redis-cli")
                    .args(["-p", &self.config.port.to_string(), "PING"])
                    .output()
                    .context("Failed to execute redis-cli PING")?;

                let result = String::from_utf8_lossy(&output.stdout);
                Ok(result.trim() == "PONG")
            }
            KvBackend::File => {
                // File backend is always "available" if we can write to it
                let file_path = self.config.file_path.as_deref().unwrap_or("~/.b00t/kv-store.json");
                let expanded = shellexpand::tilde(file_path);
                let path = std::path::Path::new(expanded.as_ref());
                
                if let Some(parent) = path.parent() {
                    Ok(parent.exists() || std::fs::create_dir_all(parent).is_ok())
                } else {
                    Ok(true)
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_kv_config_detect() {
        let config = KvConfig::detect();
        // Should always return a valid config (may be File backend)
        assert!(matches!(config.backend, KvBackend::Valkey | KvBackend::Redis | KvBackend::ForgeKV | KvBackend::File));
    }

    #[test]
    fn test_kv_store_creation() {
        let config = KvConfig::default();
        let store = KvStore::new(config);
        assert_eq!(store.backend(), KvBackend::File);
    }
}
