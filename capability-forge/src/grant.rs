use crate::tiers::SkillTier;
use anyhow::{bail, Result};
use b00t_c0re_gov::errors::ScopeError;
use b00t_c0re_gov::scope_store::{ScopeEnvelope, ScopeOp, ScopeOpResult, ScopeStore, TransactionalScopeStore};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use uuid::Uuid;

/// A single skill grant issued to an agent, persisted at
/// `capforge:grant:{jti}` via [`persist_grant`].
///
/// # Envelope warning
/// Grant records are written through `TransactionalScopeStore::transaction`'s
/// `Put` op, which wraps every value in `ScopeEnvelope` (CAS generation +
/// expiry) before it lands in the backing table. A future `get_raw`
/// read of `capforge:grant:{jti}` (e.g. a hypothetical `get_grant` helper)
/// will see that envelope, not a bare `Grant` — deserializing straight into
/// `Grant` will fail. Unwrap `ScopeEnvelope` first; see `get_revoked_set`
/// below for the pattern (`capforge:revoked` hits the identical trap).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Grant {
    pub jti: String,
    pub agent_id: String,
    pub skills: Vec<String>,
    pub tier_source: HashMap<String, SkillTier>,
    pub issued_at: DateTime<Utc>,
    pub expires_at: DateTime<Utc>,
}

impl Grant {
    pub fn new(
        agent_id: &str,
        skills: Vec<String>,
        tier_source: HashMap<String, SkillTier>,
        ttl: chrono::Duration,
    ) -> Self {
        let now = Utc::now();
        Self {
            jti: Uuid::new_v4().to_string(),
            agent_id: agent_id.to_string(),
            skills,
            tier_source,
            issued_at: now,
            expires_at: now + ttl,
        }
    }
}

fn grant_key(jti: &str) -> String {
    format!("capforge:grant:{jti}")
}

const REVOKED_KEY: &str = "capforge:revoked";

/// Atomically writes the grant record — a single key today, but done via
/// `transaction()` so it's ready if a second key joins it later.
///
/// See the envelope warning on [`Grant`]: this writes through
/// `transaction()`'s `Put`, so the stored bytes are `ScopeEnvelope`-wrapped,
/// not bare grant JSON — any future direct `get_raw` read of this key must
/// unwrap that envelope first.
pub fn persist_grant(store: &mut dyn TransactionalScopeStore, grant: &Grant) -> Result<()> {
    let results = store.transaction(vec![ScopeOp::Put {
        key: grant_key(&grant.jti),
        value: serde_json::to_value(grant)?,
        expect_gen: None,
        expires_at: Some(grant.expires_at),
    }])?;
    match results.as_slice() {
        [ScopeOpResult::Written { .. }] => Ok(()),
        other => bail!("unexpected transaction result persisting grant: {other:?}"),
    }
}

// `transaction()`'s `Put`/`Get` wrap values in `ScopeEnvelope` (carries the
// CAS generation), but `ScopeStore::get_raw` — the only read available on
// the non-transactional `&dyn ScopeStore` that `is_revoked` takes — reads
// the same table entry back without unwrapping that envelope. Reading a
// transaction-written key through `get_raw` therefore requires unwrapping
// `ScopeEnvelope` by hand here; deserializing straight to `Vec<String>`
// would fail against the envelope's `{"v":...,"gen":...}` shape.
fn get_revoked_set(store: &dyn ScopeStore) -> Result<Vec<String>> {
    match store.get_raw(REVOKED_KEY)? {
        Some(raw) => {
            let envelope: ScopeEnvelope = serde_json::from_value(raw)?;
            if envelope.is_expired(Utc::now()) {
                return Ok(Vec::new());
            }
            Ok(serde_json::from_value(envelope.v)?)
        }
        None => Ok(Vec::new()),
    }
}

