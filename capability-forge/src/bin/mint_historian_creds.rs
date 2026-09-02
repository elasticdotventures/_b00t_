use anyhow::{Context, Result};
use capability_forge::jwt_mint::mint_readonly_service_jwt;
use nkeys::KeyPair;
use std::env;

/// One-time (or rotate-by-rerunning) mint of a genuinely read-only NATS user identity for
/// `b00t-historian` under the CAPFORGE account (elasticdotventures/_b00t_#1235) — historian
/// archives hive coordination traffic, it never needs to publish anything.
///
/// Deliberately under CAPFORGE's existing, already-preloaded account rather than a new
/// dedicated account: minting a user under an account the server already trusts needs no
/// server-config redeploy, only the account's own signing seed (already in the secret store
/// as CAPFORGE_ACCOUNT_SEED). A cleaner, dedicated "HIVE" account is the longer-term fix (see
/// the #1235 comment this was posted from) but needs live access to redeploy
/// pods/nats/nats-pod-configured.yaml's resolver_preload — out of reach without it.
///
/// Subject coverage spans both known historian deployments: `hive.>` (the LAN bus, see
/// historian-run.log's own "subscribing to 'hive.sm3ll-fung1.>'") and `souls.>`/`vultr.>`/
/// `b00t.hive.mesh.>` (PR #1230's k8s manifest, the vultr1/k0s deployment). Broader than any
/// single deployment strictly needs, but subscribe-only + publish-explicitly-denied keeps the
/// actual risk of that breadth low - this credential can read hive traffic, it cannot
/// originate any.
fn main() -> Result<()> {
    let account_seed = env::var("CAPFORGE_ACCOUNT_SEED").context("CAPFORGE_ACCOUNT_SEED not set")?;
    let account_key = KeyPair::from_seed(&account_seed).context("invalid CAPFORGE_ACCOUNT_SEED")?;
    let account_pubkey = account_key.public_key();

    let ttl_days: i64 = env::var("HISTORIAN_CREDS_TTL_DAYS")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(365);
    let ttl = chrono::Duration::days(ttl_days);

    let historian_user = KeyPair::new_user();
    let historian_jwt = mint_readonly_service_jwt(
        &account_key,
        &account_pubkey,
        &historian_user.public_key(),
        &[
            "hive.>".to_string(),
            "souls.>".to_string(),
            "vultr.>".to_string(),
            "b00t.hive.mesh.>".to_string(),
        ],
        ttl,
    )?;

    println!("# historian NATS creds minted (TTL: {ttl_days} days) — subscribe-only, publish denied.");
    println!("# Store as HISTORIAN_NATS_CREDS in the secret store; wire b00t_historian.py to");
    println!("# connect with a .creds file (nats.connect(user_credentials=...)) instead of");
    println!("# plain user/password once elasticdotventures/_b00t_#1235's server-side auth");
    println!("# mode issue is otherwise addressed — this mint alone doesn't flip that switch.");
    println!();
    println!("-----BEGIN NATS USER JWT-----");
    println!("{historian_jwt}");
    println!("------END NATS USER JWT------");
    println!();
    println!("************************* IMPORTANT *************************");
    println!("NKEY Seed printed below can be used to sign and prove identity.");
    println!("NKEYs are sensitive and should be treated as secrets.");
    println!();
    println!("-----BEGIN USER NKEY SEED-----");
    println!("{}", historian_user.seed()?);
    println!("------END USER NKEY SEED------");
    println!("*************************************************************");

    Ok(())
}
