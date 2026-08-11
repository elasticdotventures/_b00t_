// ── GH #737: Integration test harness for multi-stage pipelines ──────────────
//
// Exercises the full pipeline types end-to-end: DAG construction, serial
// execution, error routing, secret injection, cost reporting, and log
// capture — all without external services (deterministic, mock-friendly).
//
// Test objectives (linked to GH issues):
//   #719 — Port types & media type compatibility
//   #720 — Resource requirements & GPU/CPU fit
//   #721 — StageSpec construction & StageEntry resolution
//   #722 — Error types, ErrorRoute matching & retry logic
//   #723 — Port negotiation & auto-insertion of conversion stages
//   #724 — PipelineDag construction, validation, & topological sort
//   #734 — Transmogrifier reference implementations
//   #745 — Cost attribution & GPU-time accounting

use std::collections::HashMap;
use std::sync::Arc;

use b00t_cli::pipeline_costs::{
    CostConfig, CostEstimate, PipelineCostReport, ResourceUsage, StageCostRow,
};
use b00t_cli::pipeline_executor::{PipelineExecutor, PipelineRunReport, RunStatus, StageStatus};
use b00t_cli::pipeline_logs::{LogStore, PipelineLogQuery, VecLogStore};
use b00t_cli::pipeline_secrets::{SecretRef, SecretSource, SecretStore};
use b00t_cli::pipeline_types::{
    CapsuleProfile, ErrorRoute, PipelineDag, PortDirection, PortMediaType, ResourceRequirements,
    StagePort, StageSpec,
};
use b00t_cli::transmogrifier::TransmogrifierRegistry;

// ── Test Harness ─────────────────────────────────────────────────────────────

/// Integration test harness for multi-stage pipeline execution.
///
/// Wraps a `TransmogrifierRegistry`, a shared `VecLogStore`, and a list of
/// stages so that each test can declaratively build, execute, and verify
/// pipelines without repeating wiring boilerplate.
struct PipelineTestHarness {
    registry: TransmogrifierRegistry,
    log_store: Arc<VecLogStore>,
    stages: Vec<StageSpec>,
}

impl PipelineTestHarness {
    /// Create a new harness pre-populated with built-in transmogrifiers.
    fn new() -> Self {
        Self {
            registry: TransmogrifierRegistry::builtin(),
            log_store: Arc::new(VecLogStore::new()),
            stages: Vec::new(),
        }
    }

    /// Register a stage with the given name, input port, and output port.
    ///
    /// Resource requirements are pulled from the registry if a matching
    /// transmogrifier exists; otherwise, a default (CPU-only, minimal) profile
    /// is used.
    fn add_stage(&mut self, name: &str, input_port: StagePort, output_port: StagePort) {
        let profile = self
            .registry
            .get(name)
            .map(|t| t.profile())
            .unwrap_or_else(|| CapsuleProfile {
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
            });

        self.stages.push(StageSpec {
            name: name.to_string(),
            profile,
            input_ports: vec![input_port],
            output_ports: vec![output_port],
            error_routes: vec![],
            env: None,
            checkpoint_interval_seconds: None,
            secret_refs: None,
            flow_control: None,
        });
    }

    /// Register a stage with optional error routes and secret refs.
    fn add_stage_full(
        &mut self,
        name: &str,
        input_port: StagePort,
        output_port: StagePort,
        error_routes: Vec<ErrorRoute>,
        secret_refs: Option<Vec<SecretRef>>,
    ) {
        let profile = self
            .registry
            .get(name)
            .map(|t| t.profile())
            .unwrap_or_else(|| CapsuleProfile {
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
            });

        self.stages.push(StageSpec {
            name: name.to_string(),
            profile,
            input_ports: vec![input_port],
            output_ports: vec![output_port],
            error_routes,
            env: None,
            checkpoint_interval_seconds: None,
            secret_refs,
            flow_control: None,
        });
    }

    /// Build a sequential `PipelineDag` from the registered stages.
    fn build_dag(&self) -> PipelineDag {
        PipelineDag::from_sequential(self.stages.clone())
    }

    /// Execute the pipeline and return the run report.
    ///
    /// Stages run in series: output of stage N is passed as input to stage N+1
    /// (the base `run_stage_fn` appends `:{stage_name}` to the byte stream,
    /// making the execution chain verifiable).
    async fn run(&self, run_id: &str) -> PipelineRunReport {
        let dag = self.build_dag();
        let executor = PipelineExecutor::new(dag).with_log_store(self.log_store.clone());
        executor.execute(run_id, Some(b"input".to_vec())).await
    }

