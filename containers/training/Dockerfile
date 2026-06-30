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
# 🤓 MINIMAL: only add pyyaml (for YAML config parsing in train recipe).
#    mlflow removed — causes numpy dist-info/binary mismatch (2.2.6→2.4.1) via hf_xet.
#    huggingface_hub + numpy already present in base image; don't touch them.
ENV VIRTUAL_ENV=/opt/venv
RUN uv pip install pyyaml

# Smoke-test: yaml importable + numpy version consistent
RUN /opt/venv/bin/python3 -c "import yaml; print('yaml', yaml.__version__)"
RUN /opt/venv/bin/python3 -c "import importlib.metadata, numpy as np; v=importlib.metadata.version('numpy'); assert v == np.__version__, f'numpy mismatch: {np.__version__} vs {v}'; print('numpy OK', v)"

# Set UNSLOTH_CACHE_DIR to /tmp to avoid /opt/ permission issues (base image may run non-root)
# 🤓 /tmp is always writable at HF Jobs runtime; unsloth creates the dir on first use.
#    Pre-warming removed: internal pip calls in unsloth import can mutate numpy dist-info.
ENV UNSLOTH_CACHE_DIR=/tmp/unsloth_compiled_cache

LABEL org.opencontainers.image.source="https://github.com/elasticdotventures/_b00t_"
LABEL org.opencontainers.image.description="b00t fine-tuning image: unsloth + mlflow + uv"
LABEL b00t.tier="frontier"
LABEL b00t.role="training"
