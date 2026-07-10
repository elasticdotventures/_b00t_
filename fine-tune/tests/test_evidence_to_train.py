"""Cases in fixtures/evidence_cases.json (#595) — data in JSON per b00t rule."""
import json
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
from evidence_to_train import convert  # noqa: E402

FIXTURE = Path(__file__).parent / "fixtures" / "evidence_cases.json"


def test_convert_matches_fixture():
    fx = json.loads(FIXTURE.read_text())
    assert list(convert(fx["input_lines"])) == fx["expected"]


def test_alpaca_columns_present():
    fx = json.loads(FIXTURE.read_text())
    for example in convert(fx["input_lines"]):
        assert set(example) >= {"instruction", "input", "response"}
        assert example["verified"] is True
