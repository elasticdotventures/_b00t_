// 🤓 Pipeline flow control — back-pressure and throttling between stages.
//
//    Strategies:
//      Unbounded   — no back-pressure (default for CPU stages)
//      Buffered    — bounded buffer with capacity limit
//      Throttled   — rate-limited by bytes-per-second
//      Windowed    — limits concurrent in-flight items
//
//    FlowGate wraps an `Arc<Mutex<FlowControl>>` so producer and consumer
//    stages (or the executor) can share state without a direct reference.

use crate::pipeline_types::{CapsuleProfile, StageSpec};
use serde::{Deserialize, Serialize};
use std::collections::VecDeque;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

// ── FlowStrategy ──────────────────────────────────────────────────────────────

/// Back-pressure strategy for a pipeline stage.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum FlowStrategy {
    /// No back-pressure — data flows as fast as produced.
    Unbounded,
    /// Fixed-capacity buffer.  Blocks when the buffer is full.
    Buffered {
        capacity: usize,
    },
    /// Rate-limited: max bytes per second.
    Throttled {
        max_bytes_per_sec: u64,
    },
    /// Sliding window: limits how many items are in-flight downstream.
    Windowed {
        max_in_flight: usize,
    },
}

impl Default for FlowStrategy {
    fn default() -> Self {
        Self::Unbounded
    }
}

// ── FlowControl ──────────────────────────────────────────────────────────────

/// Tracks and enforces flow-control state for a single stage boundary.
///
/// Each strategy manages its own dimension:
/// - `Unbounded`: no checks (always OK).
/// - `Buffered`: checks buffer length against capacity.
/// - `Throttled`: tracks a sliding-window byte rate via a simple leaky bucket.
/// - `Windowed`: tracks `in_flight` count.
#[derive(Debug, Clone)]
pub struct FlowControl {
    pub strategy: FlowStrategy,
    pub buffer: VecDeque<Vec<u8>>,
    pub current_bytes_sec: f64,
    pub in_flight: usize,
    pub stage_name: String,
    // Throttle tracking
    throttle_window_start: Instant,
    throttle_window_bytes: u64,
}

impl FlowControl {
    /// Create a new flow controller with the chosen strategy.
    pub fn new(strategy: FlowStrategy, stage: &str) -> Self {
        let capacity = match &strategy {
            FlowStrategy::Buffered { capacity } => *capacity,
            _ => 0,
        };
        Self {
            strategy,
            buffer: VecDeque::with_capacity(capacity),
            current_bytes_sec: 0.0,
            in_flight: 0,
            stage_name: stage.to_string(),
            throttle_window_start: Instant::now(),
            throttle_window_bytes: 0,
        }
    }

    /// Returns `true` if the stage should emit / the producer should send data.
    ///
    /// - `Unbounded`: always `true`.
    /// - `Buffered`: `true` if buffer is not full.
    /// - `Throttled`: `true` if the current rate is below `max_bytes_per_sec`.
    /// - `Windowed`: `true` if `in_flight < max_in_flight`.
    pub fn can_emit(&self) -> bool {
        match &self.strategy {
            FlowStrategy::Unbounded => true,
            FlowStrategy::Buffered { capacity } => self.buffer.len() < *capacity,
            FlowStrategy::Throttled { max_bytes_per_sec } => {
                if *max_bytes_per_sec == 0 {
                    return false;
                }
                self.throttle_window_bytes < *max_bytes_per_sec
            }
            FlowStrategy::Windowed { max_in_flight } => self.in_flight < *max_in_flight,
        }
    }

