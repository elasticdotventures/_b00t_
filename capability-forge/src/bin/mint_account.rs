use anyhow::{Context, Result};
use nats_jwt::Token;
use nkeys::KeyPair;
use std::env;

struct MintedAccount {
    account_pubkey: String,
    account_seed: String,
    account_jwt: String,
}

/// Mints a fresh NATS account under the given operator signing key. The
/// operator key is needed only for this one call — the returned seed is
/// what capability-forge's running service actually needs day to day.
fn mint_account(operator_signing_seed: &str, account_name: &str) -> Result<MintedAccount> {
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

/// One-time bootstrap: mint the CAPFORGE account under the existing
/// b00t-operator, signed with the operator's designated signing key (not
/// its root identity key). Run once per environment; capability-forge
/// itself never needs the operator key afterward, only the printed
/// account signing seed.
fn main() -> Result<()> {
    let operator_signing_seed = env::var("NATS_OPERATOR_SIGNING_SEED")
        .context("NATS_OPERATOR_SIGNING_SEED not set — the seed for the operator's designated \
                  signing key (public key ODMSVCODGVEUVCCQUV36MPVDTQJ36Z4EA2BMW6X6KQCRG2FGF6OX2DJL \
                  per pods/nats/nats-pod-configured.yaml's committed operator JWT), not the \
                  operator's root identity key")?;

    let account_name = env::var("CAPFORGE_ACCOUNT_NAME").unwrap_or_else(|_| "CAPFORGE".to_string());

    let MintedAccount { account_pubkey, account_seed, account_jwt } =
        mint_account(&operator_signing_seed, &account_name)?;

    println!("# {account_name} account minted under b00t-operator.");
    println!("# 1. Append this entry to pods/nats/nats-pod-configured.yaml's resolver_preload");
    println!("#    (alongside the existing SYS entry), then redeploy/reload the live nats-server:");
    println!("      // Account \"{account_name}\"");
    println!("      {account_pubkey}: {account_jwt}");
    println!();
    println!("# 2. Store this seed as CAPFORGE_ACCOUNT_SEED in the secret store (config/global/");
    println!("#    capforge-account-seed) — capability-forge's running service needs only this,");
    println!("#    never the operator key:");
    println!("      CAPFORGE_ACCOUNT_SEED={account_seed}");
    println!();
    println!("# 3. Store the account public key alongside it as CAPFORGE_ACCOUNT_PUBKEY:");
    println!("      CAPFORGE_ACCOUNT_PUBKEY={account_pubkey}");

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mints_a_well_formed_jwt_signed_by_the_given_operator_key() {
        // A freshly generated operator keypair stands in for the real
        // b00t-operator here — this test proves the minting logic and the
        // nats-jwt signing call are correct, not that any particular
        // production key works.
        let fake_operator = KeyPair::new_operator();
        let seed = fake_operator.seed().unwrap();

        let minted = mint_account(&seed, "CAPFORGE-TEST").unwrap();

        assert!(minted.account_pubkey.starts_with('A'), "account pubkeys start with A");
        assert!(minted.account_seed.starts_with("SA"), "account seeds start with SA");
        assert_eq!(minted.account_jwt.split('.').count(), 3);

        use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine as _};
        let claims_segment = minted.account_jwt.split('.').nth(1).unwrap();
        let claims_json = URL_SAFE_NO_PAD.decode(claims_segment).unwrap();
        let claims_text = String::from_utf8(claims_json).unwrap();
        assert!(claims_text.contains(&minted.account_pubkey));
        assert!(claims_text.contains("CAPFORGE-TEST"));
    }

    #[test]
    fn rejects_an_invalid_operator_seed() {
        assert!(mint_account("not-a-real-seed", "CAPFORGE-TEST").is_err());
    }
}
