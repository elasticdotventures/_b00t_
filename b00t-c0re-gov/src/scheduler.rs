use std::collections::HashMap;
use std::sync::Arc;

use tokio::sync::broadcast;
use uuid::Uuid;

use crate::ring::HookRing;
use crate::types::*;

/// A registered hook inside the scheduler.
pub struct RegisteredHook {
    pub token: HookToken,
    /// Child hook IDs (for AnyOf/AllOf composite hooks)
    pub children: Vec<Uuid>,
    /// Parent hook ID (for AnyOf/AllOf — the composite parent)
    pub parent: Option<Uuid>,
    /// Whether this hook has already fired
    pub fired: bool,
    /// When this hook was created
    pub created_at: chrono::DateTime<chrono::Utc>,
}

/// Payload for an event emission.
#[derive(Debug, Clone)]
pub struct EventPayload {
    pub event_id: String,
    pub data: serde_json::Value,
}

/// The event scheduler manages hooks, timers, and event dispatch.
///
/// Runs a tokio-based event loop that:
/// - Checks expired timers each tick
/// - Listens for event emissions via a broadcast channel
/// - Pushes fired notifications to the HookRing
/// - Sleeps until next expiry (or 1s, whichever is sooner)
pub struct EventScheduler {
    hooks: HashMap<Uuid, RegisteredHook>,
    ring: Arc<HookRing>,
    event_tx: broadcast::Sender<EventPayload>,
}

impl EventScheduler {
    /// Create a new EventScheduler. The broadcast channel capacity defaults to 256.
    pub fn new(ring: Arc<HookRing>) -> Self {
        let (event_tx, _) = broadcast::channel(256);
        EventScheduler {
            hooks: HashMap::new(),
            ring,
            event_tx,
        }
    }

    /// Create a new EventScheduler with a custom broadcast channel capacity.
    pub fn with_capacity(ring: Arc<HookRing>, capacity: usize) -> Self {
        let (event_tx, _) = broadcast::channel(capacity);
        EventScheduler {
            hooks: HashMap::new(),
            ring,
            event_tx,
        }
    }

    /// Register a hook. If the hook has AnyOf/AllOf children, they are also
    /// registered and linked back to the parent.
    pub fn register(&mut self, token: HookToken) -> anyhow::Result<()> {
        let id = token.id;

        // Register the hook itself
        let hook = RegisteredHook {
            children: Vec::new(),
            parent: None,
            fired: false,
            created_at: token.created_at,
            token: token.clone(),
        };
        self.hooks.insert(id, hook);

        // If the hook is a composite (AnyOf/AllOf), register its children
        match &token.hook_type {
            HookType::AnyOf(children) | HookType::AllOf(children) => {
                let child_ids: Vec<Uuid> = children.iter().map(|c| c.id).collect();
                // Update the parent's children list
                if let Some(parent) = self.hooks.get_mut(&id) {
                    parent.children = child_ids.clone();
                }
                // Register each child, linking back to the parent
                for child in children {
                    let child_hook = RegisteredHook {
                        children: Vec::new(),
                        parent: Some(id),
                        fired: false,
                        created_at: child.created_at,
                        token: child.clone(),
                    };
                    self.hooks.insert(child.id, child_hook);
                }
            }
            _ => {}
        }

        Ok(())
    }

    /// Cancel a hook by its ID. Also cancels any children (if composite) or
    /// notifies the parent (if child of composite).
    pub fn cancel(&mut self, hook_id: Uuid) -> anyhow::Result<()> {
        if let Some(hook) = self.hooks.remove(&hook_id) {
            // If it has children, remove them too
            for child_id in &hook.children {
                self.hooks.remove(child_id);
            }
            // If it has a parent, notify the parent by sending a cancelled notification
            if let Some(parent_id) = hook.parent {
                self.ring.try_push(HookNotification {
                    hook_id: parent_id,
                    event: HookEvent::Cancelled,
                });
            }
        }
        Ok(())
    }

    /// Emit an event — all hooks subscribed to this event_id will be matched.
    pub fn emit(&self, event_id: &str, data: serde_json::Value) {
        let _ = self.event_tx.send(EventPayload {
            event_id: event_id.to_string(),
            data,
        });
    }

