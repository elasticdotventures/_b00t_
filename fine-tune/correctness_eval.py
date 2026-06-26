#!/usr/bin/env python3
"""Multi-variant b00t correctness evaluator.

Runs correctness_eval.json prompts against one or more model endpoints,
scores with deterministic judges + optional LLM-judge, outputs JSONL scorecard.

Usage:
    python fine-tune/correctness_eval.py --endpoints http://127.0.0.1:8001 http://127.0.0.1:8002
    python fine-tune/correctness_eval.py --endpoints http://127.0.0.1:8001 --judge-endpoint http://127.0.0.1:8001
"""

import argparse
import json
import sys
import time
from pathlib import Path
from urllib.request import urlopen, Request
from urllib.error import URLError

EVAL_PATH = Path(__file__).parent / "correctness_eval.json"
OUTPUT_DIR = Path(".b00t/ralph")
API_KEY = "local-b00t"


def chat(endpoint: str, prompt: str, max_tokens: int = 2048) -> tuple[str, int, float]:
    """Call /v1/chat/completions; return (content, tokens, elapsed_s).

    Uses 2048 tokens — ch0nky runs deepseek reasoning format which generates a
    <think> block (up to ~1024 tokens) before the actual answer.
    """
    url = endpoint.rstrip("/") + "/v1/chat/completions"
    payload = json.dumps({
        "model": "ch0nky",
        "messages": [{"role": "user", "content": prompt}],
        "max_tokens": max_tokens,
        "temperature": 0.1,
    }).encode()
    req = Request(url, data=payload, headers={
        "Content-Type": "application/json",
        "Authorization": f"Bearer {API_KEY}",
    })
    t0 = time.monotonic()
    try:
        with urlopen(req, timeout=60) as resp:
            d = json.load(resp)
        elapsed = time.monotonic() - t0
        msg = d["choices"][0]["message"]
        # deepseek reasoning format: actual answer in content, thinking in reasoning_content
        content = msg.get("content") or msg.get("reasoning_content", "")
        tokens = d.get("usage", {}).get("completion_tokens", 0)
        return content, tokens, elapsed
    except (URLError, KeyError, json.JSONDecodeError) as e:
        return f"ERROR: {e}", 0, time.monotonic() - t0


def judge_exact_prefix(response: str, arg: str) -> bool:
    return response.strip().startswith(arg)


def judge_pattern(response: str, arg: str) -> bool:
    return arg.lower() in response.lower()


def judge_contains_all(response: str, arg: list[str]) -> bool:
    return all(a.lower() in response.lower() for a in arg)


def judge_contains_any(response: str, arg: list[str]) -> bool:
    return any(a.lower() in response.lower() for a in arg)


def judge_semantic(response: str, arg: str, judge_endpoint: str | None) -> bool:
    """Use LLM judge if endpoint available; fall back to keyword check."""
    if judge_endpoint:
        rubric = f"Does this response satisfy: '{arg}'? Answer YES or NO only.\n\nResponse: {response}"
        verdict, _, _ = chat(judge_endpoint, rubric, max_tokens=8)
        return "YES" in verdict.upper()
    # fallback: keyword coverage
    keywords = [k.strip() for k in arg.split(",")]
    return sum(k.lower() in response.lower() for k in keywords) >= len(keywords) // 2


JUDGES = {
    "exact_prefix": lambda r, a, _: judge_exact_prefix(r, a),
    "pattern": lambda r, a, _: judge_pattern(r, a),
    "contains_all": lambda r, a, _: judge_contains_all(r, a),
    "contains_any": lambda r, a, _: judge_contains_any(r, a),
    "semantic": judge_semantic,
}


def get_model_id(endpoint: str) -> str:
    url = endpoint.rstrip("/") + "/v1/models"
    req = Request(url, headers={"Authorization": f"Bearer {API_KEY}"})
    try:
        with urlopen(req, timeout=10) as resp:
            d = json.load(resp)
        return d["data"][0].get("id", "unknown")
    except Exception:
        return "unknown"


def run_eval(endpoints: list[str], judge_endpoint: str | None, output_path: Path):
    cases = json.loads(EVAL_PATH.read_text())
    results: list[dict] = []
    variant_summaries: dict[str, dict] = {}

    for ep in endpoints:
        model_id = get_model_id(ep)
        print(f"\n=== Endpoint: {ep}  model: {model_id} ===")
        pass_count = 0
        ep_results = []

        for case in cases:
            cid = case["id"]
            prompt = case["prompt"]
            judge_fn = JUDGES.get(case["judge"], JUDGES["pattern"])

            response, tokens, elapsed = chat(ep, prompt)
            passed = judge_fn(response, case["judge_arg"], judge_endpoint)

            if passed:
                pass_count += 1
                marker = "✅"
            else:
                marker = "❌"

            print(f"  {marker} [{case['category']}] {cid} | {tokens}tok {elapsed:.1f}s")
            if not passed:
                print(f"       expected: {case['canonical'][:80]}")
                print(f"       got:      {response.strip()[:80]}")

            ep_results.append({
                "endpoint": ep,
                "model_id": model_id,
                "id": cid,
                "category": case["category"],
                "passed": passed,
                "tokens": tokens,
                "elapsed_s": round(elapsed, 2),
                "response_excerpt": response.strip()[:120],
            })

        total = len(cases)
        accuracy = round(pass_count / total, 3)
        print(f"\n  SUMMARY {ep}: pass={pass_count}/{total} accuracy={accuracy}")
        variant_summaries[ep] = {
            "model_id": model_id,
            "pass": pass_count,
            "total": total,
            "accuracy": accuracy,
        }
        results.extend(ep_results)

    # Write JSONL
    output_path.parent.mkdir(parents=True, exist_ok=True)
    with output_path.open("w") as f:
        for r in results:
            f.write(json.dumps(r) + "\n")

    # Print comparative summary
    print("\n=== MULTI-VARIANT COMPARISON ===")
    best_ep = max(variant_summaries, key=lambda e: variant_summaries[e]["accuracy"])
    for ep, s in variant_summaries.items():
        flag = " ← BEST" if ep == best_ep else ""
        print(f"  {s['model_id']:40s} accuracy={s['accuracy']:.3f} ({s['pass']}/{s['total']}){flag}")

    print(f"\nresults: {output_path}")
    return variant_summaries


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--endpoints", nargs="+", default=["http://127.0.0.1:8001"],
                        help="Model endpoints to evaluate")
    parser.add_argument("--judge-endpoint", default=None,
                        help="Endpoint for LLM-as-judge (semantic cases)")
    parser.add_argument("--output", default=None,
                        help="Output JSONL path (default: .b00t/ralph/correctness-<ts>.jsonl)")
    args = parser.parse_args()

    ts = time.strftime("%Y%m%dT%H%M%S")
    out = Path(args.output) if args.output else OUTPUT_DIR / f"correctness-{ts}.jsonl"

    summaries = run_eval(args.endpoints, args.judge_endpoint, out)

    # Exit non-zero if any variant is below 50% accuracy
    min_acc = min(s["accuracy"] for s in summaries.values())
    sys.exit(0 if min_acc >= 0.5 else 1)


if __name__ == "__main__":
    main()
