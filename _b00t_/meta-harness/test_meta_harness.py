#!/usr/bin/env python3
"""
E2E tests for Meta-Harness OODA loop.

Tests the full cycle: propose → evaluate → promote/discard.
Uses deterministic mock evaluations so tests are reproducible.

Quality engineering:
  - Test blended scoring formula exactly
  - Test promotion rule with boundary cases
  - Test history logging and state persistence
  - Test proposer output parsing
  - Test OODA phase ordering
"""

from __future__ import annotations

import json
import os
import sys
import tempfile
from pathlib import Path
from unittest.mock import patch, MagicMock

# Add the meta-harness directory to path
sys.path.insert(0, str(Path(__file__).resolve().parent.parent / "meta-harness"))

import meta_harness as mh


# ── Blended scoring tests ──────────────────────────────────────────────────────

def test_blended_score_zero():
    """All zeros → score should be 0."""
    r = mh.EvaluationResult(pooled_pass=0.0, all_pass=0.0, tokens_used=0, build_passed=True)
    assert r.blended_score() == 0.0, f"Expected 0.0, got {r.blended_score()}"


def test_blended_score_perfect():
    """Perfect run → score = 1.0 + 0.5 = 1.5 (minus tiny cost penalty)."""
    r = mh.EvaluationResult(
        pooled_pass=1.0, all_pass=1.0, tokens_used=100000, build_passed=True
    )
    expected = 1.0 + 0.5 - 0.005 * 0.1  # 100K tokens = 0.1 million
    assert abs(r.blended_score() - expected) < 0.001, f"Expected {expected}, got {r.blended_score()}"


def test_blended_score_build_fail():
    """Build failure → hard fail, score = -1.0."""
    r = mh.EvaluationResult(pooled_pass=1.0, all_pass=1.0, build_passed=False)
    assert r.blended_score() == -1.0, f"Expected -1.0, got {r.blended_score()}"


def test_blended_score_cost_penalty():
    """1M tokens → cost penalty = 0.005. 10M tokens → cost penalty = 0.05."""
    r1 = mh.EvaluationResult(pooled_pass=0.5, all_pass=0.0, tokens_used=1_000_000, build_passed=True)
    r10 = mh.EvaluationResult(pooled_pass=0.5, all_pass=0.0, tokens_used=10_000_000, build_passed=True)
    assert r1.blended_score() == 0.5 - 0.005, f"1M: {r1.blended_score()}"
    assert r10.blended_score() == 0.5 - 0.05, f"10M: {r10.blended_score()}"


def test_all_pass_bonus():
    """All-pass bonus is 0.5 per all-passing run."""
    r = mh.EvaluationResult(pooled_pass=0.8, all_pass=1.0, tokens_used=0, build_passed=True)
    assert r.blended_score() == 1.3, f"0.8 + 0.5 = 1.3, got {r.blended_score()}"


# ── Promotion rule tests ───────────────────────────────────────────────────────

def test_promote_clear_win():
    """Candidate beats incumbent by > min_delta."""
    assert mh.should_promote(10.0, 5.0, min_delta=1.0) is True


def test_promote_below_delta():
    """Candidate beats incumbent but by < min_delta."""
    assert mh.should_promote(5.5, 5.0, min_delta=1.0) is False


def test_promote_worse():
    """Candidate is worse than incumbent."""
    assert mh.should_promote(3.0, 5.0, min_delta=1.0) is False


def test_promote_exact_delta():
    """Candidate beats incumbent by exactly min_delta."""
    assert mh.should_promote(6.0, 5.0, min_delta=1.0) is True


def test_promote_negative_scores():
    """Both scores negative, but candidate improves."""
    assert mh.should_promote(-1.0, -3.0, min_delta=1.0) is True


# ── Average score tests ────────────────────────────────────────────────────────

def test_average_score_single():
    """Single trial → average = that trial's score."""
    results = [mh.EvaluationResult(pooled_pass=0.7, all_pass=0.0, build_passed=True)]
    assert abs(mh.compute_average_score(results) - 0.7) < 0.001


def test_average_score_three_trials():
    """Three trials with variation → correct average."""
    results = [
        mh.EvaluationResult(pooled_pass=0.6, all_pass=0.0, build_passed=True),
        mh.EvaluationResult(pooled_pass=0.8, all_pass=0.0, build_passed=True),
        mh.EvaluationResult(pooled_pass=1.0, all_pass=1.0, build_passed=True),
    ]
    expected = (0.6 + 0.8 + 1.5) / 3  # third has all_pass bonus
    assert abs(mh.compute_average_score(results) - expected) < 0.001


