//! ScopeChainView — ordered scope resolution + JSONPath query bridge (#893).
//!
//! Wraps an ordered `Vec<Box<dyn ScopeStore>>` (most-specific first, e.g.
//! repo → node → global) and implements the chain's resolution contract:
//! GET walks the chain, most-specific wins; SET always targets an explicit
//! scope (no "write to the closest scope" — #893: "no silent shadowing").

use crate::errors::{ScopeError, ScopeResult};
use crate::scope_audit::{AuditDirection, AuditEvent, AuditLogger, BoundaryCrossing};
use crate::scope_store::{ScopeId, ScopeStore};
use chrono::Utc;
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

    /// Same resolution as `get_raw`, but records an `AuditEvent` (#900) to
    /// `logger`: every scope checked-and-missed before the value resolved
    /// (or every scope checked if it wasn't found anywhere), as an ordered
    /// `boundaries_crossed[from,to,direction]` list -- not a boolean. A
    /// hit on the first (most-specific) scope logs zero crossings, which
    /// is itself meaningful data (see scope_audit's test for why).
    pub fn get_raw_with_audit(
        &self,
        key: &str,
        logger: &AuditLogger,
    ) -> ScopeResult<Option<Value>> {
        let mut crossings = Vec::new();
        let mut resolved_at = None;
        let mut result = None;

        for pair in self.chain.windows(2) {
            let (from, to) = (&pair[0], &pair[1]);
            if let Some(v) = from.get_raw(key)? {
                result = Some(v);
                resolved_at = Some(from.scope_id().clone());
                break;
            }
            crossings.push(BoundaryCrossing {
                from: from.scope_id().clone(),
                to: to.scope_id().clone(),
                direction: AuditDirection::Read,
            });
        }
        if result.is_none() {
            if let Some(last) = self.chain.last() {
                if let Some(v) = last.get_raw(key)? {
                    result = Some(v);
                    resolved_at = Some(last.scope_id().clone());
                }
            }
        }

        logger.append(&AuditEvent {
            timestamp: Utc::now(),
            key: key.to_string(),
            boundaries_crossed: crossings,
            resolved_at,
            direction: AuditDirection::Read,
        })?;

        Ok(result)
    }

    /// Explicit-target-only write: `target` must name a scope that is
    /// actually part of this chain. There is deliberately no "write to
    /// whichever scope is closest" method — #893's no-silent-shadowing
    /// requirement.
    ///
    /// Also runs the credential guard (#899) unconditionally, before
    /// touching any backend: a credential-shaped key is rejected at
    /// every scope, not just repo-scope — see scope_credential_guard.rs
    /// for why "everywhere" replaced the original "repo-scope only"
    /// framing.
    ///
    /// Unaudited. Prefer `set_raw_with_audit` (or the `Writable` trait,
    /// which is backed by it) at real call sites — kept `pub` rather than
    /// made implementation-private because backends/tests still construct
    /// scopes directly, but `set_raw_with_audit` is the seam #895 asks
    /// call sites to depend on.
    pub fn set_raw(&mut self, target: &ScopeId, key: &str, val: Value) -> ScopeResult<()> {
        crate::scope_credential_guard::guard_write(key)?;
        for store in &mut self.chain {
            if store.scope_id() == target {
                return store.set_raw(key, val);
            }
        }
        Err(ScopeError::InvalidScopeId(format!(
            "scope {target} is not part of this chain"
        )))
    }

    /// Same explicit-target-only write as `set_raw`, but records an
    /// `AuditEvent` (#900) to `logger` on success — the write-side mirror
    /// of `get_raw_with_audit`. A write never chain-walks (there's exactly
    /// one target, chosen by the caller, not resolved by search), so
    /// `boundaries_crossed` is always empty and `resolved_at` is always
    /// `Some(target)`; the event's top-level `direction` is what actually
    /// distinguishes it from a read in the log — see `AuditEvent::direction`
    /// for why that field exists.
    ///
    /// Only a *successful* write is audited: a credential-guard rejection
    /// or an out-of-chain target returns its error without appending
    /// anything — there's no write to attest to.
    pub fn set_raw_with_audit(
        &mut self,
        target: &ScopeId,
        key: &str,
        val: Value,
        logger: &AuditLogger,
    ) -> ScopeResult<()> {
        self.set_raw(target, key, val)?;

        logger.append(&AuditEvent {
            timestamp: Utc::now(),
            key: key.to_string(),
            boundaries_crossed: Vec::new(),
            resolved_at: Some(target.clone()),
            direction: AuditDirection::Write,
        })?;

        Ok(())
    }
}

