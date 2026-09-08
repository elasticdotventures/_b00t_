# b00t-cp-waker

Traffic-activated power switch for the dstack control node (plan
gleaming-jingling-nygaard, Phase 2.5 / 2.75).

The control-node VM (`e2-small`) is **powered off when dstack has no work** — a
systemd reaper on the box itself does that (Phase 3, `b00t-cp-idle-reaper.sh`).
This waker is the other half: it **starts** the VM when a request arrives and
reverse-proxies `:3000` (dstack server) once it's ready. It never stops the VM.

## Two deployment targets, one wake core

| | `network_mode = public` (build phase) | `network_mode = tailnet` (end state) |
|---|---|---|
| runs on | Cloud Run v2, `min-instances=0` (~$0 idle) | a tailnet service on the Vultr `b00t-node` |
| identity | `b00t-cp-waker` SA via metadata ADC (keyless) | narrow SA key (`compute.instances.get`+`start`, one instance) delivered by pyinfra `--data` → `0600` |
| reachable by | public HTTPS URL (`dstack project add --url`) | MagicDNS name on the tailnet only |
| TF | `google_cloud_run_v2_service.cp_waker[0]` | Cloud Run `count = 0`; SA + role still created so the key can be minted |

`bin/wake.sh` is the wake core and is identical in both — it only needs
`PROJECT_ID`, `CONTROL_ZONE`, `CONTROL_INSTANCE`, a token source, and the
upstream address.

## Env contract

| var | meaning |
|---|---|
| `PROJECT_ID` | GCP project |
| `CONTROL_ZONE` | e.g. `australia-southeast1-a` |
| `CONTROL_INSTANCE` | `b00t-dstack-control` |
| `DSTACK_UPSTREAM_ADDR` | `<control-internal-ip>:3000` — pingap upstream |
| `READINESS_PATH` | `/healthz` (⚠️ confirm dstack 0.20.28 serves this unauthenticated; else point at `/` and accept any 2xx/3xx, or bake an admin token) |
| `START_DEADLINE_SECONDS` | poll budget after issuing `instances.start` (default 120) |
| `IDLE_GRACE` | informational here; the reaper on the VM owns shutdown |
| `GCP_SA_KEY_FILE` | tailnet mode only — path to the `0600` SA key; unset ⇒ metadata ADC |

## v1 impl — `waker.py` (not pingap)

pingap has no native "run code and block until upstream up" request hook, needs
a sidecar for the wake, and its routing/TLS/plugins are wasted on a single
Cloud-Run-fronted upstream. So v1 is `waker.py` — stdlib-only (~140 lines):
`GET /_waker/health` → 200 without waking; anything else → `ensure_running()`
(metadata-ADC `instances.get`; `instances.start` if not RUNNING; poll
`READINESS_PATH` to `START_DEADLINE_SECONDS`, else 503 + `Retry-After`) → then
reverse-proxy method/headers/body to `DSTACK_UPSTREAM_ADDR`.

`pingap` (`vendor/pingap-devproxy-b00t`) is the **Phase 2.75** upgrade, where it
earns its keep as the *tailnet edge* (host routing, TLS, plugins) rather than a
single-upstream shim.

## Files (this dir)

- `waker.py` — v1 proxy + wake, **working** (`just smoke`).
- `Containerfile` — `python:3.12-slim` + `waker.py`.
- `justfile` — `smoke` / `build` / `push` / `deploy`.
- `bin/wake.sh` — earlier standalone wake core; superseded by `waker.py`, kept
  for the tailnet-mode / non-container path.

**Image**: `australia-southeast1-docker.pkg.dev/promptexecution/b00t/b00t-cp-waker:latest`
(Artifact Registry, in-project → Cloud Run pulls with no extra IAM). TF ref =
`var.waker_image`; `just push` to rebuild.
