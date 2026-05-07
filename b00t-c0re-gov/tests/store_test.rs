use b00t_c0re_gov::errors::StoreResult;
use b00t_c0re_gov::store::ContextStore;
use b00t_c0re_gov::types::{AgentContext, HookToken, HookType};
use chrono::Utc;
use uuid::Uuid;

fn test_token() -> HookToken {
    HookToken {
        id: Uuid::new_v4(),
        hook_type: HookType::TimerMs(1000),
        created_at: Utc::now(),
        ttl_ms: None,
        description: "integration test hook".to_string(),
    }
}

fn test_context(token: &HookToken) -> AgentContext {
    AgentContext {
        agent_id: "int-test-agent".to_string(),
        task: "integration test task".to_string(),
        gate: "int-test-gate".to_string(),
        result_so_far: serde_json::json!({"phase": "testing"}),
        reasoning: "integration testing".to_string(),
        created_at: Utc::now(),
        hook_token: token.clone(),
        continuation: "resume_integration".to_string(),
    }
}

#[test]
fn test_store_save_load_delete() -> StoreResult<()> {
    let tmpdir = tempfile::tempdir()?;
    let store = ContextStore::with_path(tmpdir.path().join("hooks"));

    let token = test_token();
    let context = test_context(&token);

    // Save
    store.save(&token, &context)?;
    assert_eq!(store.count()?, 1);

    // Load
    let loaded = store.load(&token)?.expect("should load");
    assert_eq!(loaded.agent_id, "int-test-agent");
    assert_eq!(loaded.continuation, "resume_integration");

    // Delete
    store.delete(&token.id)?;
    assert!(store.load(&token)?.is_none());
    assert_eq!(store.count()?, 0);

    Ok(())
}

#[test]
fn test_store_list_pending() -> StoreResult<()> {
    let tmpdir = tempfile::tempdir()?;
    let store = ContextStore::with_path(tmpdir.path().join("hooks"));

    let t1 = test_token();
    let t2 = test_token();
    store.save(&t1, &test_context(&t1))?;
    store.save(&t2, &test_context(&t2))?;

    let pending = store.list_pending()?;
    assert_eq!(pending.len(), 2);

    Ok(())
}

#[test]
fn test_store_crash_safety() -> StoreResult<()> {
    let tmpdir = tempfile::tempdir()?;
    let dir = tmpdir.path().join("hooks");
    std::fs::create_dir_all(&dir)?;

    // Simulate a crash by leaving a .tmp file
    let hook_id = Uuid::new_v4();
    let tmp_path = dir.join(format!("{}.json.tmp", hook_id));
    std::fs::write(&tmp_path, b"garbage from crash")?;

    // No .json should exist
    let json_path = dir.join(format!("{}.json", hook_id));
    assert!(!json_path.exists(), "crash should not leave .json");

    // Now use the store properly
    let store = ContextStore::with_path(dir.clone());
    let token = HookToken {
        id: hook_id,
        hook_type: HookType::TimerMs(500),
        created_at: Utc::now(),
        ttl_ms: None,
        description: "crash safety".to_string(),
    };
    let context = test_context(&token);
    store.save(&token, &context)?;

    // Tmp should be gone, json should exist
    assert!(!tmp_path.exists(), "tmp should be cleaned up");
    assert!(json_path.exists(), "json should exist after save");

    Ok(())
}
