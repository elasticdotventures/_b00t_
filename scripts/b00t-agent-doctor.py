#!/usr/bin/env python3
"""b00t-agent-doctor — census of _b00t_/*.agent.toml agents, health-check
local ones, and (optionally) fix + register them.

BD-001 (census)  : classify each agent local/claude/stub, print PASS|FAIL|SKIP.
BD-002 (probe)   : prefer the structured [b00t.agent.inference] block when an
                   agent declares one — GET health + POST a 1-token completion.
BD-003 (fix)     : --fix injects the standard inference block into agents that
                   classify as local-inference but lack one, and rewrites the
                   stale `inference-qwen36-27b.service` unit name.
BD-004 (register): --register nats-pubs a presence message for each PASS agent.
"""
import argparse
import datetime as dt
import json
import os
import pathlib
import re
import shutil
import subprocess
import sys
import tomllib
import urllib.request

HEALTH_URL = "http://127.0.0.1:8001/health"
TIMEOUT = 2  # seconds, GET
COMPLETION_TIMEOUT = 3  # seconds, POST

STALE_SERVICE = "inference-qwen36-27b.service"
STALE_SERVICE_FIXED = "inference-qwen36-27b-mtp-podman.service"
STALE_SERVICE_RE = re.compile(re.escape(STALE_SERVICE))

NATS_SUBJECT = "b00t.hive.mesh.discovery.presence"
NATS_SECRETS_FILE = pathlib.Path.home() / ".b00t" / "secrets" / "hive-nats.env"


def classify(text: str, name: str) -> str:
    """Classify an agent as local, claude, or stub from its TOML text.

    This is the BD-001 heuristic — kept as the fallback path for agents that
    do not (yet) declare a [b00t.agent.inference] block (see BD-003 --fix).
    """
    # local: ch0nky or qwen anywhere, or hive_profile starts with inference-,
    # or a model= line containing local/ch0nky or local/sm0l
    if "ch0nky" in text or "qwen" in text:
        return "local"
    if re.search(r'hive_profile\s*=\s*"inference-', text):
        return "local"
    if re.search(r'model\s*=\s*".*(?:local/ch0nky|local/sm0l)', text):
        return "local"
    # claude: model line contains claude, sonnet, or frontier
    model_line = re.search(r'model\s*=\s*"([^"]*)"', text)
    if model_line and any(kw in model_line.group(1) for kw in ("claude", "sonnet", "frontier")):
        return "claude"
    return "stub"


def get_inference_block(data: dict):
    """Return the [b00t.agent.inference] table if present, else None."""
    return data.get("b00t", {}).get("agent", {}).get("inference")


def http_get_ok(url: str, timeout: float = TIMEOUT) -> bool:
    if not url:
        return False
    try:
        resp = urllib.request.urlopen(url, timeout=timeout)
        return resp.status == 200
    except Exception:
        return False


def health_check(url: str = HEALTH_URL) -> str:
    return "PASS" if http_get_ok(url) else "FAIL"


def probe_completion(endpoint: str, model: str, protocol: str, timeout: float = COMPLETION_TIMEOUT) -> bool:
    """POST a 1-token completion request. True on a 2xx response.

    Only `protocol = "openai"` is implemented against the real
    /chat/completions wire contract. acp/rpc agents have no documented
    minimal-completion shape yet, so their completion probe is a no-op
    (verdict then rests on the health GET alone) rather than a guess.
    """
    if not endpoint:
        return False
    if protocol != "openai":
        return True
    url = endpoint.rstrip("/") + "/chat/completions"
    body = json.dumps(
        {
            "model": model or "unknown",
            "messages": [{"role": "user", "content": "ping"}],
            "max_tokens": 1,
        }
    ).encode("utf-8")
    req = urllib.request.Request(
        url, data=body, headers={"Content-Type": "application/json"}, method="POST"
    )
    try:
        resp = urllib.request.urlopen(req, timeout=timeout)
        return 200 <= resp.status < 300
    except Exception:
        return False


def pick_model(text: str) -> str:
    """Reuse an existing top-level model = "..." line, else the default local slot."""
    m = re.search(r'^\s*model\s*=\s*"([^"]+)"', text, re.MULTILINE)
    if m:
        return m.group(1)
    return "qwen36-local/ch0nky"


