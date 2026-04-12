"""Native b00t tools exposed to the LangChain operator agent."""

from __future__ import annotations

import os
import subprocess
from pathlib import Path

from langchain_core.tools import BaseTool, StructuredTool
from pydantic import BaseModel, Field

from .decision_tree import DecisionTree


class StackControlInput(BaseModel):
    """Args for stack lifecycle control."""

    action: str = Field(description="activate or deactivate")
    profile: str = Field(description="Stack or profile name")
    dry_run: bool = Field(default=True, description="Preview without mutating state")
    force: bool = Field(default=False, description="Skip resource gate checks on activate")


class TaskCaptureInput(BaseModel):
    """Args for task capture."""

    title: str = Field(description="Task title")
    priority: int = Field(default=3, description="Priority 1-4")
    tags: str = Field(default="", description="Comma-separated tags")
    description: str = Field(default="", description="Optional task description")


class DecisionInput(BaseModel):
    """Args for decision-tree evaluation."""

    request: str = Field(description="Natural-language operator request")


class OperatorScriptInput(BaseModel):
    """Args for the internal operator Rhai script."""

    action: str = Field(description="Operator action, for example stack.activate")
    target: str = Field(default="", description="Action target such as a stack/profile name")
    dry_run: bool = Field(default=True, description="Preview without mutating state")
    force: bool = Field(default=False, description="Pass force through when supported")
    note: str = Field(default="", description="Short execution note")


class B00tToolset:
    """Strict wrapper around approved local `b00t` actions."""

    def __init__(self, datum_path: Path, runner=None) -> None:
        self.datum_path = datum_path
        self.runner = runner or self._run_b00t
        tree_path = datum_path / "operator-decision-tree.tomllm"
        self.decision_tree = DecisionTree.from_file(tree_path) if tree_path.exists() else None

    def build_tools(self) -> list[BaseTool]:
        """Return native tools for the operator agent."""
        return [
            StructuredTool.from_function(
                self.hive_status,
                name="b00t_hive_status",
                description="Read current hive/system resource status before dispatching work.",
            ),
            StructuredTool.from_function(
                self.stack_control,
                name="b00t_stack_control",
                description="Activate or deactivate a named b00t stack/profile.",
                args_schema=StackControlInput,
            ),
            StructuredTool.from_function(
                self.task_capture,
                name="b00t_task_capture",
                description="Create a native b00t task for a bug or follow-on item.",
                args_schema=TaskCaptureInput,
            ),
            StructuredTool.from_function(
                self.operator_decide,
                name="b00t_operator_decide",
                description="Consult the operator decision tree and return the preferred next action.",
                args_schema=DecisionInput,
            ),
            StructuredTool.from_function(
                self.operator_script,
                name="b00t_operator_script",
                description="Run the internal operator Rhai script with explicit action routing.",
                args_schema=OperatorScriptInput,
            ),
        ]

    def prompt_context(self) -> str:
        """Prompt-safe decision-tree context."""
        if not self.decision_tree:
            return ""
        return self.decision_tree.summary()

    def hive_status(self) -> str:
        """Run `b00t hive status`."""
        return self.runner(["hive", "status"])

    def stack_control(self, action: str, profile: str, dry_run: bool = True, force: bool = False) -> str:
        """Run `b00t stack activate|deactivate`."""
        if action not in {"activate", "deactivate"}:
            raise ValueError("action must be activate or deactivate")

        args = ["stack", action, profile]
        if dry_run:
            args.append("--dry-run")
        if action == "activate" and force:
            args.append("--force")
        return self.runner(args)

    def task_capture(
        self,
        title: str,
        priority: int = 3,
        tags: str = "",
        description: str = "",
    ) -> str:
        """Capture a task in the native b00t queue."""
        args = ["task", "add", "-p", str(priority)]
        if tags:
            args.extend(["-t", tags])
        if description:
            args.extend(["-d", description])
        args.append(title)
        return self.runner(args)

    def operator_decide(self, request: str) -> str:
        """Match a request against the operator decision tree."""
        if not self.decision_tree:
            return "No operator decision tree configured."

        match = self.decision_tree.match(request)
        if not match:
            return "No decision-tree rule matched; fall back to direct tool reasoning."

        rule = match.rule
        return (
            f"rule={rule.name}\n"
            f"keyword={match.keyword}\n"
            f"action={rule.action}\n"
            f"tool={rule.tool}\n"
            f"script={rule.script or ''}\n"
            f"notes={rule.notes or ''}"
        )

    def operator_script(
        self,
        action: str,
        target: str = "",
        dry_run: bool = True,
        force: bool = False,
        note: str = "",
    ) -> str:
        """Execute the internal operator Rhai script with env-driven routing."""
        env = {
            "OPERATOR_ACTION": action,
            "OPERATOR_TARGET": target,
            "OPERATOR_DRY_RUN": str(dry_run).lower(),
            "OPERATOR_FORCE": str(force).lower(),
            "OPERATOR_NOTE": note,
        }
        return self.runner(["script", "run", "operator-agent"], env=env)

    def _run_b00t(self, args: list[str], env: dict[str, str] | None = None) -> str:
        """Execute `b00t` and return merged output."""
        full_env = os.environ.copy()
        if env:
            full_env.update(env)

        proc = subprocess.run(
            ["b00t", *args],
            capture_output=True,
            text=True,
            env=full_env,
            check=False,
        )
        output = "\n".join(part for part in [proc.stdout.strip(), proc.stderr.strip()] if part).strip()
        if proc.returncode != 0:
            raise RuntimeError(output or f"b00t {' '.join(args)} failed with exit code {proc.returncode}")
        return output
