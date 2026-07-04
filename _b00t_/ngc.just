# NGC — NVIDIA GPU Cloud CLI + model API recipes
# 🧠 Load: b00t learn ngc
# Datum: _b00t_/datums/ngc.cli.tomllm
# Library: _b00t_/python/b00t_ngc/ (NvidiaClient, stdlib-only)
#
# Install once: just ngc::install
# Auth check:   just ngc::auth-check
# Model smoke:  just ngc::model-smoke

set working-directory := ".."

_py := "uv run --project _b00t_/python"

# Install b00t-ngc as a uv-managed tool (idempotent)
install:
    uv tool install --editable _b00t_/python
    @echo "✅ b00t-ngc installed — run: b00t-ngc --help"

# Verify NGC API key + show org
auth-check:
    {{ _py }} b00t-ngc auth

# List recent NGC PyTorch container tags
container-list image="pytorch" n="12":
    {{ _py }} b00t-ngc containers --image {{ image }} -n {{ n }}

# Pull a container into local podman cache
container-pull image="pytorch:24.12-py3":
    #!/usr/bin/env bash
    KEY=$(grep -m1 '^NGC_API_KEY=' ~/.b00t/.env 2>/dev/null | cut -d= -f2- | tr -d "\"'")
    podman login nvcr.io --username '$oauthtoken' --password "$KEY"
    podman pull "nvcr.io/nvidia/{{ image }}"

# List available free models on NVIDIA API
model-list filter="":
    {{ _py }} b00t-ngc models --filter "{{ filter }}"

# Quick chat prompt against a NVIDIA-hosted model
model-smoke model="nvidia/llama-3.1-nemotron-70b-instruct" prompt="What is b00t?":
    {{ _py }} b00t-ngc chat --model "{{ model }}" "{{ prompt }}"

# Streaming chat (tokens printed as they arrive)
model-stream model="nvidia/llama-3.1-nemotron-70b-instruct" prompt="Explain PolyConnector":
    {{ _py }} b00t-ngc chat --stream --model "{{ model }}" "{{ prompt }}"

# Show recommended NGC image for RunPod training
training-image:
    @echo "nvcr.io/nvidia/pytorch:24.12-py3"
    @echo "  PyTorch 2.5 | CUDA 12.6 | Python 3.10 | bash entrypoint (no supervisord)"
