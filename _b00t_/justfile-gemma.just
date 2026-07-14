# ── Gemma 4 + pi-coding-agent local inference ────────────────────────────────
# Extracted from root justfile for module isolation.

# Download Gemma 4 26B-A4B MXFP4_MOE GGUF (unsloth/gemma-4-26B-A4B-it-GGUF)
qwen36-download:
    b00t hive activate download-mode
    hf download unsloth/Qwen3.6-27B-GGUF --include "Qwen3.6-27B-Q4_K_M.gguf"
    @echo "✅ download done — run: just qwen36-serve  (or just qwen36-serve-llamacpp)"

# Activate Qwen3.6-27B via vLLM (generates + enables systemd unit)
qwen36-serve:
    b00t hive activate inference-qwen36-27b

# Activate Qwen3.6-27B via llamacpp podman (GGUF-native fallback when vLLM fails)
qwen36-serve-llamacpp:
    b00t hive activate inference-qwen36-27b-llamacpp

# Stop Qwen3.6-27B inference (whichever backend is running)
qwen36-stop:
    systemctl --user stop b00t-hive-inference-qwen36-27b.service || true
    systemctl --user stop b00t-hive-inference-qwen36-27b-llamacpp.service || true
    systemctl --user stop b00t-hive-inference-qwen36-35b-a3b-llamacpp.service || true

# Serve 35B-A3B MoE (lighter VRAM than 27B dense; preferred when MXFP4 supported)
qwen36-serve-35b:
    b00t hive activate inference-qwen36-35b-a3b-llamacpp

# Eval active ch0nky model (must be serving on :8001)
qwen36-status:
    curl -s http://localhost:8001/v1/models | python3 -m json.tool

# Run opencode one-shot against local qwen36-local/ch0nky
qwen36-test-opencode prompt="say hello in 3 words":
    opencode run --model qwen36-local/ch0nky "{{prompt}}"
