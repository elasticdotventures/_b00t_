//! Phase 3 end-to-end tests: Full pipeline — MCP bridge → NATS notification → assignment → task dispatch.
//! Run: cd ~/.b00t && cargo test -p b00t-chat -- --ignored
//! Requires: NATS server running on localhost:4222 (just nats-start)

#[cfg(test)]
mod phase3_bridge_e2e {
    use b00t_chat::{
        AssignmentEngine, AssignmentRule, ChatClient, NotificationMessage, TaskTemplate, TriggerKind,
    };
    use serde_json::json;
    use std::time::Duration;

    async fn nats_client() -> ChatClient {
        ChatClient::nats(
            Some("nats://localhost:4222".into()),
            std::env::var("B00T_HIVE_NATS_USER").ok(),
            std::env::var("B00T_HIVE_NATS_PASSWORD").ok(),
        )
        .expect("create NATS client")
    }

    #[tokio::test]
    #[ignore]
    async fn test_phase3_full_pipeline_simulated_notification() {
        let client = nats_client().await;

        let rule = AssignmentRule::new(
            "file-indexer",
            "Index new files",
            TriggerKind::Event,
            "b00t.notify.files.>",
            TaskTemplate {
                to_agent: "file-indexer-agent".into(),
                action: "index".into(),
                payload_template: json!({
                    "path": "{event.payload}"
                }),
            },
        );

        let engine = AssignmentEngine::new(client.transport().clone());
        engine.add_rule(rule).await.expect("add rule");
        engine.start().await.expect("start engine");

        let mut task_rx = client
            .subscribe_tasks("file-indexer-agent")
            .await
            .expect("subscribe tasks");

        // Simulate what an MCP bridge would do:
        // MCP server emits notifications/resources/updated → bridge converts to b00t.notify.files.resources.updated
        let notification = NotificationMessage::new(
            "files",
            "resources.updated",
            json!({"uri": "file:///data/report.pdf"}),
        );
        client
            .publish_notification(&notification)
            .await
            .expect("publish notification");

        let task = tokio::time::timeout(Duration::from_secs(3), task_rx.recv())
            .await
            .expect("timeout")
            .expect("should receive task");

        assert_eq!(task.action, "index");
        assert!(task.payload["path"].as_str().unwrap().contains("/data/report.pdf"));

        engine.stop().await;
    }

    #[tokio::test]
    #[ignore]
    async fn test_phase3_multiple_rules_multiple_notifications() {
        let client = nats_client().await;

        let engine = AssignmentEngine::new(client.transport().clone());

        engine
            .add_rule(
                AssignmentRule::new(
                    "gmail-handler",
                    "Handle gmail events",
                    TriggerKind::Event,
                    "b00t.notify.gmail.>",
                    TaskTemplate {
                        to_agent: "email-bot".into(),
                        action: "process".into(),
                        payload_template: json!({"source": "gmail"}),
                    },
                ),
            )
            .await
            .unwrap();

        engine
            .add_rule(
                AssignmentRule::new(
                    "slack-handler",
                    "Handle slack events",
                    TriggerKind::Event,
                    "b00t.notify.slack.>",
                    TaskTemplate {
                        to_agent: "chat-bot".into(),
                        action: "process".into(),
                        payload_template: json!({"source": "slack"}),
                    },
                ),
            )
            .await
            .unwrap();

        engine.start().await.expect("start engine");

        let mut email_rx = client
            .subscribe_tasks("email-bot")
            .await
            .expect("subscribe email");

        let mut chat_rx = client
            .subscribe_tasks("chat-bot")
            .await
            .expect("subscribe chat");

        client
            .publish_notification(&NotificationMessage::new("gmail", "new_email", json!({"id": 1})))
            .await
            .expect("publish gmail");

        client
            .publish_notification(&NotificationMessage::new("slack", "new_dm", json!({"id": 2})))
            .await
            .expect("publish slack");

        let timeout = Duration::from_secs(3);
        let t1 = tokio::time::timeout(timeout, email_rx.recv()).await.expect("timeout").expect("email task");
        let t2 = tokio::time::timeout(timeout, chat_rx.recv()).await.expect("timeout").expect("slack task");

        assert_eq!(t1.to_agent, "email-bot");
        assert_eq!(t2.to_agent, "chat-bot");

        engine.stop().await;
    }

    #[tokio::test]
    #[ignore]
    async fn test_phase3_notification_json_roundtrip() {
        let client = nats_client().await;

        let mut rx = client
            .subscribe_notifications("b00t.notify.>")
            .await
            .expect("subscribe");

        let original = NotificationMessage::new(
            "files",
            "new_file",
            json!({
                "uri": "file:///incoming/doc.pdf",
                "name": "doc.pdf",
                "size": 4096
            }),
        )
        .with_correlation("corr-123");

        client
            .publish_notification(&original)
            .await
            .expect("publish");

        let received = tokio::time::timeout(Duration::from_secs(3), rx.recv())
            .await
            .expect("timeout")
            .expect("should receive");

        assert_eq!(received.source, "files");
        assert_eq!(received.event_type, "new_file");
        assert_eq!(received.correlation_id, Some("corr-123".into()));
        assert_eq!(received.payload["uri"], "file:///incoming/doc.pdf");
        assert_eq!(received.payload["size"], 4096);
    }
}
