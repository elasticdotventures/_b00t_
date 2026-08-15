use anyhow::{Context, Result};
use capability_forge::bootstrap::{mint_account, MintedAccount};
use std::env;

/// One-time bootstrap: mint the CAPFORGE account under an existing operator,
/// signed with the operator's designated signing key (not its root identity
/// key). Run once per environment; capability-forge itself never needs the
/// operator key afterward, only the printed account signing seed.
fn main() -> Result<()> {
    let operator_signing_seed = env::var("NATS_OPERATOR_SIGNING_SEED")
        .context("NATS_OPERATOR_SIGNING_SEED not set — the seed for the operator's designated \
                  signing key, not its root identity key")?;

    let account_name = env::var("CAPFORGE_ACCOUNT_NAME").unwrap_or_else(|_| "CAPFORGE".to_string());

    let MintedAccount { account_pubkey, account_seed, account_jwt } =
        mint_account(&operator_signing_seed, &account_name)?;

    println!("# {account_name} account minted.");
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
