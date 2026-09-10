# SP4 — On-demand serverless MCP hosting (design)

Sub-project 4 of the `b00t.promptexecution.com` platform program (parent spec:
`2026-09-10-b00t-platform-sp123-design.md`). SP1–SP3 are complete (PR #1296);
SP3-08 already ships the **client** side — `RemoteMcpProxy` routes
`<svc>__<tool>` → `https://<svc>.<base-domain>/mcp` and forwards the caller JWT.
SP4 is the **server** side: declare an MCP server as a datum, launch it on
demand on the cheapest capable cloud, idle it back to zero, meter its cost.

## Context

The proxy (SP3) assumes a backend MCP server answers at
`https://<svc>.b00t.promptexecution.com/mcp`. Today nothing does — the wildcard
DNS is a placeholder. SP4 makes `<svc>` real:

- a **`DatumType::McpServer`** datum carries the launch spec (OCI image, resources,
  env, idle policy, budget) — the routing URL is *derived* from the datum key,
  never stored (keeps D3's "no hosting provider hard-coded")
- **`b00t mcp serve`** (task #809) is the control plane: reconcile declared
  `McpServer` datums against a placement backend, wake on first request, idle
  after `idle_timeout_seconds`, refuse to launch when the budget is spent
- **placement is pluggable**: an `McpPlacement` trait; the first impl wraps the
  already-deployed `terraform/azure/modules/standing-mcp-server` ACA module
  (scale-to-zero, managed identity, budget guard, idle ~$0); a `LocalPodman`
  impl for dev; Fly.io / Cloud Run / Cloudflare-Containers impls are additive
- backend servers authenticate the caller: they verify the forwarded JWT against
  the SP1 JWKS (reuse `b00t-c0re-identity::verify_jwt`) and receive
  `CallerIdentity` — the same contract SP3 enforces at the proxy

**Deferred to SP4 itself, not blocking:** per-server SPIFFE-SVID delivery (the
build-plane's `spiffe-csi-driver` pattern) — v1 uses the SP1 RS256 JWT the proxy
forwards; SPIFFE is the SP6/platform-internal story.

## Locked decisions (from the parent plan)

| # | Decision | SP4 consequence |
|---|---|---|
| **D3** | On-demand MCP servers = OCI containers, cost-placed, no vendor lock-in; standardize `rmcp`/FastMCP **Streamable-HTTP + `/health`** | `McpServerSpec.image` is a digest-pinned OCI ref; every backend exposes `/mcp` (streamable-http) + `/health`; `McpPlacement` is a trait, ACA is impl #1 |
| **D1** | ledgrrr integration is HTTP-only, mock-first | the budget check before a launch reuses `b00t-c0re-ledgrrr::SpendAuthorizer` (the SP3-06 crate) — a launch is a metered spend |
| **D2** | product identity = SP1 RS256 JWT | backend servers verify the forwarded JWT via `b00t-c0re-identity`; no new IdP |

## Reusable substrate (call these, don't rebuild)

- **`terraform/azure/modules/standing-mcp-server/`** (infra, DEPLOYED) — ACA
  scale-to-zero: user-assigned managed identity + AcrPull, HTTP trigger, scale
  `0→max_replicas`, `idle_timeout_seconds` (default 300), optional
  Redis/pgwire/Dapr/OTel sidecars, a `b00t-node-budget` guard, digest-pinned
  images. Outputs `fqdn`, `managed_identity_client_id`. **This is `McpPlacement`
  impl #1.**
- **`b00t-cli/src/datum_mcp.rs::McpDatum`** — already models *connecting* to an
  MCP server (`stdio` | `httpstream` with `url`). Per the 2026-08-16
  app4dog-workspace decision, do NOT add a parallel `McpServerDatum` — SP4 adds
  a `[b00t.mcp_server]` **launch** section to the same `BootDatum`, mirroring
  how SP2-05 added `[b00t.agent_profile]`.
- **`DatumType::McpServer`** — the reserved variant (`datum_types.rs:69`,
  suffix `.mcp_server`, `implies McpServer→Mcp`, `semantic_class Protocol`).
  Already wired in the type table + stereotype lattice; SP4 just gives it a
  payload struct + dispatch, exactly like SP2-01/02 did for `AgentProfile`.
- **`b00t-c0re-lib/src/remote_mcp_proxy.rs::RemoteMcpProxy`** (SP3-08) — the
  client. SP4 changes nothing here; it makes `route()`'s target real.
- **`b00t-c0re-ledgrrr` (SP3-06)** — `SpendAuthorizer` for the pre-launch budget
  check.
- **`b00t-c0re-identity` (SP1-08)** — `verify_jwt` / `CallerIdentity` for the
  backend server's auth layer (identical to SP3-01's `JwksVerifier`).
- **`nats/pyinfra/files/b00t-cp-idle-reaper.sh`** — the build-plane's proven
  idle-reaper shape (established-connection + non-terminal-work checks →
  poweroff). `b00t mcp serve`'s idle logic follows the same fail-safe
  philosophy (any uncertainty → stay up).
- **`b00t-cli/src/commands/mcp.rs`** — existing `b00t mcp` subcommands; `serve`
  is a new one alongside.
- **`*.b00t.promptexecution.com` wildcard** (`infra terraform/b00t/cloudflare.tf`)
  — already exists, "routed by Worker". SP4's per-service records / Worker route
  make specific `<svc>` names resolve to the placed backend.

## Aligned open issues (fold in, don't re-file)

b00t **#809** (`b00t mcp serve`), infra **#139** (Dapr / service-mesh RBAC,
required-service reachability — the ACA module already has the `enable_dapr`
hook), the 2026-08-16 `McpDatum` non-fork decision (`datum_types.rs:54-68`
comment), b00t **#1104** (agent-scoped tokens — a launched server inherits the
caller's), the platform plan's SP4 line.

## Architecture — three pieces

```
DATUM              _b00t_/<svc>.mcp_server.toml   ── DatumType::McpServer
                   [b00t]            name / type = "mcp_server"
                   [b00t.mcp_server] image@sha256, cpu/mem, port, env, secrets_ref,
                                     idle_timeout_seconds, budget_ceiling (cake),
                                     placement { backend = "aca" | "podman" | ... }
                                     (NO url — derived: https://<svc>.<base-domain>/mcp)

CONTROL PLANE      b00t mcp serve                 ── task #809
                   · reconcile declared McpServer datums vs the placement backend
                   · GET /_b00t/route/<svc>  → ensure placed + warm, return target FQDN
                     (or the proxy calls this before forwarding)
                   · budget precheck (b00t-c0re-ledgrrr) — deny launch if spent
                   · idle reaper: no established :<port> conn + last-hit age
                     > idle_timeout_seconds  → scale to zero
                   · McpPlacement trait: ensure(spec) -> Endpoint{fqdn}; stop(svc); status(svc)
                        AcaPlacement   (wraps standing-mcp-server module via a thin
                                        `terraform apply -target` / Azure SDK shim)
                        PodmanPlacement (dev: `podman run --rm -p` + a local record)

BACKEND SERVER     the containerised MCP server (OCI image)
                   · rmcp streamable-http at /mcp  +  GET /health
                   · an auth layer: verify `Authorization: Bearer` via
                     b00t-c0re-identity::verify_jwt(SP1 JWKS) → CallerIdentity
                     (401 when B00T_MCP_REQUIRE_AUTH=1 and absent/invalid)
                   · a reference image: `containers/b00t-mcp-remote/` wrapping an
                     existing b00t-mcp in --http mode as the first real <svc>
```

`<svc>` resolution: the proxy's `RemoteMcpProxy::call` (SP3-08) already targets
`https://<svc>.<base-domain>/mcp`. SP4 adds, on the first call for an
un-warm `<svc>`, a control-plane round-trip (`b00t mcp serve` HTTP, or a Worker
binding) that `ensure()`s the placement and blocks until `/health` is 200 —
then the forwarded request goes straight to the backend FQDN.

## Decomposition (SP4-01 … SP4-10)

| id | title | key files | contract (essentials) | dep | sz |
|---|---|---|---|---|---|
| **SP4-01** | `DatumType::McpServer` payload struct | `b00t-cli/src/datum_mcp_server.rs`(new), `boot_datum.rs` (`mcp_server: Option<McpServerSpec>`), `datum_types.rs` (arms already present — just a `from_type_token` test) | `McpServerSpec{ image:String (must contain `@sha256:` in prod), cpu:f32, memory:String, port:u16(=8080), env:BTreeMap<String,String>, secrets_ref:Vec<String>, idle_timeout_seconds:u32(=300), budget_ceiling:u64, placement:Placement }`; `Placement{ backend:PlacementBackend(Aca|Podman|Fly|CloudRun), region:Option<String>, extra:BTreeMap<String,String> }` | SP2-05 pattern | S |
| **SP4-02** | `derive_endpoint(svc, base_domain) -> Url` + no-`url` invariant | same file | `endpoint("gh", "b00t.promptexecution.com") == https://gh.b00t.promptexecution.com/mcp`; a datum that *does* carry a `url` is a hard parse error (that's an `Mcp` datum, not an `McpServer`) | SP4-01 | S |
| **SP4-03** | `McpPlacement` trait + `PodmanPlacement` (dev) | `b00t-c0re-lib/src/mcp_placement.rs`(new) | `trait McpPlacement { async fn ensure(&self, svc:&str, spec:&McpServerSpec) -> Result<Endpoint>; async fn stop(&self, svc:&str) -> Result<()>; async fn status(&self, svc:&str) -> Result<PlacementStatus> }`; `Endpoint{fqdn:String, warm:bool}`; `PodmanPlacement` = `podman run -d --rm -p 0:<port> --label b00t.mcp=<svc>` + parse the mapped host port, poll `/health` | SP4-01 | M |
| **SP4-04** | `AcaPlacement` — wrap `standing-mcp-server` | `b00t-c0re-lib/src/mcp_placement_aca.rs`(new), `infra terraform/b00t/mcp-servers.tf`(new, a `for_each` over a JSON of declared servers) | `ensure()` = write/patch the per-`<svc>` `module` block's tfvars + `terraform apply -target=module.mcp["<svc>"]` (or an Azure SDK `az containerapp update --min-replicas 1` warm-path); return `outputs.fqdn`. `stop()` = `--min-replicas 0`. Idle-to-zero is the module's own `idle_timeout_seconds`. | SP4-03 | L |
| **SP4-05** | `b00t mcp serve` control-plane command | `b00t-cli/src/commands/mcp.rs` (new `Serve` subcommand), `b00t-cli/src/mcp_serve.rs`(new) | `b00t mcp serve [--backend aca|podman] [--port 8790]` — an axum server: `GET /_b00t/route/<svc>` → load the `<svc>.mcp_server` datum → budget precheck → `placement.ensure()` → `{ "fqdn": ..., "warm": true }`; `GET /_b00t/status`; a background idle-reaper task per warm svc | SP4-03, SP4-04, SP3-06 | L |
| **SP4-06** | pre-launch budget gate | `b00t-cli/src/mcp_serve.rs` | before `ensure()` on a cold svc: `SpendAuthorizer::authorize_spend(tenant, agent, cost=<spec.budget_ceiling or a flat launch cost>, ref="launch:<svc>:<date>")`; `ok:false` → `503 {"error":"budget_exceeded"}`, no launch. mock-first (`B00T_LEDGRRR_MODE`). | SP4-05, SP3-06 | S |
| **SP4-07** | backend auth layer (reusable) | `b00t-c0re-lib/src/mcp_backend_auth.rs`(new) or reuse `b00t-mcp/src/http_auth.rs` verbatim | a `tower` layer identical to SP3-02a — verify the forwarded `Bearer` against the SP1 JWKS → `CallerIdentity` in extensions; `B00T_MCP_REQUIRE_AUTH`. Factor SP3-02a's `identity_middleware` into `b00t-c0re-lib` so both the proxy and every backend share one impl. | SP3-02a | M |
| **SP4-08** | reference backend image | `containers/b00t-mcp-remote/Containerfile`(new), `containers/b00t-mcp-remote/README.md` | a digest-pinnable OCI image running `b00t-mcp --http --port 8080` with the SP4-07 auth layer + `/health`; the first real `<svc>` (`b00t.b00t.promptexecution.com` — the proxy pointing at itself, useful for e2e) | SP4-07 | M |
| **SP4-09** | proxy ↔ control-plane wiring | `b00t-c0re-lib/src/remote_mcp_proxy.rs` (SP3-08) — add an optional `control_plane_url`; `b00t-mcp` `call_tool` remote path | when routing a `<svc>__<tool>` and `$B00T_MCP_CONTROL_URL` is set, `GET {control}/_b00t/route/<svc>` first, use the returned `fqdn`; else fall straight through to `<svc>.<base-domain>` (today's behaviour). | SP4-05, SP3-08 | M |
| **SP4-10** | `b00t datum` + `b00t mcp` parity, docs | `b00t-cli/src/commands/datum.rs` (nothing — type-agnostic), `b00t-cli/src/commands/mcp.rs` (`b00t mcp servers` lists declared `McpServer` datums + placement status), `docs/runbooks/serverless-mcp.md`(new) | `b00t mcp servers` → table {svc, image, backend, warm?, last-hit, budget-left}; runbook covers declaring a server, `terraform apply` for ACA, cost expectations | SP4-01, SP4-05 | S |

### Build order

```
Wave 0  SP4-01  SP4-02          (datum shape)
Wave A  SP4-03  SP4-07          (placement trait + shared auth layer)
Wave B  SP4-04  SP4-08          (ACA impl + reference image)
Wave C  SP4-05  SP4-06          (control plane + budget gate)
Wave D  SP4-09  SP4-10          (proxy wiring + CLI/docs)
```
Critical path: `SP4-01 → SP4-03 → SP4-05 → SP4-09`.

## Integration checkpoints

1. **CP-1 datum round-trip** (SP4-01/02): `<svc>.mcp_server.toml` parses to
   `McpServerSpec`; `derive_endpoint` matches SP3-08's `RemoteMcpProxy::route`
   host exactly (one golden test shared).
2. **CP-2 podman e2e** (SP4-03/08): `PodmanPlacement.ensure()` starts the
   reference image, `/health` goes 200, `RemoteMcpProxy::call` (base-URL
   override) reaches it and forwards the bearer.
3. **CP-3 budget-denied launch** (SP4-05/06): `MockSpendAuthorizer::always_deny`
   → `GET /_b00t/route/<svc>` returns 503, `placement.ensure` is never called
   (spy).
4. **CP-4 auth parity** (SP4-07): the factored `identity_middleware` gives byte-
   identical behaviour in `b00t-mcp` (SP3-02a's tests still pass) and in a
   backend.
5. **CP-5 warm-path** (SP4-09): with `$B00T_MCP_CONTROL_URL` set, a
   `gh__list_repos` call does one `route/<svc>` round-trip then hits the
   returned FQDN; unset → straight-through (SP3-08 regression).

## Top risks

- **ACA `ensure()` latency** — a cold ACA app can take 10–30 s to first byte.
  The control plane must hold the `route/<svc>` request open (with a deadline)
  and the proxy must have a matching timeout; document it.
- **`terraform apply -target` from a long-running daemon is ugly** — SP4-04's
  warm path should prefer the Azure SDK (`min-replicas` toggle on an
  already-`terraform`'d module) and reserve `apply` for *declaring a new*
  server. Keep the tf-JSON (`mcp-servers.tf` `for_each`) as the source of truth;
  the daemon only toggles replicas.
- **"no vendor lock-in" vs one real backend** — mitigated by the `McpPlacement`
  trait + `PodmanPlacement` shipping in the same wave as `AcaPlacement`; the
  datum's `placement.backend` is the switch; the routing URL never names a
  provider.
- **Secrets** — `McpServerSpec.secrets_ref` are *names*, resolved by the
  placement backend from its own secret store (ACA secrets / Azure Key Vault
  via the module's managed identity). The datum never carries secret values.
- **Budget cost model** — is a launch a fixed cake charge, or metered per warm-
  minute? v1: a flat `authorize_spend` at launch + rely on the ACA module's own
  `b00t-node-budget` guard for the running cost ceiling. Metered-per-minute is a
  follow-up.
- **The `McpServer` datum must reject a `url`** — otherwise it's ambiguous with
  an `Mcp` httpstream datum (the 2026-08-16 concern). Enforce in `serde` /
  a validate hook.

## Deferred / open

- Per-server **SPIFFE-SVID** (vs the forwarded SP1 JWT) — platform-internal,
  aligns with SP6 / the build-plane's `spiffe-csi-driver`.
- **Fly.io / Cloud Run / Cloudflare Containers** placement impls — additive
  `McpPlacement` impls, each its own small task.
- **Metered-per-minute billing** for warm servers → ledgrrr `record_usage` on a
  timer; v1 is launch-time `authorize_spend` + the ACA budget guard.
- **Auto-scaling beyond 1 replica** — the module supports `max_replicas` but
  single-user MCP servers stay at 1; multi-replica needs session affinity.
- SP5 (SysML/Oxigraph) and SP6 (isolation + billing rollup + `b00t-website`
  migration + `tailscale.tf` split) — separate spec→plan cycles.
