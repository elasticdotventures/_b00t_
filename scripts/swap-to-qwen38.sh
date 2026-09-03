#!/usr/bin/env bash
# swap-to-qwen38 — make Qwen3.8-27B the exclusive local ch0nky on :8001.
# Idempotent. Run after the GGUF finishes downloading to /c0de.
set -euo pipefail
GGUF=/c0de/models/qwen38-27b/Qwen3.8-27B-UD-Q4_K_XL.gguf
EXPECT=17559178144   # UD-Q4_K_XL exact size

[ -f "$GGUF" ] || { echo "✗ $GGUF missing — download first"; exit 1; }
sz=$(stat -c %s "$GGUF")
[ "$sz" -ge "$EXPECT" ] || { echo "✗ $GGUF is $sz / $EXPECT bytes — download incomplete"; exit 1; }
echo "✓ weights present: $(numfmt --to=iec "$sz")"

echo "→ stopping qwen36 mtp-podman + any :8001 llama container"
systemctl --user stop b00t-hive-inference-qwen36-27b-mtp-podman.service 2>/dev/null || true
podman rm -f b00t-ch0nky-llamacpp 2>/dev/null || true
# lower qwen36's autostart so a reboot doesn't race it back onto :8001
systemctl --user disable b00t-hive-inference-qwen36-27b-mtp-podman.service 2>/dev/null || true

echo "→ activating inference-qwen38-27b"
b00t-cli hive activate inference-qwen38-27b

echo "→ waiting for :8001 to serve Qwen3.8 (up to 180s)"
for i in $(seq 1 60); do
  if curl -sf -m3 http://127.0.0.1:8001/health >/dev/null 2>&1; then
    m=$(curl -s -m5 http://127.0.0.1:8001/v1/models | python3 -c 'import json,sys;d=json.load(sys.stdin);print(d["data"][0]["id"])' 2>/dev/null || echo "?")
    echo "✓ :8001 healthy — model id: $m"
    curl -s -m60 http://127.0.0.1:8001/v1/chat/completions -H 'Content-Type: application/json' \
      -d '{"model":"ch0nky","messages":[{"role":"user","content":"Reply with exactly: qwen38 online"}],"max_tokens":16,"temperature":0}' \
      | python3 -c 'import json,sys;print("→", json.load(sys.stdin)["choices"][0]["message"]["content"])' 2>/dev/null || true
    exit 0
  fi
  sleep 3
done
echo "✗ :8001 did not come healthy — check: podman logs b00t-ch0nky-llamacpp"
exit 1
