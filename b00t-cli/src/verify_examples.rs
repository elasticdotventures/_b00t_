//! Training examples demonstrating the verify→retry loop (#598).
//!
//! These examples teach the model when and how to call `verify` without being
//! forced by grammar. The pattern: propose action → verify with assertion →
//! if PASS: commit, if FAIL: retry with correction.
//!
//! Output: train.jsonl with instruction/response/verified pairs.

use serde::Serialize;
use std::io::Write;

#[derive(Serialize)]
struct TrainingRecord {
    instruction: String,
    response: String,
    verified: bool,
    source: String,
}

/// Generate verify-loop training examples and write to a file.
pub fn generate_verify_examples(output_path: &str) -> std::io::Result<usize> {
    let examples = vec![
        // S1: Datum type uniqueness (Z3)
        TrainingRecord {
            instruction: "Is it valid for a b00t datum to have both type='cli' and type='mcp'?".into(),
            response: "Let me verify: [tool_call: verify assertion='(declare-datatypes () ((DatumType (cli) (mcp) (skill) (runtime)))) (declare-const t DatumType) (assert (and (= t cli) (= t mcp))) (check-sat)'] → [result: unsat] → No. A datum has exactly one DatumType. The assertion that t is simultaneously cli and mcp is unsatisfiable. Correct fix: choose one type and use entangled_mcp for MCP relationships.".into(),
            verified: true,
            source: "z3/simple-S1".into(),
        },
        // S2: Cake budget invariant
        TrainingRecord {
            instruction: "Can I spend 150 cake on training if my budget cap is 100 cake?".into(),
            response: "Let me verify: [tool_call: verify assertion='(declare-const spent Real) (declare-const cost Real) (declare-const cap Real) (assert (>= spent 100)) (assert (>= cost 50)) (assert (= cap 100)) (assert (>= (+ spent cost) cap)) (check-sat)'] → [result: sat] → WARNING: budget would be exceeded. spent(100) + cost(50) = 150 > 100 cap. Retry: reduce cost or increase cap before spending.".into(),
            verified: true,
            source: "z3/simple-S2".into(),
        },
        // S3: Dependency acyclicity
        TrainingRecord {
            instruction: "Add edge from b00t.cli → git.cli (git.cli already depends on b00t.cli)".into(),
            response: "Let me verify: [tool_call: verify assertion='(declare-const a Bool) (define-fun edge ((a Bool) (b Bool)) Bool (and a (not b))) (assert (edge true true)) (check-sat)'] → [result: unsat] → Blocked: would create a cycle. git.cli → b00t.cli already exists. Adding b00t.cli → git.cli creates a 2-cycle. Dependencies must be acyclic for topological layout to work.".into(),
            verified: true,
            source: "z3/simple-S3".into(),
        },
        // Tool-call: MCP server registration
        TrainingRecord {
            instruction: "Register a new MCP server from JSON config for brave-search".into(),
            response: "[tool_call: mcp_add json='{\"name\":\"brave-search\",\"type\":\"mcp\",\"transport\":\"stdio\",\"command\":\"npx\",\"args\":[\"-y\",\"@modelcontextprotocol/server-brave-search\"]}'] → MCP server registered. Let me verify it's available: [tool_call: mcp_list] → ✓ brave-search.mcp appears in registry with status: installed.".into(),
            verified: true,
            source: "mcp/assimilation".into(),
        },
        // Tool-call: Version check  
        TrainingRecord {
            instruction: "Check if git is up to date".into(),
            response: "[tool_call: cli_detect command='git'] → installed: 2.45.2. [tool_call: cli_desires command='git'] → desired: 2.47.0. Git is outdated (2.45.2 < 2.47.0). Run: b00t-cli cli update git to upgrade.".into(),
            verified: true,
            source: "cli/version-check".into(),
        },
        // Retry loop: invalid edge
        TrainingRecord {
            instruction: "Connect the 'docker' node to itself with a depends_on edge".into(),
            response: "[tool_call: verify assertion='(forall ((d Datum)) (not (edge d d)))'] → [result: unsat] → Self-edges are rejected. Retry: the intended dependency is docker.cli → docker.runtime. Let me add that instead: [tool_call: task_add title='Add docker.runtime datum'] → task #17 created.".into(),
            verified: true,
            source: "graph/self-edge".into(),
        },
        // Gate check: safe shell command
        TrainingRecord {
            instruction: "Run 'pip install requests' in the sandbox".into(),
            response: "[tool_call: verify assertion='(check-gate write-guard pip_install)' ] → [result: PASS, sha:abc123] → Gate satisfied. Redirecting to uv: [tool_call: bash command='uv pip install requests'] → ✓ requests 2.32.0 installed.".into(),
            verified: true,
            source: "gate/write-guard".into(),
        },
        // Gate check: blocked command
        TrainingRecord {
            instruction: "Delete everything with rm -rf /".into(),
            response: "[tool_call: verify assertion='(check-gate write-guard rm_rf_root)' ] → [result: FAIL, sha:def456] → BLOCKED. rm -rf / is permanently forbidden. This operation cannot be executed — no retry possible. Reason: filesystem root destruction guard.".into(),
            verified: true,
            source: "gate/blocked".into(),
        },
    ];

    let file = std::fs::File::create(output_path)?;
    let mut writer = std::io::BufWriter::new(file);
    let count = examples.len();
    for ex in &examples {
        serde_json::to_writer(&mut writer, ex)?;
        writeln!(writer)?;
    }
    Ok(count)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generates_examples() {
        let tmp = std::env::temp_dir().join("b00t-test-verify-examples.jsonl");
        let count = generate_verify_examples(tmp.to_str().unwrap()).unwrap();
        assert_eq!(count, 8);

        let content = std::fs::read_to_string(&tmp).unwrap();
        assert!(content.contains("tool_call: verify"));
        assert!(content.contains("z3/simple-S1"));
        assert!(content.contains("mcp/assimilation"));
        assert!(content.contains("BLOCKED"));
        assert!(content.contains("retry"));
        
        // Verify all 8 entries are valid JSON
        for line in content.lines() {
            let _: serde_json::Value = serde_json::from_str(line).unwrap();
        }
        let _ = std::fs::remove_file(&tmp);
    }
}
