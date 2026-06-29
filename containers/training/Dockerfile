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

# Layer training dependencies that unsloth:latest omits
# uv pip install into the existing venv (VIRTUAL_ENV already set by base image)
RUN uv pip install --system mlflow==2.21.3 pyyaml

# Smoke-test: mlflow importable
RUN python3 -c "import mlflow; print('mlflow', mlflow.__version__)"

LABEL org.opencontainers.image.source="https://github.com/elasticdotventures/_b00t_"
LABEL org.opencontainers.image.description="b00t fine-tuning image: unsloth + mlflow + uv"
LABEL b00t.tier="frontier"
LABEL b00t.role="training"
