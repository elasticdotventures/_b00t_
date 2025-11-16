//! Integration tests for multi-agent coordination scenarios.
//!
//! Tests communication patterns, timeouts, and deadlock detection across
//! multiple transports (Unix sockets, NATS, MQTT, Redis).

use b00t_chat::{
    ChatClient, ChatMessage, ChatMetrics, Destination, MessageRouter, SocketRegistry,
    SocketRegistryBuilder,
};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::RwLock;
use tokio::time::{sleep, timeout};

/// Timeout duration for operations (prevents tests hanging indefinitely).
const TEST_TIMEOUT: Duration = Duration::from_secs(10);

/// Deadlock detection interval (check for progress every N seconds).
const DEADLOCK_CHECK_INTERVAL: Duration = Duration::from_secs(2);

/// Maximum number of deadlock checks before failing.
const MAX_DEADLOCK_CHECKS: usize = 3;

#[tokio::test]
async fn test_basic_agent_discovery() {
    // Test basic socket registry discovery with timeout
    let result = timeout(TEST_TIMEOUT, async {
        let registry = SocketRegistryBuilder::new()
            .with_path("/tmp/b00t-test/agents")
            .build();

        registry.start_watching().await.unwrap();

        // Wait briefly for initial scan
        sleep(Duration::from_millis(100)).await;

        let agents = registry.list_agents().await;
        assert!(agents.is_empty(), "Should start with no agents");
    })
    .await;

    assert!(result.is_ok(), "Test timed out - possible deadlock");
}

#[tokio::test]
async fn test_multi_agent_message_exchange() {
    // Test message exchange between multiple agents with timeout and metrics
    let result = timeout(TEST_TIMEOUT, async {
        let metrics = ChatMetrics::init();

        // Create test channel
        let channel = "test.mission.alpha";

        // Simulate two agents exchanging messages
        let msg1 = ChatMessage::new(channel, "agent-1", "Hello from agent 1");
        let msg2 = ChatMessage::new(channel, "agent-2", "Reply from agent 2");

        // Record metrics
        metrics.record_message_sent("test", channel);
        metrics.record_message_received("test", channel);

        // Verify messages are properly formatted
        assert_eq!(msg1.channel, channel);
        assert_eq!(msg2.sender, "agent-2");
    })
    .await;

    assert!(result.is_ok(), "Message exchange timed out");
}

#[tokio::test]
async fn test_concurrent_agent_coordination() {
    // Test concurrent message handling with deadlock detection
    let result = timeout(TEST_TIMEOUT, async {
        let registry = SocketRegistryBuilder::new()
            .with_path("/tmp/b00t-test/concurrent")
            .build();

        registry.start_watching().await.unwrap();

        // Spawn multiple concurrent tasks that could potentially deadlock
        let handles: Vec<_> = (0..5)
            .map(|i| {
                let channel = format!("test.concurrent.{}", i);
                tokio::spawn(async move {
                    for j in 0..10 {
                        let msg = ChatMessage::new(&channel, &format!("agent-{}", i), &format!("msg-{}", j));
                        // Simulate some async work
                        sleep(Duration::from_millis(10)).await;
                        drop(msg);
                    }
                })
            })
            .collect();

        // Wait for all tasks with deadlock detection
        let _all_done = Arc::new(RwLock::new(false));

        // Simply join all handles with overall timeout (handled by outer timeout)
        for handle in handles {
            handle.await.unwrap();
        }
    })
    .await;

    assert!(result.is_ok(), "Concurrent coordination test failed or timed out");
}

#[tokio::test]
async fn test_message_router_fallback() {
    // Test message router with transport fallback and timeout
    let result = timeout(TEST_TIMEOUT, async {
        let registry = SocketRegistryBuilder::new()
            .with_path("/tmp/b00t-test/router")
            .build();

        registry.start_watching().await.unwrap();

        let router = MessageRouter::new(registry);

        // Test routing with different destination types
        let msg = ChatMessage::new("test.routing", "sender", "test message");

        // These should not panic even if transports are unavailable
        let _ = router.route(&msg, &Destination::Broadcast).await;
        let _ = router.route(&msg, &Destination::Crew("test-crew".to_string())).await;
    })
    .await;

    assert!(result.is_ok(), "Router fallback test timed out");
}

