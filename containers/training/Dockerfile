# b00t training image — extends unsloth/unsloth:latest with uv toolchain
# Build: just containers::training::build
# Push:  just containers::training::push
# Use:   ghcr.io/elasticdotventures/b00t-training-image:latest

FROM docker.io/unsloth/unsloth:latest

# Install uv — the only Python package manager b00t uses
COPY --from=ghcr.io/astral-sh/uv:latest /uv /uvx /usr/local/bin/

# 🤓 unsloth/__init__.py calls pip to upgrade numpy 2.2→2.4 at import time (Gemma patch dep).
#    This causes "numpy was upgraded mid-session" RuntimeError in unsloth_zoo because the
#    .so files in memory stay at 2.2.6 but dist-info says 2.4.x (loaded≠installed → error).
#    Fix: pre-install numpy>=2.4 via uv so .so files are already 2.4.x at Python start.
#    When unsloth runs its internal check, it finds 2.4.x already installed → no mismatch.
ENV VIRTUAL_ENV=/opt/venv
# 🤓 HF Jobs container startup may reset PATH; put /opt/venv/bin first so just train
#    uses the venv python3 (with numpy 2.4.x), not system python3 (with numpy 2.2.6)
ENV PATH="/opt/venv/bin:$PATH"
# hf_xet: accelerates Hub checkpoint uploads (multi-part, dedup); mlflow omitted — use report_to:none
RUN uv pip install "numpy~=2.4.1" pyyaml "huggingface_hub[hf_xet]>=0.26"

# Smoke-tests: yaml importable; numpy is exactly 2.4.x and self-consistent
RUN /opt/venv/bin/python3 -c "import yaml; print('yaml', yaml.__version__)"
RUN /opt/venv/bin/python3 -c "import importlib.metadata as m, numpy as np; v=m.version('numpy'); assert v==np.__version__, f'mismatch: {np.__version__} vs {v}'; assert v.startswith('2.4'), f'{v} not 2.4.x'; print('numpy 2.4.x OK', v)"

# UNSLOTH_CACHE_DIR: /tmp always writable at HF Jobs runtime (base image may run non-root)
ENV UNSLOTH_CACHE_DIR=/tmp/unsloth_compiled_cache

LABEL org.opencontainers.image.source="https://github.com/elasticdotventures/_b00t_"
LABEL org.opencontainers.image.description="b00t fine-tuning image: unsloth + uv + numpy>=2.4"
LABEL b00t.tier="frontier"
LABEL b00t.role="training"
