"""Tests for native b00t LangChain tools."""

import tomllib
from pathlib import Path

from b00t_langchain_agent.b00t_tools import B00tToolset


# ---------------------------------------------------------------------------
# Acceptance criterion #1 — datum advertises capability + required runtimes
# ---------------------------------------------------------------------------

def test_operator_datum_advertises_capability_and_runtime() -> None:
    """langchain-operator.agent.tomllm must declare capability and Python runtime."""
    datum = Path(__file__).parents[2] / "_b00t_" / "langchain-operator.agent.tomllm"
    if not datum.exists():
        import pytest
        pytest.skip(f"datum not found at {datum}")

    with open(datum, "rb") as fh:
        data = tomllib.load(fh)

    provides = data.get("b00t", {}).get("provides", {})
    assert provides.get("capability") == "operator-automation", "missing capability=operator-automation"
    assert provides.get("protocol") == "b00t-operator-v1", "missing protocol"

    runtime = data.get("b00t", {}).get("runtime", {})
    assert runtime.get("python"), "b00t.runtime.python missing — operator role must declare required Python version"
    assert runtime.get("manager") == "uv", "b00t.runtime.manager must be uv"
    packages = runtime.get("packages", [])
    assert any("fastmcp" in p for p in packages), "fastmcp must be listed in b00t.runtime.packages"
    assert any("langchain" in p for p in packages), "langchain-core must be listed in b00t.runtime.packages"


def _write_decision_tree(tmp_path: Path) -> None:
    (tmp_path / "operator-decision-tree.tomllm").write_text(
        """
[b00t]
name = "operator-decision-tree"

[[decision_tree.rules]]
name = "stack-activate"
match_any = ["start stack", "activate profile"]
action = "stack.activate"
tool = "b00t_stack_control"
script = "operator-agent"
notes = "bring up stack"
""".strip()
    )


def test_operator_decide_matches_rule(tmp_path: Path) -> None:
    """Decision tree should route common operator requests."""
    _write_decision_tree(tmp_path)
    toolset = B00tToolset(tmp_path, runner=lambda *_args, **_kwargs: "")

    result = toolset.operator_decide("please start stack inference-qwen3")

    assert "rule=stack-activate" in result
    assert "action=stack.activate" in result
    assert "tool=b00t_stack_control" in result


def test_stack_control_builds_activate_command(tmp_path: Path) -> None:
    """Stack control should build the expected activate invocation."""
    calls: list[tuple[list[str], dict[str, str] | None]] = []

    def runner(args: list[str], env=None) -> str:
        calls.append((args, env))
        return "ok"

    toolset = B00tToolset(tmp_path, runner=runner)
    toolset.stack_control("activate", "inference-qwen3", dry_run=False, force=True)

    assert calls == [(["stack", "activate", "inference-qwen3", "--force"], None)]


def test_operator_script_sets_env(tmp_path: Path) -> None:
    """Operator script tool should route through env-driven Rhai dispatch."""
    calls: list[tuple[list[str], dict[str, str] | None]] = []

    def runner(args: list[str], env=None) -> str:
        calls.append((args, env))
        return "ok"

    toolset = B00tToolset(tmp_path, runner=runner)
    toolset.operator_script("stack.deactivate", "download-mode", dry_run=True, note="teardown")

    assert calls[0][0] == ["script", "run", "operator-agent"]
    assert calls[0][1]["OPERATOR_ACTION"] == "stack.deactivate"
    assert calls[0][1]["OPERATOR_TARGET"] == "download-mode"
    assert calls[0][1]["OPERATOR_DRY_RUN"] == "true"
    assert calls[0][1]["OPERATOR_NOTE"] == "teardown"


def test_task_capture_builds_correct_command(tmp_path: Path) -> None:
    """task_capture should delegate to `b00t task add` with priority, tags, description."""
    calls: list[list[str]] = []

    def runner(args: list[str], env=None) -> str:
        calls.append(args)
        return "ok"

    toolset = B00tToolset(tmp_path, runner=runner)
    toolset.task_capture("Fix GHCR pipeline", priority=1, tags="ci,ghcr", description="just --list fails")

    assert calls[0] == [
        "task", "add", "-p", "1", "-t", "ci,ghcr", "-d", "just --list fails",
        "Fix GHCR pipeline",
    ]


def test_task_capture_minimal_args(tmp_path: Path) -> None:
    """task_capture with only title should omit optional flags."""
    calls: list[list[str]] = []

    def runner(args: list[str], env=None) -> str:
        calls.append(args)
        return "ok"

    toolset = B00tToolset(tmp_path, runner=runner)
    toolset.task_capture("simple task")

    assert calls[0] == ["task", "add", "-p", "3", "simple task"]


def test_task_capture_rejects_bad_priority(tmp_path: Path) -> None:
    """task_capture must raise ValueError for out-of-range priority."""
    import pytest

    toolset = B00tToolset(tmp_path, runner=lambda *_a, **_k: "")
    with pytest.raises(ValueError, match="priority must be 1-4"):
        toolset.task_capture("bad priority task", priority=5)
