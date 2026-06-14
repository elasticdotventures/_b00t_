# b00t justfile - MCP chat integration and utilities

# Interactive MCP chat selection with fzf
mcp-chat:
    #!/usr/bin/env bash
    set -euo pipefail
    
    # Check if required tools are available
    if ! command -v jq &> /dev/null; then
        echo "Error: jq is required but not installed" >&2
        exit 1
    fi
    
    if ! command -v fzf &> /dev/null; then
        echo "Error: fzf is required but not installed" >&2
        exit 1
    fi
    
    if ! command -v bunx &> /dev/null; then
        echo "Error: bunx is required but not installed" >&2
        exit 1
    fi
    
    # Get available MCP servers and format for fzf
    selected=$(b00t-cli mcp list --json | jq -r '.servers[] | "\(.name) - \(.hint // "No description")"' | fzf --prompt="Select MCP server: " --height=40% --border)
    
    if [ -z "$selected" ]; then
        echo "No MCP server selected"
        exit 1
    fi
    
    # Extract server name from selection
    server_name=$(echo "$selected" | cut -d' ' -f1)
    
    # Get the command for the selected server
    server_command=$(b00t-cli mcp list --json | jq -r ".servers[] | select(.name == \"$server_name\") | \"\(.command) \(.args | join(\" \"))\"")
    
    if [ -z "$server_command" ]; then
        echo "Error: Could not find command for server '$server_name'" >&2
        exit 1
    fi
    
    echo "Starting mcp-chat with server: $server_name"
    echo "Command: $server_command"
    echo "---"
    
    # Run mcp-chat with the selected server
    bunx mcp-chat --server "$server_command"

# List available MCP servers in a readable format
mcp-list:
    b00t-cli mcp list

# List available MCP servers in JSON format
mcp-list-json:
    b00t-cli mcp list --json

# Pretty-print MCP servers with jq
mcp-pretty:
    b00t-cli mcp list --json | jq '.'

# Show MCP server commands only
mcp-commands:
    b00t-cli mcp list --json | jq -r '.servers[] | "\(.name): \(.command) \(.args | join(" "))"'

# Quick test of MCP functionality
mcp-test:
    echo "Testing MCP functionality..."
    b00t-cli mcp list
    echo "---"
    b00t-cli mcp list --json | jq '.servers[] | {name, command, hint}'

# Install b00t-mcp to current directory's .mcp.json
mcp-self-install:
    #!/usr/bin/env bash
    set -euo pipefail
    echo "🥾 Installing b00t-mcp to $(pwd)/.mcp.json"
    b00t-cli --path ~/.dotfiles/_b00t_/_b00t_ mcp install b00t-mcp dotmcpjson
    echo "✅ b00t-mcp installed to .mcp.json"

# Install b00t-mcp with custom stdio command (e.g., docker, npx)
mcp-self-install-with COMMAND:
    #!/usr/bin/env bash
    set -euo pipefail
    echo "🥾 Installing b00t-mcp to $(pwd)/.mcp.json with {{COMMAND}}"
    b00t-cli --path ~/.dotfiles/_b00t_/_b00t_ mcp install b00t-mcp dotmcpjson --stdio-command {{COMMAND}}
    echo "✅ b00t-mcp installed to .mcp.json"

# ─── Agent lifecycle (pi, opencode) ──────────────────────────────────────────

# pi: managed as b00t@pi-agent.service
pi-start:
    systemctl --user start b00t@pi-agent.service

pi-stop:
    systemctl --user stop b00t@pi-agent.service

pi-status:
    systemctl --user status b00t@pi-agent.service

pi-ch0nky prompt="hello":
    LLAMA_CPP_BASE_URL=http://127.0.0.1:8001/v1 \
      pi --provider llama-cpp --model ch0nky -p "{{prompt}}"

# opencode: managed as b00t@opencode-agent.service
opencode-start:
    systemctl --user start b00t@opencode-agent.service

opencode-stop:
    systemctl --user stop b00t@opencode-agent.service

opencode-status:
    systemctl --user status b00t@opencode-agent.service

opencode-task task="hello":
    opencode run --model gemma4-local/ch0nky "{{task}}"

# ch0nky slot swap (pi ↔ opencode share exclusion group)
ch0nky-use-pi:
    systemctl --user stop b00t@opencode-agent.service 2>/dev/null || true
    systemctl --user start b00t@pi-agent.service

ch0nky-use-opencode:
    systemctl --user stop b00t@pi-agent.service 2>/dev/null || true
    systemctl --user start b00t@opencode-agent.service

