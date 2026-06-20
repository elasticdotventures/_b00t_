use std::sync::Arc;
use std::time::Duration;

use b00t_c0re_gov::epoch3::{MissionResult, calculate_cake_payout};
use b00t_c0re_gov::ring::HookRing;
use b00t_c0re_gov::scheduler::EventScheduler;
use b00t_c0re_gov::store::ContextStore;
use b00t_c0re_gov::types::*;

/// Full governance pipeline end-to-end test:
///   scheduler -> hook -> context store -> restore -> scoring -> cake payout
#[tokio::test]
async fn test_governance_e2e_pipeline() {
    // 1. Setup with temp directory to avoid polluting the real filesystem
    let tmpdir = tempfile::tempdir().unwrap();
    let ring = Arc::new(HookRing::new());
    let store = ContextStore::with_path(tmpdir.path().join("hooks"));
    let mut scheduler = EventScheduler::new(Arc::clone(&ring));

    // 2. Create a HookToken and AgentContext for a mock task
    let token = HookToken {
        id: uuid::Uuid::new_v4(),
        hook_type: HookType::TimerMs(10), // 10ms — fires very quickly
        created_at: chrono::Utc::now(),
        ttl_ms: Some(60_000),
        description: "e2e test hook".into(),
    };

    let context = AgentContext {
        agent_id: "test-agent".into(),
        task: "e2e-test".into(),
        gate: "test-gate".into(),
        result_so_far: serde_json::json!({"step": "half-done"}),
        reasoning: "Testing the e2e pipeline".into(),
        created_at: chrono::Utc::now(),
        hook_token: token.clone(),
        continuation: "complete_the_rest".into(),
    };

    // 3. Save context to store, then register the hook with the scheduler
    store.save(&token, &context).unwrap();
    scheduler.register(token.clone()).unwrap();

    // 4. Spawn the scheduler in a background tokio task
    let handle = tokio::spawn(async move {
        scheduler.run().await;
    });

    // 5. Wait for the hook to fire (poll the ring buffer, up to 5s)
    let deadline = std::time::Instant::now() + Duration::from_secs(5);
    let mut notification = None;
    while std::time::Instant::now() < deadline {
        let notifications = ring.drain();
        if let Some(n) = notifications.first() {
            notification = Some(n.clone());
            break;
        }
        tokio::time::sleep(Duration::from_millis(5)).await;
    }

    // 6. Verify the hook notification was received
    assert!(notification.is_some(), "Hook should have fired within 5s");
    let n = notification.unwrap();
    assert_eq!(n.hook_id, token.id, "Notification hook_id should match");
    assert!(
        matches!(n.event, HookEvent::Fired),
        "Event should be Fired, got {:?}",
        n.event
    );

    // 7. Restore the agent context from the store
    let restored = store
        .load_by_id(&token.id)
        .unwrap()
        .expect("Context should exist in store after hook fired");
    assert_eq!(restored.agent_id, "test-agent");
    assert_eq!(restored.continuation, "complete_the_rest");
    assert_eq!(restored.result_so_far["step"], "half-done");

    // 8. Delete the context (simulating agent resume / cleanup)
    store.delete(&token.id).unwrap();
    assert!(
        store.load_by_id(&token.id).unwrap().is_none(),
        "Context should be gone after delete"
    );

    // 9. Create a ScoreCard for a mock experiment and verify weighted scoring
    let score = ScoreCard::new(0.8, 0.6, 0.7, 0.9, 0.5, 0.4);
    let weighted = score.weighted_score();
    assert!(
        weighted > 0.0 && weighted <= 1.0,
        "Weighted score should be in (0.0, 1.0], got {}",
        weighted
    );

    // 10. Calculate cake payout using epoch3::calculate_cake_payout
    let result = MissionResult {
        mission_id: "e2e-mission".into(),
        agent_id: "test-agent".into(),
        bounty: 100.0,
        score,
        calories_burned: 50.0,
        completed_at: chrono::Utc::now(),
    };
    let payout = calculate_cake_payout(&result, 100.0, 0.1);
    assert!(payout > 0.0, "Payout should be positive, got {}", payout);
    assert!(
        payout <= 100.0,
        "Payout should not exceed bounty (100.0), got {}",
        payout
    );

    // 11. Cleanup the scheduler task
    handle.abort();

    eprintln!("[🍰] E2E test passed! Cake payout: {:.2}", payout);
}
