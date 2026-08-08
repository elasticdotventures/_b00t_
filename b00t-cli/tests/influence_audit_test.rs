//! Integration tests for `b00t influence log/trail/stats` (#691).
//!
//! Exercises the full producer -> reader loop: `record_satisfies_with_influence`
//! (the producer, wired into `evidence record --influence`) writes an
//! `EvidenceRecord` with populated `influence`, and the new `influence`
//! module (log/trail/stats) surfaces it. Hermetic via the `B00T_EVIDENCE_LOG_PATH`
//! env override added to `commands::evidence::evidence_log_path()`.

use b00t_cli::commands::evidence::{
    EvidenceRecord, InfluenceWeight, read_evidence, record_satisfies_with_influence,
};
use b00t_cli::commands::influence::{compute_stats, filter_log, influence_records, trail};
use std::sync::Mutex;
use tempfile::TempDir;

// Serialize tests that touch B00T_EVIDENCE_LOG_PATH — env vars are process-global,
// and cargo runs #[test] fns within one integration-test binary concurrently.
static ENV_LOCK: Mutex<()> = Mutex::new(());

/// Point the evidence log at a fresh temp file for the duration of `f`.
fn with_temp_evidence_log<F: FnOnce()>(f: F) {
    let _guard = ENV_LOCK.lock().unwrap();
    let tmp = TempDir::new().unwrap();
    let log_path = tmp.path().join("satisfies.jsonl");
    unsafe {
        std::env::set_var("B00T_EVIDENCE_LOG_PATH", &log_path);
    }
    f();
    unsafe {
        std::env::remove_var("B00T_EVIDENCE_LOG_PATH");
    }
}

/// Manually append a synthetic influence-bearing EvidenceRecord (bypasses the
/// producer path — used to build fixtures spanning multiple agents/sources).
fn append_synthetic(
    subject: &str,
    agent_id: &str,
    timestamp: &str,
    sources: &[(&str, f64, f64)], // (source_key, ratio, score)
) {
    use b00t_cli::commands::evidence::append_evidence;
    let record = EvidenceRecord {
        subject: subject.to_string(),
        predicate: "satisfies".to_string(),
        object: serde_json::Value::String("requires:role:backend".to_string()),
        timestamp: timestamp.to_string(),
        agent_id: Some(agent_id.to_string()),
        influence: Some(
            sources
                .iter()
                .map(|(key, ratio, score)| InfluenceWeight {
                    source_key: key.to_string(),
                    ratio: *ratio,
                    score: *score,
                })
                .collect(),
        ),
    };
    append_evidence(&record).unwrap();
}

#[test]
fn producer_path_writes_influence_and_log_surfaces_it() {
    with_temp_evidence_log(|| {
        record_satisfies_with_influence(
            "rust.skill",
            "requires:role:backend",
            &[("grok-doc-1".to_string(), 4.0), ("lfmf-lesson-9".to_string(), 1.0)],
            Some("agent-alpha"),
            None,
        )
        .unwrap();

        // The producer path must land a record with populated influence in the
        // raw evidence log.
        let raw = read_evidence().unwrap();
        assert_eq!(raw.len(), 1);
        assert!(raw[0].influence.is_some());

        // `influence log` (via influence_records + filter_log) must surface it.
        let all = influence_records().unwrap();
        assert_eq!(all.len(), 1);
        let logged = filter_log(&all, None, None).unwrap();
        assert_eq!(logged.len(), 1);
        assert_eq!(logged[0].subject, "rust.skill");
        assert_eq!(logged[0].agent_id.as_deref(), Some("agent-alpha"));
        let weights = logged[0].influence.as_ref().unwrap();
        assert_eq!(weights.len(), 2);

        // Filtering to a non-matching agent must exclude it.
        let filtered_out = filter_log(&all, None, Some("agent-beta")).unwrap();
        assert!(filtered_out.is_empty());

        // Filtering to the matching agent must keep it.
        let filtered_in = filter_log(&all, None, Some("agent-alpha")).unwrap();
        assert_eq!(filtered_in.len(), 1);
    });
}

