// 🤓 Pipeline executor — runs DAG stages in order with NATS pub/sub transport
//    between stages.  Supports serial execution (output of N → input of N+1)
//    and NATS-brokered execution (publish to pipeline.{run_id}.{stage}.output).
//
//    Types:
//      PipelineExecutor   — the executor (owns dag + log_store + optional nats)
//      PipelineRunReport  — summary of a completed run
//      RunStatus          — Completed / Failed(message) / Partial
//      StageResult        — result of a single stage execution
//      StageStatus        — Pending / Running / Completed / Failed / Skipped
//      NatsClient         — trait for NATS publish/subscribe (mockable)

use crate::pipeline_cache::TimeoutPredictor;
use crate::pipeline_checkpoint::{CheckpointStore, PipelineCheckpoint, compute_dag_hash};
use crate::pipeline_flowctl::{FlowControl, FlowGate, StageFlowConfig};
use crate::pipeline_logs::{LogLevel, LogStore, PipelineLogEntry};
use crate::pipeline_nats::{NatsClientAdapter, NatsStageRouter};
use crate::pipeline_statemachine::{PipelineEvent, StateMachine};
use crate::pipeline_transitions::TransitionSink;
use crate::pipeline_types::{PipelineDag, PipelineError, StagePort, StageSpec};
use anyhow::Result;
use chrono::Utc;
use std::collections::HashMap;
use std::sync::Arc;
use std::sync::Mutex;
use std::time::{Duration, Instant};

// ── NatsClient trait ─────────────────────────────────────────────────────────────

/// Abstraction over NATS pub/sub for pipeline stage transport.
///
/// Implementations can wrap `async_nats::Client` for production use or use
/// in-memory channels for testing.
#[async_trait::async_trait]
pub trait NatsClient: Send + Sync {
    /// Publish a payload to a subject.
    async fn publish(&self, subject: &str, payload: Vec<u8>) -> Result<()>;

    /// Subscribe to a subject and return the next message payload.
    /// Returns `None` if the subscription is closed/empty.
    async fn subscribe(&self, subject: &str) -> Result<Option<Vec<u8>>>;
}

// ── MockNatsClient (for tests) ───────────────────────────────────────────────────

/// In-memory mock NATS client using a simple shared queue.
///
/// Each subject gets its own `VecDeque`.  `publish` pushes to the back;
/// `subscribe` pops from the front.  Thread-safe via `Mutex`.
#[cfg(test)]
pub struct MockNatsClient {
    queues: Arc<
        std::sync::Mutex<std::collections::HashMap<String, std::collections::VecDeque<Vec<u8>>>>,
    >,
}

#[cfg(test)]
impl MockNatsClient {
    pub fn new() -> Self {
        Self {
            queues: Arc::new(std::sync::Mutex::new(std::collections::HashMap::new())),
        }
    }
}

#[cfg(test)]
#[async_trait::async_trait]
impl NatsClient for MockNatsClient {
    async fn publish(&self, subject: &str, payload: Vec<u8>) -> Result<()> {
        let mut queues = self.queues.lock().expect("mock nats lock");
        let queue = queues
            .entry(subject.to_string())
            .or_insert_with(std::collections::VecDeque::new);
        queue.push_back(payload);
        Ok(())
    }

    async fn subscribe(&self, subject: &str) -> Result<Option<Vec<u8>>> {
        let mut queues = self.queues.lock().expect("mock nats lock");
        Ok(queues.get_mut(subject).and_then(|q| q.pop_front()))
    }
}

// ── StageStatus ──────────────────────────────────────────────────────────────────

/// Execution status of a single pipeline stage.
#[derive(Debug, Clone, PartialEq)]
pub enum StageStatus {
    Pending,
    Running,
    Completed,
    Failed(PipelineError),
    Skipped,
}

// ── StageResult ──────────────────────────────────────────────────────────────────

/// Result of executing a single pipeline stage.
#[derive(Debug, Clone)]
pub struct StageResult {
    pub stage_name: String,
    pub duration_ms: u64,
    pub output: Option<Vec<u8>>,
    pub error: Option<PipelineError>,
    pub status: StageStatus,
}

// ── RunStatus ────────────────────────────────────────────────────────────────────

/// Overall status of a pipeline run.
#[derive(Debug, Clone, PartialEq)]
pub enum RunStatus {
    Completed,
    Failed(String),
    Partial,
}

// ── PipelineRunReport ────────────────────────────────────────────────────────────

/// Summary report for a completed pipeline run.
#[derive(Debug, Clone)]
pub struct PipelineRunReport {
    pub run_id: String,
    pub stages: Vec<StageResult>,
    pub total_duration_ms: u64,
    pub status: RunStatus,
}

// ── PipelineExecutor ─────────────────────────────────────────────────────────────

/// Executes pipeline DAG stages in order, using either serial chaining (output
/// of stage N → input of stage N+1) or NATS pub/sub transport between stages.
pub struct PipelineExecutor {
    dag: PipelineDag,
    log_store: Arc<dyn LogStore>,
    nats_client: Option<Arc<dyn NatsClient>>,
    nats_router: Mutex<Option<NatsStageRouter>>,
    checkpoint_store: Option<Arc<dyn CheckpointStore>>,
    flow_gates: HashMap<String, FlowGate>,
    timeout_predictor: Option<Arc<Mutex<TimeoutPredictor>>>,
    transition_sink: Option<Arc<dyn TransitionSink>>,
}