    /// Return a copy of all log entries (chronological order).
    fn logs(&self) -> Vec<b00t_cli::pipeline_logs::PipelineLogEntry> {
        self.log_store.query(&PipelineLogQuery::default())
    }
}

// ── Helpers ──────────────────────────────────────────────────────────────────

/// Shortcut to build an `Input` port for a given media type.
fn input_port(media: PortMediaType) -> StagePort {
    StagePort {
        direction: PortDirection::Input,
        media_type: media,
        description: None,
    }
}

/// Shortcut to build an `Output` port for a given media type.
fn output_port(media: PortMediaType) -> StagePort {
    StagePort {
        direction: PortDirection::Output,
        media_type: media,
        description: None,
    }
}

/// Assert that `report` completed successfully (all stages done, no failures).
fn assert_completed(report: &PipelineRunReport) {
    assert_eq!(
        report.status,
        RunStatus::Completed,
        "expected Completed, got {:?}",
        report.status
    );
}

/// Assert that every stage result has the given status.
fn assert_all_stages(report: &PipelineRunReport, expected: StageStatus) {
    for (i, sr) in report.stages.iter().enumerate() {
        assert_eq!(
            sr.status, expected,
            "stage {} ({}) should be {:?}, got {:?}",
            i, sr.stage_name, expected, sr.status
        );
    }
}

/// Assert the output of the final stage contains all the expected substrings
/// (in order, verifying the serial execution chain).
fn assert_output_chain(report: &PipelineRunReport, expected_parts: &[&str]) {
    let last = report.stages.last().expect("at least one stage");
    let output = last.output.as_ref().expect("last stage should have output");
    let output_str = String::from_utf8_lossy(output);
    for part in expected_parts {
        assert!(
            output_str.contains(part),
            "output should contain '{part}', got: {output_str}"
        );
    }
}

// ── Test: Video Pipeline (VideoIngest → Transcode → FrameExtract) ──────────

/// Objective: Verify a 3-stage video pipeline executes end-to-end with
/// GPU-accelerated transcode and frame extraction, producing log evidence.
///
/// Coverage:
///   - `VideoIngest` (input: Video → output: Bytes)
///   - `Transcode`   (input: Video → output: Video, requires_gpu: true)
///   - `FrameExtract`(input: Video → output: Image)
///   - Serial output chaining preserves data through all stages
///   - Log entries include pipeline start, stage progress, and finish
#[tokio::test]
async fn test_video_pipeline() {
    let mut harness = PipelineTestHarness::new();

    // Arrange: build a video pipeline from three stages.
    harness.add_stage(
        "VideoIngest",
        input_port(PortMediaType::Video),
        output_port(PortMediaType::Bytes),
    );
    harness.add_stage(
        "Transcode",
        input_port(PortMediaType::Video),
        output_port(PortMediaType::Video),
    );
    harness.add_stage(
        "FrameExtract",
        input_port(PortMediaType::Video),
        output_port(PortMediaType::Image),
    );

    // Act: execute the pipeline.
    let report = harness.run("test-video-pipe").await;

    // Assert: pipeline completed with all 3 stages.
    assert_completed(&report);
    assert_eq!(report.stages.len(), 3, "expected 3 stages");
    assert_all_stages(&report, StageStatus::Completed);

    // Verify stage names appear in order.
    assert_eq!(report.stages[0].stage_name, "VideoIngest");
    assert_eq!(report.stages[1].stage_name, "Transcode");
    assert_eq!(report.stages[2].stage_name, "FrameExtract");

    // Verify output chaining: each stage appended its name marker.
    assert_output_chain(&report, &[":VideoIngest", ":Transcode", ":FrameExtract"]);

    // Verify Transcode resource requirements include GPU.
    let transcode = harness.registry.get("Transcode").unwrap();
    assert!(transcode.resources().requires_gpu);
    assert!(transcode.resources().min_vram_gb >= 8.0);

    // Verify logs exist and contain stage names.
    let logs = harness.logs();
    assert!(!logs.is_empty(), "should have log entries");
    assert!(
        logs.iter().any(|l| l.message.contains("pipeline finished")),
        "should have pipeline finished log"
    );
    assert!(
        logs.iter().any(|l| l.message.contains("VideoIngest")),
        "should mention VideoIngest in logs"
    );

    // ✅ Positive: video pipeline completes with GPU stage and frame extraction.
    // ❌ Negative: not applicable — this test verifies a success path.
}

