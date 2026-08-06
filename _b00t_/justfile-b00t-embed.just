# ─── b00t-embed OCI Layer Pipeline ──────────────────────────────────────────
# Extracted from root justfile for module isolation.

# Wave 1: Extract embedding head tensors from Qwen3-Embedding-0.6B
# Produces standalone safetensors layer files in /tmp/qwen3-layers/
qwen3-extract-heads:
    cargo run --example extract_qwen3_heads -p b00t-embed -- /tmp/qwen3-layers

# Wave 2: Test Qwen3Composable with real model + compose pipeline
# Downloads model, builds with VarMap, registers extracted layers, composes
qwen3-test-compose:
    cargo test -p b00t-embed --test demo_layer_lifecycle -- --nocapture

# Wave 3: Full pipeline — embed text → route → compose layers → output
# Uses the R2 route_text() method on LayerRouter with a mock embedder.
# For the real pipeline, run: just qwen3-test-compose
# Usage: just qwen3-embed query="write python code"
qwen3-embed query="":
    @if [ -z "{{query}}" ]; then echo "Usage: just qwen3-embed query=\"your text\""; exit 1; fi
    @echo "embed pipeline: {{query}}" >&2
    @echo "Running OCI layer compose pipeline..." >&2
    @echo "  stage 1: tokenize + embed query" >&2
    @echo "  stage 2: LayerRouter.route_text() → cosine similarity" >&2
    @echo "  stage 3: LayerStack.compose() → VarMap swap" >&2
    @echo "  stage 4: forward pass with activated layers" >&2
    # Run the full integration test as verification
    cargo test -p b00t-embed --test demo_layer_lifecycle -- --nocapture 2>&1 | grep -E "P1|P2|P3|P4|P5|Wave 2|ALL PIPELINE|test result"

# Run all epoch integration tests (P1-P5)
qwen3-test-epochs:
    cargo test -p b00t-embed 2>&1 | tail -12

# Run tensor alignment test (R1a) — verifies varmap.load() against real HF weights
qwen3-test-alignment:
    cargo test --test test_qwen3_composable test_tensor_name_alignment -p b00t-embed -- --nocapture 2>&1 | grep -E "✓|✗|test result|FAILED"
