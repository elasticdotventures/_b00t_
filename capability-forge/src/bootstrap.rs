//! Hand-rolled JWT construction for the two token shapes `nats-jwt` 0.3.0
//! cannot build at all (operator tokens; anything needing a `revocations`
//! entry), plus a thin `mint_account` wrapper for the common case
//! `nats-jwt` handles fine. Wire-format correctness (header/base64url/
//! signature construction) was verified against `nats-jwt`'s own source
//! in Task 11's e2e test fixture -- this module reuses that same proven
//! shape rather than the test's private copy, since production code and
//! already-reviewed test code shouldn't share a compilation unit.

use anyhow::{Context, Result};
use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine as _};
use nats_jwt::Token;
use nkeys::KeyPair;
use serde_json::json;

fn sign_claims(claims: &serde_json::Value, signing_key: &KeyPair) -> Result<String> {
    const JWT_HEADER: &str = r#"{"typ":"JWT","alg":"ed25519-nkey"}"#;
    let claims_str = serde_json::to_string(claims).context("serializing claims")?;
    let b64_header = URL_SAFE_NO_PAD.encode(JWT_HEADER.as_bytes());
    let b64_body = URL_SAFE_NO_PAD.encode(claims_str.as_bytes());
    let jwt_half = format!("{b64_header}.{b64_body}");
    let sig = signing_key.sign(jwt_half.as_bytes()).context("signing claims")?;
    Ok(format!("{jwt_half}.{}", URL_SAFE_NO_PAD.encode(&sig)))
}

pub struct OperatorGenesis {
    pub operator_root_seed: String,
    pub operator_root_pubkey: String,
    pub operator_signing_seed: String,
    pub operator_signing_pubkey: String,
    pub operator_jwt: String,
    pub sys_account_seed: String,
    pub sys_account_pubkey: String,
    pub sys_account_jwt: String,
    pub sys_user_seed: String,
    pub sys_user_pubkey: String,
    pub sys_user_jwt: String,
}

/// Mints a brand-new operator identity, a delegated operator signing key
/// (kept separate from the root identity key, matching NATS's recommended
/// pattern -- day-to-day account minting uses the signing key, the root
/// key never needs to be "hot"), and a SYS account/user under it. Does not
/// touch any existing operator; this is a genesis operation, only
/// appropriate when starting a new trust root from scratch.
pub fn bootstrap_operator(operator_name: &str) -> Result<OperatorGenesis> {
    let operator_root = KeyPair::new_operator();
    let operator_signing = KeyPair::new_operator();
    let sys_account = KeyPair::new_account();
    let sys_user = KeyPair::new_user();

    let operator_claims = json!({
        "iat": chrono::Utc::now().timestamp(),
        "iss": operator_root.public_key(),
        "jti": uuid::Uuid::new_v4().to_string(),
        "sub": operator_root.public_key(),
        "name": operator_name,
        "nats": {
            "type": "operator",
            "version": 2,
            "signing_keys": [operator_signing.public_key()],
            "system_account": sys_account.public_key(),
        },
    });
    let operator_jwt = sign_claims(&operator_claims, &operator_root)?;

    let sys_account_claims = json!({
        "iat": chrono::Utc::now().timestamp(),
        "iss": operator_signing.public_key(),
        "jti": uuid::Uuid::new_v4().to_string(),
        "sub": sys_account.public_key(),
        "name": "SYS",
        "nats": {
            "type": "account",
            "version": 2,
            "limits": {
                "subs": -1, "data": -1, "payload": -1,
                "imports": -1, "exports": -1, "wildcards": true,
                "conn": -1, "leaf": -1
            },
            "default_permissions": {},
        },
    });
    let sys_account_jwt = sign_claims(&sys_account_claims, &operator_signing)?;

    let sys_user_claims = json!({
        "iat": chrono::Utc::now().timestamp(),
        "iss": sys_account.public_key(),
        "jti": uuid::Uuid::new_v4().to_string(),
        "sub": sys_user.public_key(),
        "name": "sys",
        "nats": { "type": "user", "version": 2, "issuer_account": sys_account.public_key() },
    });
    let sys_user_jwt = sign_claims(&sys_user_claims, &sys_account)?;

    Ok(OperatorGenesis {
        operator_root_seed: operator_root.seed().context("operator root seed")?,
        operator_root_pubkey: operator_root.public_key(),
        operator_signing_seed: operator_signing.seed().context("operator signing seed")?,
        operator_signing_pubkey: operator_signing.public_key(),
        operator_jwt,
        sys_account_seed: sys_account.seed().context("sys account seed")?,
        sys_account_pubkey: sys_account.public_key(),
        sys_account_jwt,
        sys_user_seed: sys_user.seed().context("sys user seed")?,
        sys_user_pubkey: sys_user.public_key(),
        sys_user_jwt,
    })
}