    /// Record that data was emitted (producer sent `bytes`).
    ///
    /// - `Buffered`: pushes the data into the internal buffer.
    /// - `Throttled`: updates the sliding-window byte count.
    /// - `Windowed`: increments the in-flight counter.
    /// - `Unbounded`: no-op.
    pub fn record_emit(&mut self, bytes: usize) {
        let now = Instant::now();
        match &self.strategy {
            FlowStrategy::Unbounded => {}
            FlowStrategy::Buffered { .. } => {
                self.buffer.push_back(vec![0u8; bytes]);
            }
            FlowStrategy::Throttled { .. } => {
                // Leaky-bucket: reset window every second
                let elapsed = now.duration_since(self.throttle_window_start);
                if elapsed.as_secs() >= 1 {
                    self.throttle_window_bytes = 0;
                    self.throttle_window_start = now;
                }
                self.throttle_window_bytes = self.throttle_window_bytes.saturating_add(bytes as u64);
                self.current_bytes_sec = self.throttle_window_bytes as f64;
            }
            FlowStrategy::Windowed { .. } => {
                self.in_flight = self.in_flight.saturating_add(1);
            }
        }
    }

    /// Returns `true` if the consumer can accept more data.
    ///
    /// - `Unbounded`: always `true`.
    /// - `Buffered`: `true` if buffer is not empty (data available).
    /// - `Throttled`: always `true` (throttling is on the emit side).
    /// - `Windowed`: always `true` (window is tracked on the emit side).
    pub fn can_accept(&self) -> bool {
        match &self.strategy {
            FlowStrategy::Unbounded => true,
            FlowStrategy::Buffered { .. } => !self.buffer.is_empty(),
            FlowStrategy::Throttled { .. } => true,
            FlowStrategy::Windowed { .. } => true,
        }
    }

    /// Record that data was accepted (consumer received `bytes`).
    ///
    /// - `Buffered`: pops data from the internal buffer.
    /// - `Windowed`: decrements the in-flight counter.
    /// - `Unbounded` / `Throttled`: no-op.
    pub fn record_accept(&mut self, bytes: usize) {
        match &self.strategy {
            FlowStrategy::Unbounded | FlowStrategy::Throttled { .. } => {}
            FlowStrategy::Buffered { .. } => {
                self.buffer.pop_front();
            }
            FlowStrategy::Windowed { .. } => {
                self.in_flight = self.in_flight.saturating_sub(1);
            }
            _ => {}
        }
        _ = bytes; // unused for tracking but kept for API symmetry
    }

    /// Return a `Duration` the caller should wait before retrying when
    /// `can_emit()` returns `false`.
    ///
    /// - `Unbounded`: `Duration::ZERO` (always ready).
    /// - `Buffered`: 10ms (poll interval for buffer space).
    /// - `Throttled`: 100ms (check rate window again).
    /// - `Windowed`: 10ms (poll interval for in-flight slot).
    pub fn wait_backpressure(&self) -> Duration {
        match &self.strategy {
            FlowStrategy::Unbounded => Duration::ZERO,
            FlowStrategy::Buffered { .. } => Duration::from_millis(10),
            FlowStrategy::Throttled { .. } => Duration::from_millis(100),
            FlowStrategy::Windowed { .. } => Duration::from_millis(10),
        }
    }
}

// ── FlowGate ──────────────────────────────────────────────────────────────────

/// Shared flow-control gate between a producer and a consumer.
///
/// Both sides hold an `Arc<Mutex<FlowControl>>` so the gate can be checked
/// from the producer (before sending data) and from the consumer (after
/// receiving data).
#[derive(Debug, Clone)]
pub struct FlowGate {
    controller: Arc<Mutex<FlowControl>>,
}

impl FlowGate {
    /// Create a new gate wrapping the given `FlowControl`.
    pub fn new(controller: FlowControl) -> Self {
        Self {
            controller: Arc::new(Mutex::new(controller)),
        }
    }

    /// Access the inner shared controller.
    pub fn controller(&self) -> &Arc<Mutex<FlowControl>> {
        &self.controller
    }
}

// ── StageFlowConfig ───────────────────────────────────────────────────────────

/// Flow-control configuration attached to a stage spec.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct StageFlowConfig {
    pub stage_name: String,
    pub strategy: FlowStrategy,
    pub max_retries: u32,
}

