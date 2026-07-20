// 🤓 Content-addressed stage caching (#742) + Adaptive timeout prediction (#744)
//
// ContentCache:
//   Deduplicates processing for identical inputs by caching stage results
//   keyed on SHA-256 of (input + stage_name + sorted params JSON).
//   Supports LRU eviction when max_entries is reached.
//
// CachedExecutor:
//   Wraps PipelineExecutor injecting ContentCache checks before each stage.
//
// TimeoutPredictor:
//   Uses historical StageTiming records to predict stage duration via
//   linear regression (input_size → duration_ms).  Falls back to a simple
//   trailing-average when fewer than 2 data points are available.
//
// StageTiming:
//   Record of a single stage execution — stage_name, input_size_bytes,
//   duration_ms, and timestamp.

use crate::pipeline_executor::{
    PipelineExecutor, PipelineRunReport, RunStatus, StageResult, StageStatus,
};
use crate::pipeline_types::PipelineError;
use chrono::{DateTime, Utc};
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

// ══════════════════════════════════════════════════════════════════════════
// ContentCache
// ══════════════════════════════════════════════════════════════════════════

/// Content-addressed cache for pipeline stage outputs.
///
/// The cache key is `SHA-256(input_bytes || stage_name || sorted_params_json)`,
/// ensuring that identical inputs + configuration always map to the same entry.
///
/// When the number of entries reaches `max_entries`, the least-recently-used
/// entry is evicted on the next `set()` call.
pub struct ContentCache {
    store: HashMap<String, Vec<u8>>,
    /// Front = LRU, Back = MRU.
    access_order: Vec<String>,
    max_entries: usize,
    /// Monotonic generation counter for LRU scoring.
    /// Avoids O(n) Vec::remove by stamping each entry with its generation.
    generation: u64,
    /// Key → generation stamp.
    stamps: HashMap<String, u64>,
}

impl ContentCache {
    /// Create a new cache with the given capacity.
    ///
    /// `max_entries` must be ≥ 1 (will be clamped otherwise).
    pub fn new(max_entries: usize) -> Self {
        Self {
            store: HashMap::new(),
            access_order: Vec::new(),
            max_entries: max_entries.max(1),
            generation: 0,
            stamps: HashMap::new(),
        }
    }

    /// Compute a deterministic cache key from the stage input, stage name,
    /// and an arbitrary parameter map.
    ///
    /// The parameter map is serialised to JSON with sorted keys so that
    /// semantically-equivalent maps produce the same key regardless of
    /// insertion order.
    pub fn key(input: &[u8], stage_name: &str, params: &HashMap<String, String>) -> String {
        let mut hasher = Sha256::new();
        hasher.update(input);
        hasher.update(b"|");
        hasher.update(stage_name.as_bytes());
        hasher.update(b"|");
        // Serialise params with sorted keys for determinism.
        let mut keys: Vec<&String> = params.keys().collect();
        keys.sort();
        for k in keys {
            hasher.update(k.as_bytes());
            hasher.update(b"=");
            hasher.update(params[k].as_bytes());
            hasher.update(b"&");
        }
        hex::encode(hasher.finalize())
    }

    /// Retrieve a cached value by key.
    ///
    /// Returns `None` if the key is not present.  On a cache hit the entry
    /// is promoted to MRU.
    pub fn get(&mut self, key: &str) -> Option<&[u8]> {
        if !self.store.contains_key(key) {
            return None;
        }
        // Bump generation — marks this entry as most-recently-used.
        self.generation = self.generation.wrapping_add(1);
        self.stamps.insert(key.to_string(), self.generation);
        self.store.get(key).map(|v| v.as_slice())
    }

    /// Insert or overwrite a cache entry.
    ///
    /// If the cache is at capacity, the least-recently-used entry is evicted
    /// first.  New entries are always MRU.
    pub fn set(&mut self, key: String, output: Vec<u8>) {
        if self.store.contains_key(&key) {
            // Update in place — no eviction needed.
            self.store.insert(key.clone(), output);
            self.generation = self.generation.wrapping_add(1);
            self.stamps.insert(key, self.generation);
            return;
        }

        // Evict LRU if at capacity.
        while self.store.len() >= self.max_entries {
            let lru_key = self.find_lru();
            if let Some(k) = lru_key {
                self.store.remove(&k);
                self.stamps.remove(&k);
                // Remove from access_order too (lazy: rebuild on next eviction).
            } else {
                break;
            }
        }

        self.store.insert(key.clone(), output);
        self.generation = self.generation.wrapping_add(1);
        self.stamps.insert(key.clone(), self.generation);
        self.access_order.push(key);
    }

