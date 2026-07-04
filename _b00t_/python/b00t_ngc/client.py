"""NGC / NVIDIA API client — typed, minimal stdlib-only."""
from __future__ import annotations
import json
import urllib.error
import urllib.request
from dataclasses import dataclass, field
from typing import Any, Iterator

from ._auth import load_key


_NGC_BASE   = "https://api.ngc.nvidia.com/v2"
_NVIDIA_BASE = "https://integrate.api.nvidia.com/v1"
_AUTHN_BASE = "https://authn.nvidia.com"


@dataclass
class ContainerTag:
    name: str
    image: str  # full nvcr.io/… reference


@dataclass
class Model:
    id: str
    owned_by: str = ""
    context_length: int = 0


@dataclass
class ChatMessage:
    role: str
    content: str


@dataclass
class NvidiaClient:
    """Thin authenticated wrapper over NGC and NVIDIA model APIs.

    Uses only stdlib — no httpx/requests dependency.
    API key is resolved once via _auth.load_key().
    """
    _key: str = field(default_factory=load_key, repr=False)

    # ── internal helpers ───────────────────────────────────────────────

    def _get(self, url: str, *, auth: str = "Bearer") -> Any:
        req = urllib.request.Request(
            url,
            headers={"Authorization": f"{auth} {self._key}"},
        )
        with urllib.request.urlopen(req, timeout=15) as r:
            return json.load(r)

    def _post(self, url: str, payload: dict) -> Any:
        data = json.dumps(payload).encode()
        req = urllib.request.Request(
            url,
            data=data,
            headers={
                "Authorization": f"Bearer {self._key}",
                "Content-Type": "application/json",
            },
        )
        with urllib.request.urlopen(req, timeout=60) as r:
            return json.load(r)

    # ── auth ───────────────────────────────────────────────────────────

    def whoami(self) -> str:
        """Return the NGC org name for the current API key."""
        d = self._get(
            f"{_AUTHN_BASE}/token?service=ngc&scope=group/ngc:*",
            auth="ApiKey",
        )
        return d.get("user", {}).get("defaultOrgName", "?")

    # ── container registry ─────────────────────────────────────────────

    def containers(self, org: str = "nvidia", image: str = "pytorch", n: int = 12) -> list[ContainerTag]:
        """List the most recent container tags for an NGC image."""
        url = f"{_NGC_BASE}/orgs/{org}/containers/{image}/tags?page_size={n}&sort_by=name"
        d = self._get(url, auth="ApiKey")
        tags = sorted(d.get("tags") or [], key=lambda t: t.get("name", ""), reverse=True)
        return [
            ContainerTag(
                name=t["name"],
                image=f"nvcr.io/{org}/{image}:{t['name']}",
            )
            for t in tags[:n]
        ]

    # ── model API ─────────────────────────────────────────────────────

    def models(self) -> list[Model]:
        """List models available on integrate.api.nvidia.com."""
        d = self._get(f"{_NVIDIA_BASE}/models")
        return [
            Model(
                id=m.get("id", "?"),
                owned_by=m.get("owned_by", ""),
                context_length=m.get("context_length", 0),
            )
            for m in (d.get("data") or [])
        ]

    def chat(
        self,
        prompt: str,
        *,
        model: str = "nvidia/llama-3.1-nemotron-70b-instruct",
        system: str = "",
        max_tokens: int = 512,
        temperature: float = 0.2,
    ) -> str:
        """Send a chat prompt and return the assistant reply text."""
        messages: list[dict] = []
        if system:
            messages.append({"role": "system", "content": system})
        messages.append({"role": "user", "content": prompt})
        d = self._post(
            f"{_NVIDIA_BASE}/chat/completions",
            {"model": model, "messages": messages, "max_tokens": max_tokens, "temperature": temperature},
        )
        return d["choices"][0]["message"]["content"]

    def stream_chat(
        self,
        prompt: str,
        *,
        model: str = "nvidia/llama-3.1-nemotron-70b-instruct",
        max_tokens: int = 512,
    ) -> Iterator[str]:
        """Yield tokens as they arrive (SSE streaming)."""
        data = json.dumps({
            "model": model,
            "messages": [{"role": "user", "content": prompt}],
            "max_tokens": max_tokens,
            "stream": True,
        }).encode()
        req = urllib.request.Request(
            f"{_NVIDIA_BASE}/chat/completions",
            data=data,
            headers={"Authorization": f"Bearer {self._key}", "Content-Type": "application/json"},
        )
        with urllib.request.urlopen(req, timeout=120) as r:
            for raw in r:
                line = raw.decode().strip()
                if not line.startswith("data:"):
                    continue
                chunk = line[5:].strip()
                if chunk == "[DONE]":
                    break
                try:
                    d = json.loads(chunk)
                    if token := d["choices"][0].get("delta", {}).get("content", ""):
                        yield token
                except (json.JSONDecodeError, KeyError):
                    continue
