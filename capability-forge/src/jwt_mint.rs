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
pub fn mint_user_jwt(
    account_signing_key: &KeyPair,
    account_pubkey: &str,
    agent_pubkey: &str,
    granted_skills: &[String],
    ttl: chrono::Duration,
) -> Result<String> {
    anyhow::ensure!(ttl > chrono::Duration::zero(), "ttl must be positive");

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
    fn mint_rejects_non_positive_ttl() {
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
}