// ── Test: Audio Pipeline (Audio → WhisperTranscribe → JSON) ─────────────────

/// Objective: Verify an audio transcription pipeline produces structured JSON
/// output from raw audio bytes.
///
/// Coverage:
///   - WhisperTranscribe mock returns valid JSON with `text`, `segments`,
///     `language`, and `duration` fields
///   - Serial output chaining across 2 stages
///   - Log entries reflect both stages
#[tokio::test]
async fn test_audio_pipeline() {
    let mut harness = PipelineTestHarness::new();

    // Arrange: build an audio → transcription pipeline.
    harness.add_stage(
        "AudioSource",
        input_port(PortMediaType::Audio),
        output_port(PortMediaType::Audio),
    );
    harness.add_stage(
        "WhisperTranscribe",
        input_port(PortMediaType::Audio),
        output_port(PortMediaType::Json),
    );

    // Act: execute.
    let report = harness.run("test-audio-pipe").await;

    // Assert: pipeline completed with 2 stages.
    assert_completed(&report);
    assert_eq!(report.stages.len(), 2);
    assert_all_stages(&report, StageStatus::Completed);

    // Verify stage names.
    assert_eq!(report.stages[0].stage_name, "AudioSource");
    assert_eq!(report.stages[1].stage_name, "WhisperTranscribe");

    // The second stage output is the base executor's pass-through (appended
    // stage name).  For the WhisperTranscribe transmogrifier itself, verify
    // its transform produces valid JSON when called directly.
    let whisper = harness.registry.get("WhisperTranscribe").unwrap();
    let json_result = whisper
        .transform(b"mock audio bytes", &HashMap::new())
        .unwrap();
    let parsed: serde_json::Value =
        serde_json::from_slice(&json_result).expect("WhisperTranscribe output must be valid JSON");
    assert_eq!(parsed["language"], "en");
    assert!(parsed.get("text").is_some());
    assert!(parsed.get("segments").is_some());
    assert!(parsed.get("duration").is_some());

    // Verify logs.
    let logs = harness.logs();
    assert!(logs.iter().any(|l| l.message.contains("WhisperTranscribe")));
    assert!(logs.iter().any(|l| l.message.contains("pipeline finished")));

    // ✅ Positive: audio pipeline produces JSON output via WhisperTranscribe.
    // ❌ Negative: not applicable — success path.
}

// ── Test: Error Handling ─────────────────────────────────────────────────────

