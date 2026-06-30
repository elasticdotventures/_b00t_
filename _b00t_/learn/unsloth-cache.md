---
unsloth-cache: unsloth/__init__.py internally upgrades numpy 2.2→2.4; pre-install numpy>=2.4 to prevent mid-session mismatch
# summary: Unsloth numpy mid-session upgrade root cause + fix; triton_kernels.routing unavailable; 14B dense recommended
# tags: unsloth, training, numpy, triton, moe, hf-jobs, performance
# tier: frontier
# cmds: uv pip install "numpy>=2.4" pyyaml  # in Dockerfile, BEFORE anything else
# complexity: 9

## LFMF: Unsloth numpy mid-session upgrade + MoE routing

### Root cause: unsloth internally upgrades numpy at import time
- **Symptom**: `RuntimeError: numpy was upgraded mid-session (loaded: 2.2.6, installed: 2.4.1)`
- **Path**: `unsloth/__init__.py → unsloth_zoo → temporary_patches/gemma.py → utils.py:123`
- **Mechanism**: unsloth calls pip internally to upgrade numpy 2.2→2.4 (Gemma patch dep).
  After pip: dist-info says 2.4.1 but loaded .so still 2.2.6 → loaded≠installed → RuntimeError
- **Fix**: Pre-install `"numpy>=2.4"` via uv in Dockerfile BEFORE any unsloth import.
  Python then starts with numpy 2.4.x .so → unsloth finds it already upgraded → no mismatch
- **WRONG fixes tried**:
  - `numpy==2.2.6` pin → fails: unsloth still upgrades to 2.4.1 and then sees mismatch
  - Adding mlflow/hf_xet → worsened the issue (also upgrades numpy)
  - Removing cache warm step → not the cause; unsloth import itself is the trigger

### Dockerfile for working unsloth training image
```dockerfile
FROM docker.io/unsloth/unsloth:latest
COPY --from=ghcr.io/astral-sh/uv:latest /uv /uvx /usr/local/bin/
ENV VIRTUAL_ENV=/opt/venv
RUN uv pip install "numpy>=2.4" pyyaml
# Verify no mid-session upgrade risk
RUN /opt/venv/bin/python3 -c "import importlib.metadata as m, numpy as np; v=m.version('numpy'); assert v==np.__version__; assert tuple(int(x) for x in v.split('.')[:2])>=(2,4); print('numpy >=2.4 OK', v)"
ENV UNSLOTH_CACHE_DIR=/tmp/unsloth_compiled_cache
# /opt/ not writable (non-root user in base image); unsloth creates /tmp/... at runtime
```

### triton_kernels.routing: NOT available on PyPI
- `triton-kernels==0.1.0` provides only add_vectors.py + rotary_embedding.py
- `triton_kernels.routing` = internal Meta/OpenAI tooling, not public
- Without it: Qwen3-30B-A3B (MoE) trains at 184s/step on H200 (29h > 10h timeout)

### Dense 14B avoids all MoE issues
| Hardware | Model | triton routing | numpy fix | step time |
|---|---|---|---|---|
| A100-large | Qwen3-30B-A3B (MoE) | ✓ | needed | ~130s |
| H200 | Qwen3-30B-A3B (MoE) | ✗ | needed | ~184s |
| A10g-large | Qwen3-Coder-14B (dense) | N/A | needed | ~15-25s |

### Budget
- 14B dense: 573 steps × 20s × $1.50/hr = ~$4.77 ← USE THIS
- 30B MoE H200: 573 × 184s × $5.00/hr = ~$146 + won't finish in 10h timeout
- `just ai-finetune::cloud-coder-14b` → A10g-large, 5h, $4.77 estimate

### Volume mount: skip bucket, use hub_strategy
- `hf://buckets/...` FUSE mount fails intermittently → "Volume mount failed" error
- Fix: remove bucket mount; use `push_to_hub: true` in config for checkpoint persistence
- `hub_model_id: elasticdotventures/b00t-qwen3-coder-14b` saves to HF Hub directly
