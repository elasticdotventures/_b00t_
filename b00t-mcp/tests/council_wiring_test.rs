//! End-to-end check of the b00t-council wiring: agent_vote_create /
//! agent_vote_submit / agent_vote_tally going from stub NATS pings to a
//! real, durable, tallyable vote.
//!
//! NATS publish is not reachable in a sandboxed test environment, so each
//! `execute_mcp_call` (which does `record → NATS publish`) is run on its own
//! thread with a bounded join timeout: a NATS timeout/error is expected and
//! tolerated here, but the durable record it wrote *before* attempting NATS
//! is not — `agent_vote_tally` (which never touches NATS) must see it and
//! resolve a real outcome.

use b00t_mcp::mcp_tools::{AgentVoteCreateCommand, AgentVoteSubmitCommand, AgentVoteTallyCommand};
use b00t_mcp::clap_reflection::McpExecutor;
use serde_json::json;
use std::collections::HashMap;
use std::time::Duration;

fn call<T: McpExecutor>(params: HashMap<String, serde_json::Value>) -> anyhow::Result<String> {
    // Bound how long we wait for NATS (unreachable in this sandbox) so a
    // stalled connect can't hang the test suite.
    let (tx, rx) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        let _ = tx.send(T::execute_mcp_call(&params));
    });
    match rx.recv_timeout(Duration::from_secs(15)) {
        Ok(result) => result,
        Err(_) => anyhow::bail!("timed out waiting for NATS (expected without a broker)"),
    }
}

fn params(pairs: &[(&str, serde_json::Value)]) -> HashMap<String, serde_json::Value> {
    pairs
        .iter()
        .map(|(k, v)| (k.to_string(), v.clone()))
        .collect()
}

#[test]
fn vote_create_submit_tally_round_trip_is_durable_and_resolves() {
    let log_path = std::env::temp_dir().join(format!(
        "b00t-mcp-council-test-{}.jsonl",
        std::process::id()
    ));
    let _ = std::fs::remove_file(&log_path);
    // SAFETY: single-threaded test process section (no other test in this
    // binary touches B00T_MESSAGE_LOG_PATH/B00T_AGENT_ID concurrently).
    unsafe {
        std::env::set_var("B00T_MESSAGE_LOG_PATH", &log_path);
    }

    // 1. Create the proposal. NATS publish is expected to fail/timeout here
    // (no broker) — that's fine, the durable record happens first.
    let create_result = call::<AgentVoteCreateCommand>(params(&[
        ("subject", json!("Adopt b00t-council")),
        ("description", json!("Durable, generic-quorum voting")),
        ("options", json!(r#"["yes","no"]"#)),
        ("vote_type", json!("")), // -> default AtLeast(2)
        ("deadline", json!(0)),
        ("voters", json!("alice,bob")),
    ]));
    let proposal_id = match &create_result {
        Ok(msg) => msg
            .split("proposal_id=")
            .nth(1)
            .and_then(|s| s.split(')').next())
            .expect("create response should contain proposal_id=<id>")
            .to_string(),
        Err(_) => {
            // NATS unreachable — recover the id from what was actually recorded.
            let contents = std::fs::read_to_string(&log_path).expect("log should exist");
            let first: serde_json::Value =
                serde_json::from_str(contents.lines().next().expect("one record")).unwrap();
            match &first["to"] {
                serde_json::Value::Object(o) => o["Channel"].as_str().unwrap().to_string(),
                _ => panic!("expected Channel recipient"),
            }
        }
    };

    // 2. Two conflicting-then-agreeing votes from two distinct players.
    unsafe {
        std::env::set_var("B00T_AGENT_ID", "alice");
    }
    let _ = call::<AgentVoteSubmitCommand>(params(&[
        ("proposal_id", json!(proposal_id.clone())),
        ("vote", json!("yes")),
    ]));

    unsafe {
        std::env::set_var("B00T_AGENT_ID", "bob");
    }
    let _ = call::<AgentVoteSubmitCommand>(params(&[
        ("proposal_id", json!(proposal_id.clone())),
        ("vote", json!("yes")),
    ]));

    // 3. Tally never touches NATS — this must succeed unconditionally and
    // resolve a real outcome, not a canned string.
    let tally = call::<AgentVoteTallyCommand>(params(&[(
        "proposal_id",
        json!(proposal_id.clone()),
    )]))
    .expect("agent_vote_tally must not depend on NATS");

    assert!(
        tally.contains("Passed(yes)"),
        "expected the two 'yes' votes to pass under the default AtLeast(2) quorum, got: {tally}"
    );

    // 4. The log is genuinely observable: 3 envelopes (1 proposal + 2 votes),
    // each carrying real sender/player attribution.
    let contents = std::fs::read_to_string(&log_path).unwrap();
    let lines: Vec<&str> = contents.lines().collect();
    assert_eq!(lines.len(), 3, "expected proposal + 2 votes recorded, got: {contents}");
    for line in &lines {
        let env: serde_json::Value = serde_json::from_str(line).unwrap();
        assert!(env["id"].is_string());
        assert!(env["sent_at"].is_string());
        assert!(env["sender_is_player"].is_boolean());
    }

    let _ = std::fs::remove_file(&log_path);
    unsafe {
        std::env::remove_var("B00T_MESSAGE_LOG_PATH");
        std::env::remove_var("B00T_AGENT_ID");
    }
}