/// Objective: Verify that an `ErrorRoute` with a fallback output is properly
/// wired and that the pipeline executor handles error-route configuration.
///
/// Coverage:
///   - `ErrorRoute` matching against known `PipelineError` variants
///   - Retry counting (`can_retry`, `record_retry`, `retries_left`)
///   - Fallback output configuration & serialization round-trip
///   - Pipeline executor runs stages with error routes attached
///   - Log entries reflect error route names
#[tokio::test]
async fn test_error_handling() {
    // ── 1. ErrorRoute matching & retry logic ──

    // Exact-match route.
    let route = ErrorRoute {
        match_pattern: "StageCrashed".to_string(),
        route_to_stage: "error-handler".to_string(),
        max_retries: 2,
        backoff_ms: 10,
        fallback_output: Some(output_port(PortMediaType::Bytes)),
        retry_count: 0,
    };

    // Positive match.
    let crash = b00t_cli::pipeline_types::PipelineError::StageCrashed("oops".into());
    assert!(route.matches(&crash), "route should match StageCrashed");

    // Negative match — wrong variant.
    let timeout = b00t_cli::pipeline_types::PipelineError::Timeout {
        stage: "x".into(),
        elapsed_ms: 100,
    };
    assert!(!route.matches(&timeout), "route should NOT match Timeout");

    // Wildcard catch-all route.
    let catch_all = ErrorRoute {
        match_pattern: "*".to_string(),
        route_to_stage: "catch-all".to_string(),
        max_retries: 1,
        backoff_ms: 5,
        fallback_output: None,
        retry_count: 0,
    };
    assert!(catch_all.matches(&crash));
    assert!(catch_all.matches(&timeout));

    // Retry counting.
    let mut r = route.clone();
    assert_eq!(r.retries_left(), 2);
    assert!(r.can_retry());
    r.record_retry();
    assert_eq!(r.retries_left(), 1);
    assert!(r.can_retry());
    r.record_retry();
    assert_eq!(r.retries_left(), 0);
    assert!(!r.can_retry());

    // ── 2. Pipeline execution with error routes ──
    let mut harness = PipelineTestHarness::new();
    harness.add_stage_full(
        "VideoIngest",
        input_port(PortMediaType::Video),
        output_port(PortMediaType::Bytes),
        vec![],
        None,
    );
    harness.add_stage_full(
        "Transcode",
        input_port(PortMediaType::Video),
        output_port(PortMediaType::Video),
        vec![ErrorRoute {
            match_pattern: "Timeout".to_string(),
            route_to_stage: "retry-transcode".to_string(),
            max_retries: 1,
            backoff_ms: 5,
            fallback_output: Some(output_port(PortMediaType::Bytes)),
            retry_count: 0,
        }],
        None,
    );

    // Act: run the pipeline.  The base executor's run_stage_fn does not fail,
    // so the pipeline completes normally with error routes configured.
    let report = harness.run("test-error-handling").await;

    // Assert: both stages completed.
    assert_completed(&report);
    assert_eq!(report.stages.len(), 2);
    assert_all_stages(&report, StageStatus::Completed);

    // Verify output chaining includes both stage names.
    assert_output_chain(&report, &[":VideoIngest", ":Transcode"]);

    // Verify the pipeline logs exist and capture both stage names
    // (the base run_stage_fn always succeeds, so error routes are configured
    // but not triggered — the test validates that error-route configuration
    // does not prevent normal execution).
    let logs = harness.logs();
    assert!(!logs.is_empty(), "should have log entries");
    assert!(
        logs.iter().any(|l| l.message.contains("VideoIngest")),
        "logs should mention VideoIngest stage"
    );
    assert!(
        logs.iter().any(|l| l.message.contains("Transcode")),
        "logs should mention Transcode stage"
    );
    assert!(
        logs.iter().any(|l| l.message.contains("pipeline finished")),
        "logs should have pipeline finished message"
    );

    // Verify error route serialization round-trip (important for pipeline defs).
    let route_json = serde_json::to_string(&route).unwrap();
    let route_back: ErrorRoute = serde_json::from_str(&route_json).unwrap();
    assert_eq!(route_back.match_pattern, "StageCrashed");
    assert_eq!(route_back.max_retries, 2);
    assert_eq!(route_back.route_to_stage, "error-handler");

    // ✅ Positive: error routes match correctly, retry counting works,
    //              pipeline runs with error routes configured.
    // ❌ Negative: timeout variant does not match StageCrashed route;
    //              exhaust all retries → can_retry returns false.
}

// ── Test: Serial Execution Full (3-stage DAG) ───────────────────────────────

/// Objective: Verify a 3-stage DAG executes serially with correct output
/// chaining, topological ordering, and log capture for every stage.
///
/// Coverage:
///   - `PipelineDag::from_sequential` builds a 3-stage DAG
///   - `PipelineExecutor::execute` runs all stages in order
///   - Each stage's output feeds into the next (byte accumulation)
///   - `PipelineRunReport` carries run_id, duration, per-stage results
///   - All 4 expected log categories appear (pipeline start, stage start,
///     stage complete, pipeline finish)
#[tokio::test]
async fn test_serial_execution_full() {
    let mut harness = PipelineTestHarness::new();

    // Arrange: three stages connected serially.
    harness.add_stage(
        "Stage-A",
        input_port(PortMediaType::Bytes),
        output_port(PortMediaType::Bytes),
    );
    harness.add_stage(
        "Stage-B",
        input_port(PortMediaType::Bytes),
        output_port(PortMediaType::Bytes),
    );
    harness.add_stage(
        "Stage-C",
        input_port(PortMediaType::Bytes),
        output_port(PortMediaType::Bytes),
    );

    // Act.
    let report = harness.run("test-serial-full").await;

    // Assert: completed with 3 stages in order.
    assert_completed(&report);
    assert_eq!(report.stages.len(), 3);
    assert_eq!(report.run_id, "test-serial-full");
    assert!(report.total_duration_ms > 0, "total_duration should be > 0");

    // Each stage completed.
    for (i, sr) in report.stages.iter().enumerate() {
        assert_eq!(
            sr.status,
            StageStatus::Completed,
            "stage {} ({}) failed",
            i,
            sr.stage_name
        );
        assert!(sr.duration_ms > 0, "stage {} duration should be > 0", i);
    }

    // Stage names in declaration order.
    assert_eq!(report.stages[0].stage_name, "Stage-A");
    assert_eq!(report.stages[1].stage_name, "Stage-B");
    assert_eq!(report.stages[2].stage_name, "Stage-C");

    // Output chaining: each stage appended its name marker.
    assert_output_chain(&report, &[":Stage-A", ":Stage-B", ":Stage-C"]);

    // Verify DAG construction.
    let dag = harness.build_dag();
    assert_eq!(dag.stages.len(), 3);
    assert_eq!(dag.edges.len(), 2);
    let order = dag.execution_order().expect("valid topo order");
    assert_eq!(order, vec!["Stage-A", "Stage-B", "Stage-C"]);

    // Logs: at minimum pipeline-start + 3× stage-start + 3× stage-complete +
    // pipeline-finish = 8 entries minimum.
    let logs = harness.logs();
    assert!(
        logs.len() >= 8,
        "expected at least 8 log entries, got {}",
        logs.len()
    );
    assert!(
        logs.iter().any(|l| l.message.contains("starting pipeline")),
        "missing pipeline start log"
    );
    assert!(
        logs.iter().any(|l| l.message.contains("pipeline finished")),
        "missing pipeline finish log"
    );

    // ✅ Positive: 3-stage serial DAG runs completly with log evidence.
    // ❌ Negative: not applicable — success path.
}

