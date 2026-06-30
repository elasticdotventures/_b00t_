---
unsloth-cache: UNSLOTH_CACHE_DIR must exist + Triton MoE kernels control step time; pre-warm is model-specific; H200 needed for 30B MoE
# summary: Unsloth compiled cache and Triton MoE kernel gotchas for HF Jobs training
# tags: unsloth, training, triton, moe, hf-jobs, performance
# tier: frontier
# cmds: ENV UNSLOTH_CACHE_DIR=/opt/unsloth_compiled_cache; uv pip install triton-kernels
# complexity: 7

## LFMF: Unsloth Cache + MoE Step Time on HF Jobs

### Issue 1: UNSLOTH_CACHE_DIR must exist and be writable
- **Root cause of $85 bill**: `permission denied: 'unsloth_compiled_cache'` (relative path)
  → JIT recompile every forward pass → 309s/step vs 5s/step on A100 (60x cost)
- **Fix**: In Dockerfile: `ENV UNSLOTH_CACHE_DIR=/opt/unsloth_compiled_cache` + `RUN mkdir -p`
- **Note**: Pre-warming with `from unsloth import FastLanguageModel` bakes generic kernels,
  NOT model-specific ones. Model-specific kernels still JIT on first step.

### Issue 2: triton-kernels package required for MoE routing
- **Symptom**: `No module named 'triton_kernels.routing'` → fallback → 325s/step
- **Fix**: `uv pip install triton-kernels` in Dockerfile
- `triton-kernels` (PyPI) is SEPARATE from `triton` — both are needed for MoE
- `triton_kernels.routing` = fast MoE scatter/gather; without it step time 2.5x worse

### Issue 3: pip install unsloth in job command invalidates pre-warmed cache
- Upgrading unsloth at job start changes the JIT cache format/path
- **Fix**: Do NOT `pip install unsloth` in cloud-train command when using custom GHCR image
- Deps belong in Dockerfile (built once), not re-installed at every job start

### Step time expectations on HF Jobs
- A100 80GB (a100-large): ~130s/step for Qwen3-30B-MoE-128E (MoE routing scatter bound)
- A100 without triton-kernels: ~325s/step (PyTorch fallback)
- H200 141GB (h200): ~50s/step (faster HBM + better MoE routing)

### Budget implication
- 573 steps × 130s = 20.7h > 19h timeout → A100 can't finish Qwen3-30B in 1 epoch
- 573 steps × 50s = 7.9h → H200 finishes in budget (~$68 at $8.50/hr)
- Use `h200` flavor for Qwen3-30B-MoE; A100 only for smaller models
