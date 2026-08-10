# Cloudflare-Native Credential Vault + Durable Node-Data Mirror — Design

**Date:** 2026-08-10
**Status:** Approved for implementation planning
**Tracking:** b00t task #168 (parent mission); sub-project 6 (new — extends the
original 5-sub-project decomposition per operator directive 2026-08-10)
**Memory:** `project_telnyx_fax_service_ledgrrr.md`
**Depends on:** spec 3 (cloudflare-os deployed somewhere real — this spec's
constructs need an actual Cloudflare account, not just local `wrangler dev`)

## Background

The operator's 2026-08-10 directive proposed three Cloudflare-native
storage ideas in one breath: (1) b00t-cli running as "lazy-load streamable
wasm modules" for "deterministic syntax sugar," (2) a per-user MCP-key
secrets store with a Cloudflare-native "durable mirror" for consistent
global state, (3) a Spacedrive-inspired pattern for searchable, durable,
cost-efficient global mirrors of hive node-level data. Each was researched
against real, current (2026) Cloudflare documentation before design work
started, rather than taken at face value — two are sound with corrections,
one is not feasible as stated.

## Rejected: b00t-cli as a Rust/WASM Cloudflare Worker

**Not pursued.** Verified against Cloudflare's own docs and b00t-cli's
actual dependency tree:

- "Lazy-load streamable wasm modules" does not correspond to any real
  Cloudflare Workers feature — WASM modules are loaded via
  `WebAssembly.instantiate()` on pre-compiled binaries only, and
  Cloudflare's own guidance is that *larger* WASM Workers have *worse*
  cold-start latency (hence recommending `wasm-opt` to shrink them, the
  opposite of a "lazy-load" story).
- b00t-cli itself cannot compile to `wasm32-unknown-unknown`: it uses the
  full multi-threaded `tokio` runtime (Workers only supports single-
  threaded async via `wasm-bindgen-futures`), `openssl-sys` and
  `rusqlite`/`sqlx` (native C bindings, no WASM target), spawns OS
  processes in 64 files (`podman`, `systemctl`, `docker`, `kubectl` — 144
  shell-out references total), and does filesystem I/O in 127 files. None
  of this is a portability gap to close — it's b00t-cli's actual job
  (local process/container/datum orchestration), which is precisely what
  the Workers sandbox forbids by design.

The correct architecture for exposing b00t-cli's capabilities to
cloudflare-os is what **spec 4 already designed**: a natively-hosted
`b00t-mcp --http` server that a Gatekeeper Worker calls over HTTP. No
change needed there — this section exists to record that the WASM framing
was considered and rejected, not silently dropped.

## Credential vault: per-user Durable Object, not (only) Secrets Store

Cloudflare's Secrets Store (beta) is real but **deploy-time/binding-based**
— a Worker declares which secrets it can read in `wrangler.toml` at deploy
time; there's no API for a Worker to dynamically resolve an arbitrary
per-user key at request time. It fits *platform-wide* secrets (e.g. a
shared Telnyx API key all Workers use), not a "one key per user" model.

Durable Objects are **not** globally consistent — a DO instance is pinned
to one location, giving strict consistency *within that instance*, not
across regions. "Consistent global state" as a single Cloudflare feature
doesn't exist; the honest pattern is a DO as the single-writer source of
truth, with Workers KV as an eventually-consistent globally-read mirror —
which Cloudflare's own docs explicitly recommend for "credentials read at
high rates" that don't need strict global consistency.

**This pattern already has precedent inside cloudflare-os itself**:
`gatekeeper-mcp`'s `McpAccount` Durable Object — one per user/account,
storing OAuth tokens, refreshed proactively, inaccessible outside the
Worker. b00t's per-user MCP key vault should follow the same shape: one
`McpKeyAccount`-style DO per user, written once at enrollment, read by the
Gatekeeper Worker on each request.

**Confirmed by reading the actual source** (`packages/gatekeeper-mcp/src/mcp.ts`
and the shared base at `packages/mcp-shared/src/account.ts`, cloned locally
2026-08-10): `McpAccount extends McpAccountBase<Env>`, an abstract class
requiring only `baseUrl()`, `log()`, and `mintAccount()` from each
subclass. Its actual persistence is `this.ctx.storage.kv` — the Durable
Object's own built-in transactional key-value storage, not Workers KV, not
Secrets Store, not any external service. Keys observed: `server`,
`connectionGeneration`, `callback`, `nonce`, `tokens`, `oauthClient`,
`oauthDiscovery`, `oauthVerifier`, `pendingAuth`, `expiredNotified`,
`reconnecting`, `mcpSessionId` — a full connection-lifecycle state machine,
not just a flat secret store. **This retracts the earlier "KV as a
read-mirror" suggestion** — it was speculative before this read; the real
pattern doesn't need one. A single DO instance's own `ctx.storage.kv` is
already the complete, strongly-consistent, durable store for one user's
account state, matching the actual scope needed (per-user, not
cross-region). `McpKeyAccount` should extend `McpAccountBase` directly
(or closely mirror its shape) rather than reimplementing state management
from scratch — `mintAccount()`'s existing pattern (`this.ctx.exports.GatekeeperUserImpl(...)`,
Cloudflare's newer RPC-exports mechanism) is also the concrete, working
example of how a Gatekeeper Worker is meant to reach into its account DO.