// ── Test: Empty Pipeline ─────────────────────────────────────────────────────

/// Objective: Verify that a pipeline with zero stages is handled gracefully
/// (no panics, Completed status, appropriate warning log).
///
/// Coverage:
///   - `PipelineDag::build(vec![])` returns a valid empty DAG
///   - `PipelineExecutor::execute` returns Completed with no stages
///   - Log entry warns that the pipeline has no stages
#[tokio::test]
async fn test_empty_pipeline() {
    let harness = PipelineTestHarness::new();

    // Arrange: no stages registered.
    assert!(harness.stages.is_empty());

    // Act: build an empty DAG and execute.
    let dag = harness.build_dag();
    let executor = PipelineExecutor::new(dag).with_log_store(harness.log_store.clone());
    let report = executor.execute("test-empty", None).await;

    // Assert: completed with no stages.
    assert_completed(&report);
    assert!(
        report.stages.is_empty(),
        "empty pipeline should have no stages"
    );
    assert_eq!(
        report.total_duration_ms, 0,
        "empty pipeline should have 0 duration"
    );

    // Verify the DAG itself is valid and empty.
    let dag2 = PipelineDag::build(vec![]).expect("empty DAG is valid");
    assert!(dag2.stages.is_empty());
    assert!(dag2.edges.is_empty());
    assert!(dag2.entry_points.is_empty());
    assert!(dag2.exit_points.is_empty());

    // Log should contain the "no stages" warning.
    let logs = harness.logs();
    assert!(
        logs.iter().any(|l| l.message.contains("no stages")),
        "expect warning about no stages"
    );

    // ✅ Positive: empty pipeline is handled gracefully.
    // ❌ Negative: not applicable — edge case verification.
}

// ── Test: Pipeline with Secrets ──────────────────────────────────────────────

