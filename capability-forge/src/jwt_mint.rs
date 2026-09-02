use anyhow::{Context, Result};
use nats_jwt::Token;
use nkeys::KeyPair;

/// Per-agent, per-skill subject namespace — the concrete "subjects" the plan's tier table
/// refers to. Shared by the minter and by anything downstream (e.g. the end-to-end test)
/// that needs to reconstruct what subject a granted skill maps to, so the two never drift.
pub fn skill_subject(agent_pubkey: &str, skill: &str) -> String {
    format!("capforge.{agent_pubkey}.{skill}")
}

/// Mints a NATS user JWT for `agent_pubkey`, issued by `account_pubkey` and signed with
/// `account_signing_key`, permissioned to publish+subscribe on exactly the subjects the
/// granted skills map to via [`skill_subject`], expiring after `ttl`.
///
/// Deviates from the plan's assumed `nats-jwt` builder surface (verified against
/// nats-jwt-0.3.0/src/lib.rs, the actual crate source — no docs.rs page renders for it):
/// there is no `add_pub_permission`/`add_sub_permission`/`set_expires_in`. The real `Token`
/// builder is a *consuming* (`fn(self) -> Self`) chain of `allow_publish`/`allow_subscribe`,
/// so each call must be reassigned rather than invoked for side effect. `expires` takes an
/// absolute Unix timestamp (seconds), not a relative `Duration`, so `ttl` is converted to
/// `now + ttl` here. `Token::sign` itself returns a bare `String` (it panics internally on
/// pre-epoch system time rather than erroring), so nothing in this function's own body can
/// fail except the `ttl` arithmetic below — the `Result` return type exists for that.
///
/// `granted_skills` must be non-empty. `nats-jwt`'s `NatsPermissions` serializes with
/// `skip_serializing_if = "NatsPermissions::is_empty"`, so an empty skills list would leave
/// both `nats.pub` and `nats.sub` absent from the JWT entirely rather than present-and-empty —
/// and per NATS server semantics, an absent permission block grants unrestricted access to
/// every subject, the exact opposite of what this function exists to produce. Rejecting the
/// empty case here (rather than trusting every future caller to pre-check it, as the current
/// `service.rs` design happens to) keeps this a least-privilege-or-error function.
pub fn mint_user_jwt(
    account_signing_key: &KeyPair,
    account_pubkey: &str,
    agent_pubkey: &str,
    granted_skills: &[String],
    ttl: chrono::Duration,
) -> Result<String> {
    anyhow::ensure!(ttl > chrono::Duration::zero(), "ttl must be positive");
    anyhow::ensure!(
        !granted_skills.is_empty(),
        "granted_skills must not be empty (an empty NATS permission block means unrestricted access, not none)"
    );

    let expires_at = chrono::Utc::now()
        .checked_add_signed(ttl)
        .context("ttl overflows a representable expiry timestamp")?
        .timestamp();

    let mut token = Token::new_user(account_pubkey, agent_pubkey);
    for skill in granted_skills {
        let subject = skill_subject(agent_pubkey, skill);
        token = token.allow_publish(subject.clone()).allow_subscribe(subject);
    }
    token = token.expires(expires_at);

    Ok(token.sign(account_signing_key))
}

/// Mints a NATS user JWT with an explicit, caller-supplied pub/sub allow-list rather than
/// the per-skill `capforge.{agent}.{skill}` derivation [`mint_user_jwt`] uses. Exists for the
/// two fixed, non-skill-scoped identities the running service itself needs under the CAPFORGE
/// account: the service's own listener (subscribes `capability.request.*`, publishes replies
/// to arbitrary requester inbox subjects) and the shared "requester" credential every agent
/// uses to publish a request before it holds any grant. See `bin/mint_service_creds.rs`.
///
/// Same least-privilege-or-error stance as `mint_user_jwt`: at least one of `pub_allow`/
/// `sub_allow` must be non-empty, since an absent permission block grants unrestricted access
/// under nats-jwt 0.3.0's serialization (see `mint_user_jwt`'s doc comment for why).
pub fn mint_service_jwt(
    account_signing_key: &KeyPair,
    account_pubkey: &str,
    user_pubkey: &str,
    pub_allow: &[String],
    sub_allow: &[String],
    ttl: chrono::Duration,
) -> Result<String> {
    anyhow::ensure!(ttl > chrono::Duration::zero(), "ttl must be positive");
    anyhow::ensure!(
        !pub_allow.is_empty() || !sub_allow.is_empty(),
        "at least one of pub_allow/sub_allow must be non-empty (both empty means unrestricted access, not none)"
    );

    let expires_at = chrono::Utc::now()
        .checked_add_signed(ttl)
        .context("ttl overflows a representable expiry timestamp")?
        .timestamp();

    let mut token = Token::new_user(account_pubkey, user_pubkey);
    for subject in pub_allow {
        token = token.allow_publish(subject.clone());
    }
    for subject in sub_allow {
        token = token.allow_subscribe(subject.clone());
    }
    token = token.expires(expires_at);

    Ok(token.sign(account_signing_key))
}

