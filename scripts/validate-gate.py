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
from pathlib import Path
from datetime import datetime


def parse_schema_contract(schema_path: str) -> dict:
    """Parse gate.schema.toml into executable validation rules.

    The schema is a CONTRACT — not configuration. Each rule has a
    check command that can be executed as a shell one-liner.
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


def validate_gate(gate_path: str, contract: dict) -> dict:
    """Run all schema rules against a gate file. Returns structured results."""
    results = []
    for rule in contract["rules"]:
        # Substitute $FILE placeholder with actual path
        check_cmd = (
            rule["check"]
            .replace("$FILE", gate_path)
            .replace("'", "'\"'\"'")  # escaping for shell
        )

        # Run the check
        exit_code = os.system(f'bash -c \'{check_cmd}\' 2>/dev/null')

        passed = exit_code == 0
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
