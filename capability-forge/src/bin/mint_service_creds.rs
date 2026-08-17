use anyhow::{Context, Result};
use capability_forge::jwt_mint::mint_service_jwt;
use nkeys::KeyPair;
use std::env;

/// One-time (or rotate-by-rerunning) mint of the two fixed, non-skill-scoped NATS user
/// identities the CAPFORGE account needs beyond the per-agent skill grants `jwt_mint::
/// mint_user_jwt` produces at request time:
///
/// - "capforge-service": the running `capability-forge` binary's own connection. Subscribes
///   `capability.request.*`, publishes replies to whatever inbox subject each requester used.
/// - "capforge-requester": the shared low-privilege credential every agent uses to publish a
///   capability *request* before it holds any grant — the chicken-and-egg bootstrap identity.
///   Per-agent requester identity is explicitly out of scope for phase 1 (see the design doc's
///   "Whether GitHub OAuth still identifies the operator... explicitly deferred"); this is one
///   shared credential, not one per agent, and should be rotated if it ever leaks.
///
/// Both are long-lived (default 1 year) since neither goes through the same-service escalation
/// flow that per-agent grants do — rotate by rerunning this tool and redistributing the output.
fn main() -> Result<()> {
    let account_seed = env::var("CAPFORGE_ACCOUNT_SEED").context("CAPFORGE_ACCOUNT_SEED not set")?;
    let account_key = KeyPair::from_seed(&account_seed).context("invalid CAPFORGE_ACCOUNT_SEED")?;
    let account_pubkey = account_key.public_key();

    let ttl_days: i64 = env::var("CAPFORGE_SERVICE_CREDS_TTL_DAYS")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(365);
    let ttl = chrono::Duration::days(ttl_days);

    let service_user = KeyPair::new_user();
    let service_jwt = mint_service_jwt(
        &account_key,
        &account_pubkey,
        &service_user.public_key(),
        &[">".to_string()],
        &["capability.request.*".to_string()],
        ttl,
    )?;

    let requester_user = KeyPair::new_user();
    let requester_jwt = mint_service_jwt(
        &account_key,
        &account_pubkey,
        &requester_user.public_key(),
        &["capability.request.*".to_string()],
        &["_INBOX.>".to_string()],
        ttl,
    )?;

    println!("# CAPFORGE service + requester creds minted (TTL: {ttl_days} days).");
    println!("# Store each block below as a .creds file / secret and wire into cloud-init.");
    println!();
    println!("## capforge-service (the capability-forge binary's own connection):");
    println!("# Store as CAPFORGE_SERVICE_CREDS in the secret store — main.rs writes this");
    println!("# content to a temp file at startup and connects with it.");
    println!("-----BEGIN NATS USER JWT-----");
    println!("{service_jwt}");
    println!("------END NATS USER JWT------");
    println!();
    println!("************************* IMPORTANT *************************");
    println!("NKEY Seed printed below can be used to sign and prove identity.");
    println!("NKEYs are sensitive and should be treated as secrets.");
    println!();
    println!("-----BEGIN USER NKEY SEED-----");
    println!("{}", service_user.seed()?);
    println!("------END USER NKEY SEED------");
    println!("*************************************************************");
    println!();
    println!("## capforge-requester (shared bootstrap credential for any agent to call in):");
    println!("# Store as CAPFORGE_REQUESTER_CREDS — distribute to agents that need to make");
    println!("# capability requests before they hold any grant.");
    println!("-----BEGIN NATS USER JWT-----");
    println!("{requester_jwt}");
    println!("------END NATS USER JWT------");
    println!();
    println!("-----BEGIN USER NKEY SEED-----");
    println!("{}", requester_user.seed()?);
    println!("------END USER NKEY SEED------");

    Ok(())
}
