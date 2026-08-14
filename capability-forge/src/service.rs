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

            let tier = get_skill_tier(self.store, skill).unwrap_or_default();
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
            return CapabilityReply { granted, denied, jwt: None, expires_at: None };
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

        CapabilityReply { granted, denied, jwt: Some(jwt), expires_at: Some(grant.expires_at) }
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
}
