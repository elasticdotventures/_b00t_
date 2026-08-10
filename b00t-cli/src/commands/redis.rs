//! Internal KV store utilities for agent coordination
//!
//! 🤓 INTERNAL ONLY - not exposed as CLI commands
//! Provides transparent KV access for:
//! - Agent coordination and pub/sub
//! - Session storage
//! - Task state tracking
//!
//! Backend detection: Valkey > Redis > ForgeKV > File

use anyhow::Result;
use b00t_c0re_gov::redis_scope_store::RedisScopeStore;
use b00t_c0re_gov::scope_store::{ScopeId, ScopeOp, ScopeStore, TransactionalScopeStore};
use b00t_c0re_lib::kv_store::{KvBackend, KvConfig, KvStore};
use b00t_c0re_lib::redis::{BroadcastPriority, RedisConfig};
use chrono::{Duration, Utc};
use std::collections::HashMap;

/// The one company-wide `ScopeStore::Global` instance, backed by
/// `RedisConfig::default()` (localhost:6379) — same connection default
/// `KvConfig`/`kv_store.rs` already use. `RedisScopeStore::open` never
/// fails without a live connection (lazy client handle), matching
/// `get_kv_store()`'s always-succeeds contract below.
fn global_scope_store() -> RedisScopeStore {
    RedisScopeStore::open(RedisConfig::default(), ScopeId::Global, None)
        .expect("RedisScopeStore::open is infallible without a live connection")
}

/// Get internal KV store with auto-detected backend
/// 🤓 Silent detection - no output, used internally
pub fn get_kv_store() -> KvStore {
    let config = KvConfig::detect();
    KvStore::new(config)
}

/// Check if a real KV backend is available (not file fallback)
pub fn has_real_kv_backend() -> bool {
    let store = get_kv_store();
    matches!(
        store.backend(),
        KvBackend::Valkey | KvBackend::Redis | KvBackend::ForgeKV
    )
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

/// Agent coordination helpers, now backed by `ScopeStore::Global` (kept as
/// a permanent, idiomatically-named facade — not a shim to delete later).
pub mod agent_kv {
    use super::*;

    /// Register agent status. TTL 5 minutes, same as before — now enforced
    /// via `ScopeEnvelope`'s `expires_at` instead of Redis SETEX.
    pub fn register_agent(agent_id: &str, status: &str) -> Result<()> {
        let key = format!("b00t:agents:{}", agent_id);
        let mut store = global_scope_store();
        store.transaction(vec![ScopeOp::Put {
            key,
            value: serde_json::Value::String(status.to_string()),
            expect_gen: None,
            expires_at: Some(Utc::now() + Duration::seconds(300)),
        }])?;
        Ok(())
    }

    /// Get agent status.
    pub fn get_agent_status(agent_id: &str) -> Result<Option<String>> {
        let key = format!("b00t:agents:{}", agent_id);
        let store = global_scope_store();
        match store.get_raw(&key)? {
            None => Ok(None),
            Some(envelope_json) => {
                let envelope: b00t_c0re_gov::scope_store::ScopeEnvelope =
                    serde_json::from_value(envelope_json)?;
                if envelope.is_expired(Utc::now()) {
                    return Ok(None);
                }
                Ok(envelope.v.as_str().map(|s| s.to_string()))
            }
        }
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

/// Session storage, now backed by `ScopeStore::Global` (permanent facade,
/// same reasoning as `agent_kv` above).
pub mod session_kv {
    use super::*;

    /// Store session data. TTL 1 hour, same as before.
    pub fn store_session(
        session_id: &str,
        data: &HashMap<String, serde_json::Value>,
    ) -> Result<()> {
        let key = format!("b00t:sessions:{}", session_id);
        let value = serde_json::to_value(data)?;
        let mut store = global_scope_store();
        store.transaction(vec![ScopeOp::Put {
            key,
            value,
            expect_gen: None,
            expires_at: Some(Utc::now() + Duration::seconds(3600)),
        }])?;
        Ok(())
    }

    /// Retrieve session data.
    pub fn get_session(session_id: &str) -> Result<Option<HashMap<String, serde_json::Value>>> {
        let key = format!("b00t:sessions:{}", session_id);
        let store = global_scope_store();
        match store.get_raw(&key)? {
            None => Ok(None),
            Some(envelope_json) => {
                let envelope: b00t_c0re_gov::scope_store::ScopeEnvelope =
                    serde_json::from_value(envelope_json)?;
                if envelope.is_expired(Utc::now()) {
                    return Ok(None);
                }
                let data: HashMap<String, serde_json::Value> = serde_json::from_value(envelope.v)?;
                Ok(Some(data))
            }
        }
    }

    /// Clear session data. Returns 1 if a key was deleted, 0 if it was
    /// already absent — matches the old `kv::del` return-count contract.
    pub fn clear_session(session_id: &str) -> Result<usize> {
        let key = format!("b00t:sessions:{}", session_id);
        let mut store = global_scope_store();
        let existed = store.get_raw(&key)?.is_some();
        if !existed {
            return Ok(0);
        }
        store.transaction(vec![ScopeOp::Delete { key, expect_gen: None }])?;
        Ok(1)
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

    #[test]
    fn agent_kv_register_and_get_status_round_trip_when_redis_is_actually_available() {
        let store = global_scope_store();
        if !store.is_available() {
            eprintln!("skipping: no Redis reachable in this environment");
            return;
        }
        agent_kv::register_agent("parity-test-agent", "online").unwrap();
        let status = agent_kv::get_agent_status("parity-test-agent").unwrap();
        assert_eq!(status, Some("online".to_string()));
    }

    #[test]
    fn session_kv_store_and_get_round_trip_when_redis_is_actually_available() {
        let store = global_scope_store();
        if !store.is_available() {
            eprintln!("skipping: no Redis reachable in this environment");
            return;
        }
        let mut data = HashMap::new();
        data.insert("k".to_string(), serde_json::json!("v"));
        session_kv::store_session("parity-test-session", &data).unwrap();
        let round_tripped = session_kv::get_session("parity-test-session").unwrap();
        assert_eq!(round_tripped, Some(data));

        let deleted = session_kv::clear_session("parity-test-session").unwrap();
        assert_eq!(deleted, 1);
        assert_eq!(session_kv::get_session("parity-test-session").unwrap(), None);
    }
}