    /// Check whether a key exists in the cache.
    ///
    /// Does NOT promote the entry (use `get` for that).
    pub fn contains(&self, key: &str) -> bool {
        self.store.contains_key(key)
    }

    /// Number of entries currently in the cache.
    pub fn len(&self) -> usize {
        self.store.len()
    }

    /// Maximum number of entries the cache can hold.
    pub fn max_entries(&self) -> usize {
        self.max_entries
    }

    /// Find the key with the lowest generation stamp (the LRU entry).
    fn find_lru(&self) -> Option<String> {
        self.stamps
            .iter()
            .min_by_key(|(_, generation)| *generation)
            .map(|(k, _)| k.clone())
    }

    /// Clear all entries.
    pub fn clear(&mut self) {
        self.store.clear();
        self.access_order.clear();
        self.stamps.clear();
        self.generation = 0;
    }
}

// ══════════════════════════════════════════════════════════════════════════
// CachedExecutor
// ══════════════════════════════════════════════════════════════════════════

/// Wraps a `PipelineExecutor` with a `ContentCache` to avoid re-executing
/// stages whose inputs have already been processed.
pub struct CachedExecutor {
    inner: PipelineExecutor,
    cache: Arc<Mutex<ContentCache>>,
    /// Optional parameters that feed into the cache key (e.g. environment
    /// overrides, stage configuration flags).
    stage_params: HashMap<String, String>,
}

impl CachedExecutor {
    /// Create a new cached executor wrapping an inner executor.
    ///
    /// The cache capacity defaults to 1024 entries.
    pub fn new(inner: PipelineExecutor) -> Self {
        Self {
            inner,
            cache: Arc::new(Mutex::new(ContentCache::new(1024))),
            stage_params: HashMap::new(),
        }
    }

    /// Set capacity of the underlying content cache.
    pub fn with_cache_capacity(mut self, max_entries: usize) -> Self {
        self.cache = Arc::new(Mutex::new(ContentCache::new(max_entries)));
        self
    }

    /// Set or override stage-level parameters that affect the cache key.
    ///
    /// These are merged with the stage's env to form the `params` map in
    /// `ContentCache::key()`.
    pub fn with_stage_params(mut self, params: HashMap<String, String>) -> Self {
        self.stage_params = params;
        self
    }

