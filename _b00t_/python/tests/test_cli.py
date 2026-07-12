"""Smoke tests for b00t_ngc.cli — argument parsing and command dispatch.
NvidiaClient is stubbed; nothing touches the network."""
from __future__ import annotations

import io
import urllib.error
from unittest.mock import MagicMock

import pytest

from b00t_ngc import cli
from b00t_ngc.client import ContainerTag, Model


@pytest.fixture
def stub_client(monkeypatch):
    """Replace cli.NvidiaClient with a MagicMock instance factory."""
    instance = MagicMock(name="NvidiaClient()")
    monkeypatch.setattr(cli, "NvidiaClient", MagicMock(return_value=instance))
    return instance


def _run(monkeypatch, *argv: str):
    monkeypatch.setattr("sys.argv", ["b00t-ngc", *argv])
    cli.main()


# ── argparse surface ──────────────────────────────────────────────────


def test_no_subcommand_exits_with_usage_error(monkeypatch, capsys):
    with pytest.raises(SystemExit) as exc:
        _run(monkeypatch)
    assert exc.value.code == 2
    assert "usage" in capsys.readouterr().err.lower()


def test_unknown_subcommand_exits_with_usage_error(monkeypatch):
    with pytest.raises(SystemExit) as exc:
        _run(monkeypatch, "frobnicate")
    assert exc.value.code == 2


def test_chat_requires_prompt(monkeypatch):
    with pytest.raises(SystemExit) as exc:
        _run(monkeypatch, "chat")
    assert exc.value.code == 2


# ── auth ──────────────────────────────────────────────────────────────


def test_auth_prints_org(monkeypatch, capsys, stub_client):
    stub_client.whoami.return_value = "test-org"
    _run(monkeypatch, "auth")
    assert "test-org" in capsys.readouterr().out


def test_auth_http_error_exits_1(monkeypatch, capsys, stub_client):
    stub_client.whoami.side_effect = urllib.error.HTTPError(
        "https://authn.nvidia.com", 401, "Unauthorized", {}, io.BytesIO(b"")
    )
    with pytest.raises(SystemExit) as exc:
        _run(monkeypatch, "auth")
    assert exc.value.code == 1
    assert "401" in capsys.readouterr().err


def test_missing_key_exits_1(monkeypatch, capsys):
    monkeypatch.setattr(
        cli, "NvidiaClient", MagicMock(side_effect=RuntimeError("No NGC/NVIDIA API key found."))
    )
    with pytest.raises(SystemExit) as exc:
        _run(monkeypatch, "auth")
    assert exc.value.code == 1
    assert "No NGC/NVIDIA API key" in capsys.readouterr().err


# ── containers ────────────────────────────────────────────────────────


def test_containers_defaults_and_output(monkeypatch, capsys, stub_client):
    stub_client.containers.return_value = [
        ContainerTag(name="24.05-py3", image="nvcr.io/nvidia/pytorch:24.05-py3"),
    ]
    _run(monkeypatch, "containers")
    stub_client.containers.assert_called_once_with(image="pytorch", n=12)
    assert "nvcr.io/nvidia/pytorch:24.05-py3" in capsys.readouterr().out


def test_containers_custom_image_and_n(monkeypatch, stub_client):
    stub_client.containers.return_value = []
    _run(monkeypatch, "containers", "--image", "triton", "-n", "3")
    stub_client.containers.assert_called_once_with(image="triton", n=3)


# ── models ────────────────────────────────────────────────────────────

_MODELS = [
    Model(id="nvidia/llama-3.1-nemotron-70b-instruct", owned_by="nvidia"),
    Model(id="meta/llama3-8b-instruct", owned_by="meta"),
]


def test_models_lists_all(monkeypatch, capsys, stub_client):
    stub_client.models.return_value = _MODELS
    _run(monkeypatch, "models")
    out = capsys.readouterr().out
    assert "=== 2 model(s) ===" in out
    assert "meta/llama3-8b-instruct" in out


def test_models_filter_is_case_insensitive(monkeypatch, capsys, stub_client):
    stub_client.models.return_value = _MODELS
    _run(monkeypatch, "models", "--filter", "NEMOTRON")
    out = capsys.readouterr().out
    assert "=== 1 model(s) ===" in out
    assert "meta/llama3-8b-instruct" not in out


# ── chat ──────────────────────────────────────────────────────────────


def test_chat_joins_prompt_words_and_prints_reply(monkeypatch, capsys, stub_client):
    stub_client.chat.return_value = "the reply"
    _run(monkeypatch, "chat", "hello", "world")
    stub_client.chat.assert_called_once_with(
        "hello world",
        model="nvidia/llama-3.1-nemotron-70b-instruct",
        system="",
        max_tokens=512,
        temperature=0.2,
    )
    assert "the reply" in capsys.readouterr().out


def test_chat_flags_forwarded(monkeypatch, stub_client):
    stub_client.chat.return_value = "ok"
    _run(
        monkeypatch, "chat", "hi",
        "--model", "test/model", "--system", "be terse",
        "--max-tokens", "42", "--temperature", "0.9",
    )
    stub_client.chat.assert_called_once_with(
        "hi", model="test/model", system="be terse", max_tokens=42, temperature=0.9
    )


def test_chat_stream_prints_tokens(monkeypatch, capsys, stub_client):
    stub_client.stream_chat.return_value = iter(["Hel", "lo"])
    _run(monkeypatch, "chat", "hi", "--stream")
    stub_client.stream_chat.assert_called_once_with(
        "hi", model="nvidia/llama-3.1-nemotron-70b-instruct", max_tokens=512
    )
    assert capsys.readouterr().out == "Hello\n"


def test_chat_http_error_shows_body_and_exits_1(monkeypatch, capsys, stub_client):
    stub_client.chat.side_effect = urllib.error.HTTPError(
        "https://integrate.api.nvidia.com", 429, "Too Many Requests", {},
        io.BytesIO(b'{"error": "rate limited"}'),
    )
    with pytest.raises(SystemExit) as exc:
        _run(monkeypatch, "chat", "hi")
    assert exc.value.code == 1
    err = capsys.readouterr().err
    assert "429" in err
    assert "rate limited" in err
