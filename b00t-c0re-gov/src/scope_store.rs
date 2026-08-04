//! ScopeStore — object-safe scope/data-namespace-level KV abstraction.
//!
//! One trait, many eventual backends (redb local, redis distributed), with
//! explicit repo/node/global scope chaining. See _b00t_ issue #893 (the
//! umbrella plan) and #894 (this trait's own object-safety requirements).
//!
//! 🤓 Reuse note: before adding a concrete backend on top of this trait,
//!    check b00t-c0re-lib::kv_store::KvStore (Valkey/Redis/ForgeKV/File
//!    auto-detect + fallback, already working) and
//!    b00t-c0re-lib::redis::RedisComms (get/set/hget/hset + session_key
//!    prefixing) first — this repo already has two independent KV
//!    abstractions; a third one reinventing either is exactly the
//!    duplication this trait exists to consolidate away, not add to.

use crate::errors::ScopeResult;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::fmt;

/// Identifies a scope in the repo → node → global chain.
///
/// `Repo` carries an opaque identity string (e.g. `sha256(remote_url)`) so
/// distinct repos never collide; submodule boundaries are each a discrete
/// `Repo` scope, never flattened into their parent (see #893's "Resolution"
/// section — GET walks repo → submodule-parents → node → global, boundary
/// crossings logged, not merged).
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ScopeId {
    /// A single repo (or submodule) root, identified by an opaque key.
    Repo(String),
    /// A single node/host, identified by hostname.
    Node(String),
    /// The one global scope shared across all repos and nodes.
    Global,
}

impl fmt::Display for ScopeId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ScopeId::Repo(id) => write!(f, "repo:{id}"),
            ScopeId::Node(id) => write!(f, "node:{id}"),
            ScopeId::Global => write!(f, "global"),
        }
    }
}

/// Object-safe scope store: a single scope's raw get/set, plus enough
/// identity to walk the repo → node → global chain above it.
///
/// Deliberately narrow — no query layer here (see `Queryable`, planned for
/// `ScopeChainView`), no async runtime requirement baked into the trait
/// signature (methods return plain `ScopeResult<T>`; a backend whose I/O is
/// genuinely async wraps its own blocking boundary internally rather than
/// forcing every caller through `async-trait`'s object-safety tax when most
/// callers — CLI commands — are synchronous anyway).
///
/// SET always targets an explicit scope: there is no "write to the closest
/// scope" method on this trait, by design (#893: "no silent shadowing").
/// Callers that want scope-chain-aware writes go through `ScopeChainView`,
/// not this trait directly.
pub trait ScopeStore: Send + Sync {
    /// Read a single key from this scope only (no chain walking).
    fn get_raw(&self, key: &str) -> ScopeResult<Option<Value>>;

    /// Write a single key to this scope only.
    fn set_raw(&mut self, key: &str, val: Value) -> ScopeResult<()>;

    /// This store's own identity.
    fn scope_id(&self) -> &ScopeId;

    /// The scope this one falls back to on a cache miss, if any.
    /// `Global` has no parent. A `Repo` scope's parent is either its
    /// submodule parent `Repo` (if any) or `Node`; a `Node` scope's parent
    /// is `Global`.
    fn parent(&self) -> Option<&ScopeId>;
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::errors::ScopeError;
    use std::collections::HashMap;

    /// Minimal in-memory ScopeStore impl — exists only to prove the trait
    /// is object-safe (constructible as `Box<dyn ScopeStore>`) and that the
    /// resolution-order/no-implicit-shadow contract is checkable against a
    /// real (if trivial) implementation, per #893's own test checklist.
    struct MemScopeStore {
        id: ScopeId,
        parent: Option<ScopeId>,
        data: HashMap<String, Value>,
    }

    impl ScopeStore for MemScopeStore {
        fn get_raw(&self, key: &str) -> ScopeResult<Option<Value>> {
            Ok(self.data.get(key).cloned())
        }

        fn set_raw(&mut self, key: &str, val: Value) -> ScopeResult<()> {
            self.data.insert(key.to_string(), val);
            Ok(())
        }

        fn scope_id(&self) -> &ScopeId {
            &self.id
        }

        fn parent(&self) -> Option<&ScopeId> {
            self.parent.as_ref()
        }
    }

    #[test]
    fn scope_store_is_object_safe() {
        let store: Box<dyn ScopeStore> = Box::new(MemScopeStore {
            id: ScopeId::Repo("abc123".into()),
            parent: Some(ScopeId::Node("host1".into())),
            data: HashMap::new(),
        });
        assert_eq!(store.scope_id(), &ScopeId::Repo("abc123".into()));
        assert_eq!(store.parent(), Some(&ScopeId::Node("host1".into())));
    }

    #[test]
    fn get_set_round_trips_within_one_scope() {
        let mut store = MemScopeStore {
            id: ScopeId::Global,
            parent: None,
            data: HashMap::new(),
        };
        assert_eq!(store.get_raw("k").unwrap(), None);
        store.set_raw("k", Value::String("v".into())).unwrap();
        assert_eq!(store.get_raw("k").unwrap(), Some(Value::String("v".into())));
    }

    #[test]
    fn global_scope_has_no_parent() {
        let store = MemScopeStore {
            id: ScopeId::Global,
            parent: None,
            data: HashMap::new(),
        };
        assert_eq!(store.parent(), None);
    }

    #[test]
    fn scope_id_display_matches_stereotype_prefix() {
        assert_eq!(ScopeId::Repo("x".into()).to_string(), "repo:x");
        assert_eq!(ScopeId::Node("h".into()).to_string(), "node:h");
        assert_eq!(ScopeId::Global.to_string(), "global");
    }

    #[test]
    fn is_transient_taxonomy_splits_correctly() {
        assert!(ScopeError::BackendUnavailable("down".into()).is_transient());
        assert!(!ScopeError::NotFound("k".into()).is_transient());
        assert!(!ScopeError::WriteRejected("secret".into()).is_transient());
        assert!(!ScopeError::InvalidScopeId("??".into()).is_transient());
    }
}
