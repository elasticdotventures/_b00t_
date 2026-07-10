"""Tests for b00t_ngc.client.NvidiaClient — request construction and response
parsing with the HTTP layer fully mocked. Zero network."""
from __future__ import annotations

import json

import pytest

from b00t_ngc.client import ChatMessage, ContainerTag, Model, NvidiaClient
from tests.conftest import FakeHTTPResponse

KEY = "sk-unit-test"


def _client() -> NvidiaClient:
    return NvidiaClient(_key=KEY)


# ── construction / auth wiring ────────────────────────────────────────


def test_client_key_injectable_without_load_key():
    assert _client()._key == KEY


def test_client_default_factory_uses_load_key(monkeypatch):
    monkeypatch.setenv("NGC_API_KEY", "from-env")
    from b00t_ngc import _auth

    _auth.load_key.cache_clear()
    assert NvidiaClient()._key == "from-env"


def test_client_without_any_key_raises():
    with pytest.raises(RuntimeError):
        NvidiaClient()


def test_repr_does_not_leak_key():
    assert KEY not in repr(_client())


# ── whoami ────────────────────────────────────────────────────────────


def test_whoami_returns_org_and_uses_apikey_auth(fake_urlopen, fixtures):
    rec = fake_urlopen(FakeHTTPResponse(payload=fixtures["whoami_ok"]))
    assert _client().whoami() == "test-org"
    req = rec.last
    assert req.full_url.startswith("https://authn.nvidia.com/token")
    assert req.get_header("Authorization") == f"ApiKey {KEY}"


def test_whoami_missing_user_defaults_to_question_mark(fake_urlopen, fixtures):
    fake_urlopen(FakeHTTPResponse(payload=fixtures["whoami_no_user"]))
    assert _client().whoami() == "?"


# ── containers ────────────────────────────────────────────────────────


def test_containers_sorted_desc_and_fully_qualified(fake_urlopen, fixtures):
    rec = fake_urlopen(FakeHTTPResponse(payload=fixtures["container_tags"]))
    tags = _client().containers(image="pytorch", n=12)
    assert [t.name for t in tags] == ["24.05-py3", "24.01-py3", "23.12-py3"]
    assert tags[0] == ContainerTag(name="24.05-py3", image="nvcr.io/nvidia/pytorch:24.05-py3")
    req = rec.last
    assert "/orgs/nvidia/containers/pytorch/tags" in req.full_url
    assert "page_size=12" in req.full_url
    assert req.get_header("Authorization") == f"ApiKey {KEY}"


def test_containers_respects_n_limit(fake_urlopen, fixtures):
    fake_urlopen(FakeHTTPResponse(payload=fixtures["container_tags"]))
    tags = _client().containers(n=2)
    assert [t.name for t in tags] == ["24.05-py3", "24.01-py3"]


def test_containers_custom_org_in_url_and_image_ref(fake_urlopen, fixtures):
    rec = fake_urlopen(FakeHTTPResponse(payload=fixtures["container_tags"]))
    tags = _client().containers(org="nim", image="triton", n=1)
    assert "/orgs/nim/containers/triton/tags" in rec.last.full_url
    assert tags[0].image == "nvcr.io/nim/triton:24.05-py3"


def test_containers_null_tags_yields_empty_list(fake_urlopen, fixtures):
    fake_urlopen(FakeHTTPResponse(payload=fixtures["container_tags_empty"]))
    assert _client().containers() == []


# ── models ────────────────────────────────────────────────────────────


def test_models_parses_fields_with_defaults(fake_urlopen, fixtures):
    rec = fake_urlopen(FakeHTTPResponse(payload=fixtures["models_list"]))
    models = _client().models()
    assert models[0] == Model(
        id="nvidia/llama-3.1-nemotron-70b-instruct", owned_by="nvidia", context_length=131072
    )
    assert models[2] == Model(id="?", owned_by="mystery-vendor", context_length=0)
    req = rec.last
    assert req.full_url == "https://integrate.api.nvidia.com/v1/models"
    assert req.get_header("Authorization") == f"Bearer {KEY}"


def test_models_null_data_yields_empty_list(fake_urlopen, fixtures):
    fake_urlopen(FakeHTTPResponse(payload=fixtures["models_empty"]))
    assert _client().models() == []


# ── chat ──────────────────────────────────────────────────────────────


def test_chat_returns_reply_and_builds_payload(fake_urlopen, fixtures):
    rec = fake_urlopen(FakeHTTPResponse(payload=fixtures["chat_completion"]))
    reply = _client().chat("hi there", model="test/model", max_tokens=99, temperature=0.7)
    assert reply == "hello from mock"
    req = rec.last
    assert req.full_url == "https://integrate.api.nvidia.com/v1/chat/completions"
    assert req.get_header("Content-type") == "application/json"
    payload = json.loads(req.data)
    assert payload == {
        "model": "test/model",
        "messages": [{"role": "user", "content": "hi there"}],
        "max_tokens": 99,
        "temperature": 0.7,
    }


def test_chat_system_prompt_prepended(fake_urlopen, fixtures):
    rec = fake_urlopen(FakeHTTPResponse(payload=fixtures["chat_completion"]))
    _client().chat("question", system="be terse")
    messages = json.loads(rec.last.data)["messages"]
    assert messages == [
        {"role": "system", "content": "be terse"},
        {"role": "user", "content": "question"},
    ]


def test_chat_no_system_means_single_message(fake_urlopen, fixtures):
    rec = fake_urlopen(FakeHTTPResponse(payload=fixtures["chat_completion"]))
    _client().chat("question")
    assert len(json.loads(rec.last.data)["messages"]) == 1


# ── stream_chat ───────────────────────────────────────────────────────


def test_stream_chat_yields_tokens_until_done(fake_urlopen, fixtures):
    rec = fake_urlopen(FakeHTTPResponse(lines=fixtures["sse_stream_lines"]))
    tokens = list(_client().stream_chat("stream me", model="test/model", max_tokens=7))
    # non-data lines skipped, malformed JSON skipped, empty delta skipped,
    # iteration stops at [DONE] — post-DONE token never emitted
    assert tokens == ["Hel", "lo"]
    payload = json.loads(rec.last.data)
    assert payload["stream"] is True
    assert payload["model"] == "test/model"
    assert payload["max_tokens"] == 7


def test_stream_chat_is_lazy_generator(fake_urlopen, fixtures):
    rec = fake_urlopen(FakeHTTPResponse(lines=fixtures["sse_stream_lines"]))
    gen = _client().stream_chat("lazy")
    assert rec.requests == []  # no request until first next()
    assert next(gen) == "Hel"
    assert len(rec.requests) == 1


# ── dataclasses ───────────────────────────────────────────────────────


def test_chat_message_dataclass_shape():
    m = ChatMessage(role="user", content="hi")
    assert (m.role, m.content) == ("user", "hi")
