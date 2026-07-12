#!/usr/bin/env python3
"""Update .b00t/finetune-gen.json after a successful fine-tune run.

Records: generation number, timestamp, model path, train pair count, dataset hash.
Called automatically by just finetune-smol / just finetune-all.
"""

import argparse
import hashlib
import json
import time
from pathlib import Path

GEN_FILE = Path(".b00t/finetune-gen.json")
TRAIN_JSONL = Path("fine-tune/train.jsonl")


def jsonl_hash(path: Path) -> str:
    h = hashlib.sha256()
    if path.exists():
        h.update(path.read_bytes())
    return h.hexdigest()[:16]


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--model", required=True, help="Path to exported GGUF")
    parser.add_argument("--tier", required=True, choices=["smol", "ch0nky"], help="Model tier")
    args = parser.parse_args()

    GEN_FILE.parent.mkdir(parents=True, exist_ok=True)

    existing = {}
    if GEN_FILE.exists():
        existing = json.loads(GEN_FILE.read_text())

    train_pairs = sum(1 for _ in TRAIN_JSONL.open()) if TRAIN_JSONL.exists() else 0
    gen = existing.get("generation", 0) + 1

    record = {
        **existing,
        "generation": gen,
        "timestamp": time.strftime("%Y-%m-%dT%H:%M:%SZ", time.gmtime()),
        "train_pairs": train_pairs,
        "dataset_hash": jsonl_hash(TRAIN_JSONL),
        f"model_{args.tier}": str(args.model),
        f"gen_{args.tier}": gen,
    }

    GEN_FILE.write_text(json.dumps(record, indent=2))
    print(f"✅ finetune-gen.json updated: gen={gen} tier={args.tier} pairs={train_pairs} hash={record['dataset_hash']}")


if __name__ == "__main__":
    main()