/// Path-addressed, explicit-target-only, audited write — the write-side
/// counterpart to `Queryable`. `path` is a flat, dot-namespaced key (the
/// same addressing `set_raw`/`ScopeStore::set_raw` already use, e.g.
/// `"openai.credential"` or `"b00t.name"`), not a JSONPath expression into
/// an existing stored document — that's a separate, underspecified
/// follow-up (create-vs-overwrite-vs-array-append semantics need their own
/// design note, per #895's triage) deliberately not folded in here.
///
/// The trait exists so call sites depend on this seam instead of reaching
/// for a concrete type's `set_raw`/`set_raw_with_audit` directly — #893's
/// own issue body calls out "every call site reaches for `set_raw`
/// directly" as the gap this closes.
pub trait Writable {
    fn set_path(
        &mut self,
        target: &ScopeId,
        path: &str,
        val: Value,
        logger: &AuditLogger,
    ) -> ScopeResult<()>;
}

impl Writable for ScopeChainView {
    fn set_path(
        &mut self,
        target: &ScopeId,
        path: &str,
        val: Value,
        logger: &AuditLogger,
    ) -> ScopeResult<()> {
        self.set_raw_with_audit(target, path, val, logger)
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

    #[test]
    fn audited_read_records_boundaries_crossed_on_the_way_to_the_hit() {
        let dir = tempfile::tempdir().unwrap();
        let logger = crate::scope_audit::AuditLogger::open(dir.path().join("audit.jsonl"));

        let mut chain = three_tier_chain();
        // Only Global has it -- repo and node scopes must both be
        // checked-and-missed first.
        chain
            .set_raw(&ScopeId::Global, "k", Value::String("v".into()))
            .unwrap();

        let result = chain.get_raw_with_audit("k", &logger).unwrap();
        assert_eq!(result, Some(Value::String("v".into())));

        let events = logger.read_all().unwrap();
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].key, "k");
        assert_eq!(events[0].resolved_at, Some(ScopeId::Global));
        assert_eq!(
            events[0].boundaries_crossed,
            vec![
                crate::scope_audit::BoundaryCrossing {
                    from: ScopeId::Repo("myrepo".into()),
                    to: ScopeId::Node("myhost".into()),
                    direction: crate::scope_audit::AuditDirection::Read,
                },
                crate::scope_audit::BoundaryCrossing {
                    from: ScopeId::Node("myhost".into()),
                    to: ScopeId::Global,
                    direction: crate::scope_audit::AuditDirection::Read,
                },
            ]
        );
    }

    #[test]
    fn audited_read_hit_on_most_specific_scope_crosses_nothing() {
        let dir = tempfile::tempdir().unwrap();
        let logger = crate::scope_audit::AuditLogger::open(dir.path().join("audit.jsonl"));

        let mut chain = three_tier_chain();
        chain
            .set_raw(&ScopeId::Repo("myrepo".into()), "k", Value::from(1))
            .unwrap();

        chain.get_raw_with_audit("k", &logger).unwrap();

        let events = logger.read_all().unwrap();
        assert_eq!(events.len(), 1);
        assert!(events[0].boundaries_crossed.is_empty());
        assert_eq!(events[0].resolved_at, Some(ScopeId::Repo("myrepo".into())));
    }

    #[test]
    fn audited_read_miss_everywhere_still_logs_every_boundary_checked() {
        let dir = tempfile::tempdir().unwrap();
        let logger = crate::scope_audit::AuditLogger::open(dir.path().join("audit.jsonl"));

        let chain = three_tier_chain();
        let result = chain.get_raw_with_audit("nowhere", &logger).unwrap();
        assert_eq!(result, None);

        let events = logger.read_all().unwrap();
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].resolved_at, None);
        assert_eq!(events[0].boundaries_crossed.len(), 2, "checked all 3 scopes -> 2 boundaries");
    }

    #[test]
    fn set_raw_rejects_credential_shaped_key_at_every_scope() {
        // #899: not just repo-scope -- global too.
        for target in [
            ScopeId::Repo("myrepo".into()),
            ScopeId::Node("myhost".into()),
            ScopeId::Global,
        ] {
            let mut chain = three_tier_chain();
            let err = chain
                .set_raw(&target, "openai.credential", Value::String("sk-...".into()))
                .unwrap_err();
            assert!(
                matches!(err, ScopeError::WriteRejected(_)),
                "expected WriteRejected for target {target:?}, got {err:?}"
            );
        }
    }

    #[test]
    fn set_raw_still_allows_ordinary_keys() {
        let mut chain = three_tier_chain();
        chain
            .set_raw(&ScopeId::Global, "greeting", Value::String("hi".into()))
            .unwrap();
        assert_eq!(
            chain.get_raw("greeting").unwrap(),
            Some(Value::String("hi".into()))
        );
    }

    #[test]
    fn audited_write_logs_a_write_direction_event_with_no_boundary_crossings() {
        let dir = tempfile::tempdir().unwrap();
        let logger = crate::scope_audit::AuditLogger::open(dir.path().join("audit.jsonl"));

        let mut chain = three_tier_chain();
        chain
            .set_raw_with_audit(
                &ScopeId::Node("myhost".into()),
                "k",
                Value::String("v".into()),
                &logger,
            )
            .unwrap();

        // The write itself actually landed.
        assert_eq!(chain.get_raw("k").unwrap(), Some(Value::String("v".into())));

        let events = logger.read_all().unwrap();
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].key, "k");
        assert_eq!(events[0].direction, crate::scope_audit::AuditDirection::Write);
        assert_eq!(
            events[0].resolved_at,
            Some(ScopeId::Node("myhost".into()))
        );
        assert!(
            events[0].boundaries_crossed.is_empty(),
            "a write targets exactly one scope directly -- no chain walk to cross"
        );
    }

    #[test]
    fn audited_write_rejected_by_credential_guard_logs_nothing() {
        let dir = tempfile::tempdir().unwrap();
        let logger = crate::scope_audit::AuditLogger::open(dir.path().join("audit.jsonl"));

        let mut chain = three_tier_chain();
        let err = chain
            .set_raw_with_audit(
                &ScopeId::Global,
                "openai.credential",
                Value::String("sk-...".into()),
                &logger,
            )
            .unwrap_err();
        assert!(matches!(err, ScopeError::WriteRejected(_)));

        assert_eq!(
            logger.read_all().unwrap(),
            Vec::new(),
            "a rejected write never happened -- nothing to attest to"
        );
    }

    #[test]
    fn audited_write_to_scope_not_in_chain_logs_nothing() {
        let dir = tempfile::tempdir().unwrap();
        let logger = crate::scope_audit::AuditLogger::open(dir.path().join("audit.jsonl"));

        let mut chain = three_tier_chain();
        let err = chain
            .set_raw_with_audit(
                &ScopeId::Repo("some-other-repo".into()),
                "k",
                Value::from(1),
                &logger,
            )
            .unwrap_err();
        assert!(matches!(err, ScopeError::InvalidScopeId(_)));
        assert_eq!(logger.read_all().unwrap(), Vec::new());
    }

    /// Exercises `Writable` as a trait object -- the actual seam #895 asks
    /// for: a call site written against `&mut dyn Writable` never sees
    /// `ScopeChainView`'s concrete `set_raw`/`set_raw_with_audit` methods,
    /// only `set_path`.
    fn write_via_seam(target: &mut dyn Writable, scope: &ScopeId, logger: &AuditLogger) {
        target
            .set_path(scope, "seam-key", Value::String("via-trait".into()), logger)
            .unwrap();
    }

    #[test]
    fn writable_trait_seam_reaches_the_backend_and_the_audit_log() {
        let dir = tempfile::tempdir().unwrap();
        let logger = crate::scope_audit::AuditLogger::open(dir.path().join("audit.jsonl"));

        let mut chain = three_tier_chain();
        write_via_seam(&mut chain, &ScopeId::Global, &logger);

        assert_eq!(
            chain.get_raw("seam-key").unwrap(),
            Some(Value::String("via-trait".into()))
        );
        let events = logger.read_all().unwrap();
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].direction, crate::scope_audit::AuditDirection::Write);
    }
}
