# b00t training image — extends unsloth/unsloth:latest with MLflow + uv toolchain
# Build: just containers::training::build
# Push:  just containers::training::push
# Use:   elasticdotventures/b00t-training-image:latest (GHCR)
#
# 🤓 pip is FORBIDDEN in b00t. All Python deps installed via uv.
#    uv is significantly faster than pip and respects lockfiles.
#    unsloth:latest ships pip-only env; we layer uv on top without removing pip
#    (pip stays for unsloth's internal install hooks — we just don't call it ourselves).

FROM docker.io/unsloth/unsloth:latest

# Install uv — the only Python package manager b00t uses
COPY --from=ghcr.io/astral-sh/uv:latest /uv /uvx /usr/local/bin/

# Layer training dependencies into the venv that unsloth:latest ships
# 🤓 unsloth:latest ships /opt/venv; uv respects VIRTUAL_ENV when --system is absent
ENV VIRTUAL_ENV=/opt/venv
# 🤓 pin numpy first — mlflow/hf_xet transitive deps would otherwise upgrade numpy 2.2→2.4
#    which causes RuntimeError in unsloth_zoo at import time ("numpy was upgraded mid-session")
# 🤓 triton-kernels==0.1.0 does NOT provide triton_kernels.routing — removed (wrong package)
RUN uv pip install "numpy==2.2.6" mlflow==2.21.3 pyyaml "huggingface_hub[hf_xet]>=0.26"

# Smoke-test: mlflow importable inside venv
RUN /opt/venv/bin/python3 -c "import mlflow; print('mlflow', mlflow.__version__)"

# Pre-warm unsloth compiled cache so training jobs start at 5s/step not 309s/step
# 🤓 Without this, unsloth_compiled_cache permission denied → JIT recompile every forward pass
#    Cache lives in /opt/unsloth_compiled_cache (writable, baked into image layer)
ENV UNSLOTH_CACHE_DIR=/opt/unsloth_compiled_cache
RUN mkdir -p /opt/unsloth_compiled_cache && \
    /opt/venv/bin/python3 -c "from unsloth import FastLanguageModel; print('unsloth cache warm')" || true

LABEL org.opencontainers.image.source="https://github.com/elasticdotventures/_b00t_"
LABEL org.opencontainers.image.description="b00t fine-tuning image: unsloth + mlflow + uv"
LABEL b00t.tier="frontier"
LABEL b00t.role="training"
