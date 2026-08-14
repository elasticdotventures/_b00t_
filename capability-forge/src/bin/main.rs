use anyhow::{Context, Result};
use b00t_c0re_gov::redb_scope_store::RedbScopeStore;
use b00t_c0re_gov::scope_store::ScopeId;
use capability_forge::judge::OpenAiJudge;
use capability_forge::service::{handle_wire_request, CapabilityForge};
use futures::StreamExt;
use nkeys::KeyPair;
use std::env;

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber_init();

    let nats_url = env::var("NATS_URL").unwrap_or_else(|_| "nats://127.0.0.1:4222".to_string());
    let db_path = env::var("CAPFORGE_DB_PATH").context("CAPFORGE_DB_PATH not set")?;
    let account_seed = env::var("CAPFORGE_ACCOUNT_SEED").context("CAPFORGE_ACCOUNT_SEED not set")?;
    let account_pubkey = env::var("CAPFORGE_ACCOUNT_PUBKEY").context("CAPFORGE_ACCOUNT_PUBKEY not set")?;
    let judge_model = env::var("CAPFORGE_JUDGE_MODEL").unwrap_or_else(|_| "gpt-4o-mini".to_string());

    let mut store = RedbScopeStore::open(&db_path, ScopeId::Global, None)
        .with_context(|| format!("opening redb at {db_path}"))?;
    let account_signing_key = KeyPair::from_seed(&account_seed).context("invalid account seed")?;
    let judge = OpenAiJudge::new(judge_model);

    let client = async_nats::connect(&nats_url).await.context("connecting to NATS")?;
    let mut sub = client.subscribe("capability.request.*").await.context("subscribing")?;

    tracing::info!("capability-forge listening on capability.request.*");

    while let Some(msg) = sub.next().await {
        let Some(reply_subject) = msg.reply.clone() else {
            tracing::warn!("request with no reply subject, dropping");
            continue;
        };

        // Per-message deserialize -> handle_request -> serialize -> reply logic (including
        // its log-and-continue error handling) lives in `handle_wire_request` so it can be
        // exercised directly in tests against a real NATS connection -- see
        // `capability-forge/tests/e2e_local_nats.rs`'s
        // `wire_request_round_trips_through_publish_subscribe_reply` test.
        let mut forge = CapabilityForge {
            store: &mut store,
            judge: &judge,
            account_signing_key: &account_signing_key,
            account_pubkey: &account_pubkey,
            grant_ttl: chrono::Duration::minutes(30),
        };
        handle_wire_request(&mut forge, &client, reply_subject, &msg.payload).await;
    }

    Ok(())
}

fn tracing_subscriber_init() {
    let _ = tracing_subscriber::fmt::try_init();
}
