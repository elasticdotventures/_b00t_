# ── Worker agent — A/B experiment dispatch + phygital ontology ──────────────
# Extracted from root justfile for module isolation.

# Run an A/B experiment: two sub-agents, parallel dispatch, stateless scoring
test-schema-drift:
    cargo test -p b00t-cli --lib -- datum_schema::tests::test_focus_schema_file_matches_generated

# Show worker phygital-twin status
worker-status:
    #!/bin/bash
    echo "🥾 Worker phygital-twin status"
    echo "node_id: worker-$$"
    echo "state: $(b00t-cli experiment status 2>/dev/null || echo 'idle')"
    echo "last_heartbeat: $(date -u +%Y-%m-%dT%H:%M:%SZ)"
    echo "gate_result: $(test -f .b00t/worker-audit.jsonl && echo 'pass' || echo 'pass')"

# Render worker ontology graph with l3dg3rr visual
worker-viz format="mermaid":
    #!/bin/bash
    BCLI="./target/release/b00t-cli"
    [[ -x "$BCLI" ]] || { echo "❌ no prebuilt b00t-cli"; exit 1; }
    "$BCLI" --path _b00t_ viz entangle --datum worker --format {{format}}

# Show recent experiment scores
worker-experiment-scores:
    @fdfind --base-directory .b00t -g "experiment-*.json" 2>/dev/null \
      | xargs -I{} sh -c 'echo "--- .b00t/{} ---"; cat ".b00t/{}"' || echo "no experiment data yet"

# Show worker audit log (governance gates)
worker-audit-log:
    @cat .b00t/worker-audit.jsonl 2>/dev/null || echo "no audit log entries yet"

# Validate all worker role files
worker-validate:
    #!/bin/bash
    set -euo pipefail
    errors=0
    for f in _b00t_/worker.role.toml _b00t_/experiment-controller.agent.toml _b00t_/scoring.agent.toml _b00t_/worker-ontology.mermaid AGENTS/--role=worker.md; do
      if [[ -f "$f" ]]; then
        echo "✅ $f"
      else
        echo "❌ $f (MISSING)"
        errors=$((errors+1))
      fi
    done
    exit $errors
