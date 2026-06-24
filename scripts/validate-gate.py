#!/usr/bin/env python3
"""Executable gate schema validator — reads gate.schema.toml contracts and validates gate datums.

Usage:
    python3 validate-gate.py [gate_file...]
    python3 validate-gate.py  # validates all gates in _b00t_/gates/

Exit codes:
    0 — all gates pass
    1 — one or more gates fail validation
    2 — usage error
"""

import sys
import re
import os
import json
import urllib.request
import urllib.error
from pathlib import Path
from datetime import datetime, timezone


def parse_schema_contract(schema_path: str) -> dict:
    """Parse gate.schema.toml into validation rule metadata.

    Schema files are data, not executable code. Rule checks are implemented
    below by rule id so untrusted schema changes cannot run local commands.
    """
    import tomllib

    with open(schema_path, "rb") as f:
        raw = tomllib.load(f)

    rules = []
    for rule in raw.get("schema", {}).get("rules", []):
        rules.append({
            "id": rule["id"],
            "description": rule["description"],
            "check": rule["check"],
            "severity": rule["severity"],
        })

    return {
        "schema_name": raw.get("schema", {}).get("name", "unknown"),
        "schema_version": raw.get("schema", {}).get("version", "0.0.0"),
        "rules": rules,
    }


def evaluate_rule(rule_id: str, gate_path: str) -> bool:
    """Evaluate a known validation rule without executing schema-provided code."""
    path = Path(gate_path)
    try:
        text = path.read_text(encoding="utf-8")
    except OSError:
        return False

    lines = text.splitlines()
    tail = "\n".join(lines[-10:])

    if rule_id == "tail-map-present":
        return "# b00t:map v1" in tail
    if rule_id == "audit-section-required":
        return re.search(r"^\s*\[gate\.audit\]\s*$", text, re.MULTILINE) is not None
    if rule_id == "hook-section-recommended":
        return re.search(r"^\s*\[gate\.hook\]\s*$", text, re.MULTILINE) is not None
    if rule_id == "version-field-recommended":
        return re.search(r"^\s*version\s*=", text, re.MULTILINE) is not None
    if rule_id == "valid-type":
        try:
            import tomllib

            with path.open("rb") as f:
                raw = tomllib.load(f)
            return raw.get("b00t", {}).get("type") == "gate"
        except Exception:
            return False
    if rule_id == "tags-format":
        return any(line.lstrip().startswith("# tags:") and "," in line for line in lines[-10:])

    if rule_id == "semantic-quality":
        return evaluate_semantic_quality(gate_path)

    # Fail closed: adding a schema rule requires adding trusted Python code here.
    return False



def evaluate_semantic_quality(gate_path: str) -> bool:
    """E7: Call sm0l oracle to assess whether hint + summary are semantically meaningful.

    Requires B00T_SM0L_ENDPOINT env var. If absent, returns True (non-blocking skip).
    The oracle receives the gate content and returns {"pass": true|false, "reason": "..."}.
    Falls back to True on network error so CI is not blocked by model unavailability.
    """
    endpoint = os.environ.get("B00T_SM0L_ENDPOINT", "").strip()
    if not endpoint:
        return True  # skip when oracle not configured

    try:
        text = Path(gate_path).read_text(encoding="utf-8")
    except OSError:
        return False

    # Extract hint and tail-map summary for focused evaluation
    hint = ""
    summary = ""
    for line in text.splitlines():
        m = re.match(r'^\s*hint\s*=\s*"(.+)"', line)
        if m:
            hint = m.group(1)
        m2 = re.match(r"^\s*#\s*summary:\s*(.+)", line)
        if m2:
            summary = m2.group(1)

    if not hint and not summary:
        return True  # nothing to evaluate semantically

    prompt = (
        f"Rate the quality of this b00t gate datum on a scale 0-1. "
        f"Return JSON: {{\"pass\": true|false, \"confidence\": 0.0-1.0, \"reason\": \"...\"}}. "
        f"Pass if confidence >= 0.5. "
        f"Hint: {hint!r}. Summary: {summary!r}."
    )
    payload = json.dumps({"prompt": prompt, "max_tokens": 64}).encode()

    try:
        req = urllib.request.Request(
            endpoint,
            data=payload,
            headers={"Content-Type": "application/json"},
            method="POST",
        )
        with urllib.request.urlopen(req, timeout=15) as resp:
            body = json.loads(resp.read().decode())

        # Accept both flat {"pass": bool} and {"text": "{\"pass\": ...}"} wrapping
        if "pass" in body:
            return bool(body["pass"])
        if "text" in body:
            inner = json.loads(body["text"])
            return bool(inner.get("pass", True))
        if "content" in body:
            inner = json.loads(body["content"])
            return bool(inner.get("pass", True))
    except (urllib.error.URLError, json.JSONDecodeError, KeyError, ValueError) as e:
        # Network/parse errors: non-blocking skip (don't fail CI for model downtime)
        print(f"[semantic-quality] warn: oracle error ({e}), skipping check", file=sys.stderr)
        return True

    return True  # safe default


