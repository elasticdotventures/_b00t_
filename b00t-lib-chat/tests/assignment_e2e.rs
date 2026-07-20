//! Phase 2 end-to-end tests: MCP notification → AssignmentRule match → TaskMessage dispatch.
//! Run: cd ~/.b00t && cargo test -p b00t-chat -- --ignored
//! Requires: NATS server running on localhost:4222 (just nats-start)

#[cfg(test)]
mod phase2_assignment_e2e {
    use b00t_chat::{
        AssignmentEngine, AssignmentRule, ChatClient, Condition, ConditionOp, NotificationMessage,
        TaskTemplate, TimerSpec, TriggerKind,
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
    async fn test_phase2_notification_triggers_rule_dispatches_task() {
        let client = nats_client().await;

        let rule = AssignmentRule::new(
            "ci-alert",
            "CI failure alerts",
            TriggerKind::Event,
            "b00t.notify.gmail.>",
            TaskTemplate {
                to_agent: "devops-bot".into(),
                action: "investigate".into(),
                payload_template: json!({
                    "source": "{event.source}",
                    "event": "{event.type}",
                    "data": "{event.payload}"
                }),
            },
        )
        .with_condition(Condition {
            field: "subject".to_string(),
            operator: ConditionOp::Contains,
            value: "CI failed".to_string(),
        });

        let engine = AssignmentEngine::new(client.transport().clone());
        engine.add_rule(rule).await.expect("add rule");
        engine.start().await.expect("start engine");

        let mut task_rx = client
            .subscribe_tasks("devops-bot")
            .await
            .expect("subscribe tasks");

        let notification = NotificationMessage::new(
            "gmail",
            "new_email",
            json!({"from": "ci@example.com", "subject": "CI failed on main"}),
        );
        client
            .publish_notification(&notification)
            .await
            .expect("publish");

        match tokio::time::timeout(Duration::from_secs(3), task_rx.recv()).await {
            Ok(Some(task)) => {
                assert_eq!(task.to_agent, "devops-bot");
                assert_eq!(task.action, "investigate");
                assert_eq!(task.payload["source"], "gmail");
                assert_eq!(task.payload["event"], "new_email");
            }
            Ok(None) => panic!("Task channel closed unexpectedly"),
            Err(_) => panic!("Task not received within timeout — engine may not be running"),
        }
        engine.stop().await;
    }

    #[tokio::test]
    #[ignore]
    async fn test_phase2_rule_filters_non_matching_notifications() {
        let client = nats_client().await;

        let rule = AssignmentRule::new(
            "friends-only",
            "Only friends",
            TriggerKind::Event,
            "b00t.notify.gmail.>",
            TaskTemplate {
                to_agent: "chat-bot".into(),
                action: "reply".into(),
                payload_template: json!({"from": "{event.payload}"}),
            },
        )
        .with_condition(Condition {
            field: "from".to_string(),
            operator: ConditionOp::Contains,
            value: "friend@".to_string(),
        });

        let engine = AssignmentEngine::new(client.transport().clone());
        engine.add_rule(rule).await.expect("add rule");
        engine.start().await.expect("start engine");

        let mut task_rx = client
            .subscribe_tasks("chat-bot")
            .await
            .expect("subscribe tasks");

        let non_matching = NotificationMessage::new(
            "gmail",
            "new_email",
            json!({"from": "spam@ads.com", "subject": "buy now"}),
        );
        client
            .publish_notification(&non_matching)
            .await
            .expect("publish");

        let matching = NotificationMessage::new(
            "gmail",
            "new_email",
            json!({"from": "friend@personal.com", "subject": "hey"}),
        );
        client
            .publish_notification(&matching)
            .await
            .expect("publish");

        let task = tokio::time::timeout(Duration::from_secs(5), task_rx.recv())
            .await
            .expect("timeout")
            .expect("should receive one task");

        assert!(task.payload.to_string().contains("friend@"));
        engine.stop().await;
    }

    #[tokio::test]
    #[ignore]
    async fn test_phase2_engine_running_dispatch_flow() {
        let client = nats_client().await;

        let rule = AssignmentRule::new(
            "all-files",
            "Any new file",
            TriggerKind::Event,
            "b00t.notify.files.>",
            TaskTemplate {
                to_agent: "indexer".into(),
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
            .subscribe_tasks("indexer")
            .await
            .expect("subscribe tasks");

        let n = NotificationMessage::new(
            "files",
            "new_file",
            json!({"path": "/docs/readme.md", "size": 2048}),
        );
        client.publish_notification(&n).await.expect("publish");

        let task = tokio::time::timeout(Duration::from_secs(3), task_rx.recv())
            .await
            .expect("timeout")
            .expect("should receive indexed task");

        assert_eq!(task.to_agent, "indexer");
        assert_eq!(task.action, "index");
        assert!(
            task.payload["path"]
                .as_str()
                .unwrap()
                .contains("/docs/readme.md")
        );

        engine.stop().await;
    }

    #[tokio::test]
    #[ignore]
    async fn test_phase2_timer_rule_dispatches_task() {
        let client = nats_client().await;

        let rule = AssignmentRule::new(
            "heartbeat",
            "Heartbeat every 2 seconds",
            TriggerKind::Timer,
            "unused-for-timer",
            TaskTemplate {
                to_agent: "monitor".into(),
                action: "heartbeat".into(),
                payload_template: json!({"type": "timer", "rule": "heartbeat"}),
            },
        )
        .with_timer(TimerSpec::interval_secs(2));

        let engine = AssignmentEngine::new(client.transport().clone());
        engine.add_rule(rule).await.expect("add timer rule");
        engine.start().await.expect("start engine");

        let mut task_rx = client
            .subscribe_tasks("monitor")
            .await
            .expect("subscribe tasks");

        let task = tokio::time::timeout(std::time::Duration::from_secs(8), task_rx.recv())
            .await
            .expect("timeout")
            .expect("should receive timer task");

        assert_eq!(task.to_agent, "monitor");
        assert_eq!(task.action, "heartbeat");
        assert_eq!(task.payload["type"], "timer");

        engine.stop().await;
    }

    #[tokio::test]
    #[ignore]
    async fn test_phase2_cron_timer_dispatches_task() {
        let client = nats_client().await;

        let rule = AssignmentRule::new(
            "cron-every-5sec",
            "Cron every 5 seconds",
            TriggerKind::Timer,
            "unused-for-timer",
            TaskTemplate {
                to_agent: "cron-bot".into(),
                action: "cron-tick".into(),
                payload_template: json!({"type": "cron"}),
            },
        )
        .with_timer(TimerSpec::cron("1/5 * * * * * *"));

        let engine = AssignmentEngine::new(client.transport().clone());
        engine.add_rule(rule).await.expect("add cron rule");
        engine.start().await.expect("start engine");

        let mut task_rx = client
            .subscribe_tasks("cron-bot")
            .await
            .expect("subscribe tasks");

        let task = tokio::time::timeout(std::time::Duration::from_secs(15), task_rx.recv())
            .await
            .expect("timeout")
            .expect("should receive cron task");

        assert_eq!(task.to_agent, "cron-bot");
        assert_eq!(task.action, "cron-tick");
        assert_eq!(task.payload["type"], "cron");

        engine.stop().await;
    }
}
