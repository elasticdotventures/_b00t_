"""NATS Micro Service exposing this package's Docling-backed extraction over
the network, so it is a genuinely *shared*, discoverable capability (via
`nats service list`) rather than only a local one-shot CLI invocation.

Relocated from reqif-opa-mcp's `reqif_ingest_cli.nats_docling_service`
(2026-08-23) — same protocol, same on-demand lifecycle, now owned by b00t
directly rather than bundled inside a ReqIF-specific tool.

Endpoint: "extract" on service "ledgrrr-docling". Request payload:
    {"path": "<file path readable by this process>", "profile": "auto"}
Reply payload: the same JSON shape `b00t_docling_serve extract` prints, or
    {"error": "..."} on failure.

Run:
    NATS_URL=nats://127.0.0.1:14222 NATS_USER=... NATS_PASSWORD=... \\
        uv run python -m b00t_docling_serve.nats_service

On-demand lifecycle: this process is meant to be started by systemd (a
Podman Quadlet, see ../deploy/quadlet/docling-nats-service.container) only
when needed, not run perpetually — the NATS server itself is the only
thing meant to stay always-on in the b00t hive's "small standing
coordination layer, everything else on-demand" design. This process
tracks the time of its last handled request and exits cleanly (code 0)
after IDLE_TIMEOUT_SECONDS (default 300) with none.
"""

from __future__ import annotations

import asyncio
import json
import os
import time
from dataclasses import asdict

import nats
import nats.micro as micro

from b00t_docling_serve.docling_adapter import extract_docling_document

SERVICE_NAME = "ledgrrr-docling"
SERVICE_VERSION = "0.1.0"
DEFAULT_IDLE_TIMEOUT_SECONDS = 300.0


def _run_extract(path: str, profile: str) -> dict:
    result = extract_docling_document(path, profile=profile)

    from returns.result import Failure

    if isinstance(result, Failure):
        return {"error": str(result.failure())}
    graph = result.unwrap()
    return json.loads(json.dumps(asdict(graph), default=str))


async def _extract_handler(request: micro.Request, last_activity: list[float]) -> None:
    last_activity[0] = time.monotonic()
    try:
        payload = json.loads(request.data or b"{}")
        path = payload["path"]
        profile = payload.get("profile", "auto")
        reply = await asyncio.get_running_loop().run_in_executor(
            None, _run_extract, path, profile
        )
        await request.respond(json.dumps(reply).encode())
    except Exception as exc:  # noqa: BLE001 - reply with the error, don't crash the service
        await request.respond_error("500", str(exc))
    finally:
        last_activity[0] = time.monotonic()


async def _idle_watchdog(last_activity: list[float], idle_timeout: float) -> None:
    while True:
        await asyncio.sleep(5)
        if time.monotonic() - last_activity[0] >= idle_timeout:
            return


async def main() -> None:
    url = os.environ.get("NATS_URL", "nats://127.0.0.1:4222")
    user = os.environ.get("NATS_USER")
    password = os.environ.get("NATS_PASSWORD")
    idle_timeout = float(
        os.environ.get("IDLE_TIMEOUT_SECONDS", DEFAULT_IDLE_TIMEOUT_SECONDS)
    )

    nc = await nats.connect(url, user=user, password=password)
    svc = await micro.add_service(
        nc,
        config=micro.ServiceConfig(
            name=SERVICE_NAME,
            version=SERVICE_VERSION,
            description="Docling-backed document extraction (PDF/DOCX/MD), "
            "shared over NATS by b00t-docling-serve.",
        ),
    )
    group = svc.add_group(name="ledgrrr")
    last_activity = [time.monotonic()]
    await group.add_endpoint(
        name="extract",
        handler=lambda request: _extract_handler(request, last_activity),
    )

    print(f"[{SERVICE_NAME}] listening on '{url}' as subject 'ledgrrr.extract'", flush=True)
    print(
        f"[{SERVICE_NAME}] on-demand mode: exiting after {idle_timeout:.0f}s idle",
        flush=True,
    )
    try:
        await _idle_watchdog(last_activity, idle_timeout)
        print(f"[{SERVICE_NAME}] idle timeout reached, shutting down", flush=True)
    finally:
        await svc.stop()
        await nc.close()


if __name__ == "__main__":
    asyncio.run(main())