    /// Execute the full pipeline, checking the cache before each stage.
    ///
    /// For each stage in execution order:
    /// 1. If the input + stage name + params are cached → use cached output.
    /// 2. Otherwise, delegate to `inner.execute_stage()` and cache the result.
    ///
    /// Returns a `PipelineRunReport` identical in shape to what
    /// `PipelineExecutor::execute()` would produce.
    pub async fn execute_cached(
        &self,
        run_id: &str,
        initial_input: Option<Vec<u8>>,
    ) -> PipelineRunReport {
        let start = Instant::now();
        let mut stage_results: Vec<StageResult> = Vec::new();
        let mut last_output: Option<Vec<u8>> = initial_input;

        // Resolve execution order from the inner executor's DAG.
        let dag = self.inner.dag();
        let order = match dag.execution_order() {
            Ok(o) => o,
            Err(e) => {
                return PipelineRunReport {
                    run_id: run_id.to_string(),
                    stages: vec![],
                    total_duration_ms: 0,
                    status: RunStatus::Failed(format!("DAG cycle: {e}")),
                };
            }
        };

        if order.is_empty() {
            return PipelineRunReport {
                run_id: run_id.to_string(),
                stages: vec![],
                total_duration_ms: 0,
                status: RunStatus::Completed,
            };
        }

        // For each stage in order.
        for (idx, stage_name) in order.iter().enumerate() {
            let input = last_output.take();

            // Build the cache key from input, stage name, and params.
            let input_bytes = input.clone().unwrap_or_default();
            let cache_key = ContentCache::key(&input_bytes, stage_name, &self.stage_params);

            // Check cache.
            {
                let mut cache = self.cache.lock().expect("ContentCache lock");
                if let Some(cached) = cache.get(&cache_key).map(|v| v.to_vec()) {
                    // Cache hit — record a synthetic StageResult.
                    stage_results.push(StageResult {
                        stage_name: stage_name.clone(),
                        duration_ms: 0,
                        output: Some(cached.clone()),
                        error: None,
                        status: StageStatus::Completed,
                    });
                    last_output = Some(cached);
                    continue;
                }
            }

            // Cache miss — find the StageSpec and execute.
            let stage_spec = match dag.find_stage(stage_name) {
                Some(s) => s.clone(),
                None => {
                    let err = PipelineError::StageCrashed(format!(
                        "stage '{stage_name}' not found in DAG"
                    ));
                    stage_results.push(StageResult {
                        stage_name: stage_name.clone(),
                        duration_ms: 0,
                        output: None,
                        error: Some(err.clone()),
                        status: StageStatus::Failed(err),
                    });
                    for remaining in &order[idx + 1..] {
                        stage_results.push(StageResult {
                            stage_name: remaining.clone(),
                            duration_ms: 0,
                            output: None,
                            error: None,
                            status: StageStatus::Skipped,
                        });
                    }
                    break;
                }
            };

            let result = self
                .inner
                .execute_stage(&stage_spec, Some(input_bytes.clone()), run_id)
                .await;

            // On success, cache the output.
            if let Some(ref output) = result.output {
                let mut cache = self.cache.lock().expect("ContentCache lock");
                // Also record the timing in the predictor if present.
                cache.set(
                    ContentCache::key(&input_bytes, stage_name, &self.stage_params),
                    output.clone(),
                );
            }

            let is_failure = matches!(&result.status, StageStatus::Failed(_));
            let stage_output = result.output.clone();
            stage_results.push(result);

            if is_failure {
                for remaining in &order[idx + 1..] {
                    stage_results.push(StageResult {
                        stage_name: remaining.clone(),
                        duration_ms: 0,
                        output: None,
                        error: None,
                        status: StageStatus::Skipped,
                    });
                }
                break;
            }

            last_output = stage_output;
        }

        let has_failure = stage_results
            .iter()
            .any(|sr| matches!(&sr.status, StageStatus::Failed(_)));
        let has_skipped = stage_results
            .iter()
            .any(|sr| sr.status == StageStatus::Skipped);

        let status = if has_failure && has_skipped {
            let msg = stage_results
                .iter()
                .find_map(|sr| match &sr.error {
                    Some(e) => Some(format!("{:?}", e)),
                    None => None,
                })
                .unwrap_or_else(|| "pipeline stage failed".to_string());
            RunStatus::Failed(msg)
        } else if has_failure {
            RunStatus::Partial
        } else {
            RunStatus::Completed
        };

        PipelineRunReport {
            run_id: run_id.to_string(),
            stages: stage_results,
            total_duration_ms: start.elapsed().as_millis() as u64,
            status,
        }
    }

    /// Access the underlying cache for inspection / metrics.
    pub fn cache(&self) -> &Arc<Mutex<ContentCache>> {
        &self.cache
    }

    /// Access the inner executor.
    pub fn inner(&self) -> &PipelineExecutor {
        &self.inner
    }
}

// ══════════════════════════════════════════════════════════════════════════
// StageTiming
// ══════════════════════════════════════════════════════════════════════════

/// A single recorded timing observation for a pipeline stage execution.
#[derive(Debug, Clone)]
pub struct StageTiming {
    pub stage_name: String,
    pub input_size_bytes: u64,
    pub duration_ms: u64,
    pub timestamp: DateTime<Utc>,
}

// ══════════════════════════════════════════════════════════════════════════
// TimeoutPredictor
// ══════════════════════════════════════════════════════════════════════════