/// Reads the revoked-jti list plus its current CAS generation in one
/// `transaction()` call (its `Get` op already unwraps `ScopeEnvelope`, so
/// unlike `get_revoked_set` no manual unwrap is needed here). Used by
/// `revoke_grant`'s read-modify-write retry loop.
fn get_revoked_with_gen(store: &mut dyn TransactionalScopeStore) -> Result<(Vec<String>, u64)> {
    let results = store.transaction(vec![ScopeOp::Get { key: REVOKED_KEY.to_string() }])?;
    match results.as_slice() {
        [ScopeOpResult::Value { value: Some(v), generation }] => {
            Ok((serde_json::from_value(v.clone())?, *generation))
        }
        [ScopeOpResult::Value { value: None, generation }] => Ok((Vec::new(), *generation)),
        other => bail!("unexpected transaction result reading revoked set: {other:?}"),
    }
}

/// Max read-modify-write attempts before giving up under sustained CAS
/// contention. Revocation is security-critical (a lost revoke leaves a
/// compromised grant usable), so this retries rather than overwriting
/// unconditionally — but it must still terminate rather than spin forever
/// against a wedged store.
const MAX_REVOKE_RETRIES: u32 = 10;

/// Marks `jti` revoked via an optimistic-concurrency read-modify-write loop:
/// read the current revoked list + its generation, append `jti`, then write
/// back with `expect_gen` set to the generation just read. Two concurrent
/// callers (e.g. two `capability-forge` instances against a shared
/// redis-backed `ScopeStore`) racing this key would, without the CAS guard,
/// let the second unconditional write silently clobber the first caller's
/// revocation — a real gap on a security-critical path even though the
/// current single-process `service.rs` usage never triggers it. On a CAS
/// mismatch (`ScopeError::WriteRejected`) the whole read-modify-write is
/// retried; the two `transaction()` calls per attempt are individually
/// atomic (redb holds one write transaction at a time), but they are not a
/// single atomic unit together, since the write's contents depend on what
/// the read returns.
pub fn revoke_grant(store: &mut dyn TransactionalScopeStore, jti: &str) -> Result<()> {
    for _ in 0..MAX_REVOKE_RETRIES {
        let (mut revoked, generation) = get_revoked_with_gen(store)?;
        if revoked.iter().any(|r| r == jti) {
            return Ok(());
        }
        revoked.push(jti.to_string());
        let attempt = store.transaction(vec![ScopeOp::Put {
            key: REVOKED_KEY.to_string(),
            value: serde_json::to_value(&revoked)?,
            expect_gen: Some(generation),
            expires_at: None,
        }]);
        match attempt {
            Ok(results) => match results.as_slice() {
                [ScopeOpResult::Written { .. }] => return Ok(()),
                other => bail!("unexpected transaction result revoking grant: {other:?}"),
            },
            Err(ScopeError::WriteRejected(_)) => continue,
            Err(e) => return Err(e.into()),
        }
    }
    bail!("revoke_grant: exceeded {MAX_REVOKE_RETRIES} retries under CAS contention for jti={jti}")
}