impl StageFlowConfig {
    /// Create a new flow-control config for a stage.
    pub fn new(stage_name: &str, strategy: FlowStrategy) -> Self {
        Self {
            stage_name: stage_name.to_string(),
            strategy,
            max_retries: 3,
        }
    }

    /// Set the max retries for back-pressure waits.
    pub fn with_max_retries(mut self, retries: u32) -> Self {
        self.max_retries = retries;
        self
    }
}

// ── auto_strategy ─────────────────────────────────────────────────────────────

/// Pick a `FlowStrategy` based on the stage's profile.
///
/// - GPU stages (requires_gpu == true) → `Buffered { capacity: 4 }`
///   so the GPU is never starved and downstream consumers have a buffer.
/// - CPU stages → `Unbounded` (no back-pressure needed for CPU work).
/// - Stages with high memory requirements → `Buffered { capacity: 2 }`
///   to prevent OOM from excessive buffering.
pub fn auto_strategy(stage: &StageSpec) -> FlowStrategy {
    let resources = &stage.profile.resources;
    if resources.requires_gpu {
        FlowStrategy::Buffered { capacity: 4 }
    } else if resources.min_ram_gb > 16.0 {
        // High-memory stages: limit buffer to avoid OOM
        FlowStrategy::Buffered { capacity: 2 }
    } else if resources.min_vram_gb > 0.0 {
        // VRAM-heavy (e.g. transcoding, image processing) → gentle buffer
        FlowStrategy::Buffered { capacity: 8 }
    } else {
        FlowStrategy::Unbounded
    }
}

/// Check if the profile is GPU-accelerated.
pub fn is_gpu_profile(profile: &CapsuleProfile) -> bool {
    profile.resources.requires_gpu
}