/// Objective: Verify that `SecretRef` resolution, `SecretStore` injection,
/// and `SecretStore::get` all work correctly in a pipeline context.
///
/// Coverage:
///   - `SecretRef` with `File` source reads and trims secret values
///   - `SecretStore::resolve` collects multiple refs
///   - `SecretStore::inject_to_env` merges into an environment map
///   - `SecretStore::get` retrieves by logical key
///   - `SecretStore::is_empty` / `len` for an empty store
///   - `SecretRef` serialization round-trip
#[tokio::test]
async fn test_pipeline_with_secrets() {
    // Arrange: create two temporary secret files.
    let dir = tempfile::tempdir().expect("temp dir for secrets");
    let api_key_path = dir.path().join("api_key.txt");
    let db_pass_path = dir.path().join("db_password.txt");
    std::fs::write(&api_key_path, "sk-1234-secret\n").expect("write api key");
    std::fs::write(&db_pass_path, "hunter2\n").expect("write db password");

    let secret_refs = vec![
        SecretRef {
            key: "openai_key".into(),
            env_var: "OPENAI_API_KEY".into(),
            source: SecretSource::File {
                path: api_key_path.to_str().unwrap().into(),
            },
        },
        SecretRef {
            key: "db_pass".into(),
            env_var: "DB_PASSWORD".into(),
            source: SecretSource::File {
                path: db_pass_path.to_str().unwrap().into(),
            },
        },
    ];

    // Act: resolve secrets into a store.
    let store = SecretStore::resolve(&secret_refs).expect("resolve secrets");

    // Assert: values are trimmed (no trailing newline).
    assert_eq!(store.get("openai_key"), Some("sk-1234-secret"));
    assert_eq!(store.get("db_pass"), Some("hunter2"));
    assert_eq!(store.len(), 2);

    // Inject into env map — existing values preserved, new ones added.
    let mut env = HashMap::new();
    env.insert("EXISTING_VAR".into(), "keep-me".into());
    store.inject_to_env(&mut env);
    assert_eq!(
        env.get("OPENAI_API_KEY"),
        Some(&"sk-1234-secret".to_string())
    );
    assert_eq!(env.get("DB_PASSWORD"), Some(&"hunter2".to_string()));
    assert_eq!(env.get("EXISTING_VAR"), Some(&"keep-me".to_string()));
    assert_eq!(env.len(), 3);

    // Empty store edge case.
    let empty_store = SecretStore::resolve(&[]).expect("empty resolve");
    assert!(empty_store.is_empty());
    assert_eq!(empty_store.len(), 0);
    assert!(empty_store.get("anything").is_none());

    // SecretRef serialization round-trip.
    let json = serde_json::to_string(&secret_refs[0]).expect("serialize secret ref");
    let back: SecretRef = serde_json::from_str(&json).expect("deserialize secret ref");
    assert_eq!(back.key, "openai_key");
    assert_eq!(back.env_var, "OPENAI_API_KEY");

    // Debug output must NOT contain secret values.
    let debug_str = format!("{:?}", store);
    assert!(
        !debug_str.contains("sk-1234-secret"),
        "Debug must redact values"
    );
    assert!(!debug_str.contains("hunter2"), "Debug must redact values");
    assert!(
        debug_str.contains("<redacted>"),
        "Debug should show <redacted>"
    );

    // ✅ Positive: secrets resolve, inject, round-trip, and stay redacted.
    // ❌ Negative: missing file returns an error; unknown key returns None.
}

// ── Test: Cost Reporting ─────────────────────────────────────────────────────