    /// Get a receiver for subscribing to events programmatically.
    pub fn subscribe(&self) -> broadcast::Receiver<EventPayload> {
        self.event_tx.subscribe()
    }

    /// Run the scheduler loop — call this in a spawned tokio task.
    pub async fn run(&mut self) {
        let mut rx = self.event_tx.subscribe();
        loop {
            let next = self.next_expiry_ms();
            let sleep_dur = next
                .map(|ms| std::time::Duration::from_millis(ms))
                .unwrap_or(std::time::Duration::from_secs(1));

            tokio::select! {
                _ = tokio::time::sleep(sleep_dur) => {
                    let timer_count = self.check_timers();
                    let composite_count = self.check_composites();
                    if timer_count > 0 || composite_count > 0 {
                        // Notifications were pushed to the ring
                    }
                }
                Ok(event) = rx.recv() => {
                    let matched = self.match_event(&event.event_id);
                    if matched > 0 {
                        // Notifications were pushed to the ring
                    }
                    let composite_count = self.check_composites();
                    if composite_count > 0 {
                        // Composite notifications were pushed to the ring
                    }
                }
                else => {
                    // Broadcast channel closed — no more senders
                    break;
                }
            }
        }
    }

    /// Check all registered hooks for expired timers.
    /// Fires any timer/datetime hooks whose time has come.
    /// Returns the number of hooks fired.
    pub fn check_timers(&mut self) -> usize {
        let now = chrono::Utc::now();
        let now_ts = now.timestamp();
        let hook_ids: Vec<Uuid> = self.hooks.keys().copied().collect();
        let mut fired = 0usize;

        for id in hook_ids {
            let should_fire = {
                let hook = match self.hooks.get(&id) {
                    Some(h) => h,
                    None => continue,
                };

                // Skip if already fired, has a parent (composite children handled separately),
                // or if it's a composite/event hook
                if hook.fired {
                    continue;
                }
                if hook.parent.is_some() {
                    continue;
                }

                match &hook.token.hook_type {
                    HookType::TimerMs(duration_ms) => {
                        let elapsed = (now - hook.created_at)
                            .num_milliseconds();
                        elapsed >= *duration_ms as i64
                    }
                    HookType::AtTimestamp(ts) => {
                        now_ts >= *ts
                    }
                    HookType::Cron(_) => {
                        // Cron not yet implemented — skip
                        false
                    }
                    _ => false,
                }
            };

            if should_fire {
                if let Some(hook) = self.hooks.get_mut(&id) {
                    hook.fired = true;
                }
                self.ring.try_push(HookNotification {
                    hook_id: id,
                    event: HookEvent::Fired,
                });
                fired += 1;
            }
        }

        fired
    }

    /// Check composite hooks (AnyOf/AllOf) — fire if conditions met.
    /// Returns the number of composite hooks fired.
    pub fn check_composites(&mut self) -> usize {
        let hook_ids: Vec<Uuid> = self.hooks.keys().copied().collect();
        let mut fired = 0usize;

        for id in hook_ids {
            let should_fire = {
                let hook = match self.hooks.get(&id) {
                    Some(h) => h,
                    None => continue,
                };

                // Skip if already fired or not a composite
                if hook.fired {
                    continue;
                }
                if hook.parent.is_some() {
                    continue; // Children aren't composites themselves
                }

                match &hook.token.hook_type {
                    HookType::AnyOf(children) => {
                        // Fire if ANY child has fired
                        children.iter().any(|child| {
                            self.hooks.get(&child.id).map_or(false, |c| c.fired)
                        })
                    }
                    HookType::AllOf(children) => {
                        // Fire if ALL children have fired
                        children.iter().all(|child| {
                            self.hooks.get(&child.id).map_or(false, |c| c.fired)
                        })
                    }
                    _ => false,
                }
            };

            if should_fire {
                if let Some(hook) = self.hooks.get_mut(&id) {
                    hook.fired = true;
                }
                self.ring.try_push(HookNotification {
                    hook_id: id,
                    event: HookEvent::Fired,
                });
                fired += 1;
            }
        }

        fired
    }

