//! RedisScopeStore v2 (stub) — distributed ScopeStore backend (#893 checklist).
//!
//! Deliberately built on `b00t_c0re_lib::redis::RedisComms`'s existing
//! `get`/`set` rather than a new redis client -- that crate already wraps
//! connection setup, TCP-connectivity error hints, and a
//! `{prefix}:{id}:{key}` namespacing convention
//! (`RedisSessionStorage::session_key`). Reinventing any of that here is
//! exactly the duplication flagged in scope_store.rs's own module doc.
//!
//! "Stub" because: no retry/backoff policy, no connection pooling, and the
//! single-writer-vs-distributed-concurrency question (#902) is explicitly
//! unresolved -- this proves the reuse shape and the ScopeStore contract,
//! not a production-hardened backend. Real hardening is #902's territory,
//! not this commit's.

use crate::errors::{ScopeError, ScopeResult};
use crate::scope_store::{ScopeId, ScopeStore};
use b00t_c0re_lib::redis::{RedisComms, RedisConfig};
use serde_json::Value;

/// A ScopeStore backed by RedisComms, namespaced per scope.
pub struct RedisScopeStore {
    comms: RedisComms,
    id: ScopeId,
    parent: Option<ScopeId>,
}

impl RedisScopeStore {
    pub fn open(config: RedisConfig, id: ScopeId, parent: Option<ScopeId>) -> ScopeResult<Self> {
        let comms = RedisComms::new(config, format!("scope-store:{id}"))
            .map_err(|e| ScopeError::BackendUnavailable(e.to_string()))?;
        Ok(Self { comms, id, parent })
    }

    /// True when the underlying Redis server actually answers PING.
    /// `RedisComms::new` never fails on its own (redis::Client::open is a
    /// lazy URL parse, not a connection attempt) -- this is the real
    /// "is the backend up" check, matching RedisComms::is_available.
    pub fn is_available(&self) -> bool {
        self.comms.is_available()
    }

    /// Namespaced key: `scope:{scope_id}:{key}`, mirroring
    /// `RedisSessionStorage::session_key`'s `{prefix}:{id}:{key}` shape
    /// rather than inventing a new convention.
    fn scoped_key(&self, key: &str) -> String {
        format!("scope:{}:{key}", self.id)
    }
}

impl ScopeStore for RedisScopeStore {
    fn get_raw(&self, key: &str) -> ScopeResult<Option<Value>> {
        let raw = self
            .comms
            .get(&self.scoped_key(key))
            .map_err(|e| ScopeError::BackendUnavailable(e.to_string()))?;
        match raw {
            None => Ok(None),
            Some(s) => Ok(Some(serde_json::from_str(&s)?)),
        }
    }

    fn set_raw(&mut self, key: &str, val: Value) -> ScopeResult<()> {
        let serialized = serde_json::to_string(&val)?;
        self.comms
            .set(&self.scoped_key(key), &serialized)
            .map_err(|e| ScopeError::BackendUnavailable(e.to_string()))?;
        Ok(())
    }

    fn scope_id(&self) -> &ScopeId {
        &self.id
    }

    fn parent(&self) -> Option<&ScopeId> {
        self.parent.as_ref()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn store() -> RedisScopeStore {
        RedisScopeStore::open(
            RedisConfig::default(),
            ScopeId::Repo("myrepo".into()),
            Some(ScopeId::Node("myhost".into())),
        )
        .unwrap()
    }

    #[test]
    fn open_never_fails_without_a_live_connection() {
        // RedisComms::new is a lazy client handle, not a connection
        // attempt -- open() must succeed even with no Redis reachable.
        let _store = store();
    }

    #[test]
    fn scoped_key_uses_session_storage_style_prefixing() {
        let s = store();
        assert_eq!(s.scoped_key("greeting"), "scope:repo:myrepo:greeting");
    }

    #[test]
    fn scope_identity_preserved() {
        let s = store();
        assert_eq!(s.scope_id(), &ScopeId::Repo("myrepo".into()));
        assert_eq!(s.parent(), Some(&ScopeId::Node("myhost".into())));
    }

    /// Live round-trip against a real Redis server -- gated on
    /// is_available() rather than assumed, since this sandbox has none
    /// installed. Runs for real wherever Redis is actually reachable
    /// (dev machines, CI with a redis service); documents the contract
    /// either way rather than silently omitting it.
    #[test]
    fn get_set_round_trips_when_redis_is_actually_available() {
        let mut s = store();
        if !s.is_available() {
            eprintln!("skipping: no Redis reachable in this environment");
            return;
        }
        s.set_raw("k", Value::String("v".into())).unwrap();
        assert_eq!(s.get_raw("k").unwrap(), Some(Value::String("v".into())));
    }
}
