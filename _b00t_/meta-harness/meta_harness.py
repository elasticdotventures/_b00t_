#!/usr/bin/env python3
"""
Meta-Harness OODA loop — evolves the agent harness around a frozen model.

Two-layer architecture:
  Outer loop (this script):  propose → evaluate → promote/discard
  Inner loop (Ralph):        execute individual b00t tasks

Protocol (from Lee et al. 2026, Niklaus 2026):
  - Copy-and-adapt: one mechanism per iteration, compounding wins
  - Blended scoring: pooled_pass + 0.5*all_pass - 0.005*tokens/M
  - Promotion: beat incumbent by >= min_delta on three-trial average
  - No test-split peek: proposer only sees dev results

Usage:
  python meta_harness.py --iterations 20 --unattended
  python meta_harness.py --iterations 1 --dry-run
  python meta_harness.py --status
"""

from __future__ import annotations

import argparse
import json
import os
import subprocess
import sys
import time
from datetime import datetime, timezone
from pathlib import Path
from typing import Optional

# ── Configuration ──────────────────────────────────────────────────────────────

HERE = Path(__file__).resolve().parent
STATE_DIR = HERE
FRONTIER_DIR = STATE_DIR / "frontier"
HISTORY_FILE = STATE_DIR / "history.jsonl"
LOOP_STATE_FILE = STATE_DIR / "loop_state.json"

MIN_DELTA = 1.0          # score margin to promote
TRIALS = 3               # evaluations per candidate
MAX_ITERATIONS = 20      # default loop budget
ALL_PASS_WEIGHT = 0.5    # bonus per all-passing run
COST_PENALTY = 0.005     # per million tokens

# ── Data structures ────────────────────────────────────────────────────────────

class HarnessState:
    """Current state of the harness being evolved."""
    def __init__(self, iteration: int = 0, score: float = 0.0, mechanism_count: int = 0):
        self.iteration = iteration
        self.score = score
        self.mechanism_count = mechanism_count
        self.mechanisms: list[str] = []  # names of accepted mechanisms

    def to_dict(self) -> dict:
        return {
            "iteration": self.iteration,
            "score": self.score,
            "mechanism_count": self.mechanism_count,
            "mechanisms": self.mechanisms,
        }

    @classmethod
    def from_dict(cls, d: dict) -> "HarnessState":
        hs = cls(
            iteration=d.get("iteration", 0),
            score=d.get("score", 0.0),
            mechanism_count=d.get("mechanism_count", 0),
        )
        hs.mechanisms = d.get("mechanisms", [])
        return hs


class EvaluationResult:
    """Result of running a candidate harness through CI/tests."""
    def __init__(
        self,
        pooled_pass: float = 0.0,
        all_pass: float = 0.0,
        tokens_used: int = 0,
        build_passed: bool = False,
        tests_passed: int = 0,
        tests_total: int = 0,
        guard_violations: int = 0,
    ):
        self.pooled_pass = pooled_pass
        self.all_pass = all_pass
        self.tokens_used = tokens_used
        self.build_passed = build_passed
        self.tests_passed = tests_passed
        self.tests_total = tests_total
        self.guard_violations = guard_violations

    def blended_score(self) -> float:
        """Compute the blended objective from the Meta-Harness paper."""
        if not self.build_passed:
            return -1.0  # hard fail — cannot promote
        tokens_m = self.tokens_used / 1_000_000
        return (
            self.pooled_pass
            + ALL_PASS_WEIGHT * self.all_pass
            - COST_PENALTY * tokens_m
        )

    def to_dict(self) -> dict:
        return {
            "pooled_pass": self.pooled_pass,
            "all_pass": self.all_pass,
            "tokens_used": self.tokens_used,
            "build_passed": self.build_passed,
            "tests_passed": self.tests_passed,
            "tests_total": self.tests_total,
            "guard_violations": self.guard_violations,
            "blended_score": self.blended_score(),
        }


