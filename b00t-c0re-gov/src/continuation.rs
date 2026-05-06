use anyhow::Result;
use uuid::Uuid;

use crate::store::ContextStore;
use crate::types::{AgentContext, HookToken};

/// AgentContinuation handles snapshot/restore of agent state.
///
/// When a governance gate returns a Hook, the agent should:
/// 1. Snapshot its state (task, reasoning, current result, continuation string)
/// 2. Yield execution
/// 3. When the hook fires, restore state and resume
///
/// The continuation string tells the agent what to do when it resumes
/// (e.g. "retry_gate", "proceed", "abort").
pub struct AgentContinuation {
    store: ContextStore,
}

impl AgentContinuation {
    /// Create a new AgentContinuation with the given store.
    pub fn new(store: ContextStore) -> Self {
        AgentContinuation { store }
    }

    /// Create an AgentContinuation with the default store path.
    pub fn with_default_store() -> Result<Self> {
        let store = ContextStore::new()?;
        Ok(AgentContinuation { store })
    }

    /// Snapshot agent state before yielding to a hook.
    ///
    /// Parameters:
    /// - `agent_id`: The agent's identifier
    /// - `task`: What the agent was doing
    /// - `gate`: Which gate triggered the hook
    /// - `result`: The agent's result so far (partial output)
    /// - `reasoning`: The agent's reasoning chain
    /// - `token`: The HookToken that the agent is waiting on
    /// - `continuation`: A string describing what to do when resumed
    ///
    /// Returns the `AgentContext` that was saved.
    pub fn snapshot(
        agent_id: &str,
        task: &str,
        gate: &str,
        result: serde_json::Value,
        reasoning: &str,
        token: &HookToken,
        continuation: &str,
    ) -> Result<AgentContext> {
        let store = ContextStore::new()?;

        let context = AgentContext {
            agent_id: agent_id.to_string(),
            task: task.to_string(),
            gate: gate.to_string(),
            result_so_far: result,
            reasoning: reasoning.to_string(),
            created_at: chrono::Utc::now(),
            hook_token: token.clone(),
            continuation: continuation.to_string(),
        };

        store.save(token, &context)?;
        Ok(context)
    }

    /// Snapshot agent state using an existing store instance.
    pub fn snapshot_with_store(
        &self,
        agent_id: &str,
        task: &str,
        gate: &str,
        result: serde_json::Value,
        reasoning: &str,
        token: &HookToken,
        continuation: &str,
    ) -> Result<AgentContext> {
        let context = AgentContext {
            agent_id: agent_id.to_string(),
            task: task.to_string(),
            gate: gate.to_string(),
            result_so_far: result,
            reasoning: reasoning.to_string(),
            created_at: chrono::Utc::now(),
            hook_token: token.clone(),
            continuation: continuation.to_string(),
        };

        self.store.save(token, &context)?;
        Ok(context)
    }

    /// Restore agent state when a hook fires.
    ///
    /// Looks up the saved context by hook ID.
    /// Returns `None` if no context is found (e.g. already consumed or never saved).
    pub fn restore(hook_id: &Uuid) -> Result<Option<AgentContext>> {
        let store = ContextStore::new()?;
        store.load_by_id(hook_id)
    }

    /// Restore agent state using an existing store instance.
    pub fn restore_with_store(&self, hook_id: &Uuid) -> Result<Option<AgentContext>> {
        self.store.load_by_id(hook_id)
    }

    /// Resume — delete the context after successfully restoring.
    ///
    /// Should be called after `restore()` succeeds, to clean up state.
    pub fn resume(hook_id: &Uuid) -> Result<()> {
        let store = ContextStore::new()?;
        store.delete(hook_id)
    }

    /// Resume using an existing store instance.
    pub fn resume_with_store(&self, hook_id: &Uuid) -> Result<()> {
        self.store.delete(hook_id)
    }

    /// Get a reference to the underlying store.
    pub fn store(&self) -> &ContextStore {
        &self.store
    }

    /// List all pending (unresumed) hook contexts.
    pub fn list_pending(&self) -> Result<Vec<HookToken>> {
        self.store.list_pending()
    }
}

#[cfg(test)]
mod unit_tests {
    use super::*;
    use crate::types::HookType;
    use tempfile::tempdir;

    fn test_token() -> HookToken {
        HookToken {
            id: Uuid::new_v4(),
            hook_type: HookType::TimerMs(1000),
            created_at: chrono::Utc::now(),
            ttl_ms: None,
            description: "test hook".to_string(),
        }
    }

    #[test]
    fn test_snapshot_and_restore() -> Result<()> {
        let tmpdir = tempdir()?;
        let store = ContextStore::with_path(tmpdir.path().join("hooks"));
        let continuation = AgentContinuation::new(store);

        let token = test_token();
        let context = continuation.snapshot_with_store(
            "test-agent",
            "test task",
            "test-gate",
            serde_json::json!({"status": "pending"}),
            "testing snapshot/restore",
            &token,
            "resume_here",
        )?;

        assert_eq!(context.agent_id, "test-agent");
        assert_eq!(context.continuation, "resume_here");

        // Restore
        let restored = continuation.restore_with_store(&token.id)?;
        assert!(restored.is_some());
        let restored = restored.unwrap();
        assert_eq!(restored.agent_id, "test-agent");
        assert_eq!(restored.continuation, "resume_here");
        assert_eq!(restored.task, "test task");
        assert_eq!(restored.reasoning, "testing snapshot/restore");

        // Resume (delete)
        continuation.resume_with_store(&token.id)?;
        let after_resume = continuation.restore_with_store(&token.id)?;
        assert!(after_resume.is_none());

        Ok(())
    }

    #[test]
    fn test_snapshot_static_method() -> Result<()> {
        let token = test_token();

        let context = AgentContinuation::snapshot(
            "static-agent",
            "static task",
            "static-gate",
            serde_json::json!({"phase": "1"}),
            "static reasoning",
            &token,
            "static_resume",
        )?;

        assert_eq!(context.agent_id, "static-agent");

        // Clean up
        AgentContinuation::resume(&token.id)?;

        Ok(())
    }

    #[test]
    fn test_restore_nonexistent() -> Result<()> {
        let tmpdir = tempdir()?;
        let store = ContextStore::with_path(tmpdir.path().join("hooks"));
        let continuation = AgentContinuation::new(store);

        let result = continuation.restore_with_store(&Uuid::new_v4())?;
        assert!(result.is_none());

        Ok(())
    }

    #[test]
    fn test_list_pending() -> Result<()> {
        let tmpdir = tempdir()?;
        let store = ContextStore::with_path(tmpdir.path().join("hooks"));
        let continuation = AgentContinuation::new(store);

        // Should be empty initially
        assert!(continuation.list_pending()?.is_empty());

        let token1 = test_token();
        let token2 = test_token();
        continuation.snapshot_with_store(
            "agent-1",
            "task 1",
            "gate-1",
            serde_json::json!({}),
            "reasoning 1",
            &token1,
            "continue",
        )?;
        continuation.snapshot_with_store(
            "agent-2",
            "task 2",
            "gate-2",
            serde_json::json!({}),
            "reasoning 2",
            &token2,
            "continue",
        )?;

        let pending = continuation.list_pending()?;
        assert_eq!(pending.len(), 2);

        // Clean up
        continuation.resume_with_store(&token1.id)?;
        continuation.resume_with_store(&token2.id)?;

        Ok(())
    }
}
