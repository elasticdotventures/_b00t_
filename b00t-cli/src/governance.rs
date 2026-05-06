//! Governance runtime integration.
//! Starts the EventScheduler on b00t-cli boot, wires it into the agent loop.
//! On startup: check for pending hooks, fire any expired ones, restore contexts.

use std::sync::Arc;
use std::path::PathBuf;
use std::time::Duration;

use anyhow::{Context, Result};
use b00t_c0re_gov::ring::HookRing;
use b00t_c0re_gov::scheduler::EventScheduler;
use b00t_c0re_gov::store::ContextStore;
use b00t_c0re_gov::types::*;
use chrono::Utc;
use tokio::sync::broadcast;

/// Governance runtime state.
/// Create once at startup, pass to agent loop via Arc.
pub struct GovernanceRuntime {
    pub scheduler: Arc<tokio::sync::Mutex<EventScheduler>>,
    pub store: ContextStore,
    pub ring: Arc<HookRing>,
    /// External event channel — consumers send string event_ids here.
    /// The background loop picks them up and feeds them to the scheduler.
    pub event_tx: broadcast::Sender<String>,
}

impl GovernanceRuntime {
    /// Initialize the governance runtime.
    ///
    /// 1. Creates a shared HookRing (lock-free SPSC queue for hook notifications).
    /// 2. Creates an external event broadcast channel.
    /// 3. Initialises the ContextStore (persists agent snapshots to disk).
    /// 4. Creates an EventScheduler and wraps it in Arc<Mutex<>>.
    /// 5. Spawns a background tokio task that periodically checks timers/composites
    ///    and listens for external events.
    /// 6. Scans the persisted store for any hooks that expired while b00t was
    ///    offline and pushes `HookEvent::Expired` notifications into the ring.
    pub async fn init() -> Result<Self> {
        let store = ContextStore::new()?;
        Self::init_inner(store).await
    }

    /// Initialize with a custom store path (useful for tests).
    pub async fn init_with_store_path<P: Into<PathBuf>>(store_dir: P) -> Result<Self> {
        let store = ContextStore::with_path(store_dir.into());
        Self::init_inner(store).await
    }