#[test]
fn influence_trail_returns_full_chain_in_order() {
    with_temp_evidence_log(|| {
        // Out-of-order writes — trail() must sort oldest-first.
        append_synthetic(
            "puppy.pipeline",
            "agent-alpha",
            "2026-03-01T00:00:00Z",
            &[("source-c", 1.0, 1.0)],
        );
        append_synthetic(
            "puppy.pipeline",
            "agent-beta",
            "2026-01-01T00:00:00Z",
            &[("source-a", 0.5, 1.0), ("source-b", 0.5, 1.0)],
        );
        append_synthetic(
            "other.fact",
            "agent-alpha",
            "2026-02-01T00:00:00Z",
            &[("source-a", 1.0, 1.0)],
        );
        append_synthetic(
            "puppy.pipeline",
            "agent-alpha",
            "2026-02-01T00:00:00Z",
            &[("source-d", 1.0, 1.0)],
        );

        let chain = trail("puppy.pipeline").unwrap();
        assert_eq!(chain.len(), 3);
        assert_eq!(chain[0].timestamp, "2026-01-01T00:00:00Z");
        assert_eq!(chain[1].timestamp, "2026-02-01T00:00:00Z");
        assert_eq!(chain[2].timestamp, "2026-03-01T00:00:00Z");
        assert!(chain.iter().all(|r| r.subject == "puppy.pipeline"));

        // First hop carries the two-source weight breakdown.
        let first_weights = chain[0].influence.as_ref().unwrap();
        assert_eq!(first_weights.len(), 2);
    });
}

#[test]
fn influence_stats_aggregates_across_agents_and_sources() {
    with_temp_evidence_log(|| {
        // 3 synthetic records spanning 2 agents with overlapping source keys.
        append_synthetic(
            "fact-1",
            "agent-alpha",
            "2026-01-01T00:00:00Z",
            &[("shared-source", 0.6, 3.0), ("only-in-1", 0.4, 2.0)],
        );
        append_synthetic(
            "fact-2",
            "agent-alpha",
            "2026-01-02T00:00:00Z",
            &[("shared-source", 1.0, 5.0)],
        );
        append_synthetic(
            "fact-3",
            "agent-beta",
            "2026-01-03T00:00:00Z",
            &[("shared-source", 0.5, 1.0), ("only-in-3", 0.5, 1.0)],
        );

        let stats = compute_stats().unwrap();
        assert_eq!(stats.total_influence_records, 3);

        // top_agents: agent-alpha has 2 records, agent-beta has 1.
        assert_eq!(stats.top_agents[0], ("agent-alpha".to_string(), 2));
        assert!(stats.top_agents.contains(&("agent-beta".to_string(), 1)));

        // top_sources: shared-source appears in all 3 records; the others once each.
        let shared = stats
            .top_sources
            .iter()
            .find(|(k, _, _)| k == "shared-source")
            .expect("shared-source present");
        assert_eq!(shared.1, 3);
        assert!((shared.2 - (0.6 + 1.0 + 0.5)).abs() < 1e-9);

        let only_in_1 = stats
            .top_sources
            .iter()
            .find(|(k, _, _)| k == "only-in-1")
            .expect("only-in-1 present");
        assert_eq!(only_in_1.1, 1);

        // shared-source must rank first (highest appearance count).
        assert_eq!(stats.top_sources[0].0, "shared-source");
    });
}

#[test]
fn influence_stats_zero_records_does_not_panic() {
    with_temp_evidence_log(|| {
        // Fresh/empty evidence log — no records written.
        let stats = compute_stats().unwrap();
        assert_eq!(stats.total_influence_records, 0);
        assert!(stats.top_agents.is_empty());
        assert!(stats.top_sources.is_empty());
        // orphan_fact_count depends on the (uncontrolled) store manifest state —
        // just assert it's computed without panicking.
        let _ = stats.orphan_fact_count;
    });
}
