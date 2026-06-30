---
unsloth-cache: UNSLOTH_CACHE_DIR + triton_kernels.routing (NOT on PyPI) control MoE step time; dense models avoid the issue
# summary: Unsloth cache, triton MoE routing unavailability, and dense model fallback
# tags: unsloth, training, triton, moe, hf-jobs, performance
# tier: frontier
# cmds: ENV UNSLOTH_CACHE_DIR=/opt/unsloth_compiled_cache
# complexity: 8

## LFMF: Unsloth Cache + MoE Triton Routing on HF Jobs

### Issue 1: UNSLOTH_CACHE_DIR must exist and be writable
- **Root cause of $85 bill**: `permission denied: 'unsloth_compiled_cache'` (relative path)
  → JIT recompile every forward pass → 309s/step vs 5s/step on A100 (60x cost)
- **Fix**: In Dockerfile: `ENV UNSLOTH_CACHE_DIR=/opt/unsloth_compiled_cache` + `RUN mkdir -p`

### Issue 2: triton_kernels.routing is NOT available on PyPI
- **Symptom**: `No module named 'triton_kernels.routing'` → 184s/step on H200, 325s/step on A100
- **Wrong fix**: `uv pip install triton-kernels` ← triton-kernels==0.1.0 provides ONLY
  add_vectors.py + rotary_embedding.py — does NOT provide routing submodule
- **Root cause**: `triton_kernels.routing` is internal Meta/OpenAI tooling (gpt_oss_triton_kernels_moe.py)
  This module is NOT available on any public PyPI index
- **Workaround**: Use DENSE models (Qwen3-Coder-14B) instead of MoE models (Qwen3-30B-A3B)

### Issue 3: pip install unsloth in job command invalidates pre-warmed cache
- Upgrading unsloth at job start changes cache format/path
- **Fix**: Do NOT `pip install unsloth` in cloud-train command; deps pre-baked in GHCR image

### Step time without triton_kernels.routing
| Hardware | Model | triton routing | step time |
|---|---|---|---|
| A100 80GB | Qwen3-30B-A3B (MoE) | ✓ | ~130s |
| A100 80GB | Qwen3-30B-A3B (MoE) | ✗ | ~325s |
| H200 141GB | Qwen3-30B-A3B (MoE) | ✗ | ~184s |
| A10g 24GB | Qwen3-Coder-14B (dense) | N/A | ~15-25s |

### Budget comparison
- 573 steps × 184s = 29h → H200 MoE (no triton) CANNOT finish in 10h timeout
- 573 steps × 20s = 3.2h → A10g-large dense 14B → ~$4.50 ← USE THIS

### Correct recipe
- `just ai-finetune::cloud-coder-14b` → A10g-large, 5h timeout, ~$4.50/epoch
- `just ai-finetune::cloud-coder` → H200 10h, use ONLY if triton_kernels.routing is fixed