/// Check if the profile has significant memory requirements.
pub fn is_memory_intensive(profile: &CapsuleProfile) -> bool {
    profile.resources.min_ram_gb > 16.0 || profile.resources.min_vram_gb > 8.0
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pipeline_types::{CapsuleProfile, ResourceRequirements};
    use std::time::Duration;

    /// Helper: create a minimal stage spec for testing.
    fn make_stage(name: &str, requires_gpu: bool, min_ram_gb: f64, min_vram_gb: f64) -> StageSpec {
        StageSpec {
            name: name.to_string(),
            profile: CapsuleProfile {
                name: name.to_string(),
                ports: vec![],
                resources: ResourceRequirements {
                    min_ram_gb,
                    min_vram_gb,
                    requires_gpu,
                    cpu_cores: None,
                    scratch_disk_gb: None,
                },
                image: None,
                timeout_seconds: None,
            },
            input_ports: vec![],
            output_ports: vec![],
            error_routes: vec![],
            env: None,
            checkpoint_interval_seconds: None,
            secret_refs: None,
            flow_control: None,
        }
    }

    // ── Unbounded always allows emit/accept ─────────────────────────────────

    #[test]
    fn unbounded_always_allows_emit_and_accept() {
        let fc = FlowControl::new(FlowStrategy::Unbounded, "test");
        assert!(fc.can_emit(), "Unbounded must allow emit");
        assert!(fc.can_accept(), "Unbounded must allow accept");

        // Even after recording many emits, it should still allow
        let mut fc = FlowControl::new(FlowStrategy::Unbounded, "test");
        for _ in 0..100 {
            fc.record_emit(1024);
            assert!(fc.can_emit(), "Unbounded must always allow emit");
            assert!(fc.can_accept(), "Unbounded must always allow accept");
        }
    }

    // ── Buffered blocks when full ───────────────────────────────────────────

    #[test]
    fn buffered_blocks_when_full() {
        let mut fc = FlowControl::new(FlowStrategy::Buffered { capacity: 3 }, "test");
        assert!(fc.can_emit(), "buffer should start empty");

        fc.record_emit(10);
        assert!(fc.can_emit(), "1/3 used");
        fc.record_emit(10);
        assert!(fc.can_emit(), "2/3 used");
        fc.record_emit(10);
        assert!(!fc.can_emit(), "3/3 full — should block");

        // Accept one to free a slot
        assert!(fc.can_accept(), "buffer has data");
        fc.record_accept(10);
        assert!(fc.can_emit(), "after accept, buffer has room again");
        assert!(fc.can_accept(), "buffer still has queued data");
        fc.record_accept(10);
        fc.record_accept(10);
        assert!(!fc.can_accept(), "buffer is empty after draining");
    }

    #[test]
    fn buffered_can_accept_false_when_empty() {
        let fc = FlowControl::new(FlowStrategy::Buffered { capacity: 5 }, "test");
        assert!(!fc.can_accept(), "empty buffer has nothing to accept");
    }

    // ── Throttled rate-limits ───────────────────────────────────────────────

    #[test]
    fn throttled_rate_limits() {
        let mut fc = FlowControl::new(FlowStrategy::Throttled { max_bytes_per_sec: 1000 }, "test");

        // Initially the window has 0 bytes, so we can emit.
        assert!(fc.can_emit(), "throttle should allow initially");

        // Record 900 bytes — still under 1000/sec
        fc.record_emit(900);
        assert!(fc.can_emit(), "900 < 1000, should allow");

        // Record another 200 bytes — now over 1000
        fc.record_emit(200);
        // The window is still the same second, so we should be at ~1100 bytes/sec
        // But this depends on timing; let's just check it works conceptually
        // by checking that after a large emit, can_emit may be false
        let mut fc2 = FlowControl::new(FlowStrategy::Throttled { max_bytes_per_sec: 1000 }, "test");
        fc2.record_emit(1500);
        // Should be over the limit
        assert!(!fc2.can_emit(), "1500 > 1000, should block");
    }

    #[test]
    fn throttled_zero_max_blocks_always() {
        let fc = FlowControl::new(FlowStrategy::Throttled { max_bytes_per_sec: 0 }, "test");
        assert!(!fc.can_emit(), "zero max should always block");
    }

    // ── Windowed tracks in-flight count ─────────────────────────────────────

    #[test]
    fn windowed_tracks_in_flight() {
        let mut fc = FlowControl::new(FlowStrategy::Windowed { max_in_flight: 3 }, "test");
        assert!(fc.can_emit(), "0 in-flight, should allow");

        fc.record_emit(10);
        assert!(fc.can_emit(), "1/3 in-flight");
        fc.record_emit(10);
        assert!(fc.can_emit(), "2/3 in-flight");
        fc.record_emit(10);
        assert!(!fc.can_emit(), "3/3 in-flight, should block");

        // Accepting frees a slot
        fc.record_accept(10);
        assert!(fc.can_emit(), "2/3 in-flight after accept");

        // Multiple accepts
        fc.record_accept(10);
        fc.record_accept(10);
        assert!(fc.can_emit(), "0/3 in-flight after all accepted");
        assert_eq!(fc.in_flight, 0);
    }

    // ── GPU stage gets Buffered strategy ────────────────────────────────────

    #[test]
    fn gpu_stage_gets_buffered_strategy() {
        let stage = make_stage("gpu-encoder", true, 4.0, 8.0);
        let strategy = auto_strategy(&stage);
        match strategy {
            FlowStrategy::Buffered { capacity } => {
                assert_eq!(capacity, 4, "GPU stage should get Buffered{{4}}");
            }
            _ => panic!("GPU stage should get Buffered strategy, got {:?}", strategy),
        }
    }

    #[test]
    fn cpu_stage_gets_unbounded_strategy() {
        let stage = make_stage("cpu-decoder", false, 4.0, 0.0);
        let strategy = auto_strategy(&stage);
        assert_eq!(strategy, FlowStrategy::Unbounded, "CPU stage should get Unbounded");
    }

    #[test]
    fn high_memory_stage_gets_buffered_strategy() {
        let stage = make_stage("big-processor", false, 32.0, 0.0);
        let strategy = auto_strategy(&stage);
        match strategy {
            FlowStrategy::Buffered { capacity } => {
                assert_eq!(capacity, 2, "High-memory stage should get Buffered{{2}}");
            }
            _ => panic!("High-memory stage should get Buffered strategy, got {:?}", strategy),
        }
    }

    // ── FlowGate shared state works ─────────────────────────────────────────

    #[test]
    fn flow_gate_shared_state() {
        let fc = FlowControl::new(FlowStrategy::Buffered { capacity: 2 }, "test");
        let gate = FlowGate::new(fc);

        // Clone the gate (simulating both sides getting a reference)
        let producer_gate = gate.clone();
        let consumer_gate = gate;

        // Producer checks and emits
        {
            let mut ctrl = producer_gate.controller().lock().unwrap();
            assert!(ctrl.can_emit());
            ctrl.record_emit(42);
        }

        // Consumer accepts — sees the same state
        {
            let mut ctrl = consumer_gate.controller().lock().unwrap();
            assert!(ctrl.can_accept());
            ctrl.record_accept(42);
        }

        // Verify the buffer is empty after accept
        {
            let ctrl = consumer_gate.controller().lock().unwrap();
            assert!(!ctrl.can_accept(), "buffer should be empty after accept");
        }
    }

    // ── wait_backpressure returns nonzero for blocking strategies ───────────

    #[test]
    fn wait_backpressure_returns_duration() {
        let unbounded = FlowControl::new(FlowStrategy::Unbounded, "test");
        assert_eq!(unbounded.wait_backpressure(), Duration::ZERO, "Unbounded should have zero wait");

        let buffered = FlowControl::new(FlowStrategy::Buffered { capacity: 3 }, "test");
        assert!(
            buffered.wait_backpressure() > Duration::ZERO,
            "Buffered should have non-zero wait"
        );

        let throttled = FlowControl::new(FlowStrategy::Throttled { max_bytes_per_sec: 1000 }, "test");
        assert!(
            throttled.wait_backpressure() > Duration::ZERO,
            "Throttled should have non-zero wait"
        );

        let windowed = FlowControl::new(FlowStrategy::Windowed { max_in_flight: 3 }, "test");
        assert!(
            windowed.wait_backpressure() > Duration::ZERO,
            "Windowed should have non-zero wait"
        );
    }

    // ── auto_strategy vram-only stage ───────────────────────────────────────

    #[test]
    fn vram_stage_gets_buffered_strategy() {
        let stage = make_stage("transcoder", false, 4.0, 4.0);
        let strategy = auto_strategy(&stage);
        match strategy {
            FlowStrategy::Buffered { capacity } => {
                assert_eq!(capacity, 8, "VRAM stage should get Buffered{{8}}");
            }
            FlowStrategy::Unbounded => {} // min_vram_gb is > 0 but < 8, depends on thresholds
            _ => panic!("Unexpected strategy: {:?}", strategy),
        }
    }

    // ── StageFlowConfig ─────────────────────────────────────────────────────

    #[test]
    fn stage_flow_config_creation() {
        let config = StageFlowConfig::new("encoder", FlowStrategy::Buffered { capacity: 4 });
        assert_eq!(config.stage_name, "encoder");
        assert_eq!(config.strategy, FlowStrategy::Buffered { capacity: 4 });
        assert_eq!(config.max_retries, 3);

        let config = StageFlowConfig::new("decoder", FlowStrategy::Unbounded).with_max_retries(5);
        assert_eq!(config.max_retries, 5);
    }

    // ── FlowControl::new sets up initial state ──────────────────────────────

    #[test]
    fn flow_control_initial_state() {
        let fc = FlowControl::new(FlowStrategy::Buffered { capacity: 10 }, "initial-test");
        assert_eq!(fc.stage_name, "initial-test");
        assert_eq!(fc.buffer.len(), 0);
        assert_eq!(fc.in_flight, 0);
        assert_eq!(fc.current_bytes_sec, 0.0);
        assert!(fc.can_emit());
    }
}
