use crate::tiers::SkillTier;
use anyhow::{bail, Result};
use b00t_c0re_gov::scope_store::{ScopeEnvelope, ScopeOp, ScopeOpResult, ScopeStore, TransactionalScopeStore};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use uuid::Uuid;

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

pub fn revoke_grant(store: &mut dyn TransactionalScopeStore, jti: &str) -> Result<()> {
    let mut revoked = get_revoked_set(store)?;
    if !revoked.contains(&jti.to_string()) {
        revoked.push(jti.to_string());
    }
    let results = store.transaction(vec![ScopeOp::Put {
        key: REVOKED_KEY.to_string(),
        value: serde_json::to_value(revoked)?,
        expect_gen: None,
        expires_at: None,
    }])?;
    match results.as_slice() {
        [ScopeOpResult::Written { .. }] => Ok(()),
        other => bail!("unexpected transaction result revoking grant: {other:?}"),
    }
}

pub fn is_revoked(store: &dyn ScopeStore, jti: &str) -> Result<bool> {
    Ok(get_revoked_set(store)?.contains(&jti.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use b00t_c0re_gov::redb_scope_store::RedbScopeStore;
    use b00t_c0re_gov::scope_store::ScopeId;

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
}