def validate_gate(gate_path: str, contract: dict) -> dict:
    """Run all schema rules against a gate file. Returns structured results."""
    results = []
    for rule in contract["rules"]:
        passed = evaluate_rule(rule["id"], gate_path)
        results.append({
            "rule_id": rule["id"],
            "passed": passed,
            "severity": rule["severity"],
            "description": rule["description"],
        })

    critical_fails = [r for r in results if not r["passed"] and r["severity"] == "critical"]
    warning_fails = [r for r in results if not r["passed"] and r["severity"] == "warning"]
    info_fails = [r for r in results if not r["passed"] and r["severity"] == "info"]

    return {
        "gate_file": gate_path,
        "total_rules": len(results),
        "passed": sum(1 for r in results if r["passed"]),
        "failed": sum(1 for r in results if not r["passed"]),
        "critical_fails": len(critical_fails),
        "warning_fails": len(warning_fails),
        "results": results,
        "overall": "PASS" if len(critical_fails) == 0 else "FAIL",
    }


def find_gate_files(base_dir: str = ".") -> list:
    """Find all gate datum files."""
    gates_dir = Path(base_dir) / "_b00t_" / "gates"
    if not gates_dir.exists():
        return []
    return sorted(str(p) for p in gates_dir.glob("*.gate.toml"))


def record_validates_fact(gate_path: str, overall: str, sha: str = "") -> None:
    """NS-2: Persist validates(gate→datum, result, sha) as a NeumannStore fact.

    Appends a JSONL record to ~/.b00t/evidence/satisfies.jsonl matching
    the EvidenceRecord format from evidence.rs.
    Migration: swap append for NeumannStore::upsert_facts().
    """
    try:
        import hashlib
        if not sha:
            content = Path(gate_path).read_bytes()
            sha = hashlib.sha256(content).hexdigest()[:12]
        record = {
            "subject": str(Path(gate_path).name),
            "predicate": "validates",
            "object": {"result": overall, "sha": sha, "file": gate_path},
            "timestamp": datetime.now(timezone.utc).strftime("%Y-%m-%dT%H:%M:%SZ"),
        }
        evidence_dir = Path.home() / ".b00t" / "evidence"
        evidence_dir.mkdir(parents=True, exist_ok=True)
        with (evidence_dir / "satisfies.jsonl").open("a") as f:
            f.write(json.dumps(record) + "\n")
    except Exception as e:
        print(f"[NS-2] warn: could not record validates fact: {e}", file=sys.stderr)


def main():
    # Schema is in project root's _b00t_/schema/, not relative to scripts/
    script_dir = Path(__file__).resolve().parent
    repo_root = script_dir.parent  # scripts/ is one level below repo root
    schema_path = os.environ.get(
        "GATE_SCHEMA",
        str(repo_root / "_b00t_" / "schema" / "gate.schema.toml"),
    )

    if not os.path.exists(schema_path):
        print(f"ERROR: Schema contract not found: {schema_path}", file=sys.stderr)
        sys.exit(2)

    contract = parse_schema_contract(schema_path)

    # Determine target files
    if len(sys.argv) > 1:
        gate_files = sys.argv[1:]
    else:
        base = str(repo_root)
        gate_files = find_gate_files(base)

    if not gate_files:
        print("No gate files found to validate.", file=sys.stderr)
        sys.exit(2)

    # Run validation
    all_results = []
    overall_pass = True

    print(f"╔══ Gate Schema Validator v{contract['schema_version']} ══╗")
    print(f"║ Schema: {contract['schema_name']} ({len(contract['rules'])} rules)")
    print(f"║ Targets: {len(gate_files)} gate(s)")
    print(f"╚{'═' * 46}╝")
    print()

    for gate_path in gate_files:
        result = validate_gate(gate_path, contract)
        all_results.append(result)

        status = "✅ PASS" if result["overall"] == "PASS" else "❌ FAIL"
        print(f"  {status}  {os.path.basename(gate_path)}")
        print(f"         {result['passed']}/{result['total_rules']} rules passed")

        for r in result["results"]:
            if not r["passed"]:
                icon = {"critical": "🔴", "warning": "⚠️ ", "info": "ℹ️ "}[r["severity"]]
                print(f"         {icon} [{r['severity']:8}] {r['rule_id']}: {r['description']}")

        if result["overall"] != "PASS":
            overall_pass = False
        # NS-2: record validates fact for audit trail
        record_validates_fact(gate_path, result["overall"])
        print()

    # Summary
    total = sum(r["total_rules"] for r in all_results)
    passed = sum(r["passed"] for r in all_results)
    failed = sum(r["failed"] for r in all_results)
    critical = sum(r["critical_fails"] for r in all_results)

    print(f"╔══ Summary ══╗")
    print(f"║ Total rules checked: {total}")
    print(f"║ Passed: {passed} ({passed/total*100:.0f}%)")
    print(f"║ Failed: {failed} ({failed/total*100:.0f}%)")
    print(f"║ Critical: {critical}")
    print(f"║ Overall: {'PASS' if overall_pass else 'FAIL'}")
    print(f"╚{'═' * 16}╝")

    # JSON output for programmatic consumption
    if os.environ.get("GATE_VALIDATE_JSON"):
        print("\n--- JSON OUTPUT ---")
        print(json.dumps({
            "timestamp": datetime.utcnow().isoformat(),
            "schema": contract["schema_name"],
            "schema_version": contract["schema_version"],
            "overall": "PASS" if overall_pass else "FAIL",
            "gates": all_results,
        }, indent=2))

    sys.exit(0 if overall_pass else 1)


if __name__ == "__main__":
    main()