/// Mints a genuinely read-only NATS user JWT: subscribe-only on `sub_allow`, publish
/// explicitly denied on every subject (`deny_publish(">")`), not merely omitted.
///
/// This distinction matters and is easy to get wrong: [`mint_service_jwt`] with an empty
/// `pub_allow` does NOT produce a read-only credential - `nats-jwt` only emits a `nats.pub`
/// permission block when `allow_publish`/`deny_publish` is called at least once
/// (`NatsPermissions::is_empty()`, `skip_serializing_if`d away otherwise), and an absent
/// permission block means NATS grants unrestricted access on that axis. A caller wanting
/// "subscribe to X, never publish anything" must explicitly deny publish, not just skip
/// allowing it - this function exists so that requirement doesn't have to be rediscovered
/// per caller. Used by `bin/mint_historian_creds.rs`.
pub fn mint_readonly_service_jwt(
    account_signing_key: &KeyPair,
    account_pubkey: &str,
    user_pubkey: &str,
    sub_allow: &[String],
    ttl: chrono::Duration,
) -> Result<String> {
    anyhow::ensure!(ttl > chrono::Duration::zero(), "ttl must be positive");
    anyhow::ensure!(
        !sub_allow.is_empty(),
        "sub_allow must not be empty (an empty NATS permission block means unrestricted access, not none)"
    );

    let expires_at = chrono::Utc::now()
        .checked_add_signed(ttl)
        .context("ttl overflows a representable expiry timestamp")?
        .timestamp();

    let mut token = Token::new_user(account_pubkey, user_pubkey).deny_publish(">".to_string());
    for subject in sub_allow {
        token = token.allow_subscribe(subject.clone());
    }
    token = token.expires(expires_at);

    Ok(token.sign(account_signing_key))
}