# ── Evaluation runner ──────────────────────────────────────────────────────────

def evaluate_harness(harness_dir: Path, trials: int = TRIALS) -> list[EvaluationResult]:
    """Run the candidate harness through the evaluation pipeline N times."""
    results = []
    for trial in range(trials):
        result = _run_single_evaluation(harness_dir, trial)
        results.append(result)
    return results


def _run_single_evaluation(harness_dir: Path, trial: int) -> EvaluationResult:
    """Run one evaluation trial — CI build + test + guard check."""
    # ── Build check ────────────────────────────────────────────────────────
    build_passed = _run_build_check()
    if not build_passed:
        return EvaluationResult(build_passed=False)

    # ── Test run ───────────────────────────────────────────────────────────
    tests_passed, tests_total = _run_test_check()

    # ── Guard compliance ────────────────────────────────────────────────────
    guard_violations = _check_guard_compliance()

    # ── Token estimation (from CI logs or heuristic) ──────────────────────
    tokens_used = _estimate_tokens()

    pooled = tests_passed / max(tests_total, 1)
    all_pass = 1.0 if tests_passed == tests_total else 0.0

    return EvaluationResult(
        pooled_pass=pooled,
        all_pass=all_pass,
        tokens_used=tokens_used,
        build_passed=True,
        tests_passed=tests_passed,
        tests_total=tests_total,
        guard_violations=guard_violations,
    )


def _run_build_check() -> bool:
    """Run the CI build check. Returns True if build passes."""
    try:
        # Check if justfile has a build recipe
        result = subprocess.run(
            ["just", "--list"], capture_output=True, text=True, timeout=10
        )
        if "build" in result.stdout:
            proc = subprocess.run(
                ["just", "build"], capture_output=True, text=True, timeout=300
            )
            return proc.returncode == 0
        # Fallback: check if Cargo.toml exists and try cargo check
        if Path("Cargo.toml").exists():
            proc = subprocess.run(
                ["cargo", "check"], capture_output=True, text=True, timeout=300
            )
            return proc.returncode == 0
        return True  # no build system = trivially passes
    except (subprocess.TimeoutExpired, FileNotFoundError):
        return False


def _run_test_check() -> tuple[int, int]:
    """Run tests. Returns (passed, total)."""
    passed, total = 0, 1  # default: assume 1 trivial test
    try:
        # Try just test first
        proc = subprocess.run(
            ["just", "test"], capture_output=True, text=True, timeout=300
        )
        if proc.returncode == 0:
            return 1, 1

        # Try cargo test
        if Path("Cargo.toml").exists():
            proc = subprocess.run(
                ["cargo", "test"], capture_output=True, text=True, timeout=300
            )
            output = proc.stdout + proc.stderr
            # Parse test results from cargo output
            for line in output.split("\n"):
                if "test result:" in line and "passed;" in line:
                    parts = line.split(";")
                    for p in parts:
                        p = p.strip()
                        if "passed" in p:
                            passed = int(p.split()[0])
                        if "failed" in p:
                            failed = int(p.split()[0])
                            total = passed + failed
                            break
                    break
    except (subprocess.TimeoutExpired, FileNotFoundError):
        pass
    return passed, total


def _check_guard_compliance() -> int:
    """Count guard violations in recent operations."""
    # Check if b00t guard audit log exists
    audit_path = Path(".b00t/bouncer-audit.jsonl")
    if not audit_path.exists():
        return 0
    violations = 0
    try:
        for line in audit_path.read_text().splitlines():
            if '"action":"warn"' in line or '"action":"block"' in line:
                violations += 1
    except Exception:
        pass
    return violations


def _estimate_tokens() -> int:
    """Estimate tokens used from CI logs or default."""
    # Heuristic: each test run uses ~50K tokens (context + output)
    return 50000


# ── Promotion logic ────────────────────────────────────────────────────────────