    /// Match an emitted event against registered Event hooks.
    /// Returns the number of hooks matched (and pushed to the ring).
    pub fn match_event(&mut self, event_id: &str) -> usize {
        let hook_ids: Vec<Uuid> = self.hooks.keys().copied().collect();
        let mut matched = 0usize;

        for id in hook_ids {
            let should_fire = {
                let hook = match self.hooks.get(&id) {
                    Some(h) => h,
                    None => continue,
                };

                if hook.fired {
                    continue;
                }

                match &hook.token.hook_type {
                    HookType::Event(eid) => eid == event_id,
                    _ => false,
                }
            };

            if should_fire {
                if let Some(hook) = self.hooks.get_mut(&id) {
                    hook.fired = true;
                }
                self.ring.try_push(HookNotification {
                    hook_id: id,
                    event: HookEvent::Fired,
                });
                matched += 1;
            }
        }

        matched
    }

    /// Calculate the milliseconds until the next timer expiry.
    /// Returns `None` if no timers are pending.
    fn next_expiry_ms(&self) -> Option<u64> {
        let now = chrono::Utc::now();
        let now_ts = now.timestamp();
        let mut nearest: Option<u64> = None;

        for hook in self.hooks.values() {
            if hook.fired || hook.parent.is_some() {
                continue;
            }

            let ms_until = match &hook.token.hook_type {
                HookType::TimerMs(duration_ms) => {
                    let elapsed = (now - hook.created_at).num_milliseconds();
                    if elapsed < 0 {
                        Some(0)
                    } else if elapsed >= *duration_ms as i64 {
                        Some(0)
                    } else {
                        Some((*duration_ms as i64 - elapsed) as u64)
                    }
                }
                HookType::AtTimestamp(ts) => {
                    let remaining = *ts - now_ts;
                    if remaining <= 0 {
                        Some(0)
                    } else {
                        Some(remaining as u64 * 1000)
                    }
                }
                _ => None,
            };

            if let Some(ms) = ms_until {
                match nearest {
                    Some(current) if ms < current => nearest = Some(ms),
                    None => nearest = Some(ms),
                    _ => {}
                }
            }
        }

        nearest
    }

    /// Get a reference to the internal hooks map (for introspection).
    pub fn hooks(&self) -> &HashMap<Uuid, RegisteredHook> {
        &self.hooks
    }

    /// Get a mutable reference to the internal hooks map.
    pub fn hooks_mut(&mut self) -> &mut HashMap<Uuid, RegisteredHook> {
        &mut self.hooks
    }
}

#[cfg(test)]
mod unit_tests {
    use super::*;
    use crate::ring::HookRing;
    use std::sync::Arc;
    use uuid::Uuid;

    #[test]
    fn test_register_timer_hook() {
        let ring = Arc::new(HookRing::new());
        let mut scheduler = EventScheduler::new(ring.clone());

        let token = HookToken {
            id: Uuid::new_v4(),
            hook_type: HookType::TimerMs(100),
            created_at: chrono::Utc::now(),
            ttl_ms: Some(5000),
            description: "test timer".to_string(),
        };

        scheduler.register(token.clone()).unwrap();
        assert!(scheduler.hooks().contains_key(&token.id));
    }

    #[test]
    fn test_register_event_hook() {
        let ring = Arc::new(HookRing::new());
        let mut scheduler = EventScheduler::new(ring.clone());

        let token = HookToken {
            id: Uuid::new_v4(),
            hook_type: HookType::Event("test-event".to_string()),
            created_at: chrono::Utc::now(),
            ttl_ms: None,
            description: "test event hook".to_string(),
        };

        scheduler.register(token.clone()).unwrap();
        assert!(scheduler.hooks().contains_key(&token.id));
    }

    #[test]
    fn test_cancel_hook() {
        let ring = Arc::new(HookRing::new());
        let mut scheduler = EventScheduler::new(ring.clone());

        let token = HookToken {
            id: Uuid::new_v4(),
            hook_type: HookType::TimerMs(100),
            created_at: chrono::Utc::now(),
            ttl_ms: Some(5000),
            description: "cancellable".to_string(),
        };

        scheduler.register(token.clone()).unwrap();
        assert!(scheduler.hooks().contains_key(&token.id));

        scheduler.cancel(token.id).unwrap();
        assert!(!scheduler.hooks().contains_key(&token.id));
    }

