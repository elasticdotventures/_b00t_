use anyhow::Result;
use capability_forge::bootstrap::{bootstrap_operator, mint_account, MintedAccount, OperatorGenesis};
use std::env;

/// Genesis operation: mints a brand-new NATS operator, SYS account/user
/// under it, and a CAPFORGE account — for use only when the original
/// operator's signing key is genuinely unrecoverable and a fresh trust
/// root is the deliberate choice (this orphans whatever currently trusts
/// the old operator identity; it is not a routine operation).
fn main() -> Result<()> {
    let operator_name = env::var("NATS_OPERATOR_NAME").unwrap_or_else(|_| "b00t-operator".to_string());

    let genesis: OperatorGenesis = bootstrap_operator(&operator_name)?;
    let MintedAccount {
        account_pubkey: capforge_pubkey,
        account_seed: capforge_seed,
        account_jwt: capforge_jwt,
    } = mint_account(&genesis.operator_signing_seed, "CAPFORGE")?;

    println!("# Fresh NATS operator \"{operator_name}\" genesis. This REPLACES the trust root —");
    println!("# nothing that trusts the old operator will keep working until updated.");
    println!();
    println!("## 1. Replace pods/nats/nats-pod-configured.yaml's nats-server.conf ConfigMap data with:");
    println!();
    println!("    // Operator \"{operator_name}\"");
    println!("    operator: {}", genesis.operator_jwt);
    println!();
    println!("    system_account: {}", genesis.sys_account_pubkey);
    println!();
    println!("    resolver: MEMORY");
    println!();
    println!("    resolver_preload: {{");
    println!("      // Account \"SYS\"");
    println!("      {}: {}", genesis.sys_account_pubkey, genesis.sys_account_jwt);
    println!();
    println!("      // Account \"CAPFORGE\"");
    println!("      {capforge_pubkey}: {capforge_jwt}");
    println!("    }}");
    println!();
    println!("## 2. Secrets to store (config/global/* in the secret store, matching the");
    println!("##    VULTR_API_KEY pattern in terraform/b00t/vultr_node.tf):");
    println!();
    println!("# Root identity key — keep this the MOST restricted of everything here; it is");
    println!("# never needed again after this genesis unless a NEW signing key must be added.");
    println!("      NATS_OPERATOR_ROOT_SEED={}", genesis.operator_root_seed);
    println!();
    println!("# Designated signing key — this is what mint_account (and any future account");
    println!("# creation) actually needs going forward, not the root key above.");
    println!("      NATS_OPERATOR_SIGNING_SEED={}", genesis.operator_signing_seed);
    println!();
    println!("      NATS_SYS_ACCOUNT_SEED={}", genesis.sys_account_seed);
    println!("      NATS_SYS_USER_SEED={}", genesis.sys_user_seed);
    println!("      NATS_SYS_USER_JWT={}", genesis.sys_user_jwt);
    println!();
    println!("      CAPFORGE_ACCOUNT_SEED={capforge_seed}");
    println!("      CAPFORGE_ACCOUNT_PUBKEY={capforge_pubkey}");
    println!();
    println!("## 3. NOT done by this tool: actually applying the new nats-server.conf to the");
    println!("##    live Vultr node (tofu apply + a nats-server reload/restart) — that's a real");
    println!("##    production cutover with brief NATS downtime, needs its own explicit go-ahead");
    println!("##    at the moment it's actually run, separate from generating these values.");

    Ok(())
}
