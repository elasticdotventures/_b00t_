#!/usr/bin/env python3
"""b00t-agent-doctor — census of _b00t_/*.agent.toml agents, health-check local ones."""
import argparse, json, pathlib, re, sys, tomllib, urllib.request

HEALTH_URL = "http://127.0.0.1:8001/health"
TIMEOUT = 2  # seconds


def classify(text: str, name: str) -> str:
    """Classify an agent as local, claude, or stub from its TOML text."""
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


def health_check() -> str:
    try:
        resp = urllib.request.urlopen(HEALTH_URL, timeout=TIMEOUT)
        return "PASS" if resp.status == 200 else "FAIL"
    except Exception:
        return "FAIL"


def main():
    parser = argparse.ArgumentParser(description="Agent-Doctor census")
    parser.add_argument("--check", action="store_true", default=True, help="Run health checks (default)")
    parser.add_argument("--json", action="store_true", help="Emit JSON array instead of TSV")
    args = parser.parse_args()

    base = pathlib.Path(__file__).resolve().parent.parent / "_b00t_"
    agents = sorted(base.glob("*.agent.toml"))
    if not agents:
        print("No .agent.toml files found.", file=sys.stderr)
        sys.exit(0)

    results = []
    any_fail = False
    for fpath in agents:
        text = fpath.read_text()
        data = tomllib.loads(text)
        name = data.get("b00t", {}).get("name", fpath.stem.replace(".agent", ""))
        cls = classify(text, name)

        if cls == "local":
            verdict = health_check()
            if verdict == "FAIL":
                any_fail = True
        else:
            verdict = "SKIP"

        results.append({"name": name, "class": cls, "verdict": verdict})

    if args.json:
        print(json.dumps(results))
    else:
        for r in results:
            print(f"{r['name']}\t{r['class']}\t{r['verdict']}")

    sys.exit(1 if any_fail else 0)


if __name__ == "__main__":
    main()