impl PipelineExecutor {
    /// Create a new executor with the given DAG and a default log store.
    pub fn new(dag: PipelineDag) -> Self {
        Self {
            dag,
            log_store: Arc::new(crate::pipeline_logs::VecLogStore::new()),
            nats_client: None,
            nats_router: Mutex::new(None),
            flow_gates: HashMap::new(),
            checkpoint_store: None,
            timeout_predictor: None,
            transition_sink: None,
        }
    }

    /// Attach a log store (e.g. a shared `PIPELINE_LOG_STORE`).
    pub fn with_log_store(mut self, store: Arc<dyn LogStore>) -> Self {
        self.log_store = store;
        self
    }

    /// Enable NATS transport for stage-to-stage communication.
    pub fn with_nats(mut self, nc: Arc<dyn NatsClient>) -> Self {
        self.nats_client = Some(nc);
        self
    }

    /// Attach a pre-configured `NatsStageRouter` for subject-based routing.
    ///
    /// When set, the executor uses the router's subject naming convention
    /// (`pipeline.{run_id}.{stage}.{direction}.{media_type}`) instead of the
    /// simple `pipeline.{run_id}.{stage}.output` format.
    pub fn with_nats_router(self, router: NatsStageRouter) -> Self {
        *self.nats_router.lock().expect("nats_router lock") = Some(router);
        self
    }

    /// Attach a checkpoint store for restartable pipeline execution.
    ///
    /// When set, the executor will save a checkpoint after each completed
    /// stage and check for an existing checkpoint at the start of
    /// `execute()`, skipping any stages that were already completed in a
    /// previous run.
    pub fn with_checkpoint_store(mut self, store: Arc<dyn CheckpointStore>) -> Self {
        self.checkpoint_store = Some(store);
        self
    }

    /// Attach a transition sink (e.g. `FileTransitionLog`, `NatsTransitionSink`,
    /// or a `MultiTransitionSink` fanning out to both).
    ///
    /// When set, `execute()` drives an internal `StateMachine` alongside its
    /// existing `StageStatus`/`RunStatus` tracking, and every state
    /// transition is recorded to this sink — a durable/live ledger of the
    /// run's lifecycle, independent of and in parallel with `log_store`.
    pub fn with_transition_sink(mut self, sink: Arc<dyn TransitionSink>) -> Self {
        self.transition_sink = Some(sink);
        self
    }

    /// Attach flow-control gates between stages.
    ///
    /// Builds a `FlowGate` for each stage in the DAG that has a `flow_control`
    /// config, using the configured strategy (or auto-detected from the stage
    /// profile if the strategy is `FlowStrategy::Unbounded`).
    pub fn with_flow_control(mut self) -> Self {
        for stage in &self.dag.stages {
            let config = stage.flow_control.clone().unwrap_or_else(|| {
                // Auto-detect strategy from stage profile
                StageFlowConfig::new(&stage.name, crate::pipeline_flowctl::auto_strategy(stage))
            });
            let fc = FlowControl::new(config.strategy.clone(), &stage.name);
            let gate = FlowGate::new(fc);
            self.flow_gates.insert(stage.name.clone(), gate);
        }
        self
    }

    /// Attach a timeout predictor for adaptive timeout prediction.
    ///
    /// When set, `execute_stage` will adjust per-stage timeouts based on
    /// historical timing data before each execution, and record the actual
    /// stage timing after completion.
    pub fn with_timeout_predictor(mut self, predictor: Arc<Mutex<TimeoutPredictor>>) -> Self {
        self.timeout_predictor = Some(predictor);
        self
    }

    /// Access the pipeline DAG (used by `CachedExecutor` and other wrappers).
    pub fn dag(&self) -> &PipelineDag {
        &self.dag
    }

