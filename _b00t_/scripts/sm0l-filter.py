#!/usr/bin/env python3
"""
b00t sm0l-filter — pipe stdin through a sm0l LLM, output only error lines.

Priority endpoint discovery (ControlFlow<Break, Continue> semantics):
  1. B00T_AI_SM0L_BASE env (explicit override)
  2. localhost:8001/v1  (b00t hive ch0nky vllm)
  3. localhost:8000/v1  (local llama-server / any OpenAI-compat)
  4. HuggingFace Inference API (serverless, requires HF_TOKEN)
  5. Exit 2 (no endpoint) — never silently pass through unfiltered

Usage (just recipes call this):
  cargo test 2>&1 | python3 sm0l-filter.py --model sm0l
  cargo build 2>&1 | python3 sm0l-filter.py --model ch0nky --task "output only lines with error:"
"""
import sys, os, argparse, http.client, json, urllib.request

SYSTEM_PROMPT = (
    "You are a terse error extractor. "
    "Output ONLY lines from the input that contain errors, failures, or panics. "
    "Never repeat a line. Never add commentary. If there are no errors, output nothing."
)

def probe_openai_endpoint(base_url: str) -> bool:
    """Return True if the endpoint responds to /health or /v1/models."""
    for path in ("/health", "/v1/models"):
        try:
            req = urllib.request.Request(base_url.rstrip("/") + path, method="GET")
            with urllib.request.urlopen(req, timeout=2) as r:
                return r.status < 500
        except Exception:
            pass
    return False


def call_openai(base_url: str, model: str, api_key: str, content: str, task: str) -> str:
    prompt = task or SYSTEM_PROMPT
    payload = json.dumps({
        "model": model,
        "messages": [
            {"role": "system", "content": prompt},
            {"role": "user", "content": content},
        ],
        "max_tokens": 2048,
        "temperature": 0.0,
    }).encode()
    req = urllib.request.Request(
        base_url.rstrip("/") + "/chat/completions",
        data=payload,
        headers={
            "Content-Type": "application/json",
            "Authorization": f"Bearer {api_key}",
        },
        method="POST",
    )
    with urllib.request.urlopen(req, timeout=30) as r:
        resp = json.loads(r.read())
    return resp["choices"][0]["message"]["content"].strip()


def call_hf_inference(model_id: str, content: str, task: str) -> str:
    """Fallback: HuggingFace Inference API (serverless)."""
    try:
        from huggingface_hub import InferenceClient  # type: ignore
    except ImportError:
        print("[sm0l-filter] ERROR: huggingface_hub not installed. Run: uv pip install huggingface-hub", file=sys.stderr)
        sys.exit(2)
    prompt = task or SYSTEM_PROMPT
    client = InferenceClient(model=model_id, token=os.environ.get("HF_TOKEN"))
    result = client.text_generation(
        f"<|system|>\n{prompt}\n<|user|>\n{content}\n<|assistant|>\n",
        max_new_tokens=2048,
        temperature=0.01,
    )
    return result.strip()


def discover_endpoint(model_hint: str) -> tuple[str, str, str]:
    """Return (base_url, model_name, api_key) using priority order."""
    override = os.environ.get("B00T_AI_SM0L_BASE")
    if override:
        m = os.environ.get("B00T_AI_SM0L_MODEL", "sm0l")
        return override, m, os.environ.get("B00T_AI_SM0L_KEY", "local-b00t")

    for url, model, key in [
        ("http://127.0.0.1:8001/v1", "ch0nky",   "local-b00t"),
        ("http://127.0.0.1:8000/v1", "sm0l",      "local-b00t"),
    ]:
        if probe_openai_endpoint(url):
            return url, model_hint or model, key

    # HF Inference API fallback
    hf_token = os.environ.get("HF_TOKEN", "")
    if hf_token:
        return "hf://", model_hint or "Qwen/Qwen2-0.5B-Instruct", hf_token

    print("[sm0l-filter] ERROR: no local endpoint at :8001/:8000 and no HF_TOKEN. "
          "Run: b00t hive activate inference-qwen36-27b", file=sys.stderr)
    sys.exit(2)


def main():
    ap = argparse.ArgumentParser(description="sm0l LLM stdin filter")
    ap.add_argument("--model", default="", help="Model name override")
    ap.add_argument("--task",  default="", help="Custom system prompt override")
    ap.add_argument("--max-bytes", type=int, default=32000,
                    help="Max stdin bytes (truncate to fit context)")
    args = ap.parse_args()

    stdin_data = sys.stdin.buffer.read(args.max_bytes).decode("utf-8", errors="replace")
    if not stdin_data.strip():
        sys.exit(0)

    base_url, model, api_key = discover_endpoint(args.model)

    if base_url == "hf://":
        result = call_hf_inference(model, stdin_data, args.task)
    else:
        result = call_openai(base_url, model, api_key, stdin_data, args.task)

    if result:
        print(result)


if __name__ == "__main__":
    main()
