//! Live two-node mesh integration test.
//!
//! Skips unless `NATS_URL` is set to a reachable NATS server. Exercises the
//! full `--agent=b00t-comms --skill=nats` path: presence, discovery, direct
//! send, and finops receipt minting.

use b00t_chat::ledgrrr::{Ledgrrr, MockLedgrrr};
use b00t_chat::mesh::{MeshFrame, MeshNodeConfig, NatsMeshNode};
use b00t_chat::message::ChatMessage;
use std::sync::Arc;
use std::time::Duration;

#[tokio::test]
async fn two_node_mesh_discovers_and_exchanges() {
    let url = match std::env::var("NATS_URL") {
        Ok(u) => u,
        Err(_) => {
            eprintln!("skip: NATS_URL unset — set it to a reachable NATS server to run");
            return;
        }
    };

    let ledger: Arc<dyn Ledgrrr> = Arc::new(MockLedgrrr::mock());

    let a = NatsMeshNode::new(
        MeshNodeConfig::new("itest-a", &url)
            .with_project("itest")
            .with_ledgrrr(ledger.clone()),
    );
    let b = NatsMeshNode::new(
        MeshNodeConfig::new("itest-b", &url)
            .with_project("itest")
            .with_ledgrrr(ledger.clone()),
    );

    a.connect().await.expect("a connect");
    b.connect().await.expect("b connect");
    a.start_presence().await;
    b.start_presence().await;
    // Let presence heartbeats land before querying.
    tokio::time::sleep(Duration::from_millis(300)).await;

    let peers = a
        .discover_with_timeout(Duration::from_secs(2))
        .await
        .unwrap();
    assert!(
        peers.iter().any(|p| p.agent_id == "itest-b"),
        "a should discover b"
    );

    let msg = ChatMessage::new("itest", "itest-a", "hello mesh");
    a.send("itest-b", &msg).await.unwrap();

    // Drain b's inbox until the direct message arrives (presence frames may
    // arrive first).
    let deadline = tokio::time::Instant::now() + Duration::from_secs(3);
    let mut got_body = None;
    while tokio::time::Instant::now() < deadline {
        match tokio::time::timeout(Duration::from_millis(500), b.recv()).await {
            Ok(Ok(Some(MeshFrame::Direct(m)))) => {
                got_body = Some(m.body);
                break;
            }
            Ok(Ok(Some(_))) => continue, // presence / discovery frame
            Ok(Ok(None)) | Ok(Err(_)) | Err(_) => break,
        }
    }
    assert_eq!(got_body.as_deref(), Some("hello mesh"));

    // Every capability execution minted a finops code for the project.
    assert!(
        !ledger.codes_for("itest").is_empty(),
        "finops codes should be minted for itest"
    );

    a.close().await.ok();
    b.close().await.ok();
}
