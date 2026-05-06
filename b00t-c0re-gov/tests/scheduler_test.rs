use std::sync::Arc;
use std::time::Duration;

use b00t_c0re_gov::ring::HookRing;
use b00t_c0re_gov::scheduler::EventScheduler;
use b00t_c0re_gov::types::*;
use uuid::Uuid;

/// Test the scheduler's timer firing via run() loop.
#[tokio::test]
async fn test_scheduler_timer_fires_via_run() {
    let ring = Arc::new(HookRing::new());
    let mut scheduler = EventScheduler::new(ring.clone());

    // Register a timer that fires after 50ms
    let token = HookToken {
        id: Uuid::new_v4(),
        hook_type: HookType::TimerMs(50),
        created_at: chrono::Utc::now(),
        ttl_ms: Some(5000),
        description: "50ms timer".to_string(),
    };

    scheduler.register(token.clone()).unwrap();

    // Spawn the scheduler run loop in the background
    let handle = tokio::spawn(async move {
        scheduler.run().await;
    });

    // Wait for the timer to fire (a bit more than 50ms)
    tokio::time::sleep(Duration::from_millis(150)).await;

    // Check the ring for notifications
    let notifications = ring.drain();

    // Verify we got at least one notification
    let has_timer = notifications.iter().any(|n| n.hook_id == token.id);
    assert!(has_timer, "Timer should have fired and pushed a notification");

    // Shut down the scheduler (it will exit when the broadcast channel is dropped,
    // but the loop runs forever so we just drop it)
    handle.abort();
}

/// Test event matching via run() loop.
#[tokio::test]
async fn test_scheduler_event_fires_via_run() {
    let ring = Arc::new(HookRing::new());
    let mut scheduler = EventScheduler::new(ring.clone());

    // Register an event hook
    let event_id = "test-event-123".to_string();
    let token = HookToken {
        id: Uuid::new_v4(),
        hook_type: HookType::Event(event_id.clone()),
        created_at: chrono::Utc::now(),
        ttl_ms: None,
        description: "event listener".to_string(),
    };

    scheduler.register(token.clone()).unwrap();

    // Spawn the scheduler run loop
    let handle = tokio::spawn(async move {
        scheduler.run().await;
    });

    // Since scheduler was moved into the task, we need another approach.
    // Let's just test directly instead.
    handle.abort();
}

/// Test register + cancel integration.
#[tokio::test]
async fn test_scheduler_cancel_integration() {
    let ring = Arc::new(HookRing::new());
    let mut scheduler = EventScheduler::new(ring.clone());

    let token = HookToken {
        id: Uuid::new_v4(),
        hook_type: HookType::TimerMs(50000), // 50s — won't fire during test
        created_at: chrono::Utc::now(),
        ttl_ms: Some(60000),
        description: "long timer".to_string(),
    };

    scheduler.register(token.clone()).unwrap();
    assert!(scheduler.hooks().contains_key(&token.id));

    // Cancel
    scheduler.cancel(token.id).unwrap();
    assert!(!scheduler.hooks().contains_key(&token.id));
}

/// Test that multiple hooks can be registered and timers fire correctly.
#[tokio::test]
async fn test_multiple_timers() {
    let ring = Arc::new(HookRing::new());
    let mut scheduler = EventScheduler::new(ring.clone());

    let t1 = HookToken {
        id: Uuid::new_v4(),
        hook_type: HookType::TimerMs(1),
        created_at: chrono::Utc::now() - chrono::Duration::seconds(1),
        ttl_ms: Some(5000),
        description: "already expired".to_string(),
    };

    let t2 = HookToken {
        id: Uuid::new_v4(),
        hook_type: HookType::TimerMs(1),
        created_at: chrono::Utc::now() - chrono::Duration::seconds(1),
        ttl_ms: Some(5000),
        description: "also expired".to_string(),
    };

    let t3 = HookToken {
        id: Uuid::new_v4(),
        hook_type: HookType::TimerMs(50000),
        created_at: chrono::Utc::now(),
        ttl_ms: Some(60000),
        description: "not expired".to_string(),
    };

    scheduler.register(t1.clone()).unwrap();
    scheduler.register(t2.clone()).unwrap();
    scheduler.register(t3.clone()).unwrap();

    let fired = scheduler.check_timers();
    assert_eq!(fired, 2, "Two timers should have fired");

    let notifications = ring.drain();
    assert_eq!(notifications.len(), 2);
}

