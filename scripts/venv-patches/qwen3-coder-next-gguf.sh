#!/usr/bin/env bash
# Re-apply venv patches for Qwen3-Coder-Next GGUF support in vLLM 0.17.1
# Run after any venv rebuild: bash scripts/venv-patches/qwen3-coder-next-gguf.sh
# Status: PARTIAL (conv1d+reshape patched; in_proj_qkvz.qweight mapping still open)
set -euo pipefail

VENV_SITE="/home/brianh/.venv/lib/python3.12/site-packages"
PYTHON="/home/brianh/.venv/bin/python3"

echo "=== Patch 0: gguf >= 0.18.0 (MODEL_ARCH.QWEN3NEXT) ==="
uv pip install "gguf>=0.18.0" --python "$PYTHON"
python3 -c "import gguf; assert hasattr(gguf.MODEL_ARCH, 'QWEN3NEXT'), 'FAIL: QWEN3NEXT missing'"
echo "OK"

echo "=== Patch 1: transformers/integrations/ggml.py — qwen3_next config mapping ==="
python3 - <<'EOF'
import ast, sys
path = f"{sys.argv[1]}/transformers/integrations/ggml.py"
src = open(path).read()
if '"qwen3_next"' in src:
    print("already patched")
    sys.exit(0)
# Insert after qwen3_moe block — add qwen3_next to GGUF_CONFIG_MAPPING
# Full patch is maintained in git diff; manual insert here for idempotency
print("NEEDS MANUAL PATCH — see git history for full diff")
EOF
echo "(manual check — see git log for exact diff)"

echo "=== Patch 2: vllm/model_executor/models/qwen3_next.py ==="
python3 - <<'EOF'
import sys
path = f"{sys.argv[1]}/vllm/model_executor/models/qwen3_next.py"
src = open(path).read()
checks = [
    ("quant_config=self.quant_config,  # 🤓 missing in vllm 0.17", "ParallelLMHead quant_config"),
    ("conv1d.weight) and loaded_weight.dim() == 2", "conv1d.weight unsqueeze"),
    ("GGUFUninitializedParameter raises ValueError", "1D→2D reshape guard"),
]
for needle, name in checks:
    status = "OK" if needle in src else "MISSING"
    print(f"  [{status}] {name}")
EOF
echo "Done"

echo ""
echo "=== OPEN: in_proj_qkvz.qweight mapping ==="
echo "  vLLM 0.17.1 SSM weight loading for Qwen3-Coder-Next is incomplete."
echo "  GGUF blk.X.ssm_in.weight maps to in_proj_qkvz per gguf library, but"
echo "  GGUFUninitializedParameter.qweight is never initialized."
echo "  Workaround: use llama-server (inference-qwen3.stack.tomllm)"