    #[test]
    fn test_emit_and_subscribe() {
        let ring = Arc::new(HookRing::new());
        let scheduler = EventScheduler::new(ring);

        let mut rx = scheduler.subscribe();

        scheduler.emit("my-event", serde_json::json!({"key": "value"}));

        // The send is synchronous on the broadcast channel (non-blocking if buffer not full)
        let payload = rx.try_recv().expect("Should have received event");
        assert_eq!(payload.event_id, "my-event");
        assert_eq!(payload.data, serde_json::json!({"key": "value"}));
    }

    #[test]
    fn test_match_event_direct() {
        let ring = Arc::new(HookRing::new());
        let mut scheduler = EventScheduler::new(ring.clone());

        let token = HookToken {
            id: Uuid::new_v4(),
            hook_type: HookType::Event("my-event".to_string()),
            created_at: chrono::Utc::now(),
            ttl_ms: None,
            description: "listens to my-event".to_string(),
        };

        scheduler.register(token.clone()).unwrap();
        let matched = scheduler.match_event("my-event");
        assert_eq!(matched, 1);

        // Hook should be marked as fired
        let hook = scheduler.hooks().get(&token.id).unwrap();
        assert!(hook.fired);
    }

    #[test]
    fn test_match_event_wrong_id() {
        let ring = Arc::new(HookRing::new());
        let mut scheduler = EventScheduler::new(ring.clone());

        let token = HookToken {
            id: Uuid::new_v4(),
            hook_type: HookType::Event("my-event".to_string()),
            created_at: chrono::Utc::now(),
            ttl_ms: None,
            description: "listens to my-event".to_string(),
        };

        scheduler.register(token.clone()).unwrap();
        let matched = scheduler.match_event("other-event");
        assert_eq!(matched, 0);

        // Hook should not be marked as fired
        let hook = scheduler.hooks().get(&token.id).unwrap();
        assert!(!hook.fired);
    }

    #[test]
    fn test_next_expiry_ms_timer() {
        let ring = Arc::new(HookRing::new());
        let mut scheduler = EventScheduler::new(ring.clone());

        // A timer that expires in the far future
        let token = HookToken {
            id: Uuid::new_v4(),
            hook_type: HookType::TimerMs(50000), // 50 seconds
            created_at: chrono::Utc::now(),
            ttl_ms: Some(60000),
            description: "long timer".to_string(),
        };

        scheduler.register(token.clone()).unwrap();
        let next = scheduler.next_expiry_ms();
        assert!(next.is_some());
        // Should be roughly 50000ms (within 500ms tolerance)
        let ms = next.unwrap();
        assert!(ms > 49000 && ms <= 50000, "Expected ~50000ms, got {ms}ms");
    }

    #[test]
    fn test_next_expiry_ms_already_expired() {
        let ring = Arc::new(HookRing::new());
        let mut scheduler = EventScheduler::new(ring.clone());

        // A timer that expired 1 second ago
        let token = HookToken {
            id: Uuid::new_v4(),
            hook_type: HookType::TimerMs(1), // 1ms
            created_at: chrono::Utc::now() - chrono::Duration::seconds(1),
            ttl_ms: Some(5000),
            description: "expired timer".to_string(),
        };

        scheduler.register(token.clone()).unwrap();
        let next = scheduler.next_expiry_ms();
        assert!(next.is_some());
        assert_eq!(next.unwrap(), 0, "Expired timer should have 0ms remaining");
    }

    #[test]
    fn test_check_timers() {
        let ring = Arc::new(HookRing::new());
        let mut scheduler = EventScheduler::new(ring.clone());

        // A timer that should already be expired (1ms with past creation time)
        let token = HookToken {
            id: Uuid::new_v4(),
            hook_type: HookType::TimerMs(1),
            created_at: chrono::Utc::now() - chrono::Duration::seconds(1),
            ttl_ms: Some(5000),
            description: "should fire".to_string(),
        };

        scheduler.register(token.clone()).unwrap();
        let fired = scheduler.check_timers();
        assert_eq!(fired, 1);

        // Should have pushed notification to ring
        let notifs = ring.drain();
        assert_eq!(notifs.len(), 1);
        assert_eq!(notifs[0].hook_id, token.id);
    }

