# D · Health-Metrics — make the bus legible

[[Watchdog]] publishes `b00t.hive.mesh.health.{crashloop,snapshot}`;
[[Agent-Doctor]] publishes `b00t.hive.mesh.discovery.presence`. Nothing consumes
them yet.

## Stories

- **HM-001 — `b00t hive health`** a subcommand (or `scripts/b00t-hive-health.sh`
  for now — no cargo build on the constrained host): subscribe to
  `b00t.hive.mesh.health.>` + `…discovery.presence` for ~3s, plus scrape the
  NATS monitor at `http://localhost:8222/varz` + `/connz`, and print a table:
  unit / active / NRestarts / last-snapshot-age / tripped?.
- **HM-002 — retention** append every `snapshot`/`crashloop` to
  `.b00t/hive-health.jsonl` via a tiny always-on `nats sub` service (mirror
  [[Watchdog]]'s datum). Rotate at 50 MB.
- **HM-003 — Prometheus (optional)** a `nats sub` → textfile-collector shim
  writing `/var/lib/node_exporter/textfile/b00t_hive.prom`
  (`b00t_hive_unit_restarts{unit=…}`, `b00t_hive_crashloop_total`). Only if a
  node_exporter is already running — else skip.

## Non-goals

No Grafana provisioning, no new TSDB. The bus + JSONL + an on-demand table is
enough to never be blind again.

Depends on [[Watchdog]] (shipped) and [[Agent-Doctor]] BD-004. See [[Review-Gate]].