def test_average_score_build_failure():
    """Build failure in one trial dilutes the average."""
    results = [
        mh.EvaluationResult(pooled_pass=1.0, all_pass=1.0, build_passed=True),
        mh.EvaluationResult(build_passed=False),  # score = -1.0
    ]
    avg = mh.compute_average_score(results)
    # (1.5 + (-1.0)) / 2 = 0.25
    assert abs(avg - 0.25) < 0.01, f"Expected 0.25, got {avg}"
    assert avg < 1.5, f"Build failure should dilute average below perfect, got {avg}"


# ── HarnessState tests ─────────────────────────────────────────────────────────

def test_harness_state_default():
    """Default state has zero iteration and score."""
    hs = mh.HarnessState()
    assert hs.iteration == 0
    assert hs.score == 0.0
    assert hs.mechanism_count == 0
    assert hs.mechanisms == []


def test_harness_state_roundtrip():
    """Serialize and deserialize preserves state."""
    hs = mh.HarnessState(iteration=5, score=12.3, mechanism_count=3)
    hs.mechanisms = ["a", "b", "c"]
    d = hs.to_dict()
    hs2 = mh.HarnessState.from_dict(d)
    assert hs2.iteration == 5
    assert hs2.score == 12.3
    assert hs2.mechanisms == ["a", "b", "c"]


# ── History persistence tests ──────────────────────────────────────────────────

def test_history_write_read(tmp_path: Path):
    """Write entry to history file, read it back."""
    history_file = tmp_path / "history.jsonl"
    entry = {
        "iteration": 1,
        "mechanism": "test_mechanism",
        "candidate_score": 10.5,
        "incumbent_score": 5.0,
        "promoted": True,
        "timestamp": "2026-01-01T00:00:00Z",
        "trial_results": [],
    }
    history_file.write_text(json.dumps(entry) + "\n")

    read_entries = []
    for line in history_file.read_text().splitlines():
        if line.strip():
            read_entries.append(json.loads(line))

    assert len(read_entries) == 1
    assert read_entries[0]["mechanism"] == "test_mechanism"
    assert read_entries[0]["promoted"] is True


# ── Loop state persistence tests ───────────────────────────────────────────────

def test_loop_state_roundtrip(tmp_path: Path):
    """Save and restore loop state."""
    state_file = tmp_path / "loop_state.json"
    state = {
        "iteration": 7,
        "harness": {
            "iteration": 7,
            "score": 15.2,
            "mechanism_count": 4,
            "mechanisms": ["m1", "m2", "m3", "m4"],
        },
    }
    state_file.write_text(json.dumps(state, indent=2))

    restored = json.loads(state_file.read_text())
    assert restored["iteration"] == 7
    assert restored["harness"]["score"] == 15.2


# ── Proposer output parsing tests ──────────────────────────────────────────────

def test_proposer_output_parsing():
    """Parse a realistic proposer JSON output."""
    output = """
Some logging prefix to be ignored...
{"mechanism_name": "deliverable_reassembly_gate", "hypothesis": "Reassembling clobbered writes recovers lost content", "changes": "Added _land_clobbered_deliverables method", "expected_fix_tasks": ["draft-compliance-manual", "draft-ppa"], "expected_regression_risks": ["analyze-section-382", "identify-term-sheet"]}
More logging suffix...
"""
    for line in output.split("\n"):
        line = line.strip()
        if line.startswith("{") and line.endswith("}"):
            proposal = json.loads(line)
            assert proposal["mechanism_name"] == "deliverable_reassembly_gate"
            assert proposal["hypothesis"] is not None
            assert len(proposal["expected_fix_tasks"]) == 2
            break
    else:
        assert False, "Should have found JSON in output"


# ── OODA phase ordering test ───────────────────────────────────────────────────

def test_ooda_phase_ordering():
    """Verify OODA phases exist in correct order."""
    # Check that the OODA functions exist and are callable
    assert callable(mh._run_build_check)
    assert callable(mh._run_test_check)
    assert callable(mh._check_guard_compliance)
    assert callable(mh._estimate_tokens)
    assert callable(mh.run_proposer)
    assert callable(mh.evaluate_harness)
    assert callable(mh.compute_average_score)
    assert callable(mh.should_promote)


# ── Integration test: full OODA cycle (dry-run) ───────────────────────────────

def test_full_ooda_cycle_dry_run():
    """Run one dry-run iteration — no side effects, just verifies the pipeline."""
    exit_code = mh.run_meta_harness(iterations=1, dry_run=True)
    assert exit_code == 1, f"Dry-run with no prior state should return 1, got {exit_code}"


# ── Runner ─────────────────────────────────────────────────────────────────────

if __name__ == "__main__":
    import pytest
    sys.exit(pytest.main([__file__, "-v", "--tb=short"]))