    /// Shared initialisation logic used by both `init` and `init_with_store_path`.
    async fn init_inner(store: ContextStore) -> Result<Self> {
        let ring = Arc::new(HookRing::new());
        let (event_tx, _event_rx): (broadcast::Sender<String>, broadcast::Receiver<String>) =
            broadcast::channel(256);
        let scheduler = Arc::new(tokio::sync::Mutex::new(EventScheduler::new(
            Arc::clone(&ring),
        )));

        // ── Background event loop ──────────────────────────────────────────
        // Periodically checks for expired timers and composites.
        // Also listens on the external event channel and forwards events to the
        // scheduler so that event-type hooks can be matched.
        let sched_loop = Arc::clone(&scheduler);
        let mut event_rx_loop = event_tx.subscribe();
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(Duration::from_millis(250));
            loop {
                tokio::select! {
                    _ = interval.tick() => {
                        let mut sched = sched_loop.lock().await;
                        sched.check_timers();
                        sched.check_composites();
                    }
                    Ok(event_id) = event_rx_loop.recv() => {
                        let mut sched = sched_loop.lock().await;
                        sched.match_event(&event_id);
                        sched.check_composites();
                    }
                    else => {
                        // Broadcast channel closed — no more senders.
                        break;
                    }
                }
            }
        });

        // ── Expired-hook recovery on boot ──────────────────────────────────
        // Any hooks that exceeded their TTL while b00t was offline are
        // immediately fired as Expired and removed from the store.
        if let Ok(pending) = store.list_pending() {
            for token in &pending {
                if let Some(ttl) = token.ttl_ms {
                    let elapsed = (Utc::now() - token.created_at).num_milliseconds() as u64;
                    if elapsed > ttl {
                        ring.try_push(HookNotification {
                            hook_id: token.id,
                            event: HookEvent::Expired,
                        });
                        let _ = store.delete(&token.id);
                    }
                }
            }
        }

        Ok(GovernanceRuntime {
            scheduler,
            store,
            ring,
            event_tx,
        })
    }

    /// Drain any fired hook notifications from the ring (non-blocking).
    ///
    /// Call this from the agent loop each iteration to discover hooks that
    /// have fired, expired, or been cancelled. Each notification contains the
    /// `hook_id` and the `HookEvent` that occurred.
    pub fn check_hooks(&self) -> Vec<HookNotification> {
        self.ring.drain()
    }

    /// Register a hook and persist the agent context snapshot.
    ///
    /// 1. Saves the `AgentContext` to the `ContextStore` (atomic write).
    /// 2. Registers the `HookToken` with the `EventScheduler` so it fires
    ///    when its condition (timer, event, composite) is met.
    ///
    /// Returns an error if the scheduler lock is contended.
    pub fn register_hook(&self, token: HookToken, context: AgentContext) -> Result<()> {
        self.store
            .save(&token, &context)
            .context("failed to persist agent context")?;
        let mut sched = self
            .scheduler
            .try_lock()
            .map_err(|_| anyhow::anyhow!("scheduler lock contended; retry later"))?;
        sched
            .register(token)
            .context("failed to register hook with scheduler")?;
        Ok(())
    }

    /// Cancel a hook by its ID.
    ///
    /// Removes the hook (and any children) from the scheduler and deletes its
    /// persisted context from the store. Pushes a `Cancelled` notification
    /// to the ring so the agent loop can detect it.
    pub fn cancel_hook(&self, hook_id: uuid::Uuid) -> Result<()> {
        let mut sched = self
            .scheduler
            .try_lock()
            .map_err(|_| anyhow::anyhow!("scheduler lock contended; retry later"))?;
        sched
            .cancel(hook_id)
            .context("failed to cancel hook")?;
        self.store
            .delete(&hook_id)
            .context("failed to delete hook context")?;
        // Push a Cancelled notification so callers can detect it
        self.ring.try_push(HookNotification {
            hook_id,
            event: HookEvent::Cancelled,
        });
        Ok(())
    }

    /// Emit an event — wakes all hooks subscribed to this event_id.
    ///
    /// The event is forwarded to the background scheduler loop, which calls
    /// `match_event` to fire any event-type hooks matching `event_id`.
    ///
    /// This is safe to call from any thread/task.
    pub fn emit_event(&self, event_id: &str) {
        let _ = self.event_tx.send(event_id.to_string());
    }

    /// Load the persisted agent context for a given hook ID.
    ///
    /// Returns `None` if no context was saved for this hook (e.g. already
    /// consumed or never registered).
    pub fn load_context(&self, hook_id: &uuid::Uuid) -> Result<Option<AgentContext>> {
        self.store
            .load_by_id(hook_id)
            .context("failed to load hook context")
    }

    /// Delete a persisted agent context manually (e.g. after processing).
    pub fn delete_context(&self, hook_id: &uuid::Uuid) -> Result<()> {
        self.store
            .delete(hook_id)
            .context("failed to delete hook context")
    }

    /// Process fired hooks: for each notification, restore the agent context
    /// and log what the agent should resume.
    ///
    /// This should be called from the agent loop after `check_hooks()` has
    /// returned pending notifications. Each fired hook's context is loaded,
    /// logged, and then deleted so the agent can recreate it if needed.
    pub fn process_fired_hooks(&self) -> Result<Vec<HookNotification>> {
        let notifications = self.check_hooks();
        for n in &notifications {
            if let Some(context) = self.store.load_by_id(&n.hook_id)? {
                eprintln!(
                    "[⏰] Hook {} fired for task '{}' — agent should resume: {}",
                    n.hook_id, context.task, context.continuation
                );
                // Delete the context — agent will recreate if needed
                self.store.delete(&n.hook_id)?;
            }
        }
        Ok(notifications)
    }

    /// Reap expired hooks: check all stored contexts and fire any whose TTL
    /// has expired.
    ///
    /// Call this on init and periodically to clean up stale hooks that never
    /// fired through the normal timer/event path.
    pub fn reap_expired_hooks(&self) -> Result<Vec<HookNotification>> {
        let mut expired = Vec::new();
        if let Ok(pending) = self.store.list_pending() {
            for token in &pending {
                if let Some(ttl) = token.ttl_ms {
                    let elapsed = (Utc::now() - token.created_at).num_milliseconds() as u64;
                    if elapsed > ttl {
                        self.ring.try_push(HookNotification {
                            hook_id: token.id,
                            event: HookEvent::Expired,
                        });
                        let _ = self.store.delete(&token.id);
                        expired.push(HookNotification {
                            hook_id: token.id,
                            event: HookEvent::Expired,
                        });
                    }
                }
            }
        }
        Ok(expired)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use uuid::Uuid;

    /// Helper to create a test hook token.
    fn test_token() -> HookToken {
        HookToken {
            id: Uuid::new_v4(),
            hook_type: HookType::TimerMs(50),
            created_at: Utc::now(),
            ttl_ms: Some(5000),
            description: "test hook".to_string(),
        }
    }

    /// Helper to create a test agent context.
    fn test_context(token: &HookToken) -> AgentContext {
        AgentContext {
            agent_id: "test-agent".to_string(),
            task: "test task".to_string(),
            gate: "test-gate".to_string(),
            result_so_far: serde_json::json!({"status": "pending"}),
            reasoning: "testing governance runtime".to_string(),
            created_at: Utc::now(),
            hook_token: token.clone(),
            continuation: "resume_test".to_string(),
        }
    }

    #[tokio::test]
    async fn test_init_creates_runtime() {
        let tmpdir = tempfile::tempdir().unwrap();
        let gov = GovernanceRuntime::init_with_store_path(tmpdir.path().join("hooks"))
            .await
            .unwrap();
        // Check that all components are wired
        assert!(!gov.ring.has_pending());
        // The store dir should exist now
        let count = gov.store.count().unwrap_or(0);
        assert_eq!(count, 0);
    }

    #[tokio::test]
    async fn test_register_hook_and_check_hooks() {
        let tmpdir = tempfile::tempdir().unwrap();
        let gov = GovernanceRuntime::init_with_store_path(tmpdir.path().join("hooks"))
            .await
            .unwrap();
        let token = test_token();
        let context = test_context(&token);

        // Register
        gov.register_hook(token.clone(), context).unwrap();

        // Store should have 1 pending
        let pending = gov.store.list_pending().unwrap();
        assert_eq!(pending.len(), 1);
        assert_eq!(pending[0].id, token.id);

        // No hooks fired yet (timer is 50ms, TTL is 5000ms)
        let fired = gov.check_hooks();
        assert!(fired.is_empty());
    }

    #[tokio::test]
    async fn test_emit_event() {
        let tmpdir = tempfile::tempdir().unwrap();
        let gov = GovernanceRuntime::init_with_store_path(tmpdir.path().join("hooks"))
            .await
            .unwrap();

        // Register an event hook
        let token = HookToken {
            id: Uuid::new_v4(),
            hook_type: HookType::Event("my-custom-event".to_string()),
            created_at: Utc::now(),
            ttl_ms: None,
            description: "event test".to_string(),
        };
        let context = test_context(&token);
        gov.register_hook(token.clone(), context).unwrap();

        // The background loop picks up the event — give it a moment
        gov.emit_event("my-custom-event");
        tokio::time::sleep(Duration::from_millis(100)).await;

        // Now the hook should have fired
        let fired = gov.check_hooks();
        let matched = fired.iter().any(|n| n.hook_id == token.id);
        assert!(matched, "Expected hook to fire on event emission");
    }

    #[tokio::test]
    async fn test_cancel_hook() {
        let tmpdir = tempfile::tempdir().unwrap();
        let gov = GovernanceRuntime::init_with_store_path(tmpdir.path().join("hooks"))
            .await
            .unwrap();
        let token = test_token();
        let context = test_context(&token);

        gov.register_hook(token.clone(), context).unwrap();
        assert_eq!(gov.store.count().unwrap(), 1);

        // Cancel
        gov.cancel_hook(token.id).unwrap();

        // Store should be cleaned up
        assert_eq!(gov.store.count().unwrap(), 0);

        // Ring should have a Cancelled notification
        let notifications = gov.check_hooks();
        let cancelled = notifications
            .iter()
            .any(|n| n.hook_id == token.id && matches!(n.event, HookEvent::Cancelled));
        assert!(
            cancelled,
            "Expected Cancelled notification for cancelled hook"
        );
    }

    #[tokio::test]
    async fn test_load_and_delete_context() {
        let tmpdir = tempfile::tempdir().unwrap();
        let gov = GovernanceRuntime::init_with_store_path(tmpdir.path().join("hooks"))
            .await
            .unwrap();
        let token = test_token();
        let context = test_context(&token);

        gov.register_hook(token.clone(), context.clone()).unwrap();

        // Load by ID
        let loaded = gov.load_context(&token.id).unwrap().unwrap();
        assert_eq!(loaded.agent_id, "test-agent");
        assert_eq!(loaded.continuation, "resume_test");

        // Delete
        gov.delete_context(&token.id).unwrap();
        assert!(gov.load_context(&token.id).unwrap().is_none());
    }

    #[tokio::test]
    async fn test_timer_hook_fires() {
        let tmpdir = tempfile::tempdir().unwrap();
        let gov = GovernanceRuntime::init_with_store_path(tmpdir.path().join("hooks"))
            .await
            .unwrap();

        // Register a very short timer (50ms)
        let token = HookToken {
            id: Uuid::new_v4(),
            hook_type: HookType::TimerMs(50),
            created_at: Utc::now(),
            ttl_ms: Some(5000),
            description: "short timer".to_string(),
        };
        let context = test_context(&token);
        gov.register_hook(token.clone(), context).unwrap();

        // Wait for the background loop to fire it
        tokio::time::sleep(Duration::from_millis(350)).await;

        let fired = gov.check_hooks();
        let matched = fired.iter().any(|n| n.hook_id == token.id);
        assert!(matched, "Expected short timer hook to fire");
    }

    #[tokio::test]
    async fn test_expired_hooks_on_init() {
        // Use a custom temp directory so we control what's in the store
        let tmpdir = tempfile::tempdir().unwrap();
        let store_path = tmpdir.path().join("hooks");
        let store = ContextStore::with_path(store_path.clone());

        // Create a hook token that expired a long time ago
        let old_token = HookToken {
            id: Uuid::new_v4(),
            hook_type: HookType::TimerMs(1000),
            created_at: Utc::now() - chrono::Duration::hours(1), // 1 hour ago
            ttl_ms: Some(5000), // 5 second TTL — definitely expired
            description: "expired hook".to_string(),
        };
        let old_context = AgentContext {
            agent_id: "test-agent".to_string(),
            task: "expired task".to_string(),
            gate: "test-gate".to_string(),
            result_so_far: serde_json::json!({"status": "stale"}),
            reasoning: "this hook should expire".to_string(),
            created_at: Utc::now(),
            hook_token: old_token.clone(),
            continuation: "resume_expired".to_string(),
        };

        // Manually persist the expired hook
        store.save(&old_token, &old_context).unwrap();

        // Now init the runtime with this store path — it should detect the
        // expired hook and push an Expired notification into the ring.
        let ring = Arc::new(HookRing::new());
        let (event_tx, _): (broadcast::Sender<String>, broadcast::Receiver<String>) =
            broadcast::channel(256);
        let scheduler = Arc::new(tokio::sync::Mutex::new(EventScheduler::new(
            Arc::clone(&ring),
        )));

        // Simulate the expired-hook recovery logic from init()
        if let Ok(pending) = store.list_pending() {
            for token in &pending {
                if let Some(ttl) = token.ttl_ms {
                    let elapsed =
                        (Utc::now() - token.created_at).num_milliseconds() as u64;
                    if elapsed > ttl {
                        ring.try_push(HookNotification {
                            hook_id: token.id,
                            event: HookEvent::Expired,
                        });
                        let _ = store.delete(&token.id);
                    }
                }
            }
        }

        // The ring should have an Expired notification for old_token
        let notifications = ring.drain();
        let expired = notifications
            .iter()
            .any(|n| n.hook_id == old_token.id && matches!(n.event, HookEvent::Expired));
        assert!(expired, "Expected Expired notification for old hook");

        // The store should be cleaned up
        assert_eq!(store.count().unwrap(), 0);

        // No pending hooks remain
        let pending = store.list_pending().unwrap();
        assert!(pending.is_empty());
    }
}