def compute_average_score(results: list[EvaluationResult]) -> float:
    """Average blended score across all trials."""
    scores = [r.blended_score() for r in results]
    return sum(scores) / len(scores) if scores else -1.0


def should_promote(
    candidate_score: float,
    incumbent_score: float,
    min_delta: float = MIN_DELTA,
) -> bool:
    """Promote if candidate beats incumbent by at least min_delta."""
    return candidate_score >= incumbent_score + min_delta


# ── Proposer interface ─────────────────────────────────────────────────────────

def run_proposer(
    harness_state: HarnessState,
    history: list[dict],
    dry_run: bool = False,
) -> Optional[str]:
    """
    Run the proposer (frontier model via OpenCode) to generate ONE mechanism.

    Returns the proposed mechanism name, or None if proposal failed.
    """
    harness_desc = json.dumps(harness_state.to_dict(), indent=2)
    history_desc = json.dumps(history[-5:] if history else [], indent=2)

    prompt = f"""You are a Meta-Harness proposer. Your job: evolve the agent harness.

CURRENT FRONTIER HARNESS:
{harness_desc}

RECENT HISTORY (last 5 iterations):
{history_desc}

PROTOCOL:
1. Copy the current best harness exactly.
2. Add exactly ONE new mechanism. It must help on unfamiliar inputs.
3. Never read the test split — only dev results are available.
4. Write a hypothesis explaining what you changed and why.

OUTPUT FORMAT (JSON only, no markdown):
{{"mechanism_name": "snake_case_name",
  "hypothesis": "what you changed and why",
  "changes": "specific code/prompt change made",
  "expected_fix_tasks": ["task_type_1", "task_type_2"],
  "expected_regression_risks": ["risk_1", "risk_2"]}}"""

    if dry_run:
        print("[DRY RUN] Would call proposer with prompt:")
        print(prompt[:500] + "...")
        return "dry_run_mechanism"

    try:
        result = subprocess.run(
            ["opencode", "run", "--model", "qwen36-local/sm0l", prompt],
            capture_output=True, text=True, timeout=120,
            env={**os.environ, "OPENCODE_NO_COLOR": "1"},
        )
        output = result.stdout.strip()

        # Extract JSON from output (may have logging prefix)
        for line in output.split("\n"):
            line = line.strip()
            if line.startswith("{"):
                proposal = json.loads(line)
                return proposal.get("mechanism_name", "unknown")
    except (subprocess.TimeoutExpired, json.JSONDecodeError, FileNotFoundError):
        pass

    return None


# ── OODA Loop ──────────────────────────────────────────────────────────────────

