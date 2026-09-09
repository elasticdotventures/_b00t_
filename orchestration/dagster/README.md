# b00t · Dagster over the dstack plane

Trial of [Dagster](https://github.com/dagster-io/dagster) as the **DAG /
sequencing layer** above dstack. dstack owns *elastic spot capacity + teardown*;
Dagster owns *ordering, retries, backfills, schedules, and run monitoring* — the
things dstack has no native answer for. First DAG: `ci_build_plane`, the
build-plane CI pipeline as a graph.

```
build_archive ──► run_1_4 ─┐
              ├─► run_2_4 ─┤
              ├─► run_3_4 ─┼─► collect
              └─► run_4_4 ─┘
```

Each op shells `dstack apply -f <task.yaml> -n <name> -e … --yes` (the same
contract as `just remote-*` and `.github/workflows/ci-build-plane.yml`). No
dstack Python SDK — the CLI is the seam.

## Setup

```sh
cd orchestration/dagster
uv sync
```

## Run

Offline / CI (echoes the dstack commands, provisions nothing):

```sh
B00T_DSTACK_DRY_RUN=1 uv run dagster job execute -m b00t_dagster.definitions -j ci_build_plane
```

Live (needs `dstack project add main …` configured — see the repo justfile):

```sh
uv run dagster job execute -m b00t_dagster.definitions -j ci_build_plane \
  --config-json '{"ops":{"build_archive":{"config":{"ci_ref":"<sha>"}},
                  "run_1_4":{"config":{"ci_ref":"<sha>"}},
                  "run_2_4":{"config":{"ci_ref":"<sha>"}},
                  "run_3_4":{"config":{"ci_ref":"<sha>"}},
                  "run_4_4":{"config":{"ci_ref":"<sha>"}}}}'
```

UI:

```sh
uv run dagster dev            # http://localhost:3000
```

## Telemetry

Ops emit OpenTelemetry spans (`dstack.apply`, `ci.build_archive`,
`ci.run_partition`) to `OTEL_EXPORTER_OTLP_ENDPOINT` (default
`http://localhost:4317` — the collector in `containers/otel-collector/`). If the
OTel SDK or endpoint is absent the spans degrade to no-ops.

## Env

| var | default | meaning |
|---|---|---|
| `B00T_DSTACK_DRY_RUN` | `0` | `1` = echo `dstack` commands, don't provision |
| `B00T_REPO_ROOT` | repo root inferred from this file | cwd for `dstack apply` (needs `dev-env/*.task.yaml`) |
| `DSTACK_PROJECT` | `main` | dstack project |
| `OTEL_EXPORTER_OTLP_ENDPOINT` | `http://localhost:4317` | collector |

## Why not just the GitHub Actions matrix?

The workflow is fine for one repo's CI. Dagster earns its place when there are
*many* multi-stage jobs sharing the plane — backfills, cross-job dependencies,
partitioned runs, a single monitoring surface, ret/alerting — i.e. the
"1000s of cold-standby services / multi-stage batch" direction. This scaffold is
the smallest real thing that proves the seam.
