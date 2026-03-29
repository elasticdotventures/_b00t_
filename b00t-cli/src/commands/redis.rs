//! Internal KV store utilities for agent coordination
//!
//! 🤓 INTERNAL ONLY - not exposed as CLI commands
//! Provides transparent KV access for:
//! - Agent coordination and pub/sub
//! - Session storage
//! - Task state tracking
//!
//! Backend detection: Valkey > Redis > ForgeKV > File

use anyhow::{Context, Result};
use b00t_c0re_lib::kv_store::{KvConfig, KvStore, KvBackend};
use b00t_c0re_lib::redis::{AgentMessage, BroadcastPriority, RedisComms, RedisConfig};
use std::collections::HashMap;

/// Get internal KV store with auto-detected backend
/// 🤓 Silent detection - no output, used internally
pub fn get_kv_store() -> KvStore {
    let config = KvConfig::detect();
    KvStore::new(config)
}

/// Check if a real KV backend is available (not file fallback)
pub fn has_real_kv_backend() -> bool {
    let store = get_kv_store();
    matches!(store.backend(), KvBackend::Valkey | KvBackend::Redis | KvBackend::ForgeKV)
}

/// Get the current backend type for logging/debugging
pub fn get_backend_type() -> KvBackend {
    let store = get_kv_store();
    store.backend()
}

/// Simple KV operations for internal use
pub mod kv {
    use super::*;

    /// Get a value from KV store
    pub fn get(key: &str) -> Result<Option<String>> {
        let store = get_kv_store();
        store.get(key)
    }

    /// Set a value in KV store
    pub fn set(key: &str, value: &str, expire_secs: Option<u64>) -> Result<()> {
        let store = get_kv_store();
        store.set(key, value, expire_secs)
    }

    /// Delete a key from KV store
    pub fn del(key: &str) -> Result<usize> {
        let store = get_kv_store();
        store.del(key)
    }

    /// Check if key exists
    pub fn exists(key: &str) -> Result<bool> {
        let store = get_kv_store();
        store.exists(key)
    }

    /// Publish a message to a channel
    pub fn publish(channel: &str, message: &str) -> Result<usize> {
        let store = get_kv_store();
        store.publish(channel, message)
    }
}

/// Agent coordination helpers using KV store
pub mod agent_kv {
    use super::*;

    /// Register agent status
    pub fn register_agent(agent_id: &str, status: &str) -> Result<()> {
        let key = format!("b00t:agents:{}", agent_id);
        kv::set(&key, status, Some(300)) // 5 min TTL
    }

    /// Get agent status
    pub fn get_agent_status(agent_id: &str) -> Result<Option<String>> {
        let key = format!("b00t:agents:{}", agent_id);
        kv::get(&key)
    }

    /// List all registered agents
    pub fn list_agents() -> Result<Vec<String>> {
        // File backend doesn't support SCAN, so this is limited
        // Real backends would use SCAN b00t:agents:*
        Ok(vec![])
    }

    /// Broadcast message to all agents
    pub fn broadcast(message: &str, priority: BroadcastPriority) -> Result<usize> {
        let channel = "b00t:broadcast";
        let payload = serde_json::json!({
            "type": "broadcast",
            "message": message,
            "priority": format!("{:?}", priority)
        });
        kv::publish(channel, &payload.to_string())
    }
}

/// Session storage using KV backend
pub mod session_kv {
    use super::*;

    /// Store session data
    pub fn store_session(session_id: &str, data: &HashMap<String, serde_json::Value>) -> Result<()> {
        let key = format!("b00t:sessions:{}", session_id);
        let json = serde_json::to_string(data)?;
        kv::set(&key, &json, Some(3600)) // 1 hour TTL
    }

    /// Retrieve session data
    pub fn get_session(session_id: &str) -> Result<Option<HashMap<String, serde_json::Value>>> {
        let key = format!("b00t:sessions:{}", session_id);
        match kv::get(&key)? {
            Some(json) => {
                let data: HashMap<String, serde_json::Value> = serde_json::from_str(&json)?;
                Ok(Some(data))
            }
            None => Ok(None),
        }
    }

    /// Clear session data
    pub fn clear_session(session_id: &str) -> Result<usize> {
        let key = format!("b00t:sessions:{}", session_id);
        kv::del(&key)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_kv_store_creation() {
        let store = get_kv_store();
        // Should always return a valid store (may be File backend)
        assert!(matches!(
            store.backend(),
            KvBackend::Valkey | KvBackend::Redis | KvBackend::ForgeKV | KvBackend::File
        ));
    }

    #[test]
    fn test_backend_detection() {
        let backend = get_backend_type();
        // Just verify it returns a valid backend
        assert!(matches!(
            backend,
            KvBackend::Valkey | KvBackend::Redis | KvBackend::ForgeKV | KvBackend::File
        ));
    }
}
