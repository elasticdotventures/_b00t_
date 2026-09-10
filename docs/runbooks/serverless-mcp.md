# Runbook — serverless / on-demand MCP hosting (SP4)

How a backend MCP server gets declared, woken on demand, and scaled back to
zero. Design: `docs/superpowers/specs/2026-09-10-sp4-serverless-mcp-hosting-design.md`.

## Pieces

| piece | where | role |
|---|---|---|
| **DATUM** | `_b00t_/<svc>.mcp_server.toml` (`DatumType::McpServer`, `[b00t.mcp_server]` = `McpServerSpec`) | the launch spec. The connect URL is **derived** (`https://<svc>.b00t.promptexecution.com/mcp`), never stored. |
| **CONTROL PLANE** | `b00t mcp serve` (`b00t-cli/src/mcp_serve.rs`) | `GET /_b00t/route/<svc>` → load datum → ledgrrr budget gate → `placement.ensure()` → `{fqdn, warm}`. Idle reaper scales to zero. |
| **PLACEMENT** | `b00t-c0re-lib`: `PodmanPlacement` (dev), `AcaPlacement` (Azure Container Apps) | bring a `<svc>` up / down / report status. Provider-agnostic `McpPlacement` trait. |
| **BACKEND** | `containers/b00t-mcp-remote/` | `b00t-mcp --http --port 8080` + SP4-07 auth layer + `/health`. |
| **PROXY** | `b00t-c0re-lib::RemoteMcpProxy` | routes `<svc>__<tool>` calls; with `$B00T_MCP_CONTROL_URL` set, asks the control plane first. |

## Declare a server

`_b00t_/gh.mcp_server.toml`:

```toml
[b00t]
name = "gh"
type = "mcp_server"

[b00t.mcp_server]
image = "ghcr.io/promptexecution/gh-mcp@sha256:…"   # prod MUST be digest-pinned
port = 8080
memory = "1Gi"
cpu = 0.5
idle_timeout_seconds = 300
budget_ceiling = 5000                               # cake; 0 → flat launch cost of 1

[b00t.mcp_server.env]
GH_HOST = "github.com"

[b00t.mcp_server.placement]
backend = "aca"            # aca | podman | fly | cloudrun
region = "australiaeast"
```

Verify: `b00t mcp servers` (add `--json` for machine output).

## Run the control plane

```sh
# dev — podman backend, mock ledgrrr
b00t mcp serve                       # 127.0.0.1:8790, --backend podman

# prod — Azure Container Apps, real ledgrrr
export B00T_ACA_RESOURCE_GROUP=b00t-mcp
export B00T_LEDGRRR_MODE=http
export B00T_LEDGRRR_URL=https://ledgrrr.promptexecution.com
b00t mcp serve --backend aca --port 8790
```

Point the proxy at it:

```sh
export B00T_MCP_CONTROL_URL=http://127.0.0.1:8790
```

Now a `gh__list_repos` call through `b00t-mcp` does one
`GET /_b00t/route/gh` (which wakes a cold backend), then hits the returned
`fqdn`. Unset `B00T_MCP_CONTROL_URL` → straight-through to
`gh.b00t.promptexecution.com` (no waking).

## ACA: declaring the container app

`AcaPlacement` only toggles `--min-replicas` on an app **Terraform already
created**. Declaring a new `<svc>` is a Terraform step in
`PromptExecution/infrastructure` — a `for_each` over the declared servers in
`terraform/b00t/mcp-servers.tf`, each instantiating the deployed
`terraform/azure/modules/standing-mcp-server` module (scale-to-zero, managed
identity, `b00t-node-budget` guard, `idle_timeout_seconds` default 300,
output `fqdn`). `b00t mcp serve` never runs `terraform apply`.

Rough flow for a new ACA-backed service:

1. add `_b00t_/<svc>.mcp_server.toml` (this repo).
2. add `<svc>` to the `mcp-servers.tf` `for_each` map (infra repo) and
   `terraform apply` — creates the app at `min-replicas = 0`.
3. `b00t mcp serve --backend aca` picks it up on the first
   `GET /_b00t/route/<svc>`.

## Cost model (v1)

- One flat `authorize_spend` at cold launch (`cost = budget_ceiling` or `1`),
  idempotency ref `launch:<svc>:<YYYY-MM-DD>`. `ok:false` → `503
  {"error":"budget_exceeded"}`, `ensure()` never called.
- Running-cost ceiling is the ACA module's own `b00t-node-budget` guard.
- Metered-per-warm-minute billing is a follow-up (ledgrrr `record_usage` on a
  timer).

## Idle / teardown

The control plane's reaper wakes every 30 s and, for any warm `<svc>` whose
last `route/<svc>` hit is older than its `idle_timeout_seconds`, calls
`placement.stop(<svc>)` (podman `stop`; ACA `--min-replicas 0`) and drops it
from the warm set. The next call cold-starts it again.

## Troubleshooting

| symptom | check |
|---|---|
| `route/<svc>` → 404 `unknown_service` | `b00t mcp servers` — is the datum present and `[b00t.mcp_server]` populated? key is `<svc>` (or `<svc>.mcp_server`). |
| `route/<svc>` → 503 `budget_exceeded` | ledgrrr denied. `B00T_LEDGRRR_MODE=mock` for dev; check the tenant's cake balance in prod. |
| `route/<svc>` → 502 `launch_failed` | podman/`az` error in the detail field. `--backend` mismatch with the datum's `placement.backend` is only a warning, not the cause. |
| backend never goes `warm:true` | `/health` not 200 within the deadline (podman 30 s, ACA 45 s). Exec the container, `curl localhost:8080/health`. |
| proxy ignores the control plane | `$B00T_MCP_CONTROL_URL` unset, or a `base_url_override` (test) is in force. |