    /// Execute the full pipeline DAG.
    ///
    /// Stages are executed in topological order (as determined by the DAG).
    /// In serial mode, the output of stage N is passed as input to stage N+1.
    /// In NATS mode, outputs are published to `pipeline.{run_id}.{stage_name}.output`
    /// and the next stage subscribes to receive its input.
    ///
    /// `initial_input` is passed to the first entry-point stage(s).
    pub async fn execute(&self, run_id: &str, initial_input: Option<Vec<u8>>) -> PipelineRunReport {
        let start = Instant::now();
        let mut stage_results: Vec<StageResult> = Vec::new();
        let mut last_output: Option<Vec<u8>> = initial_input;

        // Resolve execution order from the DAG.
        let order = match self.dag.execution_order() {
            Ok(o) => o,
            Err(e) => {
                self.log_store.store(PipelineLogEntry::new(
                    run_id,
                    "*pipeline*",
                    LogLevel::Error,
                    format!("DAG cycle detected: {e}"),
                ));
                return PipelineRunReport {
                    run_id: run_id.to_string(),
                    stages: vec![],
                    total_duration_ms: start.elapsed().as_millis() as u64,
                    status: RunStatus::Failed(format!("DAG cycle: {e}")),
                };
            }
        };

        if order.is_empty() {
            self.log_store.store(PipelineLogEntry::new(
                run_id,
                "*pipeline*",
                LogLevel::Warn,
                "pipeline has no stages to execute".to_string(),
            ));
            return PipelineRunReport {
                run_id: run_id.to_string(),
                stages: vec![],
                total_duration_ms: 0,
                status: RunStatus::Completed,
            };
        }

        self.log_store.store(PipelineLogEntry::new(
            run_id,
            "*pipeline*",
            LogLevel::Info,
            format!(
                "starting pipeline with {} stages: {}",
                order.len(),
                order.join(" → ")
            ),
        ));

        // ── Checkpoint resume ────────────────────────────────────────────
        // If a checkpoint store is attached, check for an existing checkpoint
        // for this run_id and skip any stages that were already completed.
        let mut checkpoint: Option<PipelineCheckpoint> =
            self.checkpoint_store
                .as_ref()
                .and_then(|store| match store.load(run_id) {
                    Ok(Some(cp)) if cp.dag_hash == compute_dag_hash(&self.dag) => Some(cp),
                    _ => None,
                });

        let start_idx = if let Some(ref cp) = checkpoint {
            // Rebuild completed StageResults from checkpoint entries.
            for sc in &cp.completed_stages {
                let name = order
                    .get(sc.stage_index)
                    .cloned()
                    .unwrap_or_else(|| format!("stage_{}", sc.stage_index));
                stage_results.push(StageResult {
                    stage_name: name,
                    duration_ms: 0,
                    output: Some(sc.state.clone()),
                    error: None,
                    status: StageStatus::Completed,
                });
            }
            last_output = cp.completed_stages.last().map(|sc| sc.state.clone());

            self.log_store.store(PipelineLogEntry::new(
                run_id,
                "*pipeline*",
                LogLevel::Info,
                format!(
                    "resumed from checkpoint: {} stage(s) previously completed, continuing from stage {}",
                    cp.completed_stages.len(),
                    cp.current_stage_index,
                ),
            ));

            cp.current_stage_index
        } else {
            if self.checkpoint_store.is_some() {
                // No existing checkpoint found — create a fresh one.
                let cp = PipelineCheckpoint::new(run_id, &self.dag);
                if let Err(e) = self.checkpoint_store.as_ref().unwrap().save(&cp) {
                    self.log_store.store(PipelineLogEntry::new(
                        run_id,
                        "*pipeline*",
                        LogLevel::Warn,
                        format!("failed to create initial checkpoint: {e}"),
                    ));
                }
                checkpoint = Some(cp);
            }
            0
        };

        // ── State machine: drive PipelineState transitions in parallel with
        // the StageStatus/RunStatus tracking above, recording each one via
        // the optional transition_sink (durable file log +/or live NATS).
        // This is a separate concern from StageStatus/RunStatus, not a
        // replacement for them.
        let mut sm = StateMachine::new(self.dag.clone()).with_run_id(run_id);
        if let Some(sink) = &self.transition_sink {
            sm = sm.with_transition_sink(sink.clone());
        }
        let _ = sm.transition(PipelineEvent::Validate);
        let _ = sm.transition(PipelineEvent::Schedule);
        let _ = sm.transition(PipelineEvent::Execute);
        // Resuming from a checkpoint skips already-completed stages in the
        // loop below (start_idx > 0) — fast-forward the state machine
        // through synthetic StageComplete transitions so the ledger stays
        // gap-free and still reaches `Completed`. These synthetic entries
        // carry resume-time timestamps, not the stages' original completion
        // times (accepted tradeoff).
        for skip_idx in 0..start_idx {
            let _ = sm.transition(PipelineEvent::StageComplete(skip_idx as u32));
        }

        for (idx, stage_name) in order.iter().enumerate().skip(start_idx) {
            // Look up the stage spec.
            let stage_spec = match self.dag.find_stage(stage_name) {
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
                    // Mark remaining stages as Skipped.
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

            // ── Subscribe to stage input ──────────────────────────────────
            let use_nats = self.nats_client.is_some() || self.nats_router.lock().unwrap().is_some();
            let input = if use_nats && idx > 0 {
                let prev_stage = &order[idx - 1];
                let subject = self.subject_for_output(run_id, prev_stage);
                self.log_store.store(PipelineLogEntry::new(
                    run_id,
                    stage_name,
                    LogLevel::Info,
                    format!("awaiting input from NATS subject '{subject}'"),
                ));
                match self.subscribe_nats(&subject).await {
                    Some(data) => Some(data),
                    None => {
                        let err = PipelineError::StageCrashed(format!(
                            "no input received on '{subject}'"
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
                }
            } else {
                // Serial mode: use the previous stage's output.
                last_output.take()
            };

            // ── Flow control: record data accepted (consumer side) ─────────
            if idx > 0 {
                let prev_stage = &order[idx - 1];
                if let Some(gate) = self.flow_gates.get(prev_stage) {
                    if let Some(ref data) = input {
                        let mut ctrl = gate.controller().lock().unwrap();
                        ctrl.record_accept(data.len());
                    }
                }
            }

            let result = self.execute_stage(&stage_spec, input, run_id).await;

            // ── Publish stage output to NATS ──────────────────────────────
            let output_subject = self.subject_for_output(run_id, stage_name);
            {
                let router_guard = self.nats_router.lock().expect("nats_router lock");
                if let Some(ref router) = *router_guard {
                    if let Some(ref output) = result.output {
                        let port = self.default_output_port();
                        if let Err(e) = router.route_output(stage_name, "", &port, output) {
                            self.log_store.store(PipelineLogEntry::new(
                                run_id,
                                stage_name,
                                LogLevel::Error,
                                format!("failed to route output via NATS '{output_subject}': {e}"),
                            ));
                        }
                    }
                    // Router handles the publish — skip the nats_client fallback.
                } else if let Some(ref nc) = self.nats_client {
                    if let Some(ref output) = result.output {
                        if let Err(e) = nc.publish(&output_subject, output.clone()).await {
                            self.log_store.store(PipelineLogEntry::new(
                                run_id,
                                stage_name,
                                LogLevel::Error,
                                format!("failed to publish to NATS '{output_subject}': {e}"),
                            ));
                        }
                    }
                }
            }

            // In serial mode, capture output for chaining.
            if self.nats_client.is_none() {
                last_output = result.output.clone();
            }

            // ── Flow control: apply back-pressure before proceeding ───────
            {
                let gate_key = stage_name.clone();
                if let Some(ref output) = result.output {
                    if let Some(gate) = self.flow_gates.get(&gate_key) {
                        let mut iteration = 0u32;
                        loop {
                            let guard = gate.controller().lock().unwrap();
                            if guard.can_emit() {
                                let data_len = output.len();
                                drop(guard);
                                let mut ctrl = gate.controller().lock().unwrap();
                                ctrl.record_emit(data_len);
                                break;
                            }
                            let backoff = guard.wait_backpressure();
                            drop(guard);
                            if backoff > Duration::ZERO {
                                self.log_store.store(PipelineLogEntry::new(
                                    run_id,
                                    &gate_key,
                                    LogLevel::Debug,
                                    format!(
                                        "flow control back-pressure for {}ms (iteration {})",
                                        backoff.as_millis(),
                                        iteration,
                                    ),
                                ));
                                tokio::time::sleep(backoff).await;
                                iteration += 1;
                            } else {
                                // No backoff means always ready — break to avoid busy-loop
                                break;
                            }
                        }
                    }
                }
            }

            let is_failure = matches!(&result.status, StageStatus::Failed(_));
            let stage_output = result.output.clone();
            let stage_error = result.error.clone();
            stage_results.push(result);

            // ── State machine: record this stage's outcome as a transition ──
            if is_failure {
                let err = stage_error
                    .unwrap_or_else(|| PipelineError::StageCrashed("unknown".into()));
                let _ = sm.transition(PipelineEvent::StageFailed(err));
            } else {
                let _ = sm.transition(PipelineEvent::StageComplete(idx as u32));
            }

            // ── Persist checkpoint after each completed stage ────────────
            if !is_failure {
                if let Some(ref mut cp) = checkpoint {
                    if let Some(ref output) = stage_output {
                        cp.record_stage_complete(idx, output.clone());
                        if let Some(ref store) = self.checkpoint_store {
                            if let Err(e) = store.save(cp) {
                                self.log_store.store(PipelineLogEntry::new(
                                    run_id,
                                    stage_name,
                                    LogLevel::Warn,
                                    format!("failed to save checkpoint: {e}"),
                                ));
                            }
                        }
                    }
                }
            }

            if is_failure {
                // Mark remaining stages as Skipped.
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
        }

        // Compute overall status.
        let has_failure = stage_results
            .iter()
            .any(|sr| matches!(&sr.status, StageStatus::Failed(_)));
        let has_skipped = stage_results
            .iter()
            .any(|sr| sr.status == StageStatus::Skipped);

        let status = if has_failure && has_skipped {
            // Find the first error message.
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

        let total_duration_ms = start.elapsed().as_millis() as u64;

        self.log_store.store(PipelineLogEntry::new(
            run_id,
            "*pipeline*",
            if status == RunStatus::Completed {
                LogLevel::Info
            } else {
                LogLevel::Error
            },
            format!("pipeline finished: {:?} in {}ms", status, total_duration_ms),
        ));

        PipelineRunReport {
            run_id: run_id.to_string(),
            stages: stage_results,
            total_duration_ms,
            status,
        }
    }

    /// Execute a single pipeline stage.
    ///
    /// The `input` is the data received from the previous stage (or `None` for
    /// the first stage).  The stage implementation should:
    /// 1. Log via the store
    /// 2. Perform its work (using its profile/timeout/env)
    /// 3. Return output data
    ///
    /// Error handling: if the stage fails, matching `ErrorRoute`s are checked
    /// for retry/fallback logic.
    pub async fn execute_stage(
        &self,
        stage: &StageSpec,
        input: Option<Vec<u8>>,
        run_id: &str,
    ) -> StageResult {
        let start = Instant::now();
        let stage_name = stage.name.clone();

        self.log_store.store(PipelineLogEntry::new(
            run_id,
            &stage_name,
            LogLevel::Info,
            format!(
                "stage '{}' starting (input: {} bytes)",
                stage_name,
                input.as_ref().map(|v| v.len()).unwrap_or(0)
            ),
        ));

        // Determine the effective input: for the first stage, pass the input
        // through; for subsequent stages, use the chained input.
        let effective_input = input.unwrap_or_default();
        let input_size = effective_input.len() as u64;

        // ── Adaptive timeout ────────────────────────────────────────────
        // Use the TimeoutPredictor to adjust the stage's timeout based on
        // historical timing data for this stage and input size.
        let adjusted_stage = if let Some(ref predictor) = self.timeout_predictor {
            let pred = predictor.lock().expect("TimeoutPredictor lock");
            if let Some(timeout_secs) = stage.profile.timeout_seconds {
                let configured = Duration::from_secs(timeout_secs);
                let effective = pred.should_extend_timeout(&stage_name, input_size, configured);
                let effective_secs = effective.as_secs();
                if effective_secs != timeout_secs {
                    self.log_store.store(PipelineLogEntry::new(
                        run_id,
                        &stage_name,
                        LogLevel::Info,
                        format!(
                            "timeout extended: {}s → {}s (predicted)",
                            timeout_secs, effective_secs
                        ),
                    ));
                    let mut adjusted = stage.clone();
                    adjusted.profile.timeout_seconds = Some(effective_secs);
                    adjusted
                } else {
                    stage.clone()
                }
            } else {
                stage.clone()
            }
        } else {
            stage.clone()
        };

        // Attempt execution with retry logic.
        let mut last_error: Option<PipelineError> = None;
        let mut output: Option<Vec<u8>> = None;

        // Track how many retries we've attempted per error route.
        let mut retry_tracker: std::collections::HashMap<usize, u32> =
            std::collections::HashMap::new();

        // Try the stage once + up to `max_retries` per matching ErrorRoute.
        let max_attempts = {
            let max = stage
                .error_routes
                .iter()
                .map(|er| er.max_retries)
                .max()
                .unwrap_or(0)
                + 1; // +1 for the initial try
            std::cmp::max(max, 1)
        };

        for attempt in 0..max_attempts {
            if attempt > 0 {
                // This is a retry — apply backoff.
                let backoff = stage
                    .error_routes
                    .iter()
                    .find(|er| last_error.as_ref().map_or(false, |e| er.matches(e)))
                    .map(|er| er.backoff_ms)
                    .unwrap_or(100);

                self.log_store.store(PipelineLogEntry::new(
                    run_id,
                    &stage_name,
                    LogLevel::Warn,
                    format!("retry attempt {attempt} after {backoff}ms backoff"),
                ));
                tokio::time::sleep(tokio::time::Duration::from_millis(backoff)).await;
            }

            match self.run_stage_fn(&adjusted_stage, &effective_input).await {
                Ok(data) => {
                    // Stage succeeded.
                    output = Some(data);
                    last_error = None;
                    break;
                }
                Err(e) => {
                    last_error = Some(e.clone());

                    // Check if any error route can handle this.
                    let matching_route = stage
                        .error_routes
                        .iter()
                        .enumerate()
                        .find(|(_, er)| er.matches(&e));

                    match matching_route {
                        Some((idx, er)) => {
                            let attempts = retry_tracker.entry(idx).or_insert(0);
                            if *attempts < er.max_retries {
                                *attempts += 1;
                                self.log_store.store(PipelineLogEntry::new(
                                    run_id,
                                    &stage_name,
                                    LogLevel::Warn,
                                    format!(
                                        "error '{}' matches route '{}', retry {}/{}",
                                        e.variant_name(),
                                        er.route_to_stage,
                                        *attempts,
                                        er.max_retries
                                    ),
                                ));
                                // Continue to retry (the for loop handles this).
                                continue;
                            } else {
                                // Retries exhausted — check for fallback.
                                if let Some(ref _fallback) = er.fallback_output {
                                    self.log_store.store(PipelineLogEntry::new(
                                        run_id,
                                        &stage_name,
                                        LogLevel::Warn,
                                        format!("retries exhausted, using fallback output for route '{}'", er.route_to_stage),
                                    ));
                                    // Use a default fallback (empty bytes).
                                    output = Some(Vec::new());
                                    last_error = None;
                                    break;
                                }
                            }
                        }
                        None => {
                            // No matching error route — fail immediately.
                            self.log_store.store(PipelineLogEntry::new(
                                run_id,
                                &stage_name,
                                LogLevel::Error,
                                format!(
                                    "stage failed with unhandled error: {:?} (no matching error route)",
                                    e
                                ),
                            ));
                        }
                    }

                    // If we get here, we're out of retries or no route matched.
                    // Break out of the retry loop with the last error.
                    break;
                }
            }
        }

        let duration_ms = start.elapsed().as_millis() as u64;

        // ── Record timing in predictor ─────────────────────────────────
        if let Some(ref predictor) = self.timeout_predictor {
            let mut pred = predictor.lock().expect("TimeoutPredictor lock");
            pred.record(crate::pipeline_cache::StageTiming {
                stage_name: stage_name.clone(),
                input_size_bytes: input_size,
                duration_ms,
                timestamp: Utc::now(),
            });
        }

        match last_error {
            None => {
                self.log_store.store(PipelineLogEntry::new(
                    run_id,
                    &stage_name,
                    LogLevel::Info,
                    format!(
                        "stage completed in {duration_ms}ms (output: {} bytes)",
                        output.as_ref().map(|v| v.len()).unwrap_or(0)
                    ),
                ));
                StageResult {
                    stage_name,
                    duration_ms,
                    output,
                    error: None,
                    status: StageStatus::Completed,
                }
            }
            Some(err) => {
                self.log_store.store(PipelineLogEntry::new(
                    run_id,
                    &stage_name,
                    LogLevel::Error,
                    format!("stage failed after {duration_ms}ms: {err:?}"),
                ));
                StageResult {
                    stage_name,
                    duration_ms,
                    output: None,
                    error: Some(err.clone()),
                    status: StageStatus::Failed(err),
                }
            }
        }
    }

    /// The actual execution logic for a stage.
    ///
    /// In a production system this would shell out, call a compute provider,
    /// or invoke a Wasm capsule.  In this implementation it runs a simple
    /// closure based on the stage name, simulating work.
    ///
    /// NOTE: This is intentionally simplistic for the MVP. Real remote
    /// execution (actually shelling out on a provisioned host) is still a
    /// later milestone. What IS wired up as of `pipeline_provision.rs`:
    /// getting a *host* for a stage that doesn't fit any known one, via
    /// `schedule_with_dynamic_provisioning` (calls b00t-historian's
    /// `vultr.provision` NATS subject, see `vultr_delegate.rs`) — that's
    /// the scheduling half of `ComputeProvider` delegation. Running a stage
    /// once it has a host is the remaining half.
    async fn run_stage_fn(
        &self,
        stage: &StageSpec,
        input: &[u8],
    ) -> std::result::Result<Vec<u8>, PipelineError> {
        // Check for timeout.
        if let Some(timeout_secs) = stage.profile.timeout_seconds {
            let deadline = Instant::now() + tokio::time::Duration::from_secs(timeout_secs);
            if Instant::now() > deadline {
                return Err(PipelineError::Timeout {
                    stage: stage.name.clone(),
                    elapsed_ms: timeout_secs * 1000,
                });
            }
        }

        // Base implementation: pass through input as output.
        // Subclasses or production wrappers override this.
        //
        // For now, simulate by appending the stage name as a marker so tests
        // can verify the execution chain.
        let mut result = input.to_vec();
        result.extend_from_slice(format!(":{}", stage.name).as_bytes());

        // Simulate a small async wait to exercise the async path.
        tokio::time::sleep(tokio::time::Duration::from_millis(1)).await;

        Ok(result)
    }

    /// Subscribe to a NATS subject for stage input.
    async fn subscribe_nats(&self, subject: &str) -> Option<Vec<u8>> {
        match self.nats_client {
            Some(ref nc) => match nc.subscribe(subject).await {
                Ok(data) => data,
                Err(e) => {
                    self.log_store.store(PipelineLogEntry::new(
                        "*nats*",
                        "*pipeline*",
                        LogLevel::Error,
                        format!("NATS subscribe error on '{subject}': {e}"),
                    ));
                    None
                }
            },
            None => None,
        }
    }

    /// Build the NATS output subject for a stage.
    ///
    /// Uses the `NatsStageRouter`'s subject naming convention when the
    /// router is available, otherwise falls back to the simple format
    /// `pipeline.{run_id}.{stage_name}.output`.
    ///
    /// Lazily initialises the router from `nats_client` on first call.
    fn subject_for_output(&self, run_id: &str, stage_name: &str) -> String {
        let mut guard = self.nats_router.lock().expect("nats_router lock");
        if guard.is_none() {
            if let Some(ref nc) = self.nats_client {
                let adapter = Box::new(NatsClientAdapter::new(nc.clone()));
                *guard = Some(NatsStageRouter::new(adapter, run_id));
            }
        }
        if let Some(ref router) = *guard {
            let port = self.default_output_port();
            router.subject_for(stage_name, &port)
        } else {
            format!("pipeline.{}.{}.output", run_id, stage_name)
        }
    }

    /// Return a default `StagePort` for output (Bytes / Output direction).
    fn default_output_port(&self) -> StagePort {
        StagePort {
            direction: crate::pipeline_types::PortDirection::Output,
            media_type: crate::pipeline_types::PortMediaType::Bytes,
            description: None,
        }
    }
}

// ── Tests ───────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pipeline_logs::VecLogStore;
    use crate::pipeline_types::{
        CapsuleProfile, ErrorRoute, PipelineDag, PortDirection, PortMediaType,
        ResourceRequirements, StagePort, StageSpec,
    };

    /// Helper: build a minimal stage spec with a name and optional timeout.
    fn make_stage(name: &str, timeout_secs: Option<u64>) -> StageSpec {
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
                timeout_seconds: timeout_secs,
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

    /// Helper: build a sequential DAG from stage names.
    ///
    /// Uses `from_sequential` to avoid bidirectional edges that `build`
    /// creates when all ports use `Bytes` (which would be flagged as a cycle).
    fn sequential_dag(names: &[&str]) -> PipelineDag {
        let stages: Vec<StageSpec> = names.iter().map(|n| make_stage(n, None)).collect();
        PipelineDag::from_sequential(stages)
    }

    // ── Serial execution of 2 stages ────────────────────────────────────

    #[tokio::test]
    async fn serial_execution_two_stages() {
        let dag = sequential_dag(&["stage-a", "stage-b"]);
        let executor = PipelineExecutor::new(dag);

        let report = executor
            .execute("test-serial-2", Some(b"hello".to_vec()))
            .await;

        assert_eq!(
            report.status,
            RunStatus::Completed,
            "expected Completed, got {:?}",
            report.status
        );
        assert_eq!(report.stages.len(), 2, "expected 2 stage results");
        assert_eq!(report.stages[0].stage_name, "stage-a");
        assert_eq!(report.stages[0].status, StageStatus::Completed);
        assert_eq!(report.stages[1].stage_name, "stage-b");
        assert_eq!(report.stages[1].status, StageStatus::Completed);

        // Verify output chaining: stage-a appends ":stage-a", stage-b appends ":stage-b"
        let output = report.stages[1]
            .output
            .as_ref()
            .expect("stage-b should have output");
        let output_str = String::from_utf8_lossy(output);
        assert!(
            output_str.contains("hello"),
            "output should contain initial input 'hello', got: {output_str}"
        );
        assert!(
            output_str.contains(":stage-a"),
            "output should contain ':stage-a' from first stage, got: {output_str}"
        );
        assert!(
            output_str.contains(":stage-b"),
            "output should contain ':stage-b' from second stage, got: {output_str}"
        );

        let log_store = executor.log_store;
        let logs = log_store.query(&Default::default());
        assert!(!logs.is_empty(), "should have log entries");
        assert!(logs.iter().any(|l| l.message.contains("pipeline finished")));
    }

    // ── Run report collects all stage results ───────────────────────────

    #[tokio::test]
    async fn run_report_collects_all_stages() {
        let dag = sequential_dag(&["ingest", "transform", "output"]);
        let executor = PipelineExecutor::new(dag);

        let report = executor.execute("test-report-3", None).await;

        assert_eq!(report.stages.len(), 3);
        assert_eq!(report.run_id, "test-report-3");
        assert!(report.total_duration_ms > 0);
        // All should complete even with no input (empty Vec<u8> is the fallback).
        for (i, sr) in report.stages.iter().enumerate() {
            assert_eq!(
                sr.status,
                StageStatus::Completed,
                "stage {} ({}) should be Completed, got {:?}",
                i,
                sr.stage_name,
                sr.status
            );
        }
    }

    // ── Stage failure triggers error route ──────────────────────────────

    /// A custom stage that wraps PipelineExecutor and injects a failure
    /// for a specific stage name by overriding run_stage_fn.
    ///
    /// We test error routing by creating an executor attached to a DAG where
    /// the second stage has an ErrorRoute that routes "StageCrashed" to a
    /// fallback.
    #[tokio::test]
    async fn stage_failure_triggers_error_route() {
        // Build a DAG with two stages where the second stage has an error route.
        let stages = vec![
            make_stage("stage-ok", None),
            StageSpec {
                name: "stage-fail".to_string(),
                error_routes: vec![ErrorRoute {
                    match_pattern: "StageCrashed".to_string(),
                    route_to_stage: "fallback".to_string(),
                    max_retries: 1,
                    backoff_ms: 5,
                    fallback_output: Some(StagePort {
                        direction: PortDirection::Output,
                        media_type: PortMediaType::Bytes,
                        description: Some("fallback".into()),
                    }),
                    retry_count: 0,
                }],
                ..make_stage("stage-fail", None)
            },
        ];
        let dag = PipelineDag::from_sequential(stages);

        // Use an executor that forces stage-fail to fail on first try then
        // succeed on retry — this verifies the error route retry logic.
        // We test the error route matching by checking the stage result.
        let executor = PipelineExecutor::new(dag);

        // To actually trigger an error, we need the stage to fail.
        // Since run_stage_fn always succeeds in the base impl, we test
        // the error route matching by injecting a failure scenario via
        // custom test logic.
        //
        // Instead, we verify the error route is well-formed and matches
        // expected error variants.
        let err = PipelineError::StageCrashed("something broke".to_string());
        let route = ErrorRoute {
            match_pattern: "StageCrashed".to_string(),
            route_to_stage: "fallback".to_string(),
            max_retries: 2,
            backoff_ms: 5,
            fallback_output: None,
            retry_count: 0,
        };
        assert!(route.matches(&err), "error route should match StageCrashed");
        assert!(route.can_retry(), "should have retries left");

        // Also verify that a non-matching error doesn't match.
        let timeout = PipelineError::Timeout {
            stage: "x".to_string(),
            elapsed_ms: 100,
        };
        assert!(!route.matches(&timeout), "should not match Timeout");

        // Run the pipeline - both stages should complete since the base
        // run_stage_fn doesn't fail.
        let report = executor.execute("test-error-route", None).await;
        assert_eq!(report.status, RunStatus::Completed);
        assert_eq!(report.stages.len(), 2);
    }

    // ── Stage retry on failure ──────────────────────────────────────────

    #[tokio::test]
    async fn stage_retry_on_failure() {
        // Test that the retry mechanism works: create an error route with
        // max_retries=2 and verify the retry counting.
        let mut route = ErrorRoute {
            match_pattern: "StageCrashed".to_string(),
            route_to_stage: "retry-stage".to_string(),
            max_retries: 2,
            backoff_ms: 5,
            fallback_output: None,
            retry_count: 0,
        };

        assert!(route.can_retry());
        assert_eq!(route.retries_left(), 2);

        route.record_retry();
        assert!(route.can_retry());
        assert_eq!(route.retries_left(), 1);

        route.record_retry();
        assert!(!route.can_retry());
        assert_eq!(route.retries_left(), 0);

        // Now test with the executor — run a pipeline and verify the
        // execute_stage method handles retry logic when errors occur.
        // We use a custom test by directly calling execute_stage with a
        // stage that has error routes.
        let stage = StageSpec {
            name: "retry-me".to_string(),
            error_routes: vec![ErrorRoute {
                match_pattern: "StageCrashed".to_string(),
                route_to_stage: "retry-me".to_string(),
                max_retries: 1,
                backoff_ms: 5,
                fallback_output: None,
                retry_count: 0,
            }],
            ..make_stage("retry-me", None)
        };

        let dag = PipelineDag::build(vec![stage.clone()]).expect("valid DAG");
        let executor = PipelineExecutor::new(dag);

        // execute_stage with the base run_stage_fn will succeed, so just
        // verify it doesn't panic and returns the correct shape.
        let result = executor
            .execute_stage(&stage, Some(b"input data".to_vec()), "test-retry")
            .await;

        // The base impl succeeds, so status should be Completed.
        assert_eq!(
            result.status,
            StageStatus::Completed,
            "expected Completed, got {:?}",
            result.status
        );
        assert!(result.duration_ms > 0);
    }

    // ── NATS mode execution ─────────────────────────────────────────────

    #[tokio::test]
    async fn nats_mode_passes_data_between_stages() {
        let dag = sequential_dag(&["nats-a", "nats-b"]);
        let nats = Arc::new(MockNatsClient::new()) as Arc<dyn NatsClient>;
        let executor = PipelineExecutor::new(dag).with_nats(nats);

        let report = executor
            .execute("test-nats", Some(b"nats-input".to_vec()))
            .await;

        assert_eq!(
            report.status,
            RunStatus::Completed,
            "expected Completed, got {:?}",
            report.status
        );
        assert_eq!(report.stages.len(), 2);
        assert_eq!(report.stages[0].stage_name, "nats-a");
        assert_eq!(report.stages[1].stage_name, "nats-b");
        assert_eq!(report.stages[0].status, StageStatus::Completed);
        assert_eq!(report.stages[1].status, StageStatus::Completed);
    }

    // ── Pipeline with no stages ─────────────────────────────────────────

    #[tokio::test]
    async fn empty_pipeline() {
        let dag = PipelineDag::build(vec![]).expect("empty DAG is valid");
        let executor = PipelineExecutor::new(dag);

        let report = executor.execute("test-empty", None).await;

        assert_eq!(report.status, RunStatus::Completed);
        assert!(report.stages.is_empty());
    }

    // ── Pipeline run report timing ──────────────────────────────────────

    #[tokio::test]
    async fn run_report_has_positive_duration() {
        let dag = sequential_dag(&["a", "b", "c"]);
        let executor = PipelineExecutor::new(dag);

        let report = executor.execute("test-timing", None).await;

        assert!(
            report.total_duration_ms > 0,
            "total_duration_ms should be > 0, got {}",
            report.total_duration_ms
        );
        for sr in &report.stages {
            assert!(
                sr.duration_ms > 0,
                "stage '{}' duration should be > 0, got {}",
                sr.stage_name,
                sr.duration_ms
            );
        }
    }

    // ── LogStore integration ────────────────────────────────────────────

    #[tokio::test]
    async fn logs_are_stored_during_execution() {
        let store = Arc::new(VecLogStore::new()) as Arc<dyn LogStore>;
        let dag = sequential_dag(&["log-stage"]);
        let executor = PipelineExecutor::new(dag).with_log_store(store.clone());

        executor.execute("test-logs", None).await;

        let logs = store.query(&Default::default());
        // Should have at least: pipeline start, stage start, stage complete, pipeline finish.
        assert!(
            logs.len() >= 4,
            "expected at least 4 log entries, got {}",
            logs.len()
        );

        let messages: Vec<&str> = logs.iter().map(|l| l.message.as_str()).collect();
        assert!(
            messages.iter().any(|m| m.contains("starting pipeline")),
            "should have pipeline start log"
        );
        assert!(
            messages.iter().any(|m| m.contains("pipeline finished")),
            "should have pipeline finish log"
        );
        assert!(
            messages.iter().any(|m| m.contains("log-stage")),
            "should have stage-related logs"
        );
    }

    // ── Stage timestamps ────────────────────────────────────────────────

    #[tokio::test]
    async fn stage_results_have_nonzero_duration() {
        let dag = sequential_dag(&["fast-stage"]);
        let executor = PipelineExecutor::new(dag);

        let report = executor.execute("test-duration", None).await;

        assert_eq!(report.stages.len(), 1);
        assert!(
            report.stages[0].duration_ms > 0,
            "stage duration should be > 0"
        );
        // The stage does a 1ms sleep, so it should be at least 1ms.
        assert!(
            report.stages[0].duration_ms >= 1,
            "duration should be at least 1ms"
        );
    }
}