**What this does not solve:** how a secret gets from its origin (e.g. the
operator's Windows machine, per the backlogged credproxy spec) into
Cloudflare in the first place. That's a separate, client-side
bootstrapping problem — an enrollment flow or an authorized `wrangler
secret put`/API call from the trusted source machine. Credproxy (task
#169) and this DO-based cloud vault are **complementary, not
alternatives**: credproxy solves local extraction, this solves cloud-side
storage/access-control once a secret is en route to Cloudflare.

## Durable node-data mirror: R2 + R2 SQL, Spacedrive as partial inspiration

Spacedrive's actually-transferable ideas are **content-addressing**
(BLAKE3 hash-as-identity, used for dedup) and **decoupling the index from
physical storage location** (its `SdPath` universal address). Its
P2P/HLC sync protocol does **not** transfer — that solves multi-device,
multi-writer, offline-first conflict resolution for end-user files, a
different problem than single-writer, append-mostly, server-side hive
telemetry (each node is the sole writer of its own data; there's no
conflict to resolve).

The load-bearing prior art is Cloudflare's own current stack: **R2 Data
Catalog + R2 SQL** (open beta, 2026) — a distributed SQL engine querying
Apache Iceberg tables backed by R2, priced per TB scanned. Pattern:

```
hive node → Worker ingest endpoint
  → raw telemetry/state blob written to R2, content-addressed key
    (e.g. hash(payload) or node/timestamp, borrowing Spacedrive's
    identity-by-hash idea for natural dedup)
  → structured row appended to an Iceberg table via R2 Data Catalog
  → a Worker exposes a query API (R2 SQL) resolving matching R2 keys,
    streamed back via the R2 binding
```

For simpler, lower-volume needs (not yet established this needs
Iceberg-scale analytics), **R2 + D1** is the lighter alternative: raw
blobs in R2, metadata rows in D1, hand-rolled indexing — full control,
less infrastructure, worth defaulting to for P0 rather than R2 SQL, which
should be adopted if/when actual query volume or data scale justifies it.

## Sequenced, not yet actionable: MSI installer + realtime visualization

Both require a **real Cloudflare deployment** to exist first — everything
in specs 3/4 so far runs locally via `wrangler dev`/podman. Recording the
dependency honestly rather than designing against infrastructure that
isn't there yet:

- **Installable MSI connecting to the Cloudflare-hosted MCP** — needs an
  actual deployed Gatekeeper Worker endpoint (spec 4, `wrangler deploy`,
  not `wrangler dev`) to connect to. Once that exists, the MSI itself is a
  bounded, separate packaging task (Rust cross-compiled to
  `x86_64-pc-windows-msvc`, packaged via `cargo-wix`) — not designed
  further here since designing a client for an endpoint that doesn't
  exist yet would be speculative.
- **Realtime visualization of process state hosted on Cloudflare** —
  once the R2/D1 (or R2 SQL) node-data mirror above exists and has real
  data flowing into it, this becomes a straightforward cloudflare-os
  Gadget or standalone Worker querying that mirror. Not designed further
  here for the same reason — no data source to visualize yet.

## Out of scope

- Any change to spec 4's already-correct native-HTTP-Worker architecture.
- Full R2 SQL / Iceberg catalog setup — P0 uses R2 + D1 per above.
- The MSI and visualization pieces themselves (see previous section).

## References

- [Rust language support — Cloudflare Workers](https://developers.cloudflare.com/workers/languages/rust/)
- [WebAssembly runtime APIs](https://developers.cloudflare.com/workers/runtime-apis/webassembly/)
- [Cloudflare Secrets Store beta](https://blog.cloudflare.com/secrets-store-beta/)
- [Durable Objects concepts](https://developers.cloudflare.com/durable-objects/concepts/what-are-durable-objects/)
- [Workers storage options comparison](https://developers.cloudflare.com/workers/platform/storage-options/)
- [Spacedrive](https://github.com/spacedriveapp/spacedrive) — VDFS/content-addressing reference, checked 2026-08-10
- [R2 SQL deep dive](https://blog.cloudflare.com/r2-sql-deep-dive/), [R2 SQL docs](https://developers.cloudflare.com/r2-sql/)