def inject_inference_block(text: str, model: str) -> str:
    """Insert the standard [b00t.agent.inference] block right after [b00t.agent],
    before whatever table follows it (mirrors the phi5-shacl.agent.toml convention).
    Falls back to inserting before the tail-map comment, else EOF, when there is
    no [b00t.agent] table to anchor on.
    """
    had_trailing_newline = text.endswith("\n")
    lines = text.splitlines()

    block = [
        "[b00t.agent.inference]",
        'endpoint = "http://127.0.0.1:8001/v1"   # OpenAI-compatible base',
        'health   = "http://127.0.0.1:8001/health"',
        f'model    = "{model}"',
        'protocol = "openai"                      # openai | acp | rpc',
        "required = true                          # false ⇒ Claude-backed / stub ⇒ doctor SKIPs",
    ]

    header_idx = None
    for i, line in enumerate(lines):
        if line.strip() == "[b00t.agent]":
            header_idx = i
            break

    if header_idx is not None:
        insert_at = len(lines)
        for i in range(header_idx + 1, len(lines)):
            if re.match(r"^\[\S", lines[i]):
                insert_at = i
                break
    else:
        insert_at = len(lines)
        for i, line in enumerate(lines):
            if line.strip() == "# b00t:map v1":
                insert_at = i
                break

    prefix = lines[:insert_at]
    while prefix and prefix[-1].strip() == "":
        prefix.pop()
    suffix = lines[insert_at:]

    new_lines = prefix + [""] + block + [""] + suffix
    out = "\n".join(new_lines)
    return out + "\n" if had_trailing_newline else out


def fix_stale_service(text: str):
    """Rewrite the stale inference-qwen36-27b.service ref inside an
    after = [...] line. Returns the new text, or None if nothing matched."""
    lines = text.splitlines()
    changed = False
    for i, line in enumerate(lines):
        if re.match(r"^\s*after\s*=", line) and STALE_SERVICE_RE.search(line):
            lines[i] = STALE_SERVICE_RE.sub(STALE_SERVICE_FIXED, line)
            changed = True
    if not changed:
        return None
    had_trailing_newline = text.endswith("\n")
    out = "\n".join(lines)
    return out + "\n" if had_trailing_newline else out


def apply_patch(fpath: pathlib.Path, content: str) -> bool:
    """Write content to fpath via `b00t-cli patch apply <file> - --yes`
    (never sed/direct write for datum files — see repo CLAUDE.md)."""
    try:
        proc = subprocess.run(
            ["b00t-cli", "patch", "apply", str(fpath), "-", "--yes"],
            input=content,
            text=True,
            capture_output=True,
            timeout=15,
        )
    except FileNotFoundError:
        print(f"FIX-SKIP {fpath.name}: b00t-cli not found on PATH", file=sys.stderr)
        return False
    except Exception as e:
        print(f"FIX-FAIL {fpath.name}: {e}", file=sys.stderr)
        return False
    if proc.returncode != 0:
        print(f"FIX-FAIL {fpath.name}: {proc.stderr.strip() or proc.stdout.strip()}", file=sys.stderr)
        return False
    print(f"FIXED {fpath.name}")
    return True


def run_fix(agents: list[pathlib.Path]) -> None:
    for fpath in agents:
        text = fpath.read_text()
        try:
            data = tomllib.loads(text)
        except Exception as e:
            print(f"FIX-SKIP {fpath.name}: TOML parse error: {e}", file=sys.stderr)
            continue

        name = data.get("b00t", {}).get("name", fpath.stem.replace(".agent", ""))
        working = text
        touched = False

        # 1. inject the standard inference block into local-inference agents that lack one
        if get_inference_block(data) is None and classify(text, name) == "local":
            model = pick_model(text)
            candidate = inject_inference_block(working, model)
            try:
                tomllib.loads(candidate)
            except Exception as e:
                print(f"FIX-SKIP {fpath.name}: injected block would break TOML: {e}", file=sys.stderr)
            else:
                working = candidate
                touched = True

        # 2. rewrite the stale inference-qwen36-27b.service unit reference
        stale_fixed = fix_stale_service(working)
        if stale_fixed is not None:
            working = stale_fixed
            touched = True

        if touched:
            apply_patch(fpath, working)