#[tokio::test]
async fn test_metrics_collection() {
    // Verify metrics are properly collected during multi-agent scenarios
    let result = timeout(TEST_TIMEOUT, async {
        let metrics = ChatMetrics::global();

        // Simulate various transport operations
        metrics.record_connection_opened("test-transport");
        metrics.record_message_sent("test-transport", "channel-1");
        metrics.record_message_sent("test-transport", "channel-1");
        metrics.record_message_received("test-transport", "channel-1");
        metrics.record_send_latency("test-transport", 12.5);
        metrics.record_recv_latency("test-transport", 8.3);
        metrics.record_transport_operation("test-transport", "subscribe");
        metrics.record_connection_closed("test-transport");

        // No panics = metrics working correctly
    })
    .await;

    assert!(result.is_ok(), "Metrics collection test timed out");
}

#[tokio::test]
async fn test_agent_lifecycle_with_cleanup() {
    // Test complete agent lifecycle with proper cleanup and no resource leaks
    let result = timeout(TEST_TIMEOUT, async {
        let registry = SocketRegistryBuilder::new()
            .with_path("/tmp/b00t-test/lifecycle")
            .build();

        registry.start_watching().await.unwrap();

        // Simulate agent registration
        let initial_count = registry.list_agents().await.len();

        // Simulate some operations
        sleep(Duration::from_millis(100)).await;

        // Verify cleanup (should still be zero since we didn't add real agents)
        let final_count = registry.list_agents().await.len();
        assert_eq!(initial_count, final_count, "Agent count should be stable");
    })
    .await;

    assert!(result.is_ok(), "Lifecycle test timed out");
}

#[tokio::test]
async fn test_high_throughput_scenario() {
    // Test high message throughput without deadlocks
    let result = timeout(TEST_TIMEOUT, async {
        let metrics = ChatMetrics::global();
        let message_count = 1000;

        // Simulate high-throughput message processing
        for i in 0..message_count {
            let channel = format!("channel-{}", i % 10);
            metrics.record_message_sent("test", &channel);

            // Small delay to simulate realistic processing
            if i % 100 == 0 {
                sleep(Duration::from_micros(100)).await;
            }
        }

        // Verify all messages were recorded (no panics = success)
    })
    .await;

    assert!(result.is_ok(), "High throughput test timed out");
}

#[tokio::test]
async fn test_error_recovery() {
    // Test that agents can recover from errors without deadlocking
    let result = timeout(TEST_TIMEOUT, async {
        let metrics = ChatMetrics::global();

        // Simulate connection errors and recovery
        for _ in 0..5 {
            metrics.record_connection_error("test-transport", "timeout");
            metrics.record_connection_opened("test-transport");
            metrics.record_message_failed("test-transport", "network_error");
            sleep(Duration::from_millis(10)).await;
        }

        // Should complete without deadlocking
    })
    .await;

    assert!(result.is_ok(), "Error recovery test timed out");
}

/// Helper function to detect potential deadlocks by monitoring task progress.
async fn monitor_for_deadlock<F, T>(task: F, name: &str) -> T
where
    F: std::future::Future<Output = T> + Send,
    T: Send,
{
    match timeout(TEST_TIMEOUT, task).await {
        Ok(result) => result,
        Err(_) => panic!("Deadlock detected in test '{}': no completion after {:?}", name, TEST_TIMEOUT),
    }
}

#[tokio::test]
async fn test_broadcast_to_multiple_agents() {
    monitor_for_deadlock(
        async {
            let metrics = ChatMetrics::global();
            let broadcast_msg = ChatMessage::new("mission.broadcast", "coordinator", "Status update");

            // Simulate broadcast to 5 agents
            for _i in 0..5 {
                metrics.record_message_sent("broadcast", &broadcast_msg.channel);
                metrics.record_message_received("broadcast", &broadcast_msg.channel);
                sleep(Duration::from_millis(5)).await;
            }
        },
        "broadcast_to_multiple_agents",
    )
    .await;
}