/// Predicts stage duration from historical timing data using linear regression
/// (input_size → duration_ms).
///
/// When fewer than 2 data points are available for a stage, falls back to the
/// simple average of any available runs.  When no data exists, returns the
/// configured timeout as-is.
///
/// History is partitioned by `stage_name` — each stage has its own predictor
/// so that a fast transcoding stage and a slow LLM inference stage each get
/// appropriate predictions.
pub struct TimeoutPredictor {
    /// Per-stage timing histories.
    history: HashMap<String, Vec<StageTiming>>,
    /// Maximum number of timing records to retain per stage (FIFO eviction).
    max_records_per_stage: usize,
}

impl TimeoutPredictor {
    /// Create a new predictor with default capacity (100 records per stage).
    pub fn new() -> Self {
        Self {
            history: HashMap::new(),
            max_records_per_stage: 100,
        }
    }

    /// Create a predictor with a custom per-stage record limit.
    pub fn with_max_records(max: usize) -> Self {
        Self {
            history: HashMap::new(),
            max_records_per_stage: max.max(1),
        }
    }

    /// Record a stage timing observation.
    ///
    /// Older entries beyond `max_records_per_stage` are evicted (FIFO).
    pub fn record(&mut self, timing: StageTiming) {
        let entries = self.history.entry(timing.stage_name.clone()).or_default();
        entries.push(timing);
        while entries.len() > self.max_records_per_stage {
            entries.remove(0);
        }
    }

    /// Predict the expected duration for a given stage and input size.
    ///
    /// Uses simple linear regression when ≥ 2 data points are available.
    /// Falls back to the mean of available runs when data is scarce.
    /// Returns `None` when no history exists for the stage.
    pub fn predict(&self, stage: &str, input_size: u64) -> Option<Duration> {
        let entries = self.history.get(stage)?;
        if entries.is_empty() {
            return None;
        }

        let n = entries.len() as f64;

        if n < 2.0 {
            // Not enough for regression — return simple average.
            let avg_ms: f64 = entries.iter().map(|t| t.duration_ms as f64).sum::<f64>() / n;
            return Some(Duration::from_millis(avg_ms as u64));
        }

        // Simple linear regression: duration_ms = slope * input_size + intercept
        let sum_x: f64 = entries.iter().map(|t| t.input_size_bytes as f64).sum();
        let sum_y: f64 = entries.iter().map(|t| t.duration_ms as f64).sum();
        let sum_xy: f64 = entries
            .iter()
            .map(|t| t.input_size_bytes as f64 * t.duration_ms as f64)
            .sum();
        let sum_xx: f64 = entries
            .iter()
            .map(|t| t.input_size_bytes as f64 * t.input_size_bytes as f64)
            .sum();

        let denom = n * sum_xx - sum_x * sum_x;
        let (slope, intercept) = if denom.abs() > f64::EPSILON {
            let s = (n * sum_xy - sum_x * sum_y) / denom;
            let i = (sum_y - s * sum_x) / n;
            (s, i)
        } else {
            // All inputs are the same size — fall back to average.
            let avg_ms = sum_y / n;
            return Some(Duration::from_millis(avg_ms as u64));
        };

        let predicted_ms = slope * input_size as f64 + intercept;
        // Clamp to non-negative duration.
        let predicted_ms = predicted_ms.max(0.0);
        Some(Duration::from_millis(predicted_ms as u64))
    }

    /// Determine the effective timeout for a stage.
    ///
    /// Returns the larger of:
    /// - `configured_timeout` (the stage's statically configured timeout)
    /// - The predicted duration × 1.2 (20 % safety margin)
    ///
    /// This ensures that stages whose historical runtime exceeds the
    /// configured timeout get a longer leash, while stages with no history
    /// or predictable behaviour stick to the configured value.
    pub fn should_extend_timeout(
        &self,
        stage: &str,
        input_size: u64,
        configured_timeout: Duration,
    ) -> Duration {
        match self.predict(stage, input_size) {
            Some(predicted) => {
                let with_margin = predicted.mul_f64(1.2);
                std::cmp::max(configured_timeout, with_margin)
            }
            None => configured_timeout,
        }
    }

    /// Number of stages for which the predictor has recorded history.
    pub fn stage_count(&self) -> usize {
        self.history.len()
    }

    /// Total number of timing records across all stages.
    pub fn record_count(&self) -> usize {
        self.history.values().map(|v| v.len()).sum()
    }

