//! Quick integration test: NATS chat + task dispatch against local NATS server.
//! Run: cd ~/.b00t && cargo test -p b00t-chat -- --ignored
//! Requires: NATS server running on localhost:4222 (just nats-start).
//! Set B00T_HIVE_NATS_USER/B00T_HIVE_NATS_PASSWORD if the server requires auth.

#[cfg(test)]
mod nats_integration {
    use b00t_chat::{ChatClient, ChatMessage, TaskMessage};
    use serde_json::json;

    #[tokio::test]
    #[ignore] // requires running NATS server
    async fn test_nats_chat_send() {
        let client = ChatClient::nats(
            Some("nats://localhost:4222".into()),
            std::env::var("B00T_HIVE_NATS_USER").ok(),
            std::env::var("B00T_HIVE_NATS_PASSWORD").ok(),
        )
        .expect("create NATS client");

        let msg = ChatMessage::new("test.channel", "fung1", "hello from Rust NATS");
        client.send(&msg).await.expect("send should succeed");
        println!("Chat message sent via NATS");
    }

    #[tokio::test]
    #[ignore] // requires running NATS server
    async fn test_nats_task_dispatch() {
        let client = ChatClient::nats(
            Some("nats://localhost:4222".into()),
            std::env::var("B00T_HIVE_NATS_USER").ok(),
            std::env::var("B00T_HIVE_NATS_PASSWORD").ok(),
        )
        .expect("create NATS client");

        let task = TaskMessage::new(
            "deploy",
            "orchestrator",
            "sm3lly",
            json!({"version": "v2.1.0", "target": "staging"}),
        )
        .with_priority("high");

        client
            .send_task(&task)
            .await
            .expect("task send should succeed");
        println!(
            "Task dispatched: {} -> {}, id={}",
            task.from_agent, task.to_agent, task.task_id
        );
    }

    #[tokio::test]
    #[ignore] // requires running NATS server
    async fn test_nats_task_send_receive() {
        let client = ChatClient::nats(
            Some("nats://localhost:4222".into()),
            std::env::var("B00T_HIVE_NATS_USER").ok(),
            std::env::var("B00T_HIVE_NATS_PASSWORD").ok(),
        )
        .expect("create NATS client");

        // Subscribe first
        let mut rx = client
            .subscribe_tasks("sm3lly")
            .await
            .expect("subscribe tasks");

        // Send a task
        let task = TaskMessage::new("test", "fung1", "sm3lly", json!({"msg": "ping"}));
        client.send_task(&task).await.expect("send task");

        // Receive it
        match tokio::time::timeout(std::time::Duration::from_secs(3), rx.recv()).await {
            Ok(Some(received)) => {
                assert_eq!(received.from_agent, "fung1");
                assert_eq!(received.to_agent, "sm3lly");
                assert_eq!(received.action, "test");
                println!(
                    "Task received OK: {} -> {} ({})",
                    received.from_agent, received.to_agent, received.task_id
                );
            }
            Ok(None) => panic!("Task channel closed"),
            Err(_) => panic!("Task receive timed out — is NATS running?"),
        }
    }

    /// Regression test for the flush-after-publish fix (client.publish() alone
    /// enqueues a frame on the client's write buffer without waiting for it to
    /// reach the socket, so a short-lived process's runtime can drop before the
    /// buffered frame is ever written -- send() would return Ok(()) while the
    /// message silently never landed). Asserts the server's own in_msgs counter
    /// actually increments after send() returns, via the monitoring HTTP API
    /// (`http_port` on nats-server, typically :8222) -- the same signal used to
    /// manually verify this fix originally.
    #[tokio::test]
    #[ignore] // requires running NATS server with monitoring enabled (-m 8222)
    async fn test_nats_send_actually_reaches_server() {
        let monitor_url =
            std::env::var("B00T_NATS_MONITOR_URL").unwrap_or_else(|_| "http://localhost:8222".into());

        async fn in_msgs(monitor_url: &str) -> u64 {
            let varz: serde_json::Value = reqwest::get(format!("{monitor_url}/varz"))
                .await
                .expect("GET /varz — is NATS monitoring enabled? (-m 8222)")
                .json()
                .await
                .expect("parse /varz JSON");
            varz["in_msgs"].as_u64().expect("in_msgs field present")
        }

        let client = ChatClient::nats(
            Some("nats://localhost:4222".into()),
            std::env::var("B00T_HIVE_NATS_USER").ok(),
            std::env::var("B00T_HIVE_NATS_PASSWORD").ok(),
        )
        .expect("create NATS client");

        let before = in_msgs(&monitor_url).await;

        let msg = ChatMessage::new("test.channel", "fung1", "flush regression probe");
        client.send(&msg).await.expect("send should succeed");

        // Give the server a moment to account the message server-side.
        tokio::time::sleep(std::time::Duration::from_millis(200)).await;
        let after = in_msgs(&monitor_url).await;

        assert!(
            after > before,
            "in_msgs did not increase after send() returned (before={before}, after={after}) \
             -- the published message never reached the server, exactly the bug this fix closes"
        );
    }
}
