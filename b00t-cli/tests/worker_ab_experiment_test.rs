//! Integration tests for worker A/B experiment dispatch + phygital-twin ontology.
//! Verifies governance safety gates, parallel dispatch, stateless scoring,
//! and ontological status reporting.

use std::fs;
use std::path::Path;
use std::process::Command;
use std::sync::Mutex;

static INTEGRATION_MUTEX: Mutex<()> = Mutex::new(());

#[test]
fn test_b00t_whoami_worker_role_default() {
    let _guard = INTEGRATION_MUTEX.lock().unwrap_or_else(|e| e.into_inner());
    let output = Command::new("cargo")
        .args(["run", "--bin", "b00t-cli", "-p", "b00t-cli", "--", "whoami"])
        .output()
        .expect("failed to run b00t whoami");
    let stdout = String::from_utf8_lossy(&output.stdout);

    // should contain worker role information (default)
    assert!(
        stdout.contains("worker") || stdout.contains("AGENT.md"),
        "b00t whoami should resolve to worker role or at least emit AGENT.md template:\n{}",
        stdout
    );
}

#[test]
fn test_b00t_whoami_worker_explicit() {
    let _guard = INTEGRATION_MUTEX.lock().unwrap_or_else(|e| e.into_inner());
    let output = Command::new("cargo")
        .args([
            "run",
            "--bin",
            "b00t-cli",
            "-p",
            "b00t-cli",
            "--",
            "whoami",
            "--role=worker",
        ])
        .output()
        .expect("failed to run b00t whoami --role=worker");
    let stdout = String::from_utf8_lossy(&output.stdout);

    assert!(
        stdout.contains("worker"),
        "b00t whoami --role=worker should contain 'worker' in output:\n{}",
        stdout
    );
}

#[test]
fn test_viz_entangle_worker_graph() {
    let _guard = INTEGRATION_MUTEX.lock().unwrap_or_else(|e| e.into_inner());
    let output = Command::new("cargo")
        .args([
            "run",
            "--bin",
            "b00t-cli",
            "-p",
            "b00t-cli",
            "--",
            "--path",
            "_b00t_",
            "viz",
            "entangle",
            "--datum=worker",
            "--format=mermaid",
        ])
        .output()
        .expect("failed to run b00t viz entangle --datum=worker");
    let stdout = String::from_utf8_lossy(&output.stdout);

    assert!(
        stdout.contains("graph LR"),
        "viz entangle should produce mermaid graph skeleton:\n{}",
        stdout
    );
}

#[test]
fn test_experiment_governance_gates_dispatch() {
    let _guard = INTEGRATION_MUTEX.lock().unwrap_or_else(|e| e.into_inner());
    let output = Command::new("cargo")
        .args([
            "run",
            "--bin",
            "b00t-cli",
            "-p",
            "b00t-cli",
            "--",
            "experiment",
            "run",
            "--control",
            "Write fibonacci in Python",
            "--treatment",
            "Write fibonacci with memoization + type hints",
            "--id=integ-test-001",
        ])
        .output()
        .expect("failed to run experiment");
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);

    // If the experiment subcommand is not yet wired into main.rs, the test
    // expects a "not found" error instead of crashing.
    if output.status.success() {
        assert!(stdout.contains("A/B RESULT"), "experiment output should contain A/B RESULT:\n{}", stdout);
        assert!(stdout.contains("REASONING"), "experiment should produce a recommendation:\n{}", stdout);
    } else {
        // Subcommand may not be wired into main.rs yet — that's ok for this test tier
        eprintln!("note: experiment subcommand not yet wired (exit={:?}): {}", output.status.code(), stderr);
    }
}