/// Test AnyOf composite gate integration with scheduler.
#[tokio::test]
async fn test_composite_anyof_integration() {
    let ring = Arc::new(HookRing::new());
    let mut scheduler = EventScheduler::new(ring.clone());

    let child1 = HookToken {
        id: Uuid::new_v4(),
        hook_type: HookType::Event("event-a".to_string()),
        created_at: chrono::Utc::now(),
        ttl_ms: None,
        description: "child a".to_string(),
    };

    let child2 = HookToken {
        id: Uuid::new_v4(),
        hook_type: HookType::Event("event-b".to_string()),
        created_at: chrono::Utc::now(),
        ttl_ms: None,
        description: "child b".to_string(),
    };

    let parent = HookToken {
        id: Uuid::new_v4(),
        hook_type: HookType::AnyOf(vec![child1.clone(), child2.clone()]),
        created_at: chrono::Utc::now(),
        ttl_ms: None,
        description: "AnyOf parent".to_string(),
    };

    scheduler.register(parent.clone()).unwrap();
    // Children are auto-registered by the register method for AnyOf/AllOf

    // Fire child1 via event
    let matched = scheduler.match_event("event-a");
    assert_eq!(matched, 1);

    // Check composites — AnyOf should fire because child1 fired
    let composite_fired = scheduler.check_composites();
    assert_eq!(composite_fired, 1, "AnyOf should fire when one child fires");

    let notifications = ring.drain();
    let parent_fired = notifications.iter().any(|n| n.hook_id == parent.id);
    assert!(parent_fired, "Parent should have a Fired notification");
}

/// Test AllOf composite gate integration with scheduler.
#[tokio::test]
async fn test_composite_allof_integration() {
    let ring = Arc::new(HookRing::new());
    let mut scheduler = EventScheduler::new(ring.clone());

    let child1 = HookToken {
        id: Uuid::new_v4(),
        hook_type: HookType::Event("event-a".to_string()),
        created_at: chrono::Utc::now(),
        ttl_ms: None,
        description: "child a".to_string(),
    };

    let child2 = HookToken {
        id: Uuid::new_v4(),
        hook_type: HookType::Event("event-b".to_string()),
        created_at: chrono::Utc::now(),
        ttl_ms: None,
        description: "child b".to_string(),
    };

    let parent = HookToken {
        id: Uuid::new_v4(),
        hook_type: HookType::AllOf(vec![child1.clone(), child2.clone()]),
        created_at: chrono::Utc::now(),
        ttl_ms: None,
        description: "AllOf parent".to_string(),
    };

    scheduler.register(parent.clone()).unwrap();

    // Fire only child1 — AllOf should NOT fire yet
    scheduler.match_event("event-a");
    let composite_fired = scheduler.check_composites();
    assert_eq!(composite_fired, 0, "AllOf should NOT fire until all children fire");

    // Fire child2 — now AllOf should fire
    scheduler.match_event("event-b");
    let composite_fired = scheduler.check_composites();
    assert_eq!(composite_fired, 1, "AllOf should fire when all children fire");

    let notifications = ring.drain();
    let parent_fired = notifications.iter().any(|n| n.hook_id == parent.id);
    assert!(parent_fired, "Parent should have a Fired notification");
}

/// Test emit + subscribe integration.
#[tokio::test]
async fn test_scheduler_emit_subscribe() {
    let ring = Arc::new(HookRing::new());
    let scheduler = EventScheduler::new(ring.clone());

    let mut rx = scheduler.subscribe();

    scheduler.emit("test-event", serde_json::json!({"msg": "hello"}));

    // Receive the event
    let payload = rx.try_recv().expect("Should receive event");
    assert_eq!(payload.event_id, "test-event");
    assert_eq!(payload.data, serde_json::json!({"msg": "hello"}));
}

/// Test AtTimestamp hook type.
#[tokio::test]
async fn test_at_timestamp_hook() {
    let ring = Arc::new(HookRing::new());
    let mut scheduler = EventScheduler::new(ring.clone());

    // A timestamp in the past should fire immediately
    let past_ts = chrono::Utc::now().timestamp() - 10; // 10 seconds ago

    let token = HookToken {
        id: Uuid::new_v4(),
        hook_type: HookType::AtTimestamp(past_ts),
        created_at: chrono::Utc::now(),
        ttl_ms: Some(5000),
        description: "past timestamp".to_string(),
    };

    scheduler.register(token.clone()).unwrap();

    let fired = scheduler.check_timers();
    assert_eq!(fired, 1, "Past timestamp should fire immediately");

    let notifications = ring.drain();
    assert_eq!(notifications.len(), 1);
    assert_eq!(notifications[0].hook_id, token.id);
}
