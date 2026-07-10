"""Shared fixtures for b00t_ngc tests — no network, no real env leakage."""
from __future__ import annotations

import json
from pathlib import Path

import pytest

from b00t_ngc import _auth

_FIXTURES_DIR = Path(__file__).parent / "fixtures"


@pytest.fixture(scope="session")
def fixtures() -> dict:
    """Sample API payloads live in JSON, not in test code (repo law)."""
    return json.loads((_FIXTURES_DIR / "ngc_responses.json").read_text())


@pytest.fixture(autouse=True)
def _isolate_auth(monkeypatch, tmp_path):
    """Keep every test hermetic:

    - scrub real NGC/NVIDIA env vars
    - point _auth._DOT_ENV away from the operator's real ~/.b00t/.env
    - clear the lru_cache on load_key before AND after each test
    """
    monkeypatch.delenv("NGC_API_KEY", raising=False)
    monkeypatch.delenv("NVIDIA_API_KEY", raising=False)
    monkeypatch.setattr(_auth, "_DOT_ENV", tmp_path / "no-such.env")
    _auth.load_key.cache_clear()
    yield
    _auth.load_key.cache_clear()


class FakeHTTPResponse:
    """Stand-in for the object returned by urllib.request.urlopen.

    Supports the three access patterns client.py uses:
    context manager, .read() (via json.load), and line iteration (SSE).
    """

    def __init__(self, payload=None, lines=None):
        self._body = b"" if payload is None else json.dumps(payload).encode()
        self._lines = [
            line.encode() if isinstance(line, str) else line for line in (lines or [])
        ]

    def read(self, *args):
        return self._body

    def __iter__(self):
        return iter(self._lines)

    def __enter__(self):
        return self

    def __exit__(self, *exc):
        return False


class UrlopenRecorder:
    """Callable that replaces urllib.request.urlopen and records requests."""

    def __init__(self, responses):
        self._responses = list(responses)
        self.requests = []
        self.timeouts = []

    def __call__(self, req, timeout=None):
        self.requests.append(req)
        self.timeouts.append(timeout)
        return self._responses.pop(0)

    @property
    def last(self):
        return self.requests[-1]


@pytest.fixture
def fake_urlopen(monkeypatch):
    """Factory: install a recording fake for urllib.request.urlopen.

    Usage: recorder = fake_urlopen(FakeHTTPResponse(payload={...}), ...)
    """

    def _install(*responses):
        recorder = UrlopenRecorder(responses)
        monkeypatch.setattr("urllib.request.urlopen", recorder)
        return recorder

    return _install
