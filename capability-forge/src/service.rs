use crate::grant::{persist_grant, Grant};
use crate::judge::{EscalationJudge, JudgeOutcome};
use crate::request::{CapabilityReply, SignedRequest};
use crate::tiers::{get_agent_allowlist, get_agent_pubkey, get_skill_description, get_skill_tier, is_agent_suspended, SkillTier};
use crate::{identity, jwt_mint};
use b00t_c0re_gov::scope_store::TransactionalScopeStore;
use chrono::Duration;
use nkeys::KeyPair;
use std::collections::HashMap;

pub struct CapabilityForge<'a> {
    pub store: &'a mut dyn TransactionalScopeStore,
    pub judge: &'a dyn EscalationJudge,
    pub account_signing_key: &'a KeyPair,
    pub account_pubkey: &'a str,
    pub grant_ttl: Duration,
}

impl<'a> CapabilityForge<'a> {
    pub async fn handle_request(&mut self, signed: SignedRequest) -> CapabilityReply {
        let deny_all = |reason: &str, skills: &[String]| CapabilityReply {
            granted: vec![],
            denied: skills.iter().map(|s| (s.clone(), reason.to_string())).collect(),
            jwt: None,
            expires_at: None,
            jti: None,
        };

        if signed.verify().is_err() {
            return deny_all("invalid signature", &signed.body.requested_skills);
        }

        let agent_id = &signed.body.agent_id;

        let Ok(Some(registered_pubkey)) = get_agent_pubkey(self.store, agent_id) else {
            return deny_all("unknown agent_id", &signed.body.requested_skills);
        };
        if registered_pubkey != signed.agent_pubkey {
            return deny_all("pubkey does not match enrollment", &signed.body.requested_skills);
        }

        // Fail closed: a store error while checking suspension must deny,
        // not silently proceed as if the agent were active.
        if is_agent_suspended(self.store, agent_id).unwrap_or(true) {
            return deny_all("agent suspended", &signed.body.requested_skills);
        }

        let allowlist = get_agent_allowlist(self.store, agent_id).unwrap_or_default();

        let mut granted: Vec<String> = Vec::new();
        let mut denied: Vec<(String, String)> = Vec::new();
        let mut tier_source: HashMap<String, SkillTier> = HashMap::new();

        for skill in &signed.body.requested_skills {
            if allowlist.contains(skill) {
                granted.push(skill.clone());
                tier_source.insert(skill.clone(), SkillTier::Base);
                continue;
            }

            // Match on the Result explicitly rather than `.unwrap_or_default()`:
            // `SkillTier::default()` is `Escalatable`, so collapsing a genuine
            // store read error (I/O failure, corrupted envelope -- not the
            // normal "key absent" case, which get_skill_tier already resolves
            // to Escalatable on its own) into that default would route a
            // possibly-Restricted skill to the LLM judge. Restricted must stay
            // un-escalatable under every circumstance, including read failures.
            let tier = match get_skill_tier(self.store, skill) {
                Ok(tier) => tier,
                Err(_) => {
                    denied.push((skill.clone(), "failed to read skill tier — denying by default".into()));
                    continue;
                }
            };
            match tier {
                SkillTier::Restricted => {
                    denied.push((skill.clone(), "restricted tier — requires admin allowlist grant".into()));
                }
                SkillTier::Base => {
                    // In the registry as base-tier but not on THIS agent's
                    // allowlist yet — same denial as an unenrolled skill,
                    // no LLM call: base tier is never escalatable either.
                    denied.push((skill.clone(), "base-tier skill not in agent's allowlist".into()));
                }
                SkillTier::Escalatable => {
                    let description = get_skill_description(self.store, skill).unwrap_or_default();
                    match self
                        .judge
                        .judge(agent_id, skill, &description, &signed.body.justification)
                        .await
                    {
                        JudgeOutcome::Granted => {
                            granted.push(skill.clone());
                            tier_source.insert(skill.clone(), SkillTier::Escalatable);
                        }
                        JudgeOutcome::Denied { reason } => denied.push((skill.clone(), reason)),
                    }
                }
            }
        }

        if granted.is_empty() {
            return CapabilityReply { granted, denied, jwt: None, expires_at: None, jti: None };
        }

        let grant = Grant::new(agent_id, granted.clone(), tier_source, self.grant_ttl);
        if persist_grant(self.store, &grant).is_err() {
            return deny_all("failed to persist grant record", &signed.body.requested_skills);
        }

        let jwt = match jwt_mint::mint_user_jwt(
            self.account_signing_key,
            self.account_pubkey,
            &signed.agent_pubkey,
            &granted,
            self.grant_ttl,
        ) {
            Ok(j) => j,
            Err(_) => return deny_all("failed to mint jwt after persisting grant", &signed.body.requested_skills),
        };

        CapabilityReply { granted, denied, jwt: Some(jwt), expires_at: Some(grant.expires_at), jti: Some(grant.jti.clone()) }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::enroll::enroll_agent;
    use crate::judge::FakeJudge;
    use crate::request::CapabilityRequest;
    use crate::tiers::set_skill_tier;
    use b00t_c0re_gov::redb_scope_store::RedbScopeStore;
    use b00t_c0re_gov::scope_store::ScopeId;

    fn store() -> RedbScopeStore {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("test.redb");
        std::mem::forget(dir);
        RedbScopeStore::open(path, ScopeId::Global, None).unwrap()
    }

    #[tokio::test]
    async fn base_skill_grants_without_llm_call() {
        let mut s = store();
        let kp = enroll_agent(&mut s, "agent-1", &["skill.read".into()]).unwrap();
        let judge = FakeJudge::always_deny("should not be called");
        let account = KeyPair::new_account();
        let mut forge = CapabilityForge {
            store: &mut s,
            judge: &judge,
            account_signing_key: &account,
            account_pubkey: &account.public_key(),
            grant_ttl: Duration::minutes(30),
        };
        let signed = crate::request::SignedRequest::sign(
            &kp,
            CapabilityRequest { agent_id: "agent-1".into(), requested_skills: vec!["skill.read".into()], justification: "".into() },
        )
        .unwrap();
        let reply = forge.handle_request(signed).await;
        assert_eq!(reply.granted, vec!["skill.read".to_string()]);
        assert!(reply.jwt.is_some());
    }

    #[tokio::test]
    async fn restricted_skill_never_reaches_the_judge() {
        let mut s = store();
        let kp = enroll_agent(&mut s, "agent-1", &[]).unwrap();
        set_skill_tier(&mut s, "skill.delete-infra", SkillTier::Restricted).unwrap();
        let judge = FakeJudge::always_grant();
        let account = KeyPair::new_account();
        let mut forge = CapabilityForge {
            store: &mut s,
            judge: &judge,
            account_signing_key: &account,
            account_pubkey: &account.public_key(),
            grant_ttl: Duration::minutes(30),
        };
        let signed = crate::request::SignedRequest::sign(
            &kp,
            CapabilityRequest { agent_id: "agent-1".into(), requested_skills: vec!["skill.delete-infra".into()], justification: "trust me".into() },
        )
        .unwrap();
        let reply = forge.handle_request(signed).await;
        assert!(reply.granted.is_empty());
        assert_eq!(reply.denied[0].0, "skill.delete-infra");
        assert!(reply.jwt.is_none());
    }

    #[tokio::test]
    async fn escalatable_skill_denied_by_judge_yields_no_grant() {
        let mut s = store();
        let kp = enroll_agent(&mut s, "agent-1", &[]).unwrap();
        let judge = FakeJudge::always_deny("insufficient justification");
        let account = KeyPair::new_account();
        let mut forge = CapabilityForge {
            store: &mut s,
            judge: &judge,
            account_signing_key: &account,
            account_pubkey: &account.public_key(),
            grant_ttl: Duration::minutes(30),
        };
        let signed = crate::request::SignedRequest::sign(
            &kp,
            CapabilityRequest { agent_id: "agent-1".into(), requested_skills: vec!["skill.write".into()], justification: "".into() },
        )
        .unwrap();
        let reply = forge.handle_request(signed).await;
        assert!(reply.granted.is_empty());
        assert!(reply.jwt.is_none());
    }

    #[tokio::test]
    async fn unknown_agent_is_denied() {
        let mut s = store();
        let judge = FakeJudge::always_grant();
        let account = KeyPair::new_account();
        let stray_kp = identity::AgentKeyPair::generate();
        let mut forge = CapabilityForge {
            store: &mut s,
            judge: &judge,
            account_signing_key: &account,
            account_pubkey: &account.public_key(),
            grant_ttl: Duration::minutes(30),
        };
        let signed = crate::request::SignedRequest::sign(
            &stray_kp,
            CapabilityRequest { agent_id: "ghost".into(), requested_skills: vec!["skill.read".into()], justification: "".into() },
        )
        .unwrap();
        let reply = forge.handle_request(signed).await;
        assert!(reply.granted.is_empty());
        assert!(reply.jwt.is_none());
    }

    #[tokio::test]
    async fn suspended_agent_is_denied_even_for_base_skills() {
        let mut s = store();
        let kp = enroll_agent(&mut s, "agent-1", &["skill.read".into()]).unwrap();
        crate::enroll::suspend_agent(&mut s, "agent-1").unwrap();
        let judge = FakeJudge::always_grant();
        let account = KeyPair::new_account();
        let mut forge = CapabilityForge {
            store: &mut s,
            judge: &judge,
            account_signing_key: &account,
            account_pubkey: &account.public_key(),
            grant_ttl: Duration::minutes(30),
        };
        let signed = crate::request::SignedRequest::sign(
            &kp,
            CapabilityRequest { agent_id: "agent-1".into(), requested_skills: vec!["skill.read".into()], justification: "".into() },
        )
        .unwrap();
        let reply = forge.handle_request(signed).await;
        assert!(reply.granted.is_empty());
    }

    #[tokio::test]
    async fn impersonation_with_wrong_keypair_is_denied() {
        let mut s = store();
        enroll_agent(&mut s, "agent-1", &["skill.read".into()]).unwrap();
        // Signed by a keypair that was never registered for "agent-1" --
        // the signature itself is internally valid (self.verify() passes),
        // so this only gets caught by the registered-pubkey comparison.
        let impostor_kp = identity::AgentKeyPair::generate();
        let judge = FakeJudge::always_grant();
        let account = KeyPair::new_account();
        let mut forge = CapabilityForge {
            store: &mut s,
            judge: &judge,
            account_signing_key: &account,
            account_pubkey: &account.public_key(),
            grant_ttl: Duration::minutes(30),
        };
        let signed = crate::request::SignedRequest::sign(
            &impostor_kp,
            CapabilityRequest { agent_id: "agent-1".into(), requested_skills: vec!["skill.read".into()], justification: "".into() },
        )
        .unwrap();
        let reply = forge.handle_request(signed).await;
        assert!(reply.granted.is_empty());
        assert!(reply.jwt.is_none());
    }

    #[tokio::test]
    async fn mixed_tier_request_yields_partial_grant() {
        let mut s = store();
        let kp = enroll_agent(&mut s, "agent-1", &["skill.read".into()]).unwrap();
        set_skill_tier(&mut s, "skill.delete-infra", SkillTier::Restricted).unwrap();
        // "skill.write" is unregistered, so it defaults to Escalatable and
        // goes through the (always-granting) judge.
        let judge = FakeJudge::always_grant();
        let account = KeyPair::new_account();
        let mut forge = CapabilityForge {
            store: &mut s,
            judge: &judge,
            account_signing_key: &account,
            account_pubkey: &account.public_key(),
            grant_ttl: Duration::minutes(30),
        };
        let signed = crate::request::SignedRequest::sign(
            &kp,
            CapabilityRequest {
                agent_id: "agent-1".into(),
                requested_skills: vec!["skill.read".into(), "skill.delete-infra".into(), "skill.write".into()],
                justification: "need write access".into(),
            },
        )
        .unwrap();
        let reply = forge.handle_request(signed).await;
        assert_eq!(reply.granted.len(), 2);
        assert!(reply.granted.contains(&"skill.read".to_string()));
        assert!(reply.granted.contains(&"skill.write".to_string()));
        assert_eq!(reply.denied.len(), 1);
        assert_eq!(reply.denied[0].0, "skill.delete-infra");
        assert!(reply.jwt.is_some());
    }

    #[tokio::test]
    async fn tampered_signature_is_denied_at_handle_request() {
        let mut s = store();
        let kp = enroll_agent(&mut s, "agent-1", &["skill.read".into()]).unwrap();
        // Judge would grant anything asked of it -- proves the tampered
        // signature is caught by handle_request's own verify() call at
        // step 1, before any tier logic or judge call could paper over it.
        let judge = FakeJudge::always_grant();
        let account = KeyPair::new_account();
        let mut forge = CapabilityForge {
            store: &mut s,
            judge: &judge,
            account_signing_key: &account,
            account_pubkey: &account.public_key(),
            grant_ttl: Duration::minutes(30),
        };
        let mut signed = crate::request::SignedRequest::sign(
            &kp,
            CapabilityRequest { agent_id: "agent-1".into(), requested_skills: vec!["skill.read".into()], justification: "".into() },
        )
        .unwrap();
        signed.body.requested_skills.push("skill.admin".into());
        let reply = forge.handle_request(signed).await;
        assert!(reply.granted.is_empty());
        assert!(reply.jwt.is_none());
    }

    /// Wraps a real `RedbScopeStore` and returns a simulated backend error
    /// for reads of one specific key, standing in for a genuine store I/O
    /// failure -- as opposed to the normal "key never written" case, which
    /// `get_skill_tier` already resolves to `Escalatable` on its own without
    /// any help from this wrapper.
    struct FailingTierRead {
        inner: RedbScopeStore,
        failing_key: String,
    }

    impl b00t_c0re_gov::scope_store::ScopeStore for FailingTierRead {
        fn get_raw(&self, key: &str) -> b00t_c0re_gov::errors::ScopeResult<Option<serde_json::Value>> {
            if key == self.failing_key {
                return Err(b00t_c0re_gov::errors::ScopeError::BackendUnavailable("simulated read failure".into()));
            }
            self.inner.get_raw(key)
        }
        fn set_raw(&mut self, key: &str, val: serde_json::Value) -> b00t_c0re_gov::errors::ScopeResult<()> {
            self.inner.set_raw(key, val)
        }
        fn scope_id(&self) -> &ScopeId {
            self.inner.scope_id()
        }
        fn parent(&self) -> Option<&ScopeId> {
            self.inner.parent()
        }
    }

    impl TransactionalScopeStore for FailingTierRead {
        fn transaction(
            &mut self,
            ops: Vec<b00t_c0re_gov::scope_store::ScopeOp>,
        ) -> b00t_c0re_gov::errors::ScopeResult<Vec<b00t_c0re_gov::scope_store::ScopeOpResult>> {
            self.inner.transaction(ops)
        }
    }

    #[tokio::test]
    async fn skill_tier_read_error_denies_the_skill_without_escalating() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("test.redb");
        std::mem::forget(dir);
        let inner = RedbScopeStore::open(path, ScopeId::Global, None).unwrap();
        let mut s = FailingTierRead { inner, failing_key: "capforge:skill:skill.mystery:tier".to_string() };

        let kp = enroll_agent(&mut s, "agent-1", &[]).unwrap();
        // Judge would grant anything asked of it -- if the fail-open bug
        // regressed (tier read error falling through to Escalatable via
        // `.unwrap_or_default()`), this judge would grant the skill and
        // this test would catch it.
        let judge = FakeJudge::always_grant();
        let account = KeyPair::new_account();
        let mut forge = CapabilityForge {
            store: &mut s,
            judge: &judge,
            account_signing_key: &account,
            account_pubkey: &account.public_key(),
            grant_ttl: Duration::minutes(30),
        };
        let signed = crate::request::SignedRequest::sign(
            &kp,
            CapabilityRequest { agent_id: "agent-1".into(), requested_skills: vec!["skill.mystery".into()], justification: "".into() },
        )
        .unwrap();
        let reply = forge.handle_request(signed).await;
        assert!(reply.granted.is_empty());
        assert!(reply.jwt.is_none());
        assert_eq!(reply.denied[0].0, "skill.mystery");
    }
}
