"""Thin dstack driver + OpenTelemetry wiring for the Dagster plane.

`dstack apply -f <cfg> -n <name> [-e K=V ...] --yes` runs a task to completion
and exits with the task's code — so each Dagster op is just a subprocess call
whose return code is the op's success. No dstack Python SDK dependency: the CLI
is the contract (matches `just remote-*` and `.github/workflows/ci-build-plane.yml`).
"""

from __future__ import annotations

import os
import shlex
import subprocess
from collections.abc import Mapping

from dagster import ConfigurableResource, get_dagster_logger

try:  # OTel is optional at import time; spans degrade to no-ops if the SDK/env is absent
    from opentelemetry import trace
    from opentelemetry.exporter.otlp.proto.grpc.trace_exporter import OTLPSpanExporter
    from opentelemetry.sdk.resources import Resource
    from opentelemetry.sdk.trace import TracerProvider
    from opentelemetry.sdk.trace.export import BatchSpanProcessor

    _OTEL = True
except Exception:  # pragma: no cover - SDK not installed
    _OTEL = False

_TRACER = None


def tracer():
    """Process-wide tracer; exports OTLP to the collector (containers/otel-collector/).

    Endpoint from OTEL_EXPORTER_OTLP_ENDPOINT (default the local collector).
    """
    global _TRACER
    if _TRACER is not None:
        return _TRACER
    if not _OTEL:
        return _NoopTracer()
    endpoint = os.environ.get("OTEL_EXPORTER_OTLP_ENDPOINT", "http://localhost:4317")
    provider = TracerProvider(
        resource=Resource.create(
            {"service.name": os.environ.get("OTEL_SERVICE_NAME", "b00t-dagster")}
        )
    )
    provider.add_span_processor(
        BatchSpanProcessor(OTLPSpanExporter(endpoint=endpoint, insecure=True))
    )
    trace.set_tracer_provider(provider)
    _TRACER = trace.get_tracer("b00t_dagster")
    return _TRACER


class _NoopSpan:
    def __enter__(self):
        return self

    def __exit__(self, *a):
        return False

    def set_attribute(self, *a, **k):
        pass


class _NoopTracer:
    def start_as_current_span(self, *a, **k):
        return _NoopSpan()


class DstackResource(ConfigurableResource):
    """Runs dstack task configs. `dry_run` echoes the command instead of
    provisioning — used by `dagster job execute` in CI / offline validation."""

    project: str = os.environ.get("DSTACK_PROJECT", "main")
    repo_root: str = os.environ.get(
        "B00T_REPO_ROOT",
        os.path.abspath(os.path.join(os.path.dirname(__file__), "..", "..", "..")),
    )
    dry_run: bool = os.environ.get("B00T_DSTACK_DRY_RUN", "0") == "1"

    def apply(self, config: str, name: str, env: Mapping[str, str] | None = None) -> int:
        log = get_dagster_logger()
        cmd = ["dstack", "apply", "-f", config, "-n", name]
        for k, v in (env or {}).items():
            cmd += ["-e", f"{k}={v}"]
        cmd += ["--yes"]
        pretty = " ".join(shlex.quote(c) for c in cmd)

        with tracer().start_as_current_span("dstack.apply") as span:
            span.set_attribute("dstack.config", config)
            span.set_attribute("dstack.run_name", name)
            span.set_attribute("dstack.project", self.project)
            span.set_attribute("dstack.dry_run", self.dry_run)

            if self.dry_run:
                log.info(f"[dry-run] (cwd={self.repo_root}) DSTACK_PROJECT={self.project} {pretty}")
                return 0

            log.info(f"(cwd={self.repo_root}) DSTACK_PROJECT={self.project} {pretty}")
            proc = subprocess.run(
                cmd,
                cwd=self.repo_root,
                env={**os.environ, "DSTACK_PROJECT": self.project},
                check=False,
            )
            span.set_attribute("dstack.exit_code", proc.returncode)
            if proc.returncode != 0:
                raise RuntimeError(f"dstack apply {name} exited {proc.returncode}")
            return proc.returncode