def run_meta_harness(iterations: int = MAX_ITERATIONS, dry_run: bool = False) -> int:
    """Run the Meta-Harness OODA loop."""

    # ── Load state ─────────────────────────────────────────────────────────
    if LOOP_STATE_FILE.exists():
        state = json.loads(LOOP_STATE_FILE.read_text())
        harness = HarnessState.from_dict(state.get("harness", {}))
        iteration_start = state.get("iteration", 0) + 1
    else:
        harness = HarnessState()
        iteration_start = 1

    history = []
    if HISTORY_FILE.exists():
        for line in HISTORY_FILE.read_text().splitlines():
            if line.strip():
                history.append(json.loads(line))

    print(f"Meta-Harness OODA start — frontier at iteration {harness.iteration}")
    print(f"  score={harness.score:.1f}  mechanisms={harness.mechanism_count}")
    print(f"  mechanisms: {harness.mechanisms}")

    # ── OODA loop ──────────────────────────────────────────────────────────
    for iteration in range(iteration_start, iteration_start + iterations):
        print(f"\n{'═' * 60}")
        print(f"OODA cycle {iteration}")

        # OBSERVE: snapshot current state
        print(f"  OBSERVE: frontier score={harness.score:.1f}, "
              f"history={len(history)} entries")

        # ORIENT: proposer generates one mechanism
        print(f"  ORIENT: running proposer...")
        mechanism = run_proposer(harness, history, dry_run=dry_run)
        if mechanism is None:
            print(f"  ORIENT: proposer failed — skipping iteration")
            continue
        print(f"  ORIENT: proposed mechanism '{mechanism}'")

        # DECIDE: evaluate candidate
        print(f"  DECIDE: evaluating candidate...")
        results = evaluate_harness(FRONTIER_DIR, trials=TRIALS)
        candidate_score = compute_average_score(results)
        print(f"  DECIDE: candidate_score={candidate_score:.2f} "
              f"(incumbent={harness.score:.2f}, delta={candidate_score - harness.score:.2f})")

        # ACT: promote or discard
        if should_promote(candidate_score, harness.score):
            print(f"  ACT: ✅ PROMOTED — score improved by "
                  f"{candidate_score - harness.score:.1f} >= {MIN_DELTA}")
            harness.iteration = iteration
            harness.score = candidate_score
            harness.mechanism_count += 1
            harness.mechanisms.append(mechanism)
        else:
            print(f"  ACT: ❌ DISCARDED — score delta "
                  f"{candidate_score - harness.score:.1f} < {MIN_DELTA}")

        # Log to history
        entry = {
            "iteration": iteration,
            "mechanism": mechanism,
            "candidate_score": candidate_score,
            "incumbent_score": harness.score,
            "promoted": candidate_score >= harness.score,
            "timestamp": datetime.now(timezone.utc).isoformat(),
            "trial_results": [r.to_dict() for r in results],
        }
        history.append(entry)
        with open(HISTORY_FILE, "a") as f:
            f.write(json.dumps(entry) + "\n")

        # Save loop state
        LOOP_STATE_FILE.write_text(json.dumps({
            "iteration": iteration,
            "harness": harness.to_dict(),
        }, indent=2))

        # Check for completion
        if _pending_count() == 0 and harness.score > 0:
            print(f"\n🍰 Meta-Harness mission accomplished in {iteration} iterations!")
            return 0

    print(f"\nMeta-Harness loop ended after {iterations} cycles.")
    print(f"  Final score: {harness.score:.2f}")
    print(f"  Mechanisms: {harness.mechanisms}")
    return 0 if harness.score > 0 else 1


def _pending_count() -> int:
    """Count pending b00t tasks."""
    tasks_path = Path(".b00t/tasks.json")
    if not tasks_path.exists():
        return 0
    try:
        tasks = json.loads(tasks_path.read_text())
        return sum(1 for t in tasks.get("tasks", []) if t.get("status") == "pending")
    except Exception:
        return 0


# ── CLI ────────────────────────────────────────────────────────────────────────

def main() -> int:
    p = argparse.ArgumentParser(description="Meta-Harness OODA loop")
    p.add_argument("--iterations", type=int, default=1, help="Number of OODA cycles")
    p.add_argument("--dry-run", action="store_true", help="Simulate without executing")
    p.add_argument("--status", action="store_true", help="Show current frontier status")
    p.add_argument("--history", action="store_true", help="Show evolution history")
    args = p.parse_args()

    if args.status:
        if LOOP_STATE_FILE.exists():
            state = json.loads(LOOP_STATE_FILE.read_text())
            print(json.dumps(state, indent=2))
        else:
            print("No frontier state — run the loop first.")
        return 0

    if args.history:
        if HISTORY_FILE.exists():
            for line in HISTORY_FILE.read_text().splitlines():
                if line.strip():
                    entry = json.loads(line)
                    print(f"  #{entry['iteration']}: {entry['mechanism']} "
                          f"score={entry['candidate_score']:.1f} "
                          f"{'✅' if entry['promoted'] else '❌'}")
        else:
            print("No history — run the loop first.")
        return 0

    return run_meta_harness(iterations=args.iterations, dry_run=args.dry_run)


if __name__ == "__main__":
    sys.exit(main())