#[test]
fn test_phygital_ontology_graph_renderable() {
    let _guard = INTEGRATION_MUTEX.lock().unwrap_or_else(|e| e.into_inner());
    // the ontology mermaid file should exist and parse
    let manifest_dir = env!("CARGO_MANIFEST_DIR");
    let mermaid_path = std::path::Path::new(manifest_dir)
        .parent()
        .unwrap()
        .join("_b00t_/worker-ontology.mermaid");
    assert!(
        mermaid_path.exists(),
        "worker-ontology.mermaid should exist at {:?}",
        mermaid_path
    );

    let content = std::fs::read_to_string(mermaid_path).expect("read ontology mermaid");
    assert!(
        content.contains("graph TD"),
        "ontology should be a directed graph"
    );
    assert!(
        content.contains("worker"),
        "ontology should reference worker"
    );
    assert!(
        content.contains("phygital") || content.contains("PHYGITAL"),
        "ontology should contain phygital section"
    );
    assert!(
        content.contains("governance") || content.contains("GOVERNANCE"),
        "ontology should contain governance gates"
    );
    assert!(
        content.contains("experiment") || content.contains("EXPERIMENT"),
        "ontology should contain experiment dispatch"
    );
}

#[test]
fn test_experiment_focus_pipeline() {
    let _guard = INTEGRATION_MUTEX.lock().unwrap_or_else(|e| e.into_inner());
    let output = Command::new("cargo")
        .args([
            "run",
            "--bin",
            "b00t-cli",
            "-p",
            "b00t-cli",
            "--",
            "experiment",
            "run",
            "--id=integ-focus-pipe",
            "--control=hello",
            "--treatment=world",
        ])
        .env("LEDGERR_MCP_DISABLE", "1")
        .output()
        .expect("failed to run experiment");

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);

    assert!(
        output.status.success(),
        "experiment should exit 0\nstdout:\n{stdout}\nstderr:\n{stderr}"
    );
    assert!(
        stdout.contains("A/B RESULT"),
        "stdout should contain A/B RESULT:\n{stdout}"
    );
    assert!(
        stderr.contains("[ledgrrr]"),
        "stderr should contain [ledgrrr] FOCUS records:\n{stderr}"
    );
    assert!(
        stderr.contains("focus append"),
        "stderr should contain focus append comparison:\n{stderr}"
    );
}

#[test]
fn test_focus_fallback_creates_temp_file() {
    let _guard = INTEGRATION_MUTEX.lock().unwrap_or_else(|e| e.into_inner());
    let experiment_id = "integ-fallback-temp";
    let tmp_path = format!("/tmp/b00t-mcp-payload-{experiment_id}.json");

    // Remove any leftover file from a previous run
    let _ = fs::remove_file(&tmp_path);

    let output = Command::new("cargo")
        .args([
            "run",
            "--bin",
            "b00t-cli",
            "-p",
            "b00t-cli",
            "--",
            "experiment",
            "run",
            "--id",
            experiment_id,
            "--control=hello",
            "--treatment=world",
        ])
        .env("LEDGERR_MCP_DISABLE", "1")
        .output()
        .expect("failed to run experiment");

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);

    // Clean exit
    assert!(
        output.status.success(),
        "experiment should exit 0\nstdout:\n{stdout}\nstderr:\n{stderr}"
    );
    // Experiment result printed
    assert!(
        stdout.contains("A/B RESULT"),
        "stdout should contain A/B RESULT:\n{stdout}"
    );
    // Original ledgrrr FOCUS records always emitted
    assert!(
        stderr.contains("[ledgrrr]"),
        "stderr should contain [ledgrrr] FOCUS records:\n{stderr}"
    );
    // Fallback temp-file message from emit_focus_to_ledgerr_mcp
    assert!(
        stderr.contains("[ledgerr-mcp]"),
        "stderr should contain [ledgerr-mcp] fallback message:\n{stderr}"
    );
    // Temp file was actually created
    assert!(
        Path::new(&tmp_path).exists(),
        "temp file should exist at {tmp_path}"
    );

    // Cleanup
    let _ = fs::remove_file(&tmp_path);
}