def load_nats_creds():
    """B00T_HIVE_NATS_USER / B00T_HIVE_NATS_PASSWORD from the environment,
    optionally sourced from ~/.b00t/secrets/hive-nats.env first. Never a
    hardcoded fallback."""
    user = os.environ.get("B00T_HIVE_NATS_USER")
    password = os.environ.get("B00T_HIVE_NATS_PASSWORD")
    if user and password:
        return user, password

    if NATS_SECRETS_FILE.exists():
        env = {}
        try:
            for line in NATS_SECRETS_FILE.read_text().splitlines():
                line = line.strip()
                if not line or line.startswith("#") or "=" not in line:
                    continue
                k, _, v = line.partition("=")
                env[k.strip()] = v.strip().strip('"').strip("'")
        except Exception:
            env = {}
        user = user or env.get("B00T_HIVE_NATS_USER")
        password = password or env.get("B00T_HIVE_NATS_PASSWORD")

    return user, password


def run_register(results: list[dict]) -> None:
    if shutil.which("nats") is None:
        print("SKIP register: `nats` CLI not found on PATH", file=sys.stderr)
        return

    user, password = load_nats_creds()
    if not user or not password:
        print(
            "SKIP register: B00T_HIVE_NATS_USER/B00T_HIVE_NATS_PASSWORD not set "
            "(env or ~/.b00t/secrets/hive-nats.env)",
            file=sys.stderr,
        )
        return

    for r in results:
        if r["verdict"] != "PASS":
            continue
        payload = {
            "agent_id": r["name"],
            "endpoint": r.get("endpoint"),
            "model": r.get("model"),
            "ts": dt.datetime.now(dt.timezone.utc).strftime("%Y-%m-%dT%H:%M:%SZ"),
        }
        cmd = [
            "nats",
            "--user", user,
            "--password", password,
            "pub", NATS_SUBJECT, json.dumps(payload),
        ]
        try:
            proc = subprocess.run(cmd, capture_output=True, text=True, timeout=5)
        except Exception as e:
            print(f"REGISTER-SKIP {r['name']}: nats pub failed to run: {e}", file=sys.stderr)
            continue
        if proc.returncode != 0:
            print(f"REGISTER-SKIP {r['name']}: {proc.stderr.strip() or proc.stdout.strip()}", file=sys.stderr)
        else:
            print(f"REGISTERED {r['name']}")


def main():
    parser = argparse.ArgumentParser(description="Agent-Doctor census + probe + fix + register")
    parser.add_argument("--check", action="store_true", default=True, help="Run health checks (default)")
    parser.add_argument("--json", action="store_true", help="Emit JSON array instead of TSV")
    parser.add_argument("--fix", action="store_true", help="Inject missing [b00t.agent.inference] blocks + fix stale service refs")
    parser.add_argument("--register", action="store_true", help="nats-pub presence for every PASS agent")
    args = parser.parse_args()

    base = pathlib.Path(__file__).resolve().parent.parent / "_b00t_"
    agents = sorted(base.glob("*.agent.toml"))
    if not agents:
        print("No .agent.toml files found.", file=sys.stderr)
        sys.exit(0)

    if args.fix:
        run_fix(agents)
        agents = sorted(base.glob("*.agent.toml"))

    results = []
    any_fail = False
    for fpath in agents:
        text = fpath.read_text()
        data = tomllib.loads(text)
        name = data.get("b00t", {}).get("name", fpath.stem.replace(".agent", ""))
        cls = classify(text, name)
        inf = get_inference_block(data)
        endpoint = None
        model = None

        if inf is not None:
            source = "block"
            required = inf.get("required", True)
            if not required:
                verdict = "SKIP"
            else:
                endpoint = inf.get("endpoint")
                health = inf.get("health")
                model = inf.get("model")
                protocol = inf.get("protocol", "openai")
                ok = http_get_ok(health) and probe_completion(endpoint, model, protocol)
                verdict = "PASS" if ok else "FAIL"
                if verdict == "FAIL":
                    any_fail = True
        else:
            source = "heuristic"
            if cls == "local":
                verdict = health_check()
                endpoint = "http://127.0.0.1:8001/v1"
                if verdict == "FAIL":
                    any_fail = True
            else:
                verdict = "SKIP"

        results.append(
            {
                "name": name,
                "class": cls,
                "verdict": verdict,
                "source": source,
                "endpoint": endpoint,
                "model": model,
            }
        )

    if args.register:
        run_register(results)

    if args.json:
        print(json.dumps(results))
    else:
        for r in results:
            print(f"{r['name']}\t{r['class']}\t{r['verdict']}\t{r['source']}")

    sys.exit(1 if any_fail else 0)


if __name__ == "__main__":
    main()
