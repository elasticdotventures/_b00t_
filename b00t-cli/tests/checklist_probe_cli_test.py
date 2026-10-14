"""End-to-end 0/1/2 checklist contract through the actual b00t CLI."""

import json
import os
from pathlib import Path
import subprocess
import tempfile
import unittest


class ChecklistProbeCliTests(unittest.TestCase):
    def test_three_valued_cli(self) -> None:
        cases = json.loads((Path(__file__).parent / "fixtures/checklist_probe_cases.json").read_text())
        binary = os.environ.get("B00T_TEST_BINARY", "b00t")
        for case in cases:
            with self.subTest(case=case["id"]), tempfile.TemporaryDirectory() as directory:
                gate = case["gate"]
                probe = gate["probe"]
                text = '[b00t]\nname = "fixture"\ntype = "checklist"\n[[b00t.check]]\nid = "probe"\n'
                if "rhai" in gate:
                    text += f'rhai = {json.dumps(gate["rhai"])}\n'
                text += '[b00t.check.probe]\n'
                text += f'argv = {json.dumps(probe["argv"])}\n'
                text += f'timeout_ms = {probe["timeout_ms"]}\n'
                (Path(directory) / "fixture.checklist.toml").write_text(text)
                result = subprocess.run(
                    [binary, "-p", directory, "is", "fixture", "--json"],
                    capture_output=True, text=True, timeout=5,
                )
                self.assertEqual(result.returncode, case["exit_code"], result.stderr)
                response = json.loads(result.stdout)
                self.assertEqual(response["disposition"]["status"], case["status"])


if __name__ == "__main__":
    unittest.main()
