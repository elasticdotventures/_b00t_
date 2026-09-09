# b00t OpenTelemetry Collector

Telemetry hub for the build / orchestration plane (plan gleaming-jingling-nygaard).
One collector per orchestration substrate; the first one runs on the dstack
**control node**.

## What it does

```
 dstack server (>=0.20.28) ─OTLP┐
 Dagster runs ─────────────OTLP─┼─► otelcol ─► file  /var/lib/otelcol/telemetry.jsonl  (durable, rotated)
 build-plane tasks ────────OTLP─┘             ├─► Prometheus  :8889/metrics  (plane)
                                              ├─► Prometheus  :8888/metrics  (collector self)
                                              └─► otlphttp    $OTEL_DOWNSTREAM_ENDPOINT  (optional: Grafana Cloud / Tempo / Mimir)
```

The base config (`otelcol-config.yaml`) is safe standalone — local capture
only. `entrypoint.sh` layers `otelcol-config.downstream.yaml` **only** when
`OTEL_DOWNSTREAM_ENDPOINT` is set, so there is no self-loop when running local.

## Run

```sh
just otel-up          # podman-compose up -d
just otel-logs        # follow
just otel-down
```

or directly:

```sh
podman-compose -f containers/otel-collector/compose.yaml up -d
```

## Wire emitters

| Emitter | Setting |
|---|---|
| dstack server | `OTEL_EXPORTER_OTLP_ENDPOINT=http://<host>:4317` (env on the server process; see `nats/pyinfra/deploy_cp_node.py`) |
| Dagster | `OTEL_EXPORTER_OTLP_ENDPOINT=http://<host>:4317` (already defaulted in `orchestration/dagster/`) |
| dstack tasks | export the same var in the task `env:` if the workload is instrumented |

## Fan out to a hosted backend

```sh
export OTEL_DOWNSTREAM_ENDPOINT=https://otlp.eu.grafana.net/otlp
export OTEL_DOWNSTREAM_AUTH="Basic $(printf '%s' "$INSTANCE_ID:$TOKEN" | base64 -w0)"
just otel-up
```

## Follow-ups

- k8s `Deployment` variant for the sm3lly single-node cluster / services fleet
  (namespace `b00t-otel`, per the b00t- prefix rule).
- `deploy_cp_node.py`: add the collector as a managed unit + set
  `OTEL_EXPORTER_OTLP_ENDPOINT` on the dstack server unit.
- Pin bump cadence: track `otel/opentelemetry-collector-contrib` releases.
