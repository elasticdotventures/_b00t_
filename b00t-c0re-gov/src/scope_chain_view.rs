//! ScopeChainView — ordered scope resolution + JSONPath query bridge (#893).
//!
//! Wraps an ordered `Vec<Box<dyn ScopeStore>>` (most-specific first, e.g.
//! repo → node → global) and implements the chain's resolution contract:
//! GET walks the chain, most-specific wins; SET always targets an explicit
//! scope (no "write to the closest scope" — #893: "no silent shadowing").

use crate::errors::{ScopeError, ScopeResult};
use crate::scope_store::{ScopeId, ScopeStore};
use jsonpath_rust::JsonPath;
use serde_json::Value;

/// JSONPath-queryable bridge over a resolved value. Kept separate from
/// `ScopeStore` itself (which only knows flat get/set) -- a scope's stored
/// value can be an arbitrary JSON document; this is what lets a caller
/// reach into it (`$.foo.bar`) without a second, parallel query API.
pub trait Queryable {
    /// Resolve `key` through the chain (most-specific wins), then evaluate
    /// `path` against the resolved value. Empty (not an error) when the key
    /// isn't set anywhere in the chain.
    fn query(&self, key: &str, path: &str) -> ScopeResult<Vec<Value>>;
}

/// An ordered view across a repo → node → global scope chain.
pub struct ScopeChainView {
    /// Most-specific first.
    chain: Vec<Box<dyn ScopeStore>>,
}

impl ScopeChainView {
    /// `chain` must already be ordered most-specific-first by the caller —
    /// this type doesn't re-sort, it just walks what it's given.
    pub fn new(chain: Vec<Box<dyn ScopeStore>>) -> Self {
        Self { chain }
    }

    pub fn len(&self) -> usize {
        self.chain.len()
    }

    pub fn is_empty(&self) -> bool {
        self.chain.is_empty()
    }

    /// Most-specific wins: the first store in the chain that has `key` set.
    pub fn get_raw(&self, key: &str) -> ScopeResult<Option<Value>> {
        for store in &self.chain {
            if let Some(v) = store.get_raw(key)? {
                return Ok(Some(v));
            }
        }
        Ok(None)
    }

    /// Which scope actually holds `key` (most-specific wins), if any —
    /// provenance without a second raw lookup, e.g. for audit logging.
    pub fn resolving_scope(&self, key: &str) -> ScopeResult<Option<&ScopeId>> {
        for store in &self.chain {
            if store.get_raw(key)?.is_some() {
                return Ok(Some(store.scope_id()));
            }
        }
        Ok(None)
    }

    /// Explicit-target-only write: `target` must name a scope that is
    /// actually part of this chain. There is deliberately no "write to
    /// whichever scope is closest" method — #893's no-silent-shadowing
    /// requirement.
    pub fn set_raw(&mut self, target: &ScopeId, key: &str, val: Value) -> ScopeResult<()> {
        for store in &mut self.chain {
            if store.scope_id() == target {
                return store.set_raw(key, val);
            }
        }
        Err(ScopeError::InvalidScopeId(format!(
            "scope {target} is not part of this chain"
        )))
    }
}

impl Queryable for ScopeChainView {
    fn query(&self, key: &str, path: &str) -> ScopeResult<Vec<Value>> {
        let Some(value) = self.get_raw(key)? else {
            return Ok(Vec::new());
        };
        value
            .query(path)
            .map(|matches| matches.into_iter().cloned().collect())
            .map_err(|e| ScopeError::InvalidScopeId(format!("bad JSONPath {path:?}: {e}")))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    struct MemScopeStore {
        id: ScopeId,
        parent: Option<ScopeId>,
        data: HashMap<String, Value>,
    }

    impl MemScopeStore {
        fn new(id: ScopeId, parent: Option<ScopeId>) -> Self {
            Self {
                id,
                parent,
                data: HashMap::new(),
            }
        }
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

    fn three_tier_chain() -> ScopeChainView {
        ScopeChainView::new(vec![
            Box::new(MemScopeStore::new(
                ScopeId::Repo("myrepo".into()),
                Some(ScopeId::Node("myhost".into())),
            )),
            Box::new(MemScopeStore::new(
                ScopeId::Node("myhost".into()),
                Some(ScopeId::Global),
            )),
            Box::new(MemScopeStore::new(ScopeId::Global, None)),
        ])
    }

    #[test]
    fn most_specific_scope_wins_on_get() {
        let mut chain = three_tier_chain();
        chain
            .set_raw(&ScopeId::Global, "k", Value::String("global".into()))
            .unwrap();
        chain
            .set_raw(
                &ScopeId::Node("myhost".into()),
                "k",
                Value::String("node".into()),
            )
            .unwrap();
        chain
            .set_raw(
                &ScopeId::Repo("myrepo".into()),
                "k",
                Value::String("repo".into()),
            )
            .unwrap();

        assert_eq!(chain.get_raw("k").unwrap(), Some(Value::String("repo".into())));
    }

    #[test]
    fn falls_back_when_more_specific_scopes_unset() {
        let mut chain = three_tier_chain();
        chain
            .set_raw(&ScopeId::Global, "k", Value::String("global".into()))
            .unwrap();

        assert_eq!(
            chain.get_raw("k").unwrap(),
            Some(Value::String("global".into()))
        );
    }

    #[test]
    fn get_returns_none_when_unset_anywhere() {
        let chain = three_tier_chain();
        assert_eq!(chain.get_raw("nope").unwrap(), None);
    }

    #[test]
    fn set_raw_rejects_scope_not_in_chain() {
        let mut chain = three_tier_chain();
        let err = chain
            .set_raw(
                &ScopeId::Repo("some-other-repo".into()),
                "k",
                Value::String("v".into()),
            )
            .unwrap_err();
        assert!(matches!(err, ScopeError::InvalidScopeId(_)));
    }

    #[test]
    fn no_implicit_shadow_on_write_set_never_guesses_a_scope() {
        // There is no set_raw(key, val) overload that omits the target
        // scope -- this test exists to document that contract, not to
        // exercise runtime behavior (a missing method is a compile error,
        // which is the actual enforcement mechanism).
        let mut chain = three_tier_chain();
        chain
            .set_raw(&ScopeId::Global, "only-in-global", Value::from(1))
            .unwrap();
        assert_eq!(
            chain.resolving_scope("only-in-global").unwrap(),
            Some(&ScopeId::Global)
        );
    }

    #[test]
    fn resolving_scope_reports_provenance() {
        let mut chain = three_tier_chain();
        chain
            .set_raw(
                &ScopeId::Node("myhost".into()),
                "k",
                Value::String("v".into()),
            )
            .unwrap();
        assert_eq!(
            chain.resolving_scope("k").unwrap(),
            Some(&ScopeId::Node("myhost".into()))
        );
        assert_eq!(chain.resolving_scope("missing").unwrap(), None);
    }

    #[test]
    fn query_reaches_into_a_nested_stored_value() {
        let mut chain = three_tier_chain();
        chain
            .set_raw(
                &ScopeId::Global,
                "config",
                serde_json::json!({"b00t": {"name": "widget", "tags": ["a", "b"]}}),
            )
            .unwrap();

        let matches = chain.query("config", "$.b00t.name").unwrap();
        assert_eq!(matches, vec![Value::String("widget".into())]);
    }

    #[test]
    fn query_on_missing_key_returns_empty_not_error() {
        let chain = three_tier_chain();
        let matches = chain.query("nope", "$.anything").unwrap();
        assert_eq!(matches, Vec::<Value>::new());
    }
}
