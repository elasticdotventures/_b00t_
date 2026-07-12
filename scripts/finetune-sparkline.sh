#!/usr/bin/env bash
# Print a loss sparkline from the active sm0l training pod
N=${1:-60}
POD=$(kubectl get pod -n b00t-finetune -l b00t.io/model=sm0l -o name 2>/dev/null | head -1)
[[ -z "$POD" ]] && { echo "no training pod running"; exit 1; }

EPOCH=$(kubectl logs -n b00t-finetune "$POD" -c trainer 2>/dev/null \
  | grep "'epoch'" | tail -1 | grep -oP "'epoch': \K[\d.]+")

TMPFILE=$(mktemp)
kubectl logs -n b00t-finetune "$POD" -c trainer 2>/dev/null \
  | grep "'loss'" \
  | awk -F"'loss': " '{print $2}' | awk -F"," '{print $1}' \
  | tail -"$N" > "$TMPFILE"

python3 - "$TMPFILE" <<'PYEOF'
import sys
with open(sys.argv[1]) as f:
    vals = [float(l.strip()) for l in f if l.strip()]
if not vals:
    print("no loss data yet")
    sys.exit(0)
mn, mx = min(vals), max(vals)
bar = '▁▂▃▄▅▆▇█'
scaled = [int((v - mn) / (mx - mn + 1e-9) * 7) for v in vals]
print(f"n={len(vals)}  min={mn:.4f}  max={mx:.4f}  last={vals[-1]:.4f}")
print(''.join(bar[s] for s in scaled))
PYEOF
rm -f "$TMPFILE"
[[ -n "$EPOCH" ]] && echo "epoch: $EPOCH / 3"
