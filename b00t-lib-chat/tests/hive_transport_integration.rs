//! Broker-free integration test for the hive transport abstraction.
//!
//! Two `NatsMeshNode`s share one in-process [`MemoryHiveTransport`] (no NATS
//! broker). Gossip is the initial discovery mechanism that hands off to the
//! (memory) broker: `announce` gossips presence, `discover` publishes a query,
//! peers learn each other via gossip + query/reply, and direct send/recv works.

use b00t_chat::hive_transport::MemoryHiveTransport;
use b00t_chat::mesh::{MeshFrame, MeshNodeConfig, NatsMeshNode};
use std::sync::Arc;
use tokio::time::Duration;

#[tokio::test]
async fn memory_transport_mesh_discovers_via_gossip() {
    let shared = Arc::new(MemoryHiveTransport::new());

    let a = NatsMeshNode::new(
        MeshNodeConfig::new("alpha", "memory://local")
            .with_transport(shared.clone())
            .with_project("test"),
    );
    let b = NatsMeshNode::new(
        MeshNodeConfig::new("bravo", "memory://local")
            .with_transport(shared.clone())
            .with_project("test"),
    );

    a.connect().await.unwrap();
    b.connect().await.unwrap();
    // Let the inbound forwarding tasks poll their streams before publishing.
    tokio::time::sleep(Duration::from_millis(150)).await;

    // Gossip-driven discovery: alpha announces, bravo learns via epidemic gossip.
    a.announce().await.unwrap();
    b.announce().await.unwrap();
    tokio::time::sleep(Duration::from_millis(150)).await;

    let discovered = a
        .discover_with_timeout(Duration::from_millis(500))
        .await
        .unwrap();
    let ids: Vec<&str> = discovered.iter().map(|e| e.agent_id.as_str()).collect();
    assert!(
        ids.contains(&"bravo"),
        "alpha should discover bravo via gossip: {ids:?}"
    );

    let discovered_b = b
        .discover_with_timeout(Duration::from_millis(500))
        .await
        .unwrap();
    let ids_b: Vec<&str> = discovered_b.iter().map(|e| e.agent_id.as_str()).collect();
    assert!(
        ids_b.contains(&"alpha"),
        "bravo should discover alpha: {ids_b:?}"
    );

    a.close().await.unwrap();
    b.close().await.unwrap();
}

#[tokio::test]
async fn memory_transport_direct_send_recv() {
    let shared = Arc::new(MemoryHiveTransport::new());

    let a = NatsMeshNode::new(
        MeshNodeConfig::new("alpha", "memory://local").with_transport(shared.clone()),
    );
    let b = NatsMeshNode::new(
        MeshNodeConfig::new("bravo", "memory://local").with_transport(shared.clone()),
    );
    a.connect().await.unwrap();
    b.connect().await.unwrap();
    a.announce().await.unwrap();

    let msg = b00t_chat::message::ChatMessage::new("chan", "alpha", "hello bravo");
    a.send("bravo", &msg).await.unwrap();

    let frame = tokio::time::timeout(Duration::from_secs(2), b.recv())
        .await
        .unwrap()
        .unwrap()
        .unwrap();
    match frame {
        MeshFrame::Direct(m) => assert_eq!(m.body, "hello bravo"),
        other => panic!("expected Direct frame, got {other:?}"),
    }

    a.close().await.unwrap();
    b.close().await.unwrap();
}
