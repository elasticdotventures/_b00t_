"""Exercise health probes against actual isolated Git repositories."""

import json
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest


class SystemNormalGitTests(unittest.TestCase):
    def test_fixture_observations(self) -> None:
        tests_dir = Path(__file__).resolve().parent
        cases = json.loads((tests_dir / "fixtures/system_normal_git_cases.json").read_text())
        probe = tests_dir.parents[1] / "_b00t_/scripts/system-normal-probe.py"
        for case in cases:
            with self.subTest(case=case["name"]), tempfile.TemporaryDirectory() as directory:
                for name, contents in case.get("files", {}).items():
                    (Path(directory) / name).write_text(contents)
                for step in case["setup"]:
                    setup = subprocess.run(step["argv"], cwd=directory, check=False, capture_output=True, timeout=5)
                    self.assertEqual(setup.returncode, step["exit_code"], setup.stderr)
                result = subprocess.run(
                    [sys.executable, str(probe), case["check"]],
                    cwd=directory, capture_output=True, text=True, timeout=5,
                )
                self.assertEqual(result.returncode, case["exit_code"], result.stderr)
                self.assertIn(result.stdout.strip(), {"satisfied", "violated", "unknown"})


if __name__ == "__main__":
    unittest.main()