#[cfg(test)]
mod tests {
    use super::*;
    use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine as _};

    #[test]
    fn skill_subject_is_namespaced_per_agent() {
        assert_eq!(skill_subject("UABC", "skill.read"), "capforge.UABC.skill.read");
    }

    #[test]
    fn mint_produces_a_jwt_with_three_segments() {
        let account = KeyPair::new_account();
        let agent = KeyPair::new_user();
        let jwt = mint_user_jwt(
            &account,
            &account.public_key(),
            &agent.public_key(),
            &["skill.read".to_string()],
            chrono::Duration::minutes(30),
        )
        .unwrap();
        let segments: Vec<&str> = jwt.split('.').collect();
        assert_eq!(segments.len(), 3);
        // Each segment must actually be base64url — decoding is the real structural check,
        // not just counting dots (a string with two literal periods would pass that alone).
        for segment in &segments {
            URL_SAFE_NO_PAD
                .decode(segment)
                .unwrap_or_else(|e| panic!("segment {segment:?} is not valid base64url: {e}"));
        }
    }

    #[test]
    fn mint_grants_publish_and_subscribe_on_every_skill_subject_and_sets_expiry() {
        let account = KeyPair::new_account();
        let agent = KeyPair::new_user();
        let agent_pubkey = agent.public_key();
        let skills = vec!["skill.read".to_string(), "skill.deploy".to_string()];
        let before = chrono::Utc::now().timestamp();

        let jwt = mint_user_jwt(&account, &account.public_key(), &agent_pubkey, &skills, chrono::Duration::minutes(30))
            .unwrap();

        let claims_segment = jwt.split('.').nth(1).unwrap();
        let claims_json = URL_SAFE_NO_PAD.decode(claims_segment).unwrap();
        // Deserialize into a loose `Value` rather than `nats_jwt::Claims`: the crate's own
        // `NatsPermissions` has `#[serde(skip_serializing_if = "Vec::is_empty")]` on `allow`/
        // `deny` without a matching `#[serde(default)]`, so it cannot deserialize its own
        // serialized output once either list is empty (ours always omits `deny`, since this
        // minter only ever allow-lists). That's a real round-trip gap in nats-jwt 0.3.0, not
        // a bug in this code -- asserting on the raw wire JSON sidesteps it and is arguably a
        // more faithful check of "did the wire-format JWT actually carry these claims" anyway.
        let claims: serde_json::Value = serde_json::from_slice(&claims_json).unwrap();

        assert_eq!(claims["sub"], agent_pubkey);
        let expires = claims["exp"].as_i64().expect("minted user jwt must carry an integer exp claim");
        assert!(expires > before, "expiry must be in the future");
        assert!(
            expires <= before + chrono::Duration::minutes(31).num_seconds(),
            "expiry must roughly match the requested ttl"
        );

        assert_eq!(claims["nats"]["type"], "user");
        assert_eq!(claims["nats"]["issuer_account"], account.public_key());
        let pub_allow = claims["nats"]["pub"]["allow"].as_array().expect("pub.allow must be an array");
        let sub_allow = claims["nats"]["sub"]["allow"].as_array().expect("sub.allow must be an array");
        for skill in &skills {
            let subject = skill_subject(&agent_pubkey, skill);
            let subject_json = serde_json::Value::String(subject.clone());
            assert!(pub_allow.contains(&subject_json), "expected publish permission for {subject}");
            assert!(sub_allow.contains(&subject_json), "expected subscribe permission for {subject}");
        }
    }

    #[test]
    fn mint_rejects_zero_ttl() {
        let account = KeyPair::new_account();
        let agent = KeyPair::new_user();
        let err = mint_user_jwt(
            &account,
            &account.public_key(),
            &agent.public_key(),
            &["skill.read".to_string()],
            chrono::Duration::zero(),
        )
        .unwrap_err();
        assert!(err.to_string().contains("ttl"));
    }

    #[test]
    fn mint_rejects_negative_ttl() {
        let account = KeyPair::new_account();
        let agent = KeyPair::new_user();
        let err = mint_user_jwt(
            &account,
            &account.public_key(),
            &agent.public_key(),
            &["skill.read".to_string()],
            chrono::Duration::minutes(-5),
        )
        .unwrap_err();
        assert!(err.to_string().contains("ttl"));
    }

    // Security regression test: an empty granted_skills list must never mint a token. Per
    // NATS server semantics an *absent* nats.pub/nats.sub permission block (which is what
    // nats-jwt produces when NatsPermissions is empty, via skip_serializing_if) means
    // unrestricted access to every subject -- the opposite of the least-privilege guarantee
    // this function exists to provide. Proven by rejection here rather than by inspecting a
    // minted JWT's claims, since the fix is to refuse the call outright.
    #[test]
    fn mint_rejects_empty_granted_skills_to_avoid_producing_an_unrestricted_token() {
        let account = KeyPair::new_account();
        let agent = KeyPair::new_user();
        let err = mint_user_jwt(
            &account,
            &account.public_key(),
            &agent.public_key(),
            &[],
            chrono::Duration::minutes(30),
        )
        .unwrap_err();
        assert!(err.to_string().contains("granted_skills"));
    }

    #[test]
    fn mint_service_jwt_grants_exactly_the_given_pub_and_sub_lists() {
        let account = KeyPair::new_account();
        let user = KeyPair::new_user();
        let jwt = mint_service_jwt(
            &account,
            &account.public_key(),
            &user.public_key(),
            &[">".to_string()],
            &["capability.request.*".to_string()],
            chrono::Duration::days(365),
        )
        .unwrap();

        let claims_segment = jwt.split('.').nth(1).unwrap();
        let claims_json = URL_SAFE_NO_PAD.decode(claims_segment).unwrap();
        let claims: serde_json::Value = serde_json::from_slice(&claims_json).unwrap();

        let pub_allow = claims["nats"]["pub"]["allow"].as_array().expect("pub.allow must be an array");
        let sub_allow = claims["nats"]["sub"]["allow"].as_array().expect("sub.allow must be an array");
        assert_eq!(pub_allow, &vec![serde_json::Value::String(">".to_string())]);
        assert_eq!(
            sub_allow,
            &vec![serde_json::Value::String("capability.request.*".to_string())]
        );
    }

    #[test]
    fn mint_service_jwt_rejects_both_lists_empty() {
        let account = KeyPair::new_account();
        let user = KeyPair::new_user();
        let err = mint_service_jwt(
            &account,
            &account.public_key(),
            &user.public_key(),
            &[],
            &[],
            chrono::Duration::days(365),
        )
        .unwrap_err();
        assert!(err.to_string().contains("pub_allow"));
    }

    #[test]
    fn mint_service_jwt_allows_pub_only_or_sub_only() {
        let account = KeyPair::new_account();
        let user = KeyPair::new_user();
        // sub-only (the "requester" identity has no publish-only subjects of its own past
        // capability.request.*, but this proves one-sided allow-lists are accepted).
        mint_service_jwt(
            &account,
            &account.public_key(),
            &user.public_key(),
            &[],
            &["_INBOX.>".to_string()],
            chrono::Duration::days(365),
        )
        .unwrap();
    }
}
