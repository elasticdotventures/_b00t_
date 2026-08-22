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

/// Cap on `requested_skills` per request. Any skill explicitly tiered `SkillTier::Escalatable`
/// costs one serially-awaited judge call (`judge.rs`'s `OpenAiJudge` gives each up to a 15s
/// timeout), and `bin/main.rs`'s subscribe loop awaits `handle_request` inline with no
/// per-message concurrency -- so an unbounded skill list in a single request can occupy the
/// whole service, blocking every other agent's request behind it. 20 comfortably covers any
/// legitimate agent's real skill list for one grant request while bounding the worst case (all
/// escalatable, all timing out) to a few minutes instead of unbounded.
const MAX_REQUESTED_SKILLS: usize = 20;

/// Skill names become a raw NATS subject token via `jwt_mint::skill_subject`
/// (`capforge.<agent_pubkey>.<skill>`). An unvalidated skill name lets an agent-supplied
/// string reach that subject directly: `*` or `>` are NATS wildcards that would grant far
/// more than the single requested skill (including this agent's own Restricted-tier skills,
/// since the wildcard is scoped only by the agent's own pubkey prefix, e.g. requesting the
/// literal skill name `>` grants `capforge.<pubkey>.>`). Whitespace and other control/subject-
/// delimiter-adjacent characters are rejected the same way, via an allowlist charset rather
/// than a denylist that would have to anticipate every dangerous character.
///
/// Deliberately still allows `.`, despite the review's suggestion to reject it too: this
/// codebase's own skill-naming convention is dotted hierarchy (`skill.read`, `skill.write`,
/// `skill.delete-infra` -- used throughout `enroll.rs`, this file's own tests, and the real
/// production flow proven end-to-end in `tests/e2e_local_nats.rs`), and NATS subjects are
/// themselves conventionally dot-delimited. A `.` in a skill name only adds a further
/// *literal* token to the minted subject (`capforge.<pubkey>.skill.read` is an exact-match
/// subject, not a pattern) -- it does not confer wildcard matching the way `*`/`>` do, so it
/// does not defeat the tier system's guarantee this validation exists to protect. Rejecting
/// it would break the established naming convention (54 call sites) without closing any
/// additional attack surface beyond what rejecting `*`/`>` already closes.
fn is_valid_skill_name(skill: &str) -> bool {
    !skill.is_empty() && skill.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-' || b == b'.')
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

        // Reject oversized requests before any store lookups or judge calls -- see
        // MAX_REQUESTED_SKILLS's doc comment for why this bound exists.
        if signed.body.requested_skills.len() > MAX_REQUESTED_SKILLS {
            return deny_all("too many requested skills in a single request", &signed.body.requested_skills);
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

        // Fail closed, same rationale as the tier-read error case a few lines below: a
        // genuine store I/O error here (not the normal "agent has no allowlist yet" case,
        // which `get_agent_allowlist` already resolves to an empty Vec on its own) must not
        // silently collapse into an empty allowlist via `.unwrap_or_default()`. An empty
        // allowlist doesn't deny anything by itself -- every requested skill just falls
        // through to tier lookup, so a genuine read failure here would otherwise masquerade as
        // an ordinary "skill not on this agent's allowlist" denial instead of surfacing as the
        // store failure it actually is. A store failure must produce a hard deny.
        let allowlist = match get_agent_allowlist(self.store, agent_id) {
            Ok(list) => list,
            Err(_) => return deny_all("failed to read agent allowlist", &signed.body.requested_skills),
        };

        let mut granted: Vec<String> = Vec::new();
        let mut denied: Vec<(String, String)> = Vec::new();
        let mut tier_source: HashMap<String, SkillTier> = HashMap::new();

        for skill in &signed.body.requested_skills {
            if !is_valid_skill_name(skill) {
                denied.push((skill.clone(), "invalid skill name — must be non-empty and match ^[A-Za-z0-9_.-]+$".into()));
                continue;
            }

            if allowlist.contains(skill) {
                granted.push(skill.clone());
                tier_source.insert(skill.clone(), SkillTier::Base);
                continue;
            }

            // Match on the Result explicitly rather than `.unwrap_or_default()`: even though
            // `SkillTier::default()` is `Restricted` (the safe direction to fail toward),
            // collapsing a genuine store read error (I/O failure, corrupted envelope -- not
            // the normal "key absent" case, which get_skill_tier already resolves to
            // Restricted on its own) into that default would silently masquerade as an
            // ordinary restricted-tier denial instead of surfacing as the read failure it
            // actually is, hiding a real store problem from anyone investigating denials.
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
                            // Audit trail for escalation decisions: if the judge is ever
                            // fooled (prompt injection or otherwise), this is the
                            // server-side record that lets it be detected/investigated
                            // after the fact. Decision metadata only -- never the minted
                            // JWT or any key material.
                            tracing::info!(agent_id = %agent_id, skill = %skill, "escalation judge granted");
                            granted.push(skill.clone());
                            tier_source.insert(skill.clone(), SkillTier::Escalatable);
                        }
                        JudgeOutcome::Denied { reason } => {
                            tracing::warn!(agent_id = %agent_id, skill = %skill, reason = %reason, "escalation judge denied");
                            denied.push((skill.clone(), reason));
                        }
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

/// Handles exactly one wire-format request: deserialize `payload` as a `SignedRequest`, run
/// it through `handle_request`, serialize the reply, and publish it to `reply_subject` on
/// `client`. This is the real production per-message logic -- `bin/main.rs`'s subscribe loop
/// does nothing more than resolve `reply_subject` from the incoming `Message` and call this
/// function, so tests exercising this function against a real NATS connection are exercising
/// the actual wire path, not a library-level shortcut back to `handle_request` alone.
///
/// Error handling mirrors what `bin/main.rs` did inline before this was extracted: a
/// malformed payload or a reply serialize/publish failure is logged and swallowed rather than
/// propagated, so one bad message never takes the whole service down for other agents relying
/// on it.
pub async fn handle_wire_request(
    forge: &mut CapabilityForge<'_>,
    client: &async_nats::Client,
    reply_subject: async_nats::Subject,
    payload: &[u8],
) {
    let signed: SignedRequest = match serde_json::from_slice(payload) {
        Ok(s) => s,
        Err(e) => {
            tracing::warn!("malformed request: {e}");
            return;
        }
    };

    let reply = forge.handle_request(signed).await;

    let reply_payload = match serde_json::to_vec(&reply) {
        Ok(p) => p,
        Err(e) => {
            tracing::warn!("failed to serialize reply: {e}");
            return;
        }
    };
    if let Err(e) = client.publish(reply_subject, reply_payload.into()).await {
        tracing::warn!("failed to publish reply: {e}");
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
        // Unregistered skills now default to Restricted (never reaches the judge), so this
        // must be explicitly tiered Escalatable to exercise the judge-denial path this test
        // is named for.
        set_skill_tier(&mut s, "skill.write", SkillTier::Escalatable).unwrap();
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
        // "skill.write" is explicitly tiered Escalatable here -- unregistered skills now
        // default to Restricted (see tiers.rs), so without this it would be denied before
        // ever reaching the (always-granting) judge.
        set_skill_tier(&mut s, "skill.write", SkillTier::Escalatable).unwrap();
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

    /// Wraps a real `RedbScopeStore` and returns a simulated backend error for reads of one
    /// specific key, standing in for a genuine store I/O failure -- as opposed to the normal
    /// "key never written" case, which `get_skill_tier`/`get_agent_allowlist` already resolve
    /// on their own (to `Restricted` and `vec![]` respectively) without any help from this
    /// wrapper. Shared by the tier-read and allowlist-read fail-closed tests below.
    struct FailingKeyRead {
        inner: RedbScopeStore,
        failing_key: String,
    }

    impl b00t_c0re_gov::scope_store::ScopeStore for FailingKeyRead {
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

    impl TransactionalScopeStore for FailingKeyRead {
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
        let mut s = FailingKeyRead { inner, failing_key: "capforge:skill:skill.mystery:tier".to_string() };

        let kp = enroll_agent(&mut s, "agent-1", &[]).unwrap();
        // Judge would grant anything asked of it -- proves this denial comes from the tier
        // read error itself, not from the judge. If the read error were swallowed via
        // `.unwrap_or_default()` instead of matched explicitly, the resulting denial would be
        // indistinguishable from an ordinary restricted-tier denial, masking the underlying
        // store failure -- the denial reason assertion below guards against that regression.
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
        assert!(
            reply.denied[0].1.contains("failed to read skill tier"),
            "expected a denial reason naming the tier read failure, got: {}",
            reply.denied[0].1
        );
    }

    #[tokio::test]
    async fn allowlist_read_error_denies_the_request_without_falling_through_to_the_judge() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("test.redb");
        std::mem::forget(dir);
        let mut inner = RedbScopeStore::open(path, ScopeId::Global, None).unwrap();
        // Enroll on the plain store first, so the allowlist is actually persisted before the
        // failing wrapper goes on -- this test is about a read failure on the *lookup* path
        // `handle_request` uses, not about enrollment's own write failing.
        let kp = enroll_agent(&mut inner, "agent-1", &["skill.read".into()]).unwrap();
        let mut s = FailingKeyRead { inner, failing_key: "capforge:agent:agent-1:allowlist".to_string() };

        // Judge would grant anything asked of it -- if the fail-open bug regressed
        // (allowlist read error collapsing to an empty allowlist via `.unwrap_or_default()`,
        // letting the request fall through to tier lookup and then the judge), this judge
        // would grant the skill and this test would catch it.
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
        assert!(reply.jwt.is_none());
        assert_eq!(reply.denied[0].0, "skill.read");
        assert!(
            reply.denied[0].1.contains("allowlist"),
            "expected a denial reason naming the allowlist read failure, got: {}",
            reply.denied[0].1
        );
    }

    #[tokio::test]
    async fn invalid_skill_names_are_denied_without_reaching_the_judge() {
        let mut s = store();
        let kp = enroll_agent(&mut s, "agent-1", &[]).unwrap();
        // Judge would grant anything asked of it -- none of these should ever reach it: `>`
        // and `*` are NATS wildcards that would expand the minted subject to cover this
        // agent's entire skill namespace (including Restricted-tier skills) instead of the
        // single leaf requested, an empty name is meaningless, and embedded whitespace has no
        // legitimate use in a subject token. `skill.read`-style dotted names are deliberately
        // NOT included here -- they're valid (see `is_valid_skill_name`'s doc comment).
        let judge = FakeJudge::always_grant();
        let account = KeyPair::new_account();
        let mut forge = CapabilityForge {
            store: &mut s,
            judge: &judge,
            account_signing_key: &account,
            account_pubkey: &account.public_key(),
            grant_ttl: Duration::minutes(30),
        };
        let bad_names = vec![">".to_string(), "*".to_string(), "".to_string(), "skill with space".to_string()];
        let signed = crate::request::SignedRequest::sign(
            &kp,
            CapabilityRequest { agent_id: "agent-1".into(), requested_skills: bad_names.clone(), justification: "".into() },
        )
        .unwrap();
        let reply = forge.handle_request(signed).await;
        assert!(reply.granted.is_empty(), "no invalid skill name should ever be granted");
        assert!(reply.jwt.is_none());
        assert_eq!(reply.denied.len(), bad_names.len());
        for (name, reason) in &reply.denied {
            assert!(bad_names.contains(name));
            assert!(reason.contains("invalid skill name"), "unexpected denial reason for {name:?}: {reason}");
        }
    }

    #[tokio::test]
    async fn dotted_skill_names_remain_valid() {
        // Companion to invalid_skill_names_are_denied_without_reaching_the_judge: proves the
        // deliberate deviation from the review's "reject dots too" suggestion actually works
        // end-to-end, not just that is_valid_skill_name's unit logic allows it.
        let mut s = store();
        let kp = enroll_agent(&mut s, "agent-1", &["skill.delete-infra.confirm".into()]).unwrap();
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
            CapabilityRequest {
                agent_id: "agent-1".into(),
                requested_skills: vec!["skill.delete-infra.confirm".into()],
                justification: "".into(),
            },
        )
        .unwrap();
        let reply = forge.handle_request(signed).await;
        assert_eq!(reply.granted, vec!["skill.delete-infra.confirm".to_string()]);
        assert!(reply.jwt.is_some());
    }

    #[tokio::test]
    async fn too_many_requested_skills_is_denied_before_any_judge_call() {
        let mut s = store();
        let kp = enroll_agent(&mut s, "agent-1", &[]).unwrap();
        // Judge would grant anything asked of it -- proves this denial comes from the
        // MAX_REQUESTED_SKILLS cap firing before any tier lookup, not from these unregistered
        // skills merely being denied by their (now Restricted) default tier.
        let judge = FakeJudge::always_grant();
        let account = KeyPair::new_account();
        let mut forge = CapabilityForge {
            store: &mut s,
            judge: &judge,
            account_signing_key: &account,
            account_pubkey: &account.public_key(),
            grant_ttl: Duration::minutes(30),
        };
        let too_many: Vec<String> = (0..(MAX_REQUESTED_SKILLS + 1)).map(|i| format!("skill.n{i}")).collect();
        let signed = crate::request::SignedRequest::sign(
            &kp,
            CapabilityRequest { agent_id: "agent-1".into(), requested_skills: too_many.clone(), justification: "".into() },
        )
        .unwrap();
        let reply = forge.handle_request(signed).await;
        assert!(reply.granted.is_empty());
        assert!(reply.jwt.is_none());
        assert_eq!(reply.denied.len(), too_many.len());
        assert!(reply.denied.iter().all(|(_, reason)| reason.contains("too many requested skills")));
    }

    #[tokio::test]
    async fn requested_skills_at_the_cap_is_not_rejected_for_size() {
        // Boundary check: exactly MAX_REQUESTED_SKILLS must not trip the too-many-skills
        // denial (only exceeding it should) -- proven by using base-tier allowlisted skills
        // so a pass here can only be explained by the cap not firing, not by the judge
        // happening to grant everything.
        let mut s = store();
        let skills: Vec<String> = (0..MAX_REQUESTED_SKILLS).map(|i| format!("skill.n{i}")).collect();
        let kp = enroll_agent(&mut s, "agent-1", &skills).unwrap();
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
            CapabilityRequest { agent_id: "agent-1".into(), requested_skills: skills.clone(), justification: "".into() },
        )
        .unwrap();
        let reply = forge.handle_request(signed).await;
        assert_eq!(reply.granted.len(), skills.len());
        assert!(reply.jwt.is_some());
    }
}
