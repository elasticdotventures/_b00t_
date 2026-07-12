#!/usr/bin/env python3
# # @b00t:harness runpod-ping
# Verifies RunPod API key is valid and account is reachable.
# Usage: uv run --with runpod python3 fine-tune/runpod_ping.py
# Exit 0 = PASS, Exit 1 = FAIL

import os, sys

def ping() -> dict:
    try:
        import runpod  # type: ignore
    except ImportError:
        return {"status": "FAIL", "reason": "runpod SDK not installed — uv pip install runpod"}

    api_key = os.environ.get("RUNPOD_API_KEY", "")
    if not api_key:
        return {"status": "FAIL", "reason": "RUNPOD_API_KEY not set"}

    runpod.api_key = api_key
    try:
        gpus = runpod.get_gpus()  # lightweight list — no billing impact
        gpu_ids = [g.get("id", "?") for g in (gpus or [])]
        return {
            "status": "PASS",
            "gpu_types_available": len(gpu_ids),
            "sample": gpu_ids[:3],
        }
    except Exception as e:
        return {"status": "FAIL", "reason": str(e)}


if __name__ == "__main__":
    import json
    from pathlib import Path

    # load .env if present
    env_path = Path(__file__).parent.parent / ".env"
    if env_path.exists():
        for line in env_path.read_text().splitlines():
            if "=" in line and not line.startswith("#"):
                k, _, v = line.partition("=")
                os.environ.setdefault(k.strip(), v.strip())

    result = ping()
    print(json.dumps(result, indent=2))
    sys.exit(0 if result["status"] == "PASS" else 1)
