#!/usr/bin/env python3
"""
b00t pipe-agent — route deterministic process output through sm0l local LLM.
Emits ONLY errors/warnings. Deduplicates. Never silently passes raw output.

Pattern: executive frontier model delegates noisy process output to sm0l tier.
sm0l collapses 50k-token cargo/podman output to <200 relevant tokens.

Priority endpoint chain (ControlFlow<Break, Continue>):
  1. B00T_AI_SM0L_BASE env  (explicit override)
  2. localhost:8001/v1      (b00t hive ch0nky — currently serving Qwen3.6-27B MTP)
  3. localhost:8000/v1      (any local OpenAI-compat server)
  4. HuggingFace Inference API (serverless, requires HF_TOKEN)
  5. exit 2                 (governance gate — never silently pass-through)

Task templates (--task <name>):
  cargo     — cargo build/test/clippy: extract error[E*], panics, test failures
  podman    — container lifecycle: pull errors, permission denied, OOM
  systemd   — journalctl/systemctl: Failed, Error, killed
  rust      — alias for cargo
  hive      — b00t hive activate/plan: gate failures, resource conflicts
  general   — default: any error/failure/panic line

Usage:
  cargo test 2>&1            | python3 sm0l-filter.py --task cargo
  podman pull image 2>&1     | python3 sm0l-filter.py --task podman
  journalctl -u svc 2>&1     | python3 sm0l-filter.py --task systemd
  <any cmd> 2>&1             | python3 sm0l-filter.py
  <any cmd> 2>&1             | python3 sm0l-filter.py --quiet  # CI: silent on success
"""
import sys, os, re, hashlib, argparse, json, urllib.request

# ─── Task templates ───────────────────────────────────────────────────────────
_SUFFIX = (
    " Return ONLY the matching lines verbatim. "
    "If there are no matching lines: respond with exactly the two characters: -\n"
    "Do NOT write 'no errors', 'no issues', 'clean', or any other commentary. "
    "Two characters: hyphen newline. That is all."
)

TASKS = {
    "cargo": (
        "You are a Rust build/test log filter. "
        "Extract ONLY lines indicating: compile errors (error[E*]), test failures (FAILED, panicked at), "
        "linker errors, missing crate errors. "
        "If the same error code appears multiple times output it ONCE with count suffix ×N. "
        "Suppress: warnings, note:, help:, ---> source paths (unless on the same line as error:)."
        + _SUFFIX
    ),
    "podman": (
        "You are a container runtime log filter. "
        "Extract ONLY: pull failures, image not found, permission denied, OOM killed, "
        "exec format error, exit code non-zero, CDI errors, ERRO level log lines."
        + _SUFFIX
    ),
    "systemd": (
        "You are a systemd/journalctl log filter. "
        "Extract ONLY lines containing: Failed, failed, Error, error, killed, segfault, "
        "start-limit-hit, dependency failed."
        + _SUFFIX
    ),
    "hive": (
        "You are a b00t hive log filter. "
        "Extract ONLY: resource gate failures (insufficient RAM/GPU/CPU), "
        "service start failures, exclusion group conflicts, guard BLOCK violations."
        + _SUFFIX
    ),
    "general": (
        "You are a terse error extractor. "
        "Extract ONLY lines containing the words: error, failed, panic, fatal, denied, killed "
        "(case-insensitive). Deduplicate repeated lines with count suffix ×N."
        + _SUFFIX
    ),
}
TASKS["rust"] = TASKS["cargo"]

# ─── ANSI strip + normalize ───────────────────────────────────────────────────
_ANSI_RE = re.compile(r"\x1b\[[0-9;]*[mGKH]")

def _normalize(line: str) -> str:
    return _ANSI_RE.sub("", line).strip()

def preprocess(raw: str, max_unique: int = 500) -> tuple[str, dict]:
    """Deduplicate + strip ANSI before sending to LLM. Returns (text, stats)."""
    seen: dict[str, int] = {}
    order: list[str] = []
    for line in raw.splitlines():
        norm = _normalize(line)
        if not norm:
            continue
        h = hashlib.md5(norm.encode()).hexdigest()[:8]
        if h not in seen:
            seen[h] = 0
            order.append(norm)
        seen[h] += 1
        if len(order) >= max_unique:
            break

    lines_with_counts = []
    for norm in order:
        h = hashlib.md5(norm.encode()).hexdigest()[:8]
        n = seen[h]
        lines_with_counts.append(f"{norm}" if n == 1 else f"{norm}  ×{n}")

    return "\n".join(lines_with_counts), {
        "raw_lines": len(raw.splitlines()),
        "unique_lines": len(order),
        "truncated": len(order) >= max_unique,
    }

# ─── Endpoint discovery ───────────────────────────────────────────────────────
def _probe(url: str) -> bool:
    for path in ("/health", "/v1/models"):
        try:
            with urllib.request.urlopen(url.rstrip("/") + path, timeout=2) as r:
                return r.status < 500
        except Exception:
            pass
    return False

