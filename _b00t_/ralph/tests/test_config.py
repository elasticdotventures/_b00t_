"""Unit tests for Ralph configuration loading."""

from __future__ import annotations

import os
from pathlib import Path
from unittest.mock import patch

import pytest

from ralph.config import RalphConfig


def test_config_from_env_defaults() -> None:
    """Test RalphConfig.from_env() with default values."""
    with patch.dict(os.environ, {}, clear=False):
        config = RalphConfig.from_env(tool="amp")

    assert config.tool == "amp"
    assert config.codex_model == "gpt-5-codex"
    assert config.codex_reasoning_effort == "high"
    assert config.codex_sandbox == "workspace-write"
    assert config.codex_full_auto is True
    assert config.codex_extra_args == ""
    assert config.codex_prompt_file.name == "CLAUDE.md"


def test_config_from_env_custom_values() -> None:
    """Test RalphConfig.from_env() with custom environment variables."""
    env_vars = {
        "CODEX_PROMPT_FILE": "/tmp/custom.md",
        "CODEX_MODEL": "gpt-4o",
        "CODEX_REASONING_EFFORT": "medium",
        "CODEX_SANDBOX": "workspace-read",
        "CODEX_FULL_AUTO": "false",
        "CODEX_EXTRA_ARGS": "--verbose --debug",
    }

    with patch.dict(os.environ, env_vars, clear=False):
        config = RalphConfig.from_env(tool="codex")

    assert config.tool == "codex"
    assert config.codex_model == "gpt-4o"
    assert config.codex_reasoning_effort == "medium"
    assert config.codex_sandbox == "workspace-read"
    assert config.codex_full_auto is False
    assert config.codex_extra_args == "--verbose --debug"
    assert config.codex_prompt_file == Path("/tmp/custom.md")


def test_config_from_env_full_auto_variations() -> None:
    """Test RalphConfig.from_env() with different CODEX_FULL_AUTO values."""
    test_cases = [
        ("true", True),
        ("True", True),
        ("TRUE", True),
        ("false", False),
        ("False", False),
        ("FALSE", False),
        ("0", False),
        ("1", False),
        ("", False),
    ]

    for env_value, expected in test_cases:
        with patch.dict(os.environ, {"CODEX_FULL_AUTO": env_value}, clear=False):
            config = RalphConfig.from_env()

        assert config.codex_full_auto is expected, f"Failed for CODEX_FULL_AUTO={env_value!r}"


def test_config_immutability() -> None:
    """Test that RalphConfig is immutable (frozen dataclass)."""
    config = RalphConfig.from_env(tool="amp")

    with pytest.raises(AttributeError):
        config.tool = "claude"  # type: ignore[misc]

    with pytest.raises(AttributeError):
        config.codex_model = "new-model"  # type: ignore[misc]
