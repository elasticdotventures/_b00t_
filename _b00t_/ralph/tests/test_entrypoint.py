from __future__ import annotations

from collections.abc import Sequence
from pathlib import Path
from unittest.mock import MagicMock

import pytest

from ralph import entrypoint


def test_parse_args_requires_agent() -> None:
    with pytest.raises(SystemExit) as exc:
        entrypoint._parse_args([])
    assert exc.value.code == 2


def test_parse_args_valid_agent() -> None:
    args = entrypoint._parse_args(["--agent", "amp", "3"])
    assert args.agent == "amp"
    assert args.max_iterations == 3


def test_progress_file_created_on_run(monkeypatch: pytest.MonkeyPatch, tmp_path: Path) -> None:
    monkeypatch.chdir(tmp_path)
    (tmp_path / "prompt.md").write_text("# Test prompt\n")
    (tmp_path / ".taskmaster" / "tasks").mkdir(parents=True)
    (tmp_path / ".taskmaster" / "tasks" / "tasks.json").write_text(
        '{"tasks":[{"id":"task-001","title":"t","description":"d","status":"pending","priority":1}],"metadata":{"project":"t","branchName":"b","taskMasterVersion":"1.0"}}'
    )

    def fake_run(_cmd: Sequence[str], stdin_path: Path | None = None) -> str:
        return "<promise>COMPLETE</promise>"

    monkeypatch.setattr(entrypoint, "_run_and_capture", fake_run)
    monkeypatch.setattr("ralph.entrypoint.time.sleep", lambda _seconds: None)

    rc = entrypoint.main(["--agent", "amp", "1"])
    assert rc == 0
    assert (tmp_path / "progress.txt").exists()


def test_parse_args_mcp_without_agent() -> None:
    """Test that --mcp flag works without requiring --agent."""
    args = entrypoint._parse_args(["--mcp"])
    assert args.mcp is True
    assert args.agent is None


def test_parse_args_mcp_with_default_transport() -> None:
    """Test that --mcp uses stdio as default transport."""
    args = entrypoint._parse_args(["--mcp"])
    assert args.mcp is True
    assert args.transport == "stdio"


def test_parse_args_mcp_with_http_transport() -> None:
    """Test that --mcp accepts HTTP transport with host and port."""
    args = entrypoint._parse_args([
        "--mcp",
        "--transport", "http",
        "--host", "0.0.0.0",
        "--port", "9000",
    ])
    assert args.mcp is True
    assert args.transport == "http"
    assert args.host == "0.0.0.0"
    assert args.port == 9000


def test_parse_args_mcp_with_default_host_port() -> None:
    """Test that --mcp uses default host and port values."""
    args = entrypoint._parse_args(["--mcp", "--transport", "http"])
    assert args.host == "127.0.0.1"
    assert args.port == 8000


def test_main_mcp_calls_run_with_stdio(monkeypatch: pytest.MonkeyPatch) -> None:
    """Test that main() calls mcp.run() with correct parameters for stdio."""
    mock_mcp_run = MagicMock()
    monkeypatch.setattr("ralph.entrypoint.mcp.run", mock_mcp_run)

    rc = entrypoint.main(["--mcp"])

    assert rc == 0
    mock_mcp_run.assert_called_once_with(transport="stdio")


def test_main_mcp_calls_run_with_http(monkeypatch: pytest.MonkeyPatch) -> None:
    """Test that main() calls mcp.run() with correct parameters for HTTP."""
    mock_mcp_run = MagicMock()
    monkeypatch.setattr("ralph.entrypoint.mcp.run", mock_mcp_run)

    rc = entrypoint.main([
        "--mcp",
        "--transport", "http",
        "--host", "0.0.0.0",
        "--port", "9000",
    ])

    assert rc == 0
    mock_mcp_run.assert_called_once_with(transport="http", host="0.0.0.0", port=9000)
