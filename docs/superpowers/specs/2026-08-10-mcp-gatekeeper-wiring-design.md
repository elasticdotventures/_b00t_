# MCP Gatekeeper Wiring (ledgerr-mcp + b00t-mcp → cloudflare-os) — Design

**Date:** 2026-08-10
**Status:** Approved for implementation planning
**Tracking:** b00t task #168 (parent mission); this spec covers sub-project 4 of 5
**Memory:** `project_telnyx_fax_service_ledgrrr.md`
**Depends on:** spec 3 (cloudflare-os local deployment) for a place to point these at;
independent of spec 2 (fax MVP) — this wires the *transport*, not fax-specific tools.

## Background

Originally framed as "write custom Gatekeeper Workers for ledgerr-mcp and
b00t-mcp." Research corrected that: cloudflare-os already ships a generic
MCP bridge, `packages/gatekeeper-mcp` ("Connects any MCP server as a
Gadgets capability... one Worker covers every MCP server"), plus
`gatekeeper-mcp-portal` — an administrator-preconfigured variant that needs
no user to paste an endpoint and can mark a server **vetted** (auto-apply
capable) rather than **byo** (every non-read-only call needs human
approval via the Overseer). Since b00t-mcp and ledgerr-mcp are our own
known internal servers, not third-party services a user connects
ad hoc, `gatekeeper-mcp-portal` is the right fit — no new Worker code
needed on the cloudflare-os side at all.

The real gap is on *our* side: `gatekeeper-mcp`/`gatekeeper-mcp-portal`
connect over **Streamable HTTP**, not stdio.

- **b00t-mcp already supports both.** `b00t-mcp/src/main.rs` has `--stdio`
  and `--http` flags, using `rmcp::transport::streamable_http_server::
  StreamableHttpService`. Nothing to build here.
- **ledgerr-mcp is stdio-only, hand-rolled.**
  `crates/ledgerr-mcp/src/bin/ledgerr-mcp-server.rs` doesn't use the `rmcp`
  crate at all — it's a manual loop reading `stdin`/writing `stdout`, with
  hardcoded `initialize`/`tools/list`/`tools/call` matching. No HTTP mode
  exists.

## Decision: migrate ledgerr-mcp to `rmcp`

Rather than bolting a thin HTTP wrapper onto the hand-rolled loop, migrate
`ledgerr-mcp-server` to implement `rmcp`'s `ServerHandler` trait, mirroring
`b00t-mcp/src/mcp_server_rusty.rs`'s pattern exactly:

- `list_tools()` — returns what `mcp_adapter::tool_descriptors()` already
  produces, reshaped into `rmcp`'s expected type.
- `call_tool()` — wraps the *existing* big match-statement dispatch (all
  ~30 `mcp_adapter::handle_*` functions, `dispatch_reconciliation`,
  `dispatch_hsm`, etc.) unchanged. `b00t-mcp` proves this shape works:
  it implements the same two generic methods rather than one Rust method
  per tool, which is exactly ledgerr-mcp's current dispatch style.

This gets stdio *and* HTTP dual-mode "for free" from `rmcp` (matching the
explicit requirement that both servers support both transport modes),
without touching any of the actual business logic in the `handle_*`
functions — only the outer protocol/transport plumbing changes.

## Architecture

```
cloudflare-os (Gadget, agent session)
  → gatekeeper-mcp-portal Worker (admin-configured endpoint, vetted tier)
    → HTTP → b00t-mcp --http :<port>        (already works, no change)
    → HTTP → ledgerr-mcp --http :<port>      (rmcp migration, this spec)
```

Both MCP servers run as podman services (matching the hive's existing
convention), reachable from the cloudflare-os podman container over the
local network — no public exposure needed for local dev.

**Local dev auth:** `gatekeeper-mcp-portal`'s README (via `mcp-shared`)
follows the same `MCP_ALLOW_INSECURE=true` / `.dev.vars` local-dev path as
`gatekeeper-mcp` — no OAuth needed to connect a localhost/podman-network
endpoint during development. Production would need a real trust
decision (vetted-tier auto-apply is a meaningful capability grant) — out
of scope here.

## P0 test loop

Mirrors the mission's established playable-first pattern: get ONE tool
call working end-to-end through the full chain before anything else,
using the side that's already ready (b00t-mcp) rather than blocking on
the ledgerr-mcp migration:

1. cloudflare-os chat asks for something that maps to a real b00t-mcp tool
   (e.g. `b00t task list`).
2. Confirm the call reaches `gatekeeper-mcp-portal` → b00t-mcp's `--http`
   endpoint → a real result comes back into the chat.
3. Only after that loop is proven, verify the same path through the
   migrated ledgerr-mcp.

## Out of scope

- The fax-specific tools themselves (spec 2's concern, not this spec's).
- Production OAuth / vetted-tier trust decisions beyond local dev.
- Any new Gatekeeper Worker code — confirmed unnecessary; `gatekeeper-mcp-portal` already exists.
- `mcp-shared`'s scope grammar / tool-selection UI — out of the box from cloudflare-os, not something this hive builds.

## Risks

- `ledgerr-mcp`'s migration touches its main dispatch entry point, used by
  every existing tool — needs real test coverage (the crate already has
  `mcp-outcome-test.rs`; extend rather than replace) before trusting it
  over stdio *and* HTTP.
- `gatekeeper-mcp`/`gatekeeper-mcp-portal` are themselves part of an
  "early access... many rough edges" product (per the parent cloudflare-os
  README, spec 3) — treat friction there as expected, not necessarily a
  bug in this wiring.

## References

- [`packages/gatekeeper-mcp/README.md`](https://github.com/cloudflare/cloudflare-os/blob/main/packages/gatekeeper-mcp/README.md) — generic MCP bridge, checked 2026-08-10
- `b00t-mcp/src/main.rs`, `b00t-mcp/src/mcp_server_rusty.rs` — existing dual-mode reference implementation
- `vendor/ledgrrr/crates/ledgerr-mcp/src/bin/ledgerr-mcp-server.rs` — current stdio-only implementation to migrate