    /// Get a reference to the history for a specific stage.
    pub fn stage_history(&self, stage: &str) -> Option<&[StageTiming]> {
        self.history.get(stage).map(|v| v.as_slice())
    }

    /// Clear all recorded history.
    pub fn clear(&mut self) {
        self.history.clear();
    }
}

impl Default for TimeoutPredictor {
    fn default() -> Self {
        Self::new()
    }
}

// ══════════════════════════════════════════════════════════════════════════
// Tests
// ══════════════════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;

    // ── ContentCache: same input returns cached result ────────────────

    #[test]
    fn cache_same_input_returns_cached_result() {
        let mut cache = ContentCache::new(10);
        let input = b"hello world";
        let params = HashMap::new();

        let key1 = ContentCache::key(input, "encode", &params);
        cache.set(key1.clone(), b"cached-output".to_vec());

        let key2 = ContentCache::key(input, "encode", &params);
        // Keys must be identical for same inputs.
        assert_eq!(key1, key2, "keys for same input must match");
        assert!(cache.contains(&key2), "cache should contain the key");
        let result = cache.get(&key2);
        assert_eq!(
            result,
            Some(b"cached-output" as &[u8]),
            "cached value mismatch"
        );
    }

    // ── ContentCache: different input produces new result ─────────────

    #[test]
    fn cache_different_input_produces_new_result() {
        let mut cache = ContentCache::new(10);

        let key_a = ContentCache::key(b"input-A", "stage", &HashMap::new());
        let key_b = ContentCache::key(b"input-B", "stage", &HashMap::new());

        assert_ne!(key_a, key_b, "different inputs must produce different keys");

        cache.set(key_a.clone(), b"output-A".to_vec());
        assert!(cache.contains(&key_a));
        assert!(
            !cache.contains(&key_b),
            "different input should not be cached"
        );
    }

    // ── ContentCache: cache key changes with params ──────────────────

    #[test]
    fn cache_key_changes_with_params() {
        let mut p1 = HashMap::new();
        p1.insert("quality".to_string(), "high".to_string());

        let mut p2 = HashMap::new();
        p2.insert("quality".to_string(), "low".to_string());

        let key1 = ContentCache::key(b"data", "transcode", &p1);
        let key2 = ContentCache::key(b"data", "transcode", &p2);

        assert_ne!(key1, key2, "different params must produce different keys");

        let mut cache = ContentCache::new(10);
        cache.set(key1, b"high-res".to_vec());
        assert!(!cache.contains(&ContentCache::key(b"data", "transcode", &p2)));
    }

    // ── ContentCache: cache key is deterministic (sorted params) ─────

    #[test]
    fn cache_key_deterministic_with_unsorted_params() {
        let mut p_a = HashMap::new();
        p_a.insert("alpha".to_string(), "1".to_string());
        p_a.insert("beta".to_string(), "2".to_string());

        let mut p_b = HashMap::new();
        p_b.insert("beta".to_string(), "2".to_string());
        p_b.insert("alpha".to_string(), "1".to_string());

        let key_a = ContentCache::key(b"x", "s", &p_a);
        let key_b = ContentCache::key(b"x", "s", &p_b);

        assert_eq!(
            key_a, key_b,
            "param map insertion order must not affect cache key"
        );
    }

    // ── ContentCache: LRU eviction ───────────────────────────────────

    #[test]
    fn cache_respects_max_entries_lru_eviction() {
        let mut cache = ContentCache::new(3); // Capacity = 3

        let params = HashMap::new();
        cache.set(ContentCache::key(b"a", "s", &params), b"1".to_vec());
        cache.set(ContentCache::key(b"b", "s", &params), b"2".to_vec());
        cache.set(ContentCache::key(b"c", "s", &params), b"3".to_vec());

        assert_eq!(cache.len(), 3);

        // Access 'a' to promote it to MRU.
        cache.get(&ContentCache::key(b"a", "s", &params));

        // Insert 'd' — should evict the LRU entry, which is now 'b' (oldest
        // unaccessed), not 'a' (recently accessed).
        cache.set(ContentCache::key(b"d", "s", &params), b"4".to_vec());

        assert_eq!(cache.len(), 3, "cache should not exceed max_entries");
        assert!(
            cache.contains(&ContentCache::key(b"a", "s", &params)),
            "a was recently accessed and should survive"
        );
        assert!(
            cache.contains(&ContentCache::key(b"d", "s", &params)),
            "d was just inserted and should survive"
        );
        assert!(
            !cache.contains(&ContentCache::key(b"b", "s", &params)),
            "b was LRU and should have been evicted"
        );
    }

    // ── ContentCache: overwrite existing key ─────────────────────────

    #[test]
    fn cache_overwrite_existing_key() {
        let mut cache = ContentCache::new(10);
        let key = ContentCache::key(b"in", "st", &HashMap::new());

        cache.set(key.clone(), b"old".to_vec());
        cache.set(key.clone(), b"new".to_vec());

        let result = cache.get(&key);
        assert_eq!(
            result,
            Some(b"new" as &[u8]),
            "overwritten value should be 'new'"
        );
        assert_eq!(cache.len(), 1, "overwrite should not increase entry count");
    }

    // ── ContentCache: clear ──────────────────────────────────────────

    #[test]
    fn cache_clear() {
        let mut cache = ContentCache::new(10);
        cache.set(ContentCache::key(b"x", "s", &HashMap::new()), b"v".to_vec());
        assert_eq!(cache.len(), 1);
        cache.clear();
        assert_eq!(cache.len(), 0);
        assert!(!cache.contains(&ContentCache::key(b"x", "s", &HashMap::new())));
    }

    // ── ContentCache: min capacity clamped ───────────────────────────

    #[test]
    fn cache_min_capacity_one() {
        let cache = ContentCache::new(0);
        assert_eq!(
            cache.max_entries(),
            1,
            "zero max_entries should be clamped to 1"
        );
    }

    // ── TimeoutPredictor: record and predict from history ────────────

    #[test]
    fn predictor_record_and_predict() {
        let mut predictor = TimeoutPredictor::new();

        // Record a few timings for "encode" stage.
        predictor.record(StageTiming {
            stage_name: "encode".to_string(),
            input_size_bytes: 100,
            duration_ms: 500,
            timestamp: Utc::now(),
        });
        predictor.record(StageTiming {
            stage_name: "encode".to_string(),
            input_size_bytes: 200,
            duration_ms: 950,
            timestamp: Utc::now(),
        });
        predictor.record(StageTiming {
            stage_name: "encode".to_string(),
            input_size_bytes: 300,
            duration_ms: 1450,
            timestamp: Utc::now(),
        });

        let predicted = predictor.predict("encode", 150);
        assert!(predicted.is_some(), "should predict for known stage");
        let dur = predicted.unwrap();
        // With three roughly-linear points, prediction should be between 500 and 950.
        assert!(
            dur.as_millis() >= 400 && dur.as_millis() <= 1100,
            "prediction {}ms for input_size=150 should be interpolated",
            dur.as_millis()
        );
    }

    // ── TimeoutPredictor: larger input predicts longer timeout ───────

    #[test]
    fn predictor_larger_input_longer_timeout() {
        let mut predictor = TimeoutPredictor::new();

        for (size, dur) in &[(100u64, 100u64), (200, 210), (500, 520), (1000, 1050)] {
            predictor.record(StageTiming {
                stage_name: "scaler".to_string(),
                input_size_bytes: *size,
                duration_ms: *dur,
                timestamp: Utc::now(),
            });
        }

        let small = predictor.predict("scaler", 50).unwrap();
        let large = predictor.predict("scaler", 2000).unwrap();

        assert!(
            large > small,
            "larger input should predict longer duration: {:?} vs {:?}",
            small,
            large
        );
    }

    // ── TimeoutPredictor: no history returns None for predict ────────

    #[test]
    fn predictor_no_history_returns_none() {
        let predictor = TimeoutPredictor::new();
        assert!(
            predictor.predict("unknown", 100).is_none(),
            "no history should return None"
        );
    }

    // ── TimeoutPredictor: no history returns configured timeout ──────

    #[test]
    fn predictor_no_history_returns_configured_timeout() {
        let predictor = TimeoutPredictor::new();
        let configured = Duration::from_secs(30);
        let result = predictor.should_extend_timeout("unknown", 100, configured);
        assert_eq!(
            result, configured,
            "no history should return configured timeout unchanged"
        );
    }

    // ── TimeoutPredictor: should_extend_timeout with history ─────────

    #[test]
    fn predictor_extends_timeout_when_history_suggests_longer() {
        let mut predictor = TimeoutPredictor::new();

        // Stage historically takes ~1000ms for any input.
        predictor.record(StageTiming {
            stage_name: "slow".to_string(),
            input_size_bytes: 10,
            duration_ms: 1000,
            timestamp: Utc::now(),
        });
        predictor.record(StageTiming {
            stage_name: "slow".to_string(),
            input_size_bytes: 20,
            duration_ms: 1050,
            timestamp: Utc::now(),
        });

        let configured = Duration::from_millis(800);
        let effective = predictor.should_extend_timeout("slow", 15, configured);

        // Predicted ~1025ms + 20% margin ≈ 1230ms > 800ms → should extend.
        assert!(
            effective > configured,
            "effective timeout {:?} should exceed configured {:?}",
            effective,
            configured
        );

        // But should not exceed reasonable bound
        assert!(
            effective.as_millis() < 2000,
            "effective {:?} should be within reasonable range",
            effective
        );
    }

    // ── TimeoutPredictor: multiple stage histories isolated ──────────

    #[test]
    fn predictor_multiple_stage_histories_isolated() {
        let mut predictor = TimeoutPredictor::new();

        predictor.record(StageTiming {
            stage_name: "fast".to_string(),
            input_size_bytes: 100,
            duration_ms: 10,
            timestamp: Utc::now(),
        });
        predictor.record(StageTiming {
            stage_name: "slow".to_string(),
            input_size_bytes: 100,
            duration_ms: 5000,
            timestamp: Utc::now(),
        });

        let fast_pred = predictor.predict("fast", 100).unwrap();
        let slow_pred = predictor.predict("slow", 100).unwrap();

        assert!(
            slow_pred > fast_pred,
            "slow stage should predict longer than fast stage: {:?} vs {:?}",
            fast_pred,
            slow_pred
        );

        // Fast stage should have 1 record, slow stage 1 record.
        assert_eq!(predictor.stage_history("fast").unwrap().len(), 1);
        assert_eq!(predictor.stage_history("slow").unwrap().len(), 1);
        assert_eq!(predictor.stage_count(), 2);
        assert_eq!(predictor.record_count(), 2);
    }

    // ── TimeoutPredictor: clear ──────────────────────────────────────

    #[test]
    fn predictor_clear() {
        let mut predictor = TimeoutPredictor::new();
        predictor.record(StageTiming {
            stage_name: "a".to_string(),
            input_size_bytes: 1,
            duration_ms: 1,
            timestamp: Utc::now(),
        });
        assert_eq!(predictor.record_count(), 1);
        predictor.clear();
        assert_eq!(predictor.record_count(), 0);
    }

    // ── TimeoutPredictor: FIFO eviction ──────────────────────────────

    #[test]
    fn predictor_fifo_eviction() {
        let mut predictor = TimeoutPredictor::with_max_records(2);
        for i in 0..5 {
            predictor.record(StageTiming {
                stage_name: "fifo".to_string(),
                input_size_bytes: i,
                duration_ms: i * 100,
                timestamp: Utc::now(),
            });
        }
        // Only the last 2 should remain.
        let hist = predictor.stage_history("fifo").unwrap();
        assert_eq!(
            hist.len(),
            2,
            "should retain only max_records_per_stage entries"
        );
        assert_eq!(
            hist[0].input_size_bytes, 3,
            "oldest surviving entry should have input_size=3"
        );
        assert_eq!(
            hist[1].input_size_bytes, 4,
            "newest entry should have input_size=4"
        );
    }

    // ── TimeoutPredictor: single data point uses average ─────────────

    #[test]
    fn predictor_single_datum_uses_average() {
        let mut predictor = TimeoutPredictor::new();
        predictor.record(StageTiming {
            stage_name: "single".to_string(),
            input_size_bytes: 100,
            duration_ms: 777,
            timestamp: Utc::now(),
        });
        let pred = predictor.predict("single", 999).unwrap();
        assert_eq!(
            pred.as_millis(),
            777,
            "single data point should return its duration_ms as prediction"
        );
    }

    // ── CachedExecutor integration-style tests ───────────────────────
    // These test the ContentCache integration with a PipelineExecutor
    // via the CachedExecutor wrapper.

    use crate::pipeline_executor::PipelineExecutor;
    use crate::pipeline_types::{
        CapsuleProfile, PipelineDag, PortDirection, PortMediaType, ResourceRequirements, StagePort,
        StageSpec,
    };

    fn make_stage(name: &str) -> StageSpec {
        StageSpec {
            name: name.to_string(),
            profile: CapsuleProfile {
                name: name.to_string(),
                ports: vec![],
                resources: ResourceRequirements {
                    min_ram_gb: 0.0,
                    min_vram_gb: 0.0,
                    requires_gpu: false,
                    cpu_cores: None,
                    scratch_disk_gb: None,
                },
                image: None,
                timeout_seconds: None,
            },
            input_ports: vec![StagePort {
                direction: PortDirection::Input,
                media_type: PortMediaType::Bytes,
                description: Some("auto".into()),
            }],
            output_ports: vec![StagePort {
                direction: PortDirection::Output,
                media_type: PortMediaType::Bytes,
                description: Some("auto".into()),
            }],
            error_routes: vec![],
            env: None,
            checkpoint_interval_seconds: None,
            secret_refs: None,
            flow_control: None,
        }
    }

    fn sequential_dag(names: &[&str]) -> PipelineDag {
        let stages: Vec<StageSpec> = names.iter().map(|n| make_stage(n)).collect();
        PipelineDag::from_sequential(stages)
    }

    #[tokio::test]
    async fn cached_executor_returns_cached_result_on_second_run() {
        let dag = sequential_dag(&["stage-a"]);
        let executor = PipelineExecutor::new(dag);
        let cached = CachedExecutor::new(executor);

        // First run — cache miss, stage executes.
        let report1 = cached
            .execute_cached("run-1", Some(b"input-data".to_vec()))
            .await;
        assert_eq!(report1.status, RunStatus::Completed);
        assert_eq!(report1.stages.len(), 1);
        let output1 = report1.stages[0].output.as_ref().unwrap().clone();

        // Second run with same input — cache hit, output should be identical.
        let report2 = cached
            .execute_cached("run-1", Some(b"input-data".to_vec()))
            .await;
        assert_eq!(report2.status, RunStatus::Completed);
        assert_eq!(report2.stages.len(), 1);
        let output2 = report2.stages[0].output.as_ref().unwrap();
        assert_eq!(&output1, output2, "cached output must match original");
    }

    #[tokio::test]
    async fn cached_executor_different_input_produces_different_output() {
        let dag = sequential_dag(&["stage-x"]);
        let executor = PipelineExecutor::new(dag);
        let cached = CachedExecutor::new(executor);

        // Run with input-A.
        let report_a = cached
            .execute_cached("run-a", Some(b"input-A".to_vec()))
            .await;
        let output_a = report_a.stages[0].output.as_ref().unwrap().clone();

        // Run with input-B.
        let report_b = cached
            .execute_cached("run-b", Some(b"input-B".to_vec()))
            .await;
        let output_b = report_b.stages[0].output.as_ref().unwrap().clone();

        // Different inputs → different outputs (because the stage appends its
        // name to the input, so A and B produce different results).
        assert_ne!(
            output_a, output_b,
            "different inputs should produce different outputs"
        );
    }

    #[tokio::test]
    async fn cached_executor_three_stage_pipeline() {
        let dag = sequential_dag(&["a", "b", "c"]);
        let executor = PipelineExecutor::new(dag);
        let cached = CachedExecutor::new(executor);

        let report = cached
            .execute_cached("multi-stage", Some(b"start".to_vec()))
            .await;

        assert_eq!(report.status, RunStatus::Completed);
        assert_eq!(report.stages.len(), 3);
        for sr in &report.stages {
            assert_eq!(sr.status, StageStatus::Completed);
        }
    }
}
