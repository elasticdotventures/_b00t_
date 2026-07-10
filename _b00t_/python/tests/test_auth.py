"""Tests for b00t_ngc._auth.load_key — precedence, .env fallback, caching."""
from __future__ import annotations

import pytest

from b00t_ngc import _auth
from b00t_ngc._auth import load_key


def _write_dotenv(monkeypatch, tmp_path, text: str):
    env_file = tmp_path / ".env"
    env_file.write_text(text)
    monkeypatch.setattr(_auth, "_DOT_ENV", env_file)
    return env_file


# ── env var precedence ────────────────────────────────────────────────


def test_ngc_env_var_wins_over_nvidia(monkeypatch):
    monkeypatch.setenv("NGC_API_KEY", "ngc-key")
    monkeypatch.setenv("NVIDIA_API_KEY", "nvidia-key")
    assert load_key() == "ngc-key"


def test_nvidia_env_var_used_when_ngc_absent(monkeypatch):
    monkeypatch.setenv("NVIDIA_API_KEY", "nvidia-key")
    assert load_key() == "nvidia-key"


def test_env_value_is_stripped(monkeypatch):
    monkeypatch.setenv("NGC_API_KEY", "  padded-key \n")
    assert load_key() == "padded-key"


def test_empty_env_var_falls_through_to_next_name(monkeypatch):
    monkeypatch.setenv("NGC_API_KEY", "   ")
    monkeypatch.setenv("NVIDIA_API_KEY", "nvidia-key")
    assert load_key() == "nvidia-key"


# ── ~/.b00t/.env fallback ─────────────────────────────────────────────


def test_dotenv_fallback_when_no_env_vars(monkeypatch, tmp_path):
    _write_dotenv(monkeypatch, tmp_path, "OTHER=1\nNGC_API_KEY=file-key\n")
    assert load_key() == "file-key"


def test_dotenv_strips_quotes(monkeypatch, tmp_path):
    _write_dotenv(monkeypatch, tmp_path, 'NGC_API_KEY="quoted-key"\n')
    assert load_key() == "quoted-key"


def test_dotenv_strips_single_quotes(monkeypatch, tmp_path):
    _write_dotenv(monkeypatch, tmp_path, "NVIDIA_API_KEY='sq-key'\n")
    assert load_key() == "sq-key"


def test_env_var_beats_dotenv(monkeypatch, tmp_path):
    _write_dotenv(monkeypatch, tmp_path, "NGC_API_KEY=file-key\n")
    monkeypatch.setenv("NVIDIA_API_KEY", "env-key")
    assert load_key() == "env-key"


def test_dotenv_empty_value_is_skipped(monkeypatch, tmp_path):
    _write_dotenv(monkeypatch, tmp_path, "NGC_API_KEY=\nNVIDIA_API_KEY=fallback\n")
    assert load_key() == "fallback"


def test_dotenv_line_order_wins_over_name_order(monkeypatch, tmp_path):
    # ⚠️ Documented quirk: inside the .env file the FIRST matching line wins,
    # regardless of the NGC-before-NVIDIA name precedence used for env vars.
    _write_dotenv(monkeypatch, tmp_path, "NVIDIA_API_KEY=first-line\nNGC_API_KEY=second-line\n")
    assert load_key() == "first-line"


def test_commented_dotenv_line_ignored(monkeypatch, tmp_path):
    _write_dotenv(monkeypatch, tmp_path, "# NGC_API_KEY=commented\nNGC_API_KEY=real\n")
    assert load_key() == "real"


# ── missing key ───────────────────────────────────────────────────────


def test_missing_key_raises_runtime_error():
    # autouse fixture already scrubbed env and pointed _DOT_ENV at a missing file
    with pytest.raises(RuntimeError, match="No NGC/NVIDIA API key found"):
        load_key()


def test_missing_dotenv_file_does_not_crash(monkeypatch, tmp_path):
    monkeypatch.setattr(_auth, "_DOT_ENV", tmp_path / "definitely-absent.env")
    with pytest.raises(RuntimeError):
        load_key()


# ── lru_cache behavior ────────────────────────────────────────────────


def test_result_is_cached_across_env_changes(monkeypatch):
    monkeypatch.setenv("NGC_API_KEY", "first")
    assert load_key() == "first"
    monkeypatch.setenv("NGC_API_KEY", "second")
    assert load_key() == "first"  # cached — env change invisible by design


def test_cache_clear_picks_up_new_value(monkeypatch):
    monkeypatch.setenv("NGC_API_KEY", "first")
    assert load_key() == "first"
    monkeypatch.setenv("NGC_API_KEY", "second")
    load_key.cache_clear()
    assert load_key() == "second"
