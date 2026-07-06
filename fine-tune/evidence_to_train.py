#!/usr/bin/env python3
# @b00t:harness evidence-to-train (#595)
# Convert evidence/satisfies.jsonl PASS entries into verify-loop training
# examples (alpaca-compatible: instruction/input/response, plus verified +
# evidence_sha metadata columns that HF datasets carry through harmlessly).
#
# Usage: uv run python3 fine-tune/evidence_to_train.py \
#            [--evidence evidence/satisfies.jsonl] [--out fine-tune/evidence_train.jsonl]
# Exit 0 = wrote examples; exit 1 = no PASS evidence found.

import argparse
import json
import sys
from pathlib import Path


def convert(lines):
    """Yield training examples from satisfies.jsonl lines.

    Only `predicate == "validates"` with `object.result == "PASS"` qualifies —
    these are ground-truth gate-verified behaviors. Deduped by (subject, sha):
    recurring gate runs re-append identical evidence and training data must not
    over-weight a rule just because its gate ran often.
    """
    seen = set()
    for raw in lines:
        raw = raw.strip()
        if not raw:
            continue
        try:
            entry = json.loads(raw)
        except json.JSONDecodeError:
            continue  # salvage doctrine: skip garbage, never abort the batch
        obj = entry.get("object") or {}
        if entry.get("predicate") != "validates" or obj.get("result") != "PASS":
            continue
        subject, sha = entry.get("subject", ""), obj.get("sha", "")
        if not subject or (subject, sha) in seen:
            continue
        seen.add((subject, sha))
        gate_file = obj.get("file", subject)
        yield {
            "instruction": f"Apply gate rule {subject} to {gate_file} and report whether it is satisfied.",
            "input": "",
            "response": (
                f"[tool_call: verify assertion=gate:{subject} sha={sha}]"
                " → [result: PASS] → Gate satisfied; evidence appended to satisfies.jsonl."
            ),
            "verified": True,
            "evidence_sha": sha,
        }


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument("--evidence", default="evidence/satisfies.jsonl")
    ap.add_argument("--out", default="fine-tune/evidence_train.jsonl")
    args = ap.parse_args()

    evidence = Path(args.evidence)
    if not evidence.exists():
        print(f"FAIL: {evidence} not found", file=sys.stderr)
        return 1

    examples = list(convert(evidence.read_text().splitlines()))
    if not examples:
        print("FAIL: no PASS evidence entries found", file=sys.stderr)
        return 1

    out = Path(args.out)
    out.write_text("".join(json.dumps(e, ensure_ascii=False) + "\n" for e in examples))
    print(f"PASS: {len(examples)} verified training examples → {out}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
