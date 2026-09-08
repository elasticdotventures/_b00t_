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

## Proxy tier — pingap

`pingap` (`vendor/pingap-devproxy-b00t`, upstream `vicanso/pingap`) is the
reverse-proxy / TLS / health-check / JSON-status layer. Single upstream
(`DSTACK_UPSTREAM_ADDR`), config templated from env at container start.

**Open item:** pingap has no native "run code and block until upstream up"
request hook. Chosen wiring: pingap health-check on the upstream + a `webhook`
on health-status→unhealthy that calls `bin/wake.sh`; the first client request
gets a 503 + `Retry-After` while the VM boots, and `remote-doctor` / the
`remote-*` recipes already do a warm-up `curl --max-time 150` with retries. If
pingap's webhook proves awkward, fall back to a 40-line reverse proxy (hyper /
axum) that does start-then-proxy inline. Decide when building the image.

## Files (this dir)

- `bin/wake.sh` — the wake core (present).
- `Containerfile`, `pingap/*.toml.tmpl`, `bin/run.sh`, `justfile` — TODO, next
  authoring pass. Held back deliberately: the `network_mode=tailnet` decision
  (2026-09-08b) may move this off Cloud Run, which changes the entrypoint shape.