    #[test]
    fn test_check_composites_anyof() {
        let ring = Arc::new(HookRing::new());
        let mut scheduler = EventScheduler::new(ring.clone());

        let child1 = HookToken {
            id: Uuid::new_v4(),
            hook_type: HookType::TimerMs(1),
            created_at: chrono::Utc::now() - chrono::Duration::seconds(1),
            ttl_ms: Some(5000),
            description: "child1".to_string(),
        };

        let child2 = HookToken {
            id: Uuid::new_v4(),
            hook_type: HookType::TimerMs(50000),
            created_at: chrono::Utc::now(),
            ttl_ms: Some(60000),
            description: "child2 (long)".to_string(),
        };

        let parent = HookToken {
            id: Uuid::new_v4(),
            hook_type: HookType::AnyOf(vec![child1.clone(), child2.clone()]),
            created_at: chrono::Utc::now(),
            ttl_ms: Some(60000),
            description: "AnyOf parent".to_string(),
        };

        scheduler.register(parent.clone()).unwrap();
        scheduler.register(child1.clone()).unwrap();
        scheduler.register(child2.clone()).unwrap();

        // Fire child1's timer
        scheduler.check_timers();
        // Now check composites — AnyOf should fire because child1 fired
        let composite_fired = scheduler.check_composites();
        assert_eq!(composite_fired, 1);

        let parent_hook = scheduler.hooks().get(&parent.id).unwrap();
        assert!(parent_hook.fired);
    }

    #[test]
    fn test_register_composite_with_children() {
        let ring = Arc::new(HookRing::new());
        let mut scheduler = EventScheduler::new(ring.clone());

        let child1 = HookToken {
            id: Uuid::new_v4(),
            hook_type: HookType::Event("a".to_string()),
            created_at: chrono::Utc::now(),
            ttl_ms: None,
            description: "child a".to_string(),
        };

        let child2 = HookToken {
            id: Uuid::new_v4(),
            hook_type: HookType::Event("b".to_string()),
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

        // Parent should exist
        assert!(scheduler.hooks().contains_key(&parent.id));
        // Children should also be auto-registered
        assert!(scheduler.hooks().contains_key(&child1.id));
        assert!(scheduler.hooks().contains_key(&child2.id));

        // Parent's children list should match
        let parent_hook = scheduler.hooks().get(&parent.id).unwrap();
        assert_eq!(parent_hook.children.len(), 2);
    }

    #[test]
    fn test_check_composites_allof_not_fired() {
        let ring = Arc::new(HookRing::new());
        let mut scheduler = EventScheduler::new(ring.clone());

        let child1 = HookToken {
            id: Uuid::new_v4(),
            hook_type: HookType::Event("a".to_string()),
            created_at: chrono::Utc::now(),
            ttl_ms: None,
            description: "child a".to_string(),
        };

        let child2 = HookToken {
            id: Uuid::new_v4(),
            hook_type: HookType::Event("b".to_string()),
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

        // Fire child1 only — AllOf should NOT fire yet
        scheduler.match_event("a");
        let composite_fired = scheduler.check_composites();
        assert_eq!(composite_fired, 0);

        let parent_hook = scheduler.hooks().get(&parent.id).unwrap();
        assert!(!parent_hook.fired);
    }

    #[test]
    fn test_check_composites_allof_fired() {
        let ring = Arc::new(HookRing::new());
        let mut scheduler = EventScheduler::new(ring.clone());

        let child1 = HookToken {
            id: Uuid::new_v4(),
            hook_type: HookType::Event("a".to_string()),
            created_at: chrono::Utc::now(),
            ttl_ms: None,
            description: "child a".to_string(),
        };

        let child2 = HookToken {
            id: Uuid::new_v4(),
            hook_type: HookType::Event("b".to_string()),
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

        // Fire both children — AllOf should fire
        scheduler.match_event("a");
        scheduler.match_event("b");
        let composite_fired = scheduler.check_composites();
        assert_eq!(composite_fired, 1);

        let parent_hook = scheduler.hooks().get(&parent.id).unwrap();
        assert!(parent_hook.fired);
    }
}