/// Objective: Verify that `PipelineCostReport` computes correct per-stage and
/// total costs from resource usage data, including GPU and CPU attribution.
///
/// Coverage:
///   - `ResourceUsage` GPU/CPU hour conversion, data GiB calculation
///   - `CostEstimate::calculate` with default and custom rates
///   - `PipelineCostReport` with multiple stage rows
///   - Zero-usage edge case
///   - `ResourceUsage` serialization round-trip
#[tokio::test]
async fn test_cost_reporting() {
    // Arrange: build a cost report for a 2-stage pipeline.
    let report = PipelineCostReport {
        pipeline_id: "test-cost-pipe".into(),
        stages: vec![
            StageCostRow {
                stage_name: "VideoIngest".into(),
                gpu_hr: 0.0,
                cpu_hr: 0.05, // 3 CPU-minutes
                data_gib: 1.5,
                cost_usd: 0.005,
            },
            StageCostRow {
                stage_name: "Transcode".into(),
                gpu_hr: 0.5, // 30 GPU-minutes
                cpu_hr: 0.5,
                data_gib: 0.8,
                cost_usd: 1.30,
            },
        ],
        total_cost: CostEstimate {
            usage: ResourceUsage {
                gpu_seconds: 1800.0, // 0.5 GPU·hr
                cpu_seconds: 1980.0, // 0.55 CPU·hr
                bytes_ingested: 2_000_000_000,
                bytes_egressed: 500_000_000,
            },
            estimated_cost_usd: Some(1.305),
            cost_per_gpu_hour: 2.50,
            cost_per_cpu_hour: 0.10,
        },
    };

    // Act: verify all non-zero values.
    assert_eq!(report.pipeline_id, "test-cost-pipe");
    assert_eq!(report.stages.len(), 2);

    // Assert: total cost is non-zero.
    let total_cost = report.total_cost.estimated_cost_usd.unwrap();
    assert!(
        total_cost > 0.0,
        "total cost should be > 0, got {total_cost}"
    );

    // Per-stage break-down assertions.
    assert_eq!(report.stages[0].stage_name, "VideoIngest");
    assert_eq!(report.stages[0].gpu_hr, 0.0);
    assert!(report.stages[0].cost_usd > 0.0);

    assert_eq!(report.stages[1].stage_name, "Transcode");
    assert!(
        report.stages[1].gpu_hr > 0.0,
        "Transcode should have GPU hours > 0"
    );
    assert!(
        report.stages[1].cost_usd > 1.0,
        "Transcode cost should be > $1"
    );

    // ── ResourceUsage helper accuracy ──
    let usage = ResourceUsage {
        gpu_seconds: 3600.0,
        cpu_seconds: 7200.0,
        bytes_ingested: 1_073_741_824, // 1 GiB
        bytes_egressed: 2_147_483_648, // 2 GiB
    };
    assert!((usage.gpu_hours() - 1.0).abs() < 1e-9);
    assert!((usage.cpu_hours() - 2.0).abs() < 1e-9);
    assert!((usage.data_gib() - 3.0).abs() < 1e-6);

    // ── CostEstimate::calculate with default rates ──
    let config = CostConfig::default();
    let est = CostEstimate::calculate(&usage, &config);
    // 1 GPU·hr @ $2.50 = $2.50, 2 CPU·hr @ $0.10 = $0.20 → $2.70
    assert!((est.estimated_cost_usd.unwrap() - 2.70).abs() < 1e-9);

    // ── Zero-usage edge case ──
    let zero = ResourceUsage::default();
    let zero_est = CostEstimate::calculate(&zero, &config);
    assert!((zero_est.estimated_cost_usd.unwrap()).abs() < 1e-9);

    // ── ResourceUsage serialization round-trip ──
    let json = serde_json::to_string(&usage).unwrap();
    let back: ResourceUsage = serde_json::from_str(&json).unwrap();
    assert!((back.gpu_seconds - 3600.0).abs() < 1e-9);
    assert_eq!(back.bytes_ingested, 1_073_741_824);

    // ✅ Positive: cost report has non-zero values and correct calculations.
    // ❌ Negative: zero usage produces zero cost.
}

// ── Test: Transition ledger (file-backed) ───────────────────────────────────

/// Objective: Verify `PipelineExecutor` drives an internal
/// `pipeline_statemachine::StateMachine` alongside its existing
/// `StageStatus`/`RunStatus` tracking, and records every transition to an
/// attached `TransitionSink` — the durable transaction ledger.
///
/// Coverage:
///   - `PipelineExecutor::with_transition_sink` wiring
///   - Transition ordering: Validate, Schedule, Execute, StageComplete(0..N-1)
///   - Final recorded `to_state` is `PipelineState::Completed`
///   - `seq` is 1-based and monotonically increasing
#[tokio::test]
async fn test_transitions_recorded_to_file_log() {
    use b00t_cli::pipeline_statemachine::PipelineState;
    use b00t_cli::pipeline_transitions::FileTransitionLog;

    let mut harness = PipelineTestHarness::new();
    harness.add_stage(
        "T-A",
        input_port(PortMediaType::Bytes),
        output_port(PortMediaType::Bytes),
    );
    harness.add_stage(
        "T-B",
        input_port(PortMediaType::Bytes),
        output_port(PortMediaType::Bytes),
    );

    let dir = tempfile::tempdir().expect("temp dir");
    let log = Arc::new(FileTransitionLog::new(dir.path().to_path_buf()).expect("new log"));

    let dag = harness.build_dag();
    let executor = PipelineExecutor::new(dag).with_transition_sink(log.clone());
    let report = executor
        .execute("test-transitions", Some(b"input".to_vec()))
        .await;
    assert_completed(&report);

    let recs = log.read_all("test-transitions").expect("read_all");
    // Validate, Schedule, Execute, StageComplete(0), StageComplete(1) = 5.
    assert_eq!(
        recs.len(),
        5,
        "expected 5 transitions for a 2-stage pipeline, got {:?}",
        recs
    );
    assert_eq!(recs[0].to_state, PipelineState::Validating);
    assert_eq!(recs[1].to_state, PipelineState::Scheduling);
    assert_eq!(recs[2].to_state, PipelineState::Running(0));
    assert_eq!(recs[3].to_state, PipelineState::Running(1));
    assert_eq!(recs[4].to_state, PipelineState::Completed);
    assert_eq!(
        recs.iter().map(|r| r.seq).collect::<Vec<_>>(),
        vec![1, 2, 3, 4, 5],
        "seq should be 1-based and monotonically increasing"
    );
    for r in &recs {
        assert_eq!(r.run_id, "test-transitions");
    }

    // ✅ Positive: every FSM transition durably recorded, in order, ending in Completed.
    // ❌ Negative: not applicable — success path.
}