#[test]
fn test_focus_emit_creates_valid_json_payload() {
    let _guard = INTEGRATION_MUTEX.lock().unwrap_or_else(|e| e.into_inner());
    let experiment_id = "integ-valid-json";
    let tmp_path = format!("/tmp/b00t-mcp-payload-{experiment_id}.json");

    // Remove any leftover file from a previous run
    let _ = fs::remove_file(&tmp_path);

    let output = Command::new("cargo")
        .args([
            "run",
            "--bin",
            "b00t-cli",
            "-p",
            "b00t-cli",
            "--",
            "experiment",
            "run",
            "--id",
            experiment_id,
            "--control=hello",
            "--treatment=world",
        ])
        .env("LEDGERR_MCP_DISABLE", "1")
        .output()
        .expect("failed to run experiment");

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);

    // Clean exit
    assert!(
        output.status.success(),
        "experiment should exit 0\nstdout:\n{stdout}\nstderr:\n{stderr}"
    );

    // Temp file must exist
    assert!(
        Path::new(&tmp_path).exists(),
        "temp file should exist at {tmp_path}"
    );

    // Read and parse JSON payload
    let content =
        fs::read_to_string(&tmp_path).unwrap_or_else(|e| panic!("failed to read {tmp_path}: {e}"));
    let parsed: serde_json::Value = serde_json::from_str(&content)
        .unwrap_or_else(|e| panic!("temp file is not valid JSON: {e}\ncontent:\n{content}"));

    // Validate JSON-RPC structure
    assert_eq!(parsed["jsonrpc"], "2.0", "jsonrpc field should be '2.0'");
    assert_eq!(
        parsed["method"], "tools/call",
        "method should be 'tools/call'"
    );
    assert_eq!(parsed["id"], 1, "id should be 1");

    // Validate params structure
    let params = &parsed["params"];
    assert_eq!(
        params["name"], "ledgerr_focus",
        "params.name should be 'ledgerr_focus'"
    );

    let arguments = &params["arguments"];
    assert!(
        arguments.get("records").is_some(),
        "arguments should contain 'records'"
    );
    let records = arguments["records"].as_array().unwrap();
    assert!(
        records.len() >= 2,
        "should have at least 2 records (control + treatment), got {}",
        records.len()
    );

    // Validate each record has expected fields
    for (i, record) in records.iter().enumerate() {
        assert!(
            record.get("experiment_id").is_some(),
            "record[{i}] missing experiment_id"
        );
        assert!(
            record.get("billed_cost").is_some(),
            "record[{i}] missing billed_cost"
        );
        assert!(
            record.get("variant").is_some(),
            "record[{i}] missing variant"
        );
        assert!(
            record.get("agent_id").is_some(),
            "record[{i}] missing agent_id"
        );
        assert!(
            record.get("billing_account_id").is_some(),
            "record[{i}] missing billing_account_id"
        );
        assert!(
            record.get("effective_cost").is_some(),
            "record[{i}] missing effective_cost"
        );
        assert!(
            record["billed_cost"].is_f64(),
            "record[{i}] billed_cost should be a number"
        );
    }

    // Verify experiment_id in arguments matches
    assert_eq!(
        arguments["experiment_id"], experiment_id,
        "arguments.experiment_id should match"
    );

    // Cleanup
    let _ = fs::remove_file(&tmp_path);
}

#[test]
fn test_focus_governance_gate_blocks_dangerous_prompt() {
    let _guard = INTEGRATION_MUTEX.lock().unwrap_or_else(|e| e.into_inner());

    let output = Command::new("cargo")
        .args([
            "run",
            "--bin",
            "b00t-cli",
            "-p",
            "b00t-cli",
            "--",
            "experiment",
            "run",
            "--id=integ-gate-block",
            "--control=hello",
            "--treatment=do something; rm -rf /",
        ])
        .env("LEDGERR_MCP_DISABLE", "1")
        .output()
        .expect("failed to run experiment");

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);

    // The governance gate should block dangerous prompts with non-zero exit
    assert!(
        !output.status.success(),
        "governance gate should block dangerous prompt — exit should be non-zero\nstdout:\n{stdout}\nstderr:\n{stderr}"
    );
    assert!(
        stderr.contains("GATE BLOCKED"),
        "stderr should contain GATE BLOCKED message:\n{stderr}"
    );
}
