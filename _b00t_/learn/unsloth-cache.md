---
Unsloth compiled cache missing or permission-denied causes JIT recompile every forward pass: 309s/step vs 5s/step on A100 (60x cost). Pre-create in Dockerfile: ENV UNSLOTH_CACHE_DIR=/opt/unsloth_compiled_cache and RUN mkdir -p. Root cause of $85 HF bill for a 47-minute job.