# ─── Grok (PyO3/Maturin) ─────────────────────────────────────────────────────

grok-build:
    unset CONDA_PREFIX
    cd b00t-grok-py && uv run maturin develop

grok-dev: grok-build
    cd b00t-grok-py && uv run python -m uvicorn main:app --reload --port 8001

grok-clean:
    cargo clean --package b00t-grok
    cd b00t-grok-py && rm -rf build/ dist/ *.egg-info/

# ─── Browser extension ───────────────────────────────────────────────────────

browser-ext-build:
    cd b00t-browser-ext && npm ci && npm run build

browser-ext-package:
    cd b00t-browser-ext && npm run package

browser-ext-dev:
    cd b00t-browser-ext && npm run dev

# ─── Captain's Arsenal ────────────────────────────────────────────────────────

captain:
    export _B00T_ROLE="captain"
    cargo run --bin b00t-cli -- whatismy role --show-tools

operator:
    export _B00T_ROLE="operator"
    cargo run --bin b00t-cli -- whatismy role --show-tools

# TODO stubs — fill in later
create-coder LANG:
    echo "TODO: create {{LANG}} coder agent"

create-tester:
    echo "TODO: create test specialist agent"

setup-redis:
    echo "TODO: setup Redis agent channels"

install-tool TOOL:
    echo "TODO: install tool {{TOOL}}"

# ─── Qdrant vector database ──────────────────────────────────────────────────

qdrant-run:
    podman run -d --name qdrant-container -p 6333:6333 -p 6334:6334 \
      -e QDRANT__SERVICE__GRPC_PORT="6334" docker.io/qdrant/qdrant:latest

qdrant-stop:
    podman stop qdrant-container && podman rm qdrant-container

# ─── Skill improvement loop ──────────────────────────────────────────────────

skill-improve iterations="5":
    TASK=skill-test TOOL=opencode ROLE=executive \
      OPENCODE_MODEL=qwen36-local/ch0nky RALPH_METRIC_GATE=true \
      bash b00t.sh --max-iterations {{iterations}}

skill-improve-loop:
    b00t hive activate b00t-skill-improve-loop

skill-improve-stop:
    systemctl --user stop b00t-hive-b00t-skill-improve-loop || true

skill-improve-logs:
    journalctl --user -u b00t-hive-b00t-skill-improve-loop -f --no-pager

skill-improve-scores:
    @tail -20 .b00t/ralph/scores.jsonl 2>/dev/null | python3 -m json.tool || echo 'no scores yet'

skill-improve-log:
    git log --oneline --author='b00t-skill-loop' -20

# ─── Worker agent ─────────────────────────────────────────────────────────────

worker-experiment-run id="auto" control="default prompt" treatment="custom prompt":
    ID="{{id}}"
    if [[ "$ID" == "auto" ]]; then ID="exp-$(date +%s)"; fi
    cargo run -p b00t-cli --bin b00t-cli -- experiment run \
      --id "$ID" --control "{{control}}" --treatment "{{treatment}}"

worker-status:
    echo "node_id: worker-$$"
    echo "state: $(b00t-cli experiment status 2>/dev/null || echo 'idle')"
    echo "last_heartbeat: $(date -u +%Y-%m-%dT%H:%M:%SZ)"

worker-viz format="mermaid":
    cargo run -p b00t-cli --bin b00t-cli -- --path _b00t_ viz entangle \
      --datum worker --format {{format}}

worker-experiment-scores:
    @find .b00t -name "experiment-*.json" 2>/dev/null \
      -exec echo "--- {} ---" \; -exec cat {} \; || echo "no experiment data yet"

worker-audit-log:
    @cat .b00t/worker-audit.jsonl 2>/dev/null || echo "no audit log entries yet"

worker-validate:
    #!/usr/bin/env bash
    errors=0
    for f in _b00t_/worker.role.toml _b00t_/experiment-controller.agent.toml \
      _b00t_/scoring.agent.toml _b00t_/worker-ontology.mermaid AGENTS/--role=worker.md; do
        [[ -f "$f" ]] && echo "✅ $f" || { echo "❌ $f"; errors=$((errors+1)); }
    done
    exit $errors

# ─── Smoke tests ─────────────────────────────────────────────────────────────

ch0nky-pi-test:
    curl -sf http://localhost:8001/v1/models | python3 -c \
      "import sys,json; m=json.load(sys.stdin); print('✅ serving:', [x['id'] for x in m['data']])"
    LLAMA_CPP_BASE_URL=http://127.0.0.1:8001/v1 \
      pi --provider llama-cpp --model ch0nky -p "respond with: pong"

