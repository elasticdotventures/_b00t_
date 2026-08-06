# Phi-4 local CPU inference via native candle-transformers (b00t-candle-serve crate)
# No vLLM, no podman, no GPU — CPU-friendly fallback tier. See:
#   _b00t_/phi-4-candle-local.model.ai.tomllmd, _b00t_/phi.ai.tomllmd, b00t-candle-serve/
# Namespaced: just phi-candle <recipe>
# Pattern mirrored from vendor/rust-doc.just (build/start/status/stop/update for a
# "build once, run as background local process" style crate).

# Build the release binary
build:
    #!/bin/bash
    set -euo pipefail
    B00T_ROOT="$(git rev-parse --show-toplevel)"
    cargo build --release -p b00t-candle-serve --manifest-path "${B00T_ROOT}/Cargo.toml"

# Pre-fetch the GGUF weights + tokenizer into the HF cache (no inference run yet)
download:
    #!/bin/bash
    set -euo pipefail
    hf download microsoft/phi-4-gguf --include "phi-4-Q4_K.gguf"
    hf download microsoft/phi-4 --include "tokenizer.json"
    echo "✅ Phi-4 Q4_K GGUF (~8.43GB) + tokenizer cached — run: just phi-candle run"

# One-shot CLI generation for a prompt (builds first if needed). CPU-only, no server.
run prompt="Q: What is the capital of France?\nA:":
    #!/bin/bash
    set -euo pipefail
    B00T_ROOT="$(git rev-parse --show-toplevel)"
    BIN="${B00T_ROOT}/target/release/b00t-candle-serve"
    if [ ! -f "$BIN" ]; then
        echo "🔨 Building b00t-candle-serve..."
        just phi-candle build
    fi
    "$BIN" --prompt "{{prompt}}"

# Check whether the GGUF weights are already cached locally (no running-process check —
# this binary is currently one-shot CLI, not a background service; see note below)
status:
    @ls -lh ~/.cache/huggingface/hub/models--microsoft--phi-4-gguf/snapshots/*/phi-4-Q4_K.gguf 2>/dev/null \
        && echo "✅ Phi-4 Q4_K GGUF cached" \
        || echo "❌ Phi-4 GGUF not downloaded — run: just phi-candle download"

# 🤓 GAP: no persistent server/stop recipe yet. b00t-candle-serve is currently a one-shot
#    stdio CLI generator (loads GGUF, generates, exits) — not a long-running process with
#    an OpenAI-compatible HTTP endpoint. `stop`/background-service wiring is a follow-up
#    once (if) an HTTP wrapper is added around candle_transformers::models::quantized_phi3.