def discover_endpoint(model_hint: str) -> tuple[str, str, str]:
    """(base_url, model, api_key)"""
    override = os.environ.get("B00T_AI_SM0L_BASE")
    if override:
        m = os.environ.get("B00T_AI_SM0L_MODEL", model_hint or "sm0l")
        return override, m, os.environ.get("B00T_AI_SM0L_KEY", "local-b00t")

    for url, default_model in [
        ("http://127.0.0.1:8001/v1", "ch0nky"),
        ("http://127.0.0.1:8000/v1", "sm0l"),
    ]:
        if _probe(url):
            return url, model_hint or default_model, "local-b00t"

    hf_token = os.environ.get("HF_TOKEN", "")
    if hf_token:
        return "hf://", model_hint or "Qwen/Qwen2.5-3B-Instruct", hf_token

    print(
        "[pipe-agent] ERROR: no endpoint at :8001/:8000 and no HF_TOKEN.\n"
        "  Run: b00t hive activate inference-qwen36-27b-mtp-podman",
        file=sys.stderr,
    )
    sys.exit(2)

# ─── LLM call ─────────────────────────────────────────────────────────────────
def call_openai(base_url: str, model: str, api_key: str, system: str, content: str) -> str:
    payload = json.dumps({
        "model": model,
        "messages": [
            {"role": "system", "content": system},
            {"role": "user",   "content": content},
        ],
        "max_tokens": 1024,
        "temperature": 0.0,
    }).encode()
    req = urllib.request.Request(
        base_url.rstrip("/") + "/chat/completions",
        data=payload,
        headers={"Content-Type": "application/json", "Authorization": f"Bearer {api_key}"},
        method="POST",
    )
    try:
        with urllib.request.urlopen(req, timeout=60) as r:
            data = json.loads(r.read())
            return data["choices"][0]["message"]["content"].strip()
    except urllib.error.HTTPError as e:
        body = e.read().decode(errors="replace")[:200]
        print(f"[pipe-agent] ERROR: HTTP {e.code} from endpoint: {body}", file=sys.stderr)
        return "-"
    except (urllib.error.URLError, json.JSONDecodeError, KeyError) as e:
        print(f"[pipe-agent] ERROR: {type(e).__name__}: {e}", file=sys.stderr)
        return "-"

def call_hf(model_id: str, token: str, system: str, content: str) -> str:
    try:
        from huggingface_hub import InferenceClient  # type: ignore
    except ImportError:
        print("[pipe-agent] ERROR: uv pip install huggingface-hub", file=sys.stderr)
        sys.exit(2)
    client = InferenceClient(model=model_id, token=token)
    result = client.chat_completion(
        messages=[{"role": "system", "content": system}, {"role": "user", "content": content}],
        max_tokens=1024,
        temperature=0.01,
    )
    return result.choices[0].message.content.strip()

# ─── Main ─────────────────────────────────────────────────────────────────────
def session_log(session_id: str, tag: str, prompt: str, input_text: str, output: str) -> str:
    """Write sm0l I/O to ephemeral session dir. Returns output file path."""
    import time
    base = f"/tmp/b00t-sm0l-{session_id}"
    os.makedirs(base, exist_ok=True)
    n = len([f for f in os.listdir(base) if f.endswith(".output")])
    prefix = f"{base}/{n:04d}-{tag}"
    for kind, content in [("prompt", prompt), ("input", input_text), ("output", output)]:
        with open(f"{prefix}.{kind}", "w", errors="replace") as f:
            f.write(content)
    return f"{prefix}.output"


def main():
    ap = argparse.ArgumentParser(description="b00t pipe-agent: sm0l LLM error filter")
    ap.add_argument("--task",       default="general", choices=list(TASKS),
                    help="Task template (cargo|podman|systemd|hive|rust|general)")
    ap.add_argument("--model",      default="", help="Model name override")
    ap.add_argument("--max-bytes",  type=int, default=48_000,
                    help="Max stdin bytes before truncation")
    ap.add_argument("--quiet",      action="store_true",
                    help="Suppress output on success (CI mode)")
    ap.add_argument("--stats",      action="store_true",
                    help="Print line-count stats to stderr")
    ap.add_argument("--session-id", default="", metavar="ID",
                    help="Session ID for ephemeral log (/tmp/b00t-sm0l-<ID>/). "
                         "ch0nky receives log path to reference if needed.")
    args = ap.parse_args()

    raw = sys.stdin.buffer.read(args.max_bytes).decode("utf-8", errors="replace")
    if not raw.strip():
        sys.exit(0)

    deduplicated, stats = preprocess(raw)
    if args.stats:
        trunc = " [TRUNCATED]" if stats["truncated"] else ""
        print(
            f"[pipe-agent] {stats['raw_lines']} raw → {stats['unique_lines']} unique{trunc}",
            file=sys.stderr,
        )

    base_url, model, api_key = discover_endpoint(args.model)
    system_prompt = TASKS[args.task]

    if base_url == "hf://":
        result = call_hf(model, api_key, system_prompt, deduplicated)
    else:
        result = call_openai(base_url, model, api_key, system_prompt, deduplicated)

    # Log to session if requested — full I/O preserved for ch0nky to reference
    if args.session_id:
        log_path = session_log(args.session_id, args.task, system_prompt, deduplicated, result)
        print(f"[pipe-agent:log] {log_path}", file=sys.stderr)

    # Sentinel: model returns "-" to signal clean/no-errors
    if result and result.strip() not in ("-", "") and not args.quiet:
        print(result)
    elif result and result.strip() not in ("-", "") and args.quiet:
        print(result)  # quiet suppresses nothing on actual errors

if __name__ == "__main__":
    main()