pub fn is_revoked(store: &dyn ScopeStore, jti: &str) -> Result<bool> {
    Ok(get_revoked_set(store)?.contains(&jti.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use b00t_c0re_gov::redb_scope_store::RedbScopeStore;
    use b00t_c0re_gov::scope_store::ScopeId;
    use serde_json::Value;
    use std::sync::atomic::{AtomicU32, Ordering};

    fn store() -> RedbScopeStore {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("test.redb");
        std::mem::forget(dir);
        RedbScopeStore::open(path, ScopeId::Global, None).unwrap()
    }

    #[test]
    fn persisted_grant_is_not_revoked_by_default() {
        let mut s = store();
        let grant = Grant::new("agent-1", vec!["skill.read".into()], HashMap::new(), chrono::Duration::minutes(30));
        persist_grant(&mut s, &grant).unwrap();
        assert!(!is_revoked(&s, &grant.jti).unwrap());
    }

    #[test]
    fn revoke_marks_jti_revoked_without_disturbing_others() {
        let mut s = store();
        let a = Grant::new("agent-1", vec!["skill.read".into()], HashMap::new(), chrono::Duration::minutes(30));
        let b = Grant::new("agent-1", vec!["skill.write".into()], HashMap::new(), chrono::Duration::minutes(30));
        persist_grant(&mut s, &a).unwrap();
        persist_grant(&mut s, &b).unwrap();
        revoke_grant(&mut s, &a.jti).unwrap();
        assert!(is_revoked(&s, &a.jti).unwrap());
        assert!(!is_revoked(&s, &b.jti).unwrap());
    }

    /// Wraps a real `RedbScopeStore` and rejects the first N attempts to
    /// `Put` the revoked-list key with a simulated `WriteRejected`,
    /// standing in for another writer winning the race on that key. This
    /// forces `revoke_grant`'s CAS retry loop to actually retry rather than
    /// merely succeeding on an uncontended first attempt — the scenario the
    /// review flagged (two concurrent `revoke_grant` callers) is awkward to
    /// reproduce with real threads against a single `&mut dyn
    /// TransactionalScopeStore`, so this fakes the contention's observable
    /// effect (a rejected write) instead.
    struct FlakyOnRevokedKey {
        inner: RedbScopeStore,
        puts_to_reject: AtomicU32,
    }

    impl ScopeStore for FlakyOnRevokedKey {
        fn get_raw(&self, key: &str) -> b00t_c0re_gov::errors::ScopeResult<Option<Value>> {
            self.inner.get_raw(key)
        }
        fn set_raw(&mut self, key: &str, val: Value) -> b00t_c0re_gov::errors::ScopeResult<()> {
            self.inner.set_raw(key, val)
        }
        fn scope_id(&self) -> &ScopeId {
            self.inner.scope_id()
        }
        fn parent(&self) -> Option<&ScopeId> {
            self.inner.parent()
        }
    }

    impl TransactionalScopeStore for FlakyOnRevokedKey {
        fn transaction(&mut self, ops: Vec<ScopeOp>) -> b00t_c0re_gov::errors::ScopeResult<Vec<ScopeOpResult>> {
            let targets_revoked_put =
                ops.iter().any(|op| matches!(op, ScopeOp::Put { key, .. } if key == REVOKED_KEY));
            if targets_revoked_put
                && self
                    .puts_to_reject
                    .fetch_update(Ordering::SeqCst, Ordering::SeqCst, |n| (n > 0).then(|| n - 1))
                    .is_ok()
            {
                return Err(ScopeError::WriteRejected("simulated concurrent writer".into()));
            }
            self.inner.transaction(ops)
        }
    }

    #[test]
    fn revoke_retries_past_simulated_cas_contention_and_still_lands() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("flaky.redb");
        let inner = RedbScopeStore::open(path, ScopeId::Global, None).unwrap();
        let mut s = FlakyOnRevokedKey { inner, puts_to_reject: AtomicU32::new(2) };

        let grant = Grant::new("agent-1", vec!["skill.read".into()], HashMap::new(), chrono::Duration::minutes(30));
        persist_grant(&mut s, &grant).unwrap();

        revoke_grant(&mut s, &grant.jti).unwrap();

        assert_eq!(
            s.puts_to_reject.load(Ordering::SeqCst),
            0,
            "expected both simulated rejections to be consumed by retries"
        );
        assert!(is_revoked(&s, &grant.jti).unwrap());
    }

    #[test]
    fn revoke_gives_up_after_max_retries_under_permanent_contention() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("stuck.redb");
        let inner = RedbScopeStore::open(path, ScopeId::Global, None).unwrap();
        let mut s = FlakyOnRevokedKey { inner, puts_to_reject: AtomicU32::new(u32::MAX) };

        let grant = Grant::new("agent-1", vec!["skill.read".into()], HashMap::new(), chrono::Duration::minutes(30));
        persist_grant(&mut s, &grant).unwrap();

        let err = revoke_grant(&mut s, &grant.jti).unwrap_err();
        assert!(err.to_string().contains("exceeded"));
        assert!(!is_revoked(&s, &grant.jti).unwrap());
    }

    #[test]
    fn revoke_is_idempotent_when_jti_already_revoked() {
        let mut s = store();
        let grant = Grant::new("agent-1", vec!["skill.read".into()], HashMap::new(), chrono::Duration::minutes(30));
        persist_grant(&mut s, &grant).unwrap();
        revoke_grant(&mut s, &grant.jti).unwrap();
        revoke_grant(&mut s, &grant.jti).unwrap();
        assert!(is_revoked(&s, &grant.jti).unwrap());
    }
}
