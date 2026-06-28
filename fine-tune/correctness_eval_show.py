#!/usr/bin/env python3
"""Print summary of a correctness eval JSONL scorecard."""
import json
import sys
from pathlib import Path

path = Path(sys.argv[1]) if len(sys.argv) > 1 else None
if not path or not path.exists():
    print("usage: correctness_eval_show.py <scorecard.jsonl>")
    sys.exit(1)

records = [json.loads(line) for line in path.read_text().splitlines() if line.strip()]
by_ep: dict[str, dict] = {}
for r in records:
    ep = r["endpoint"]
    by_ep.setdefault(ep, {"pass": 0, "total": 0, "model": r.get("model_id", "?")})
    by_ep[ep]["total"] += 1
    if r["passed"]:
        by_ep[ep]["pass"] += 1

print(f"scorecard: {path.name}")
for ep, s in by_ep.items():
    pct = s["pass"] / s["total"]
    bar = "█" * s["pass"] + "░" * (s["total"] - s["pass"])
    print(f"  {s['model']:40s} {s['pass']:2d}/{s['total']} {pct:.0%}  {bar}")
