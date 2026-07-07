"""Tests for scripts/b00t-harvest.py — lfmf lesson mining from Claude Code transcripts.

Fixture: scripts/tests/fixtures/sample_session.jsonl (synthesized, real schema shape).
Run: uv run --with pytest pytest scripts/tests/ -q
"""

import importlib.util
import json
import subprocess
import sys
from pathlib import Path

import pytest

SCRIPTS_DIR = Path(__file__).resolve().parents[1]
SCRIPT = SCRIPTS_DIR / "b00t-harvest.py"
FIXTURE = Path(__file__).resolve().parent / "fixtures" / "sample_session.jsonl"


def _load_module():
    spec = importlib.util.spec_from_file_location("b00t_harvest", SCRIPT)
    mod = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(mod)
    return mod


@pytest.fixture(scope="module")
def harvest():
    return _load_module()


@pytest.fixture(scope="module")
def candidates(harvest):
    return harvest.harvest_file(FIXTURE)


def test_fixture_parses_without_crash(candidates):
    assert isinstance(candidates, list)
    assert len(candidates) >= 3


def test_error_resolution_pair_found(candidates):
    pairs = [c for c in candidates if c["kind"] == "error_resolution"]
    assert pairs, "expected at least one error->resolution candidate"
    dry_run = [c for c in pairs if "--dry-run" in c["evidence_excerpt"] or "--dry-run" in c["candidate_lesson"]]
    assert dry_run, "expected the b00t-cli --dry-run failure to be captured"
    c = dry_run[0]
    assert c["tool"] == "Bash"
    assert "b00t-cli" in c["candidate_lesson"]


def test_marker_extraction(candidates):
    markers = [c for c in candidates if c["kind"] == "marker"]
    assert any("keep-id" in c["candidate_lesson"] for c in markers), "🤓 line should be harvested"
    lfmf = [c for c in markers if "lfmf" in c["evidence_excerpt"] or c.get("marker") == "lfmf"]
    assert lfmf, "explicit b00t lfmf invocation should be harvested"
    assert max(c["confidence"] for c in lfmf) >= 0.9


def test_repeated_command_evolution(candidates):
    evo = [c for c in candidates if c["kind"] == "evolution"]
    assert any(c["tool"] == "curl" or "curl" in c["candidate_lesson"] for c in evo), (
        "curl flag evolution (3 variants ending in success) should be detected"
    )


def test_dedup_collapses_repeated_marker(candidates):
    # The 🤓 keep-id lesson appears twice (different sentence prefixes);
    # marker extraction takes the text after the marker so dedup emits it once.
    keepid = [c for c in candidates
              if c["kind"] == "marker" and c.get("marker") == "🤓"
              and "keep-id" in c["candidate_lesson"]]
    assert len(keepid) == 1


def test_candidate_shape(candidates):
    required = {"source_file", "session_ts", "kind", "tool", "candidate_lesson", "evidence_excerpt", "confidence"}
    for c in candidates:
        assert required <= set(c.keys()), f"missing keys in {c}"
        assert len(c["evidence_excerpt"]) <= 200
        assert 0.0 <= c["confidence"] <= 1.0
        assert "\n" not in c["candidate_lesson"]
        assert ": " in c["candidate_lesson"], "lesson must be lfmf-shaped 'topic: lesson'"


def test_normalization_dedup_key(harvest):
    a = harvest.dedup_key("marker", "Podman:  rootless bind mounts need /tmp/xyz123 keep-id")
    b = harvest.dedup_key("marker", "podman: rootless bind mounts need /tmp/abc987 keep-id")
    assert a == b


def test_cli_end_to_end(tmp_path):
    out = tmp_path / "out.jsonl"
    proc = subprocess.run(
        [sys.executable, str(SCRIPT), str(FIXTURE.parent), "-o", str(out), "--report", "--no-state"],
        capture_output=True, text=True, timeout=60,
    )
    assert proc.returncode == 0, proc.stderr
    lines = [json.loads(l) for l in out.read_text().splitlines() if l.strip()]
    assert len(lines) >= 3
    assert "error_resolution" in proc.stdout
    assert "marker" in proc.stdout


def _run_cli(tmp_path, out_name, *extra):
    out = tmp_path / out_name
    state = tmp_path / "harvest_state.json"
    proc = subprocess.run(
        [sys.executable, str(SCRIPT), str(FIXTURE.parent), "-o", str(out),
         "--state-file", str(state), *extra],
        capture_output=True, text=True, timeout=60,
    )
    assert proc.returncode == 0, proc.stderr
    lines = [json.loads(l) for l in out.read_text().splitlines() if l.strip()]
    return lines, proc.stdout, state


def test_watermark_suppresses_reprocessing(tmp_path):
    """Second run over the SAME unchanged corpus should skip every file and
    emit zero candidates -- the whole point of task: 'clear lessons after a
    harvest so we don't reprocess'."""
    first, out1, state = _run_cli(tmp_path, "run1.jsonl")
    assert len(first) >= 3
    assert state.exists()

    second, out2, _ = _run_cli(tmp_path, "run2.jsonl")
    assert second == []
    assert "skipped" in out2
    assert "wrote 0 new candidate" in out2


def test_watermark_full_bypasses_state(tmp_path):
    """--full re-emits everything even with a populated watermark."""
    first, _, _ = _run_cli(tmp_path, "run1.jsonl")
    full, _, _ = _run_cli(tmp_path, "run2.jsonl", "--full")
    assert len(full) == len(first)


def test_watermark_reset_state_clears_history(tmp_path):
    _run_cli(tmp_path, "run1.jsonl")
    reset, out, _ = _run_cli(tmp_path, "run2.jsonl", "--reset-state")
    assert len(reset) >= 3
    assert "wrote 0 new" not in out
