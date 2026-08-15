//! One-off validation, not part of the crate's normal test suite: proves a *real, externally
//! running* `capability-forge` binary (started via `bin/main.rs`, connected with a genuine
//! `CAPFORGE_SERVICE_CREDS_FILE`) answers a request sent by the shared "capforge-requester"
//! credential minted by `bin/mint_service_creds.rs` -- the two identities that make the
//! service's production NATS wiring work, exercised together over the wire rather than via
//! the in-process harness `tests/e2e_local_nats.rs` uses. Run manually:
//!   cargo run -p capability-forge --example validate_service_request -- \
//!     <nats_url> <requester_creds_path> <agent_id> <agent_seed> <skill>

use anyhow::{Context, Result};
use capability_forge::identity::AgentKeyPair;
use capability_forge::request::{CapabilityReply, CapabilityRequest, SignedRequest};
use std::env;
use std::time::Duration;

#[tokio::main]
async fn main() -> Result<()> {
    let args: Vec<String> = env::args().collect();
    let usage = "usage: validate_service_request <nats_url> <requester_creds_path> <agent_id> <agent_seed> <skill>";
    let nats_url = args.get(1).context(usage)?;
    let requester_creds_path = args.get(2).context(usage)?;
    let agent_id = args.get(3).context(usage)?;
    let agent_seed = args.get(4).context(usage)?;
    let skill = args.get(5).context(usage)?;

    let agent_kp = AgentKeyPair::from_seed(agent_seed).context("invalid agent seed")?;
    let signed = SignedRequest::sign(
        &agent_kp,
        CapabilityRequest {
            agent_id: agent_id.clone(),
            requested_skills: vec![skill.clone()],
            justification: "validate_service_request smoke test".to_string(),
        },
    )?;

    let client = async_nats::ConnectOptions::new()
        .credentials_file(requester_creds_path)
        .await?
        .connect(nats_url)
        .await
        .context("connect with capforge-requester creds failed")?;

    let subject = format!("capability.request.{agent_id}");
    let response = tokio::time::timeout(
        Duration::from_secs(5),
        client.request(subject, serde_json::to_vec(&signed)?.into()),
    )
    .await
    .context("timed out waiting for capability-forge reply")??;

    let reply: CapabilityReply = serde_json::from_slice(&response.payload)?;
    println!("granted: {:?}", reply.granted);
    println!("denied: {:?}", reply.denied);
    println!("jwt present: {}", reply.jwt.is_some());
    anyhow::ensure!(reply.granted.contains(skill), "expected {skill} to be granted, got {reply:?}");
    println!("OK: live capability-forge service granted {skill} to {agent_id} over real NATS");
    Ok(())
}
