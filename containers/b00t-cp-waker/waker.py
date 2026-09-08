#!/usr/bin/env python3
"""b00t control-plane waker — scale-to-zero reverse proxy for the dstack server.

Runs on Cloud Run (min-instances=0). On each inbound request:
  1. If the control-node VM is not RUNNING, start it (compute.instances.start
     via metadata ADC — the b00t-cp-waker SA), then wait for dstack readiness.
  2. Reverse-proxy the request to the dstack server over the VPC.

The VM powers ITSELF off when dstack is idle (systemd reaper), so this only
ever needs to START it — the SA has no stop/delete permission.

Stdlib only. Env: PROJECT_ID, CONTROL_ZONE, CONTROL_INSTANCE,
DSTACK_UPSTREAM_ADDR (host:port), READINESS_PATH (default /healthz),
START_DEADLINE_SECONDS (default 120), PORT (Cloud Run, default 8080).

⚠️ v1: a minimal hand-rolled proxy (same "our own, minimal-first" call as the
TOML provider). pingap (vendor/pingap-devproxy-b00t) is the Phase 2.75 upgrade
where its routing/TLS/plugins actually earn their keep (tailnet edge).
"""
from __future__ import annotations

import json
import os
import sys
import time
import urllib.error
import urllib.request
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer

PROJECT_ID = os.environ["PROJECT_ID"]
ZONE = os.environ["CONTROL_ZONE"]
INSTANCE = os.environ["CONTROL_INSTANCE"]
UPSTREAM = os.environ["DSTACK_UPSTREAM_ADDR"]
READINESS_PATH = os.environ.get("READINESS_PATH", "/healthz")
START_DEADLINE = int(os.environ.get("START_DEADLINE_SECONDS", "120"))
PORT = int(os.environ.get("PORT", "8080"))
HEALTH_PATH = "/_waker/health"

_META = "http://metadata.google.internal/computeMetadata/v1"
_COMPUTE = f"https://compute.googleapis.com/compute/v1/projects/{PROJECT_ID}/zones/{ZONE}"
_HOP_BY_HOP = {
    "connection", "keep-alive", "proxy-authenticate", "proxy-authorization",
    "te", "trailers", "transfer-encoding", "upgrade", "host", "content-length",
}


def log(*a: object) -> None:
    print(time.strftime("%Y-%m-%dT%H:%M:%SZ", time.gmtime()), "waker:", *a, file=sys.stderr, flush=True)


def _token() -> str:
    req = urllib.request.Request(
        f"{_META}/instance/service-accounts/default/token",
        headers={"Metadata-Flavor": "Google"},
    )
    with urllib.request.urlopen(req, timeout=5) as r:
        return json.load(r)["access_token"]


def _api(path: str, method: str = "GET") -> dict:
    req = urllib.request.Request(
        f"{_COMPUTE}{path}",
        method=method,
        headers={"Authorization": f"Bearer {_token()}", "Content-Length": "0"},
    )
    with urllib.request.urlopen(req, timeout=15) as r:
        body = r.read()
        return json.loads(body) if body else {}


def ensure_running() -> None:
    """Idempotent: start the VM if needed, then block until dstack answers."""
    status = _api(f"/instances/{INSTANCE}").get("status", "UNKNOWN")
    log(f"instance status = {status}")
    if status != "RUNNING":
        if status in ("STOPPING", "SUSPENDING"):
            log("instance settling; brief wait")
            time.sleep(10)
        log("issuing instances.start")
        _api(f"/instances/{INSTANCE}/start", method="POST")

    deadline = time.time() + START_DEADLINE
    url = f"http://{UPSTREAM}{READINESS_PATH}"
    while time.time() < deadline:
        try:
            with urllib.request.urlopen(url, timeout=3) as r:
                if r.status < 500:
                    log(f"dstack ready ({r.status}) at {url}")
                    return
        except (urllib.error.URLError, OSError):
            pass
        time.sleep(3)
    raise TimeoutError(f"dstack not ready within {START_DEADLINE}s")


class Handler(BaseHTTPRequestHandler):
    protocol_version = "HTTP/1.1"
    server_version = "b00t-cp-waker/1"

    def _health(self) -> bool:
        if self.path == HEALTH_PATH:
            self.send_response(200)
            self.send_header("Content-Length", "3")
            self.end_headers()
            self.wfile.write(b"ok\n")
            return True
        return False

    def _proxy(self) -> None:
        if self._health():
            return
        try:
            ensure_running()
        except Exception as e:  # noqa: BLE001 — surface any wake failure to the client
            log(f"wake failed: {e}")
            self.send_response(503)
            self.send_header("Retry-After", "15")
            self.send_header("Content-Length", "0")
            self.end_headers()
            return

        length = int(self.headers.get("Content-Length", 0) or 0)
        body = self.rfile.read(length) if length else None
        fwd = urllib.request.Request(
            f"http://{UPSTREAM}{self.path}", data=body, method=self.command
        )
        for k, v in self.headers.items():
            if k.lower() not in _HOP_BY_HOP:
                fwd.add_header(k, v)
        try:
            with urllib.request.urlopen(fwd, timeout=300) as up:
                self.send_response(up.status)
                payload = up.read()
                for k, v in up.headers.items():
                    if k.lower() not in _HOP_BY_HOP:
                        self.send_header(k, v)
                self.send_header("Content-Length", str(len(payload)))
                self.end_headers()
                self.wfile.write(payload)
        except urllib.error.HTTPError as e:
            payload = e.read()
            self.send_response(e.code)
            self.send_header("Content-Length", str(len(payload)))
            self.end_headers()
            self.wfile.write(payload)
        except Exception as e:  # noqa: BLE001
            log(f"proxy error: {e}")
            self.send_response(502)
            self.send_header("Content-Length", "0")
            self.end_headers()

    do_GET = do_POST = do_PUT = do_DELETE = do_PATCH = do_HEAD = _proxy

    def log_message(self, fmt: str, *args: object) -> None:  # quieter access log
        log(self.command, self.path, "->", fmt % args)


if __name__ == "__main__":
    log(f"listening :{PORT}  upstream={UPSTREAM}  instance={INSTANCE}@{ZONE}")
    ThreadingHTTPServer(("0.0.0.0", PORT), Handler).serve_forever()