gemma4-opencode-test:
    curl -sf http://localhost:8001/v1/models | python3 -c \
      "import sys,json; m=json.load(sys.stdin); print('✅ serving:', [x['id'] for x in m['data']])"
    opencode run --model gemma4-local/ch0nky "respond with: pong"

# Rootless node orchestration pattern: systemd + quadlet + podman kube
quadlet-node-install:
    b00t install quadlet-podman-kube

quadlet-node-uninstall:
    b00t uninstall quadlet-podman-kube

nvidia-cdi-observe:
    printf 'host: '; ls /dev/dri/card*
    printf '\ncdi: '; grep -o '/dev/dri/card[0-9]\+' /etc/cdi/nvidia.yaml | sort -u

nvidia-cdi-install:
    b00t install nvidia-cdi-gpu

# ─── sm0l cognitive filter recipes ──────────────────────────────────────────
# Route long command output through local LLM — output ONLY errors, never repeat.
# Endpoint priority: B00T_AI_SM0L_BASE → :8001 (ch0nky) → :8000 (sm0l) → HF Inference API
# Activate local model first: b00t hive activate inference-qwen36-27b

SM0L_FILTER := "_b00t_/scripts/sm0l-filter.py"

# Filter cargo test output — only emit failures/errors (saves frontier context budget)
cargo-test-sm0l *ARGS:
    cargo test {{ARGS}} 2>&1 | python3 {{SM0L_FILTER}} --model sm0l

# Filter cargo build output — only emit errors
cargo-build-sm0l *ARGS:
    cargo build {{ARGS}} 2>&1 | python3 {{SM0L_FILTER}} --model sm0l

# Generic sm0l filter: pipe any command through the LLM error extractor
# Usage: just sm0l-filter "cargo clippy 2>&1"
sm0l-filter CMD:
    {{CMD}} 2>&1 | python3 {{SM0L_FILTER}}

# Probe available sm0l endpoints (shows which tier will be used)
sm0l-status:
    #!/usr/bin/env bash
    curl -sf http://127.0.0.1:8001/v1/health >/dev/null 2>&1 && echo "✅ :8001 ch0nky (vllm)" || echo "⬜ :8001 ch0nky (offline)"
    curl -sf http://127.0.0.1:8000/v1/health >/dev/null 2>&1 && echo "✅ :8000 sm0l (local)" || echo "⬜ :8000 sm0l (offline)"
    [ -n "${HF_TOKEN:-}" ] && echo "HF_TOKEN: set (HF Inference API available)" || echo "HF_TOKEN: NOT SET"
    echo "activate ch0nky: b00t hive activate inference-qwen36-27b"

# Activate ch0nky (vllm Qwen3.6-27B) via b00t governance
ch0nky-activate:
    b00t hive activate inference-qwen36-27b

# Check ch0nky vllm logs
ch0nky-logs:
    journalctl --user -u b00t-hive-inference-qwen36-27b -f --no-pager
# ─── club-3090 refresh ────────────────────────────────────────────────────────
# Checks for new model additions, engine image updates, CHANGELOG diff
# Run weekly or after new model announcements

club-3090-refresh:
    #!/usr/bin/env bash
    set -euo pipefail
    echo "=== club-3090 refresh $(date -u +%Y-%m-%dT%H:%M:%SZ) ==="
    echo "--- recent commits ---"
    gh api repos/noonghunna/club-3090/commits --jq '.[0:5]|.[]|{sha:.sha[:7],msg:.commit.message[:80],date:.commit.author.date}' 2>/dev/null || echo "gh not authenticated"
    echo "--- new models since last check ---"
    gh api "repos/noonghunna/club-3090/commits?since=$(date -d '7 days ago' +%Y-%m-%dT%H:%M:%SZ)" \
      --jq '[.[]|select(.commit.message|test("model|add|support";"i"))|{sha:.sha[:7],msg:.commit.message[:60]}]' 2>/dev/null || true
    echo "--- latest llama.cpp image tag (check vs pinned b9246) ---"
    gh api "repos/ggerganov/llama.cpp/releases?per_page=3" --jq '.[].tag_name' 2>/dev/null | head -3 || true
    echo "=== done — update _b00t_/datums/club-3090.tomllmd last_checked if new models found ==="

club-3090-latest-compose:
    #!/usr/bin/env bash
    gh api repos/noonghunna/club-3090/contents/models/qwen3.6-27b/llama-cpp/compose/single/unsloth-q4km/mtp.yml \
      --jq '.content' | base64 -d 2>/dev/null || echo "gh api failed"
