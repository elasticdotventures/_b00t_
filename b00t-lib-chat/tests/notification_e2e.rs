//! End-to-end tests: MCP push notification → NATS → ACP assimilation.
//! Phase 1: NotificationMessage publish + subscribe via real NATS.
//! Run: cd ~/.b00t && cargo test -p b00t-chat -- --ignored
//! Requires: NATS server running on localhost:4222 (just nats-start)

#[cfg(test)]
mod phase1_notification_e2e {
    use b00t_chat::{ChatClient, NotificationMessage};
    use serde_json::json;
    use std::time::Duration;

    async fn nats_client() -> ChatClient {
        ChatClient::nats(
            Some("nats://localhost:4222".into()),
            Some("b00t".into()),
            Some("b00t-hive-lan".into()),
        )
        .expect("create NATS client")
    }

    #[tokio::test]
    #[ignore]
    async fn test_phase1_notification_publish() {
        let client = nats_client().await;
        let notification = NotificationMessage::new(
            "gmail",
            "new_email",
            json!({"from": "alerts@example.com", "subject": "CI failed"}),
        );
        client
            .publish_notification(&notification)
            .await
            .expect("publish notification");
    }

    #[tokio::test]
    #[ignore]
    async fn test_phase1_notification_subscribe_receive() {
        let client = nats_client().await;

        let mut rx = client
            .subscribe_notifications("b00t.notify.>")
            .await
            .expect("subscribe notifications");

        let notification = NotificationMessage::new(
            "slack",
            "new_message",
            json!({"channel": "#dev", "text": "deploy ready"}),
        );

        client
            .publish_notification(&notification)
            .await
            .expect("publish notification");

        match tokio::time::timeout(Duration::from_secs(3), rx.recv()).await {
            Ok(Some(received)) => {
                assert_eq!(received.source, "slack");
                assert_eq!(received.event_type, "new_message");
                assert_eq!(received.payload["channel"], "#dev");
                assert_eq!(received.payload["text"], "deploy ready");
            }
            Ok(None) => panic!("Notification channel closed unexpectedly"),
            Err(_) => panic!("Notification receive timed out — is NATS running?"),
        }
    }

    #[tokio::test]
    #[ignore]
    async fn test_phase1_notification_subject_routing() {
        let client = nats_client().await;

        let mut all_rx = client
            .subscribe_notifications("b00t.notify.>")
            .await
            .expect("subscribe all");
        let mut gmail_rx = client
            .subscribe_notifications("b00t.notify.gmail.>")
            .await
            .expect("subscribe gmail");
        let mut slack_rx = client
            .subscribe_notifications("b00t.notify.slack.>")
            .await
            .expect("subscribe slack");

        let gmail_notif = NotificationMessage::new("gmail", "new_email", json!({"id": 1}));
        let slack_notif = NotificationMessage::new("slack", "new_dm", json!({"id": 2}));

        client
            .publish_notification(&gmail_notif)
            .await
            .expect("publish gmail");
        client
            .publish_notification(&slack_notif)
            .await
            .expect("publish slack");

        let timeout = Duration::from_secs(3);

        let g1 = tokio::time::timeout(timeout, gmail_rx.recv()).await;
        let g2 = tokio::time::timeout(timeout, slack_rx.recv()).await;
        let a1 = tokio::time::timeout(timeout, all_rx.recv()).await;
        let a2 = tokio::time::timeout(timeout, all_rx.recv()).await;

        assert!(g1.is_ok(), "gmail should receive its notification");
        assert!(g2.is_ok(), "slack should receive its notification");
        assert!(a1.is_ok(), "wildcard should receive first notification");
        assert!(a2.is_ok(), "wildcard should receive second notification");

        let gmail_received = g1.unwrap().expect("gmail channel not closed");
        let slack_received = g2.unwrap().expect("slack channel not closed");

        assert_eq!(gmail_received.source, "gmail");
        assert_eq!(slack_received.source, "slack");
    }
}
