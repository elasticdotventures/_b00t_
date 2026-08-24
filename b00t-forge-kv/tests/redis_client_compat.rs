//! Proves b00t-forge-kv is a real drop-in for the `redis` crate client that
//! `b00t-c0re-lib::redis::RedisComms` actually uses — not just a RESP2
//! implementation in isolation. Runs the real server in-process and drives
//! it with the same crate/version RedisComms depends on.

use std::sync::Arc;

use b00t_forge_kv::{serve, store::Store};
use futures::StreamExt as _;
use redis::AsyncCommands;
use tokio::net::TcpListener;

/// Binds an ephemeral port, spawns the real server against it, and returns
/// a connected `redis` crate client pointed at it.
async fn spawn_server() -> redis::Client {
    let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind ephemeral port");
    let addr = listener.local_addr().expect("local_addr");
    tokio::spawn(serve(listener, Arc::new(Store::new())));

    redis::Client::open(format!("redis://{addr}")).expect("build redis client")
}

#[tokio::test]
async fn ping() {
    let client = spawn_server().await;
    let mut conn = client.get_multiplexed_async_connection().await.expect("connect");
    let pong: String = redis::cmd("PING").query_async(&mut conn).await.expect("PING");
    assert_eq!(pong, "PONG");
}

#[tokio::test]
async fn set_get_roundtrip() {
    let client = spawn_server().await;
    let mut conn = client.get_multiplexed_async_connection().await.expect("connect");
    let _: () = conn.set("k", "v").await.expect("SET");
    let v: String = conn.get("k").await.expect("GET");
    assert_eq!(v, "v");
}

#[tokio::test]
async fn get_missing_key_is_none() {
    let client = spawn_server().await;
    let mut conn = client.get_multiplexed_async_connection().await.expect("connect");
    let v: Option<String> = conn.get("missing").await.expect("GET");
    assert_eq!(v, None);
}

#[tokio::test]
async fn setex_then_expires() {
    let client = spawn_server().await;
    let mut conn = client.get_multiplexed_async_connection().await.expect("connect");
    let _: () = redis::cmd("SETEX")
        .arg("k")
        .arg(1u64)
        .arg("v")
        .query_async(&mut conn)
        .await
        .expect("SETEX");
    let v: Option<String> = conn.get("k").await.expect("GET before expiry");
    assert_eq!(v, Some("v".to_string()));

    tokio::time::sleep(std::time::Duration::from_millis(1100)).await;
    let v: Option<String> = conn.get("k").await.expect("GET after expiry");
    assert_eq!(v, None);
}

#[tokio::test]
async fn hset_hget_hgetall() {
    let client = spawn_server().await;
    let mut conn = client.get_multiplexed_async_connection().await.expect("connect");
    let _: i32 = conn.hset("h", "a", "1").await.expect("HSET a");
    let _: i32 = conn.hset("h", "b", "2").await.expect("HSET b");
    let v: String = conn.hget("h", "a").await.expect("HGET a");
    assert_eq!(v, "1");
    let all: std::collections::HashMap<String, String> = conn.hgetall("h").await.expect("HGETALL");
    assert_eq!(all.get("a"), Some(&"1".to_string()));
    assert_eq!(all.get("b"), Some(&"2".to_string()));
}

/// RedisComms (b00t-c0re-lib/src/redis.rs) issues literal `INCR`/`DECR`
/// (no argument) and `INCRBY` with an explicit amount — mirrored here via
/// raw `redis::cmd` rather than the `redis` crate's `.incr()`/`.decr()`
/// convenience methods, which pick INCRBY/DECRBY depending on the delta
/// argument and would silently test a different command than RedisComms
/// actually sends.
#[tokio::test]
async fn incr_decr() {
    let client = spawn_server().await;
    let mut conn = client.get_multiplexed_async_connection().await.expect("connect");

    let v: i64 = redis::cmd("INCR").arg("c").query_async(&mut conn).await.expect("INCR");
    assert_eq!(v, 1);
    let v: i64 = redis::cmd("INCRBY").arg("c").arg(5).query_async(&mut conn).await.expect("INCRBY");
    assert_eq!(v, 6);
    let v: i64 = redis::cmd("DECR").arg("c").query_async(&mut conn).await.expect("DECR");
    assert_eq!(v, 5);
    let v: i64 = redis::cmd("DECRBY").arg("c").arg(2).query_async(&mut conn).await.expect("DECRBY");
    assert_eq!(v, 3);
}

#[tokio::test]
async fn del_and_exists() {
    let client = spawn_server().await;
    let mut conn = client.get_multiplexed_async_connection().await.expect("connect");
    let _: () = conn.set("k", "v").await.expect("SET");
    let exists: i32 = conn.exists("k").await.expect("EXISTS");
    assert_eq!(exists, 1);
    let deleted: i32 = conn.del("k").await.expect("DEL");
    assert_eq!(deleted, 1);
    let exists: i32 = conn.exists("k").await.expect("EXISTS after DEL");
    assert_eq!(exists, 0);
}

/// The core RedisComms use case: publish + subscribe, exactly matching
/// `agent_coordination.rs`'s presence/routing pattern (HSET registry, then
/// PUBLISH to a channel a subscriber is already listening on).
#[tokio::test]
async fn publish_reaches_subscriber() {
    let client = spawn_server().await;

    let mut pubsub = client.get_async_pubsub().await.expect("pubsub connection");
    pubsub.subscribe("b00t:agents:presence").await.expect("SUBSCRIBE");
    let mut stream = pubsub.on_message();

    let mut publisher = client.get_multiplexed_async_connection().await.expect("connect");
    let subscribers: i32 = publisher
        .publish("b00t:agents:presence", "agent-online:fung1")
        .await
        .expect("PUBLISH");
    assert_eq!(subscribers, 1, "PUBLISH should report exactly one live subscriber");

    let msg = tokio::time::timeout(std::time::Duration::from_secs(2), stream.next())
        .await
        .expect("did not receive published message in time")
        .expect("stream ended unexpectedly");
    let payload: String = msg.get_payload().expect("decode payload");
    assert_eq!(payload, "agent-online:fung1");
}
