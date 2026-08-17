//! One-off validation, not part of the crate's normal test suite: proves a
//! specific already-running local `nats-server` (started from the real
//! `pods/nats/nats-pod-configured.yaml` content) accepts a user JWT minted
//! under the freshly-bootstrapped CAPFORGE account. Run manually:
//!   cargo run -p capability-forge --example validate_local_bootstrap -- \
//!     <nats_url> <capforge_account_seed>

use anyhow::{Context, Result};
use capability_forge::jwt_mint::mint_user_jwt;
use nkeys::KeyPair;
use std::env;
use std::io::Write;

#[tokio::main]
async fn main() -> Result<()> {
    let args: Vec<String> = env::args().collect();
    let nats_url = args.get(1).context("usage: validate_local_bootstrap <nats_url> <capforge_account_seed>")?;
    let account_seed = args.get(2).context("usage: validate_local_bootstrap <nats_url> <capforge_account_seed>")?;

    let account_key = KeyPair::from_seed(account_seed).context("invalid account seed")?;
    let account_pubkey = account_key.public_key();

    let agent = KeyPair::new_user();
    let jwt = mint_user_jwt(
        &account_key,
        &account_pubkey,
        &agent.public_key(),
        &["skill.validate".to_string()],
        chrono::Duration::minutes(5),
    )?;

    let creds_dir = tempfile::tempdir()?;
    let creds_path = creds_dir.path().join("validate.creds");
    let mut f = std::fs::File::create(&creds_path)?;
    write!(
        f,
        "-----BEGIN NATS USER JWT-----\n{jwt}\n------END NATS USER JWT------\n\n\
         -----BEGIN USER NKEY SEED-----\n{}\n------END USER NKEY SEED------\n",
        agent.seed()?
    )?;

    let client = async_nats::ConnectOptions::new()
        .credentials_file(&creds_path)
        .await?
        .connect(nats_url)
        .await
        .context("connect with CAPFORGE-minted user JWT failed")?;

    let subject = format!("capforge.{}.skill.validate", agent.public_key());
    client.publish(subject, "validated".into()).await?;
    client.flush().await?;

    println!("OK: connected to {nats_url} and published under CAPFORGE account {account_pubkey}");
    Ok(())
}