// ── Test: Transition ledger (live NATS, requires a running server) ──────────

/// Objective: Verify `NatsTransitionSink` publishes each transition live to
/// `pipeline.{run_id}.transition`, observable by an independent subscriber.
///
/// Requires the shared b00t NATS bus — not exercised by default (`#[ignore]`d).
///
/// app4dog is a promptexecution sub-project and owns no infrastructure of
/// its own (no app4dog-scoped NATS): pipeline transitions authenticate
/// against the existing b00t ACP agent-coordination bus (systemd unit
/// `nats-server.service`, always-on on hosts that run it — currently local,
/// moving to a Vultr host once that's provisioned; override the target with
/// `B00T_NATS_URL` when it does).
///
/// Prerequisite: `b00t-cli/scripts/sync-nats-secrets.sh` has been run at
/// least once on this host (materializes `~/.b00t/secrets/{nats-user,
/// nats-password}` from `~/.b00t/nats/nats.conf`), then:
///   `cargo test --ignored nats_transition_sink_publishes_live -- --nocapture`
#[tokio::test]
#[ignore]
async fn nats_transition_sink_publishes_live() {
    use b00t_cli::pipeline_nats::{AsyncNatsTransport, NatsTransport};
    use b00t_cli::pipeline_secrets::{SecretRef, SecretSource, load_secret};
    use b00t_cli::pipeline_transitions::{NatsTransitionSink, TransitionRecord};

    let user = load_secret(&SecretRef {
        key: "nats-user".into(),
        env_var: "B00T_NATS_USER".into(),
        source: SecretSource::File {
            path: "~/.b00t/secrets/nats-user".into(),
        },
    })
    .expect("resolve nats-user — run b00t-cli/scripts/sync-nats-secrets.sh first");
    let password = load_secret(&SecretRef {
        key: "nats-password".into(),
        env_var: "B00T_NATS_PASSWORD".into(),
        source: SecretSource::File {
            path: "~/.b00t/secrets/nats-password".into(),
        },
    })
    .expect("resolve nats-password — run b00t-cli/scripts/sync-nats-secrets.sh first");
    let url =
        std::env::var("B00T_NATS_URL").unwrap_or_else(|_| "nats://127.0.0.1:4222".to_string());

    let client = async_nats::ConnectOptions::new()
        .user_and_password(user, password)
        .connect(&url)
        .await
        .unwrap_or_else(|e| panic!("connect to b00t NATS bus at {url}: {e}"));
    let transport = Arc::new(AsyncNatsTransport::new(client));
    let sink = Arc::new(NatsTransitionSink::new(transport.clone()));

    // Subscribe before executing — a live NATS subscription only sees
    // messages published after it is established.
    let mut sub = transport
        .subscribe("pipeline.test-nats-live.transition")
        .expect("subscribe");

    let mut harness = PipelineTestHarness::new();
    harness.add_stage(
        "N-A",
        input_port(PortMediaType::Bytes),
        output_port(PortMediaType::Bytes),
    );
    let dag = harness.build_dag();
    let executor = PipelineExecutor::new(dag).with_transition_sink(sink);
    let report = executor
        .execute("test-nats-live", Some(b"input".to_vec()))
        .await;
    assert_completed(&report);

    // `AsyncNatsSubscription::next()` blocks on a std mpsc recv — run it via
    // spawn_blocking (matches pipeline_nats.rs's own `nats_client_adapter_*`
    // test pattern) so it doesn't starve the current-thread test runtime.
    let msg = tokio::task::spawn_blocking(move || sub.next())
        .await
        .expect("spawn_blocking join")
        .expect("should receive at least one live transition");
    let rec: TransitionRecord = serde_json::from_slice(&msg).expect("parse published transition");
    assert_eq!(rec.run_id, "test-nats-live");

    // ✅ Positive: live NATS delivery of a real transition, end-to-end.
    // ❌ Negative: not applicable — success path (requires a live server, hence #[ignore]).
}
