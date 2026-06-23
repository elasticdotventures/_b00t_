import importlib.util
import tempfile
import unittest
from pathlib import Path


def load_validate_gate_module():
    script = Path(__file__).resolve().parents[1] / "validate-gate.py"
    spec = importlib.util.spec_from_file_location("validate_gate", script)
    module = importlib.util.module_from_spec(spec)
    assert spec.loader is not None
    spec.loader.exec_module(module)
    return module


VALID_GATE = """[b00t]
name = "write-guard"
type = "gate"
version = "0.1.0"
hint = "test gate"

[gate]
mode = "mandatory"
trigger = "on-write"
condition = "test"

[gate.audit]
log_file = "audit.jsonl"
log_format = "jsonl"
fields = ["timestamp"]

[gate.hook]
timeout_ms = 1000
fallback = "deny"

# b00t:map v1
# summary: test gate
# tags: test, gate
# tier: sm0l
# cmds: validate
# complexity: 1
"""


class ValidateGateTest(unittest.TestCase):
    def test_unknown_schema_rule_fails_closed_without_executing_check(self):
        module = load_validate_gate_module()
        with tempfile.TemporaryDirectory() as tmp:
            tmp_path = Path(tmp)
            gate = tmp_path / "test.gate.toml"
            marker = tmp_path / "gate-pwn"
            gate.write_text(VALID_GATE, encoding="utf-8")
            contract = {
                "rules": [
                    {
                        "id": "malicious-rule",
                        "description": "malicious schema checks must not execute",
                        "check": f"touch {marker}",
                        "severity": "critical",
                    }
                ]
            }

            result = module.validate_gate(str(gate), contract)

            self.assertEqual(result["overall"], "FAIL")
            self.assertFalse(marker.exists())

    def test_known_rules_validate_structurally_valid_gate(self):
        module = load_validate_gate_module()
        with tempfile.TemporaryDirectory() as tmp:
            gate = Path(tmp) / "test.gate.toml"
            gate.write_text(VALID_GATE, encoding="utf-8")
            contract = {
                "rules": [
                    {"id": "tail-map-present", "description": "", "severity": "critical"},
                    {"id": "audit-section-required", "description": "", "severity": "critical"},
                    {"id": "hook-section-recommended", "description": "", "severity": "warning"},
                    {"id": "version-field-recommended", "description": "", "severity": "warning"},
                    {"id": "valid-type", "description": "", "severity": "critical"},
                    {"id": "tags-format", "description": "", "severity": "info"},
                ]
            }

            result = module.validate_gate(str(gate), contract)

            self.assertEqual(result["overall"], "PASS")
            self.assertEqual(result["failed"], 0)


if __name__ == "__main__":
    unittest.main()
