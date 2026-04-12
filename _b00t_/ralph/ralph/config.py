from __future__ import annotations

import os
from dataclasses import dataclass, field
from pathlib import Path


def _project_root() -> Path:
    return Path(__file__).resolve().parent.parent


def _default_codex_prompt_file() -> Path:
    """Return the default codex prompt file path."""
    return _project_root() / "CLAUDE.md"


@dataclass(frozen=True)
class RalphConfig:
    """Configuration for Ralph tool execution."""

    # Tool selection
    tool: str = "amp"  # amp, claude, codex, or opencode

    # TaskMaster configuration
    use_mcp: bool = False
    taskmaster_url: str | None = None

    # Codex-specific configuration
    codex_prompt_file: Path = field(default_factory=_default_codex_prompt_file)
    codex_model: str = "gpt-5-codex"
    codex_reasoning_effort: str = "high"
    codex_sandbox: str = "workspace-write"
    codex_full_auto: bool = True
    codex_extra_args: str = ""

    # OpenCode-specific configuration
    opencode_model: str = "gpt-4"
    opencode_extra_args: str = ""

    @classmethod
    def from_env(cls, tool: str = "amp", use_mcp: bool = False) -> RalphConfig:
        """Load configuration from environment variables with defaults."""
        root = _project_root()
        return cls(
            tool=tool,
            use_mcp=use_mcp,
            taskmaster_url=os.environ.get("TASKMASTER_URL"),
            codex_prompt_file=Path(os.environ.get("CODEX_PROMPT_FILE", str(root / "CLAUDE.md"))),
            codex_model=os.environ.get("CODEX_MODEL", "gpt-5-codex"),
            codex_reasoning_effort=os.environ.get("CODEX_REASONING_EFFORT", "high"),
            codex_sandbox=os.environ.get("CODEX_SANDBOX", "workspace-write"),
            codex_full_auto=os.environ.get("CODEX_FULL_AUTO", "true").lower() == "true",
            codex_extra_args=os.environ.get("CODEX_EXTRA_ARGS", ""),
            opencode_model=os.environ.get("OPENCODE_MODEL", cls.opencode_model),
            opencode_extra_args=os.environ.get("OPENCODE_EXTRA_ARGS", cls.opencode_extra_args),
        )