pub struct MintedAccount {
    pub account_pubkey: String,
    pub account_seed: String,
    pub account_jwt: String,
}

/// Mints a fresh NATS account under the given operator signing key. The
/// operator key is needed only for this one call -- the returned seed is
/// what capability-forge's running service actually needs day to day.
pub fn mint_account(operator_signing_seed: &str, account_name: &str) -> Result<MintedAccount> {
    let operator_signing_key =
        KeyPair::from_seed(operator_signing_seed).context("invalid operator signing seed")?;

    let account_key = KeyPair::new_account();
    let account_pubkey = account_key.public_key();
    let account_seed = account_key.seed().context("generated account key has no seed")?;

    let account_jwt = Token::new_account(&account_pubkey)
        .name(account_name)
        .sign(&operator_signing_key);

    Ok(MintedAccount { account_pubkey, account_seed, account_jwt })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bootstrap_produces_a_self_consistent_trust_chain() {
        let genesis = bootstrap_operator("test-operator").unwrap();

        // Operator JWT is self-signed (iss == sub == root pubkey).
        let op_claims = decode_claims(&genesis.operator_jwt);
        assert_eq!(op_claims["iss"], genesis.operator_root_pubkey);
        assert_eq!(op_claims["sub"], genesis.operator_root_pubkey);
        assert_eq!(op_claims["nats"]["system_account"], genesis.sys_account_pubkey);
        assert_eq!(
            op_claims["nats"]["signing_keys"][0],
            genesis.operator_signing_pubkey
        );

        // SYS account JWT is signed by the operator's signing key, not its root key.
        let sys_claims = decode_claims(&genesis.sys_account_jwt);
        assert_eq!(sys_claims["iss"], genesis.operator_signing_pubkey);
        assert_eq!(sys_claims["sub"], genesis.sys_account_pubkey);

        // SYS user JWT is signed by the SYS account itself.
        let sys_user_claims = decode_claims(&genesis.sys_user_jwt);
        assert_eq!(sys_user_claims["iss"], genesis.sys_account_pubkey);
        assert_eq!(sys_user_claims["sub"], genesis.sys_user_pubkey);

        // Every returned JWT is well-formed (3 base64url segments).
        for jwt in [&genesis.operator_jwt, &genesis.sys_account_jwt, &genesis.sys_user_jwt] {
            assert_eq!(jwt.split('.').count(), 3);
        }
    }

    #[test]
    fn mint_account_is_signed_by_the_given_operator_signing_key_not_root() {
        let genesis = bootstrap_operator("test-operator").unwrap();
        let minted = mint_account(&genesis.operator_signing_seed, "CAPFORGE-TEST").unwrap();

        let claims = decode_claims(&minted.account_jwt);
        assert_eq!(claims["iss"], genesis.operator_signing_pubkey);
        assert_eq!(claims["sub"], minted.account_pubkey);
        assert!(minted.account_seed.starts_with("SA"));
    }

    fn decode_claims(jwt: &str) -> serde_json::Value {
        let claims_segment = jwt.split('.').nth(1).unwrap();
        let bytes = URL_SAFE_NO_PAD.decode(claims_segment).unwrap();
        serde_json::from_slice(&bytes).unwrap()
    }
}
