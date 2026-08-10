# cloudflare-os Local Deployment — Design

**Date:** 2026-08-10
**Status:** Approved for implementation planning
**Tracking:** b00t task #168 (parent mission); this spec covers sub-project 3 of 5
**Memory:** `project_telnyx_fax_service_ledgrrr.md`
**Depends on:** nothing. Independent of credproxy (1) and fax MVP (2).

## Background

Third of five sub-projects decomposed out of the Telnyx/wrangler mission. The
operator's original framing — "wrangler-cli can manage/setup cloudflare-os
with access to both ledgrrr-mcp and b00t-mcp as sidecars" — turned out to
bundle two things that needed separating once the actual target was
researched:

1. **`wrangler` is Cloudflare's own deployment CLI**, not something b00t
   builds. It is the standard tool for running/deploying a Cloudflare
   Workers app — in this case, `cloudflare-os`
   (github.com/cloudflare/cloudflare-os, Apache-2.0, TypeScript): a real,
   actively-developed "AI productivity environment" / agent-workspace app
   that Cloudflare open-sourced.
2. **cloudflare-os does not support plain MCP for external tool
   integration.** Its own README states this explicitly: it uses a
   different, proprietary-to-this-project pattern called **Gatekeepers**
   ("like supercharged MCP servers") — each one a separate Cloudflare
   Worker wrapping an external service's API via a Cap'n Web RPC interface,
   with built-in OAuth handling, narrow access scoping, action logging, and
   asynchronous human-in-the-loop approval (the agent can queue actions
   against a *simulated* result and the human approves/rejects later, in
   bulk — not the usual synchronous blocking approval most MCP setups use).

Wiring `ledgerr-mcp` and `b00t-mcp` in as cloudflare-os "sidecars" therefore
means *writing new Gatekeeper Workers* — real TypeScript engineering
(learning the Cap'n Web API, building OAuth flows, translating each MCP
server's tool surface into the Gatekeeper pattern) — not a deployment or
config task. That work is out of scope for this spec; it becomes (or
merges into) sub-project 4, MCP sidecar wiring, designed separately.

**This spec covers only:** getting cloudflare-os running locally, as a
working chat UI, with no external tool access yet.

## Architecture

A podman container runs cloudflare-os's own documented local-dev flow:
`pnpm run-local`, which the project's README confirms "runs the whole stack
locally on wrangler and workerd" — i.e., `pnpm run-local` is a wrapper
around `wrangler dev` that starts the full multi-package Workers app
(router + backend services, per `wrangler.jsonc`'s `services` binding
setup) using Cloudflare's own local emulation (Miniflare-backed KV/D1/
Durable Objects), no custom binding reimplementation needed — matching the
scope decision already made (dev-mode local emulation, not true
self-hosting outside Cloudflare's edge).

The container image needs: Node.js, `pnpm` (the repo's declared package
manager, per `pnpm-workspace.yaml`/`pnpm-lock.yaml`), and a clone of
`cloudflare-os` at build or run time. Port 8787 (the README's documented
local URL) is published from the container bound to `0.0.0.0`, not just
loopback — matching how the qwen36-27b inference profile is already
reachable from other machines on the network (`ss` shows `*:8001`, not
`127.0.0.1:8001`), so this instance is reachable the same way rather than
being accidentally localhost-only.

## What this spec delivers

- A podman-run cloudflare-os instance, reachable at `http://0.0.0.0:8787`
  (i.e. from any machine on the network, not just this host) running the
  agent chat UI, sandboxed "Gadget" app creation, and the built-in
  Gatekeepers cloudflare-os already ships with (whatever those are —
  GitHub/Google integrations are mentioned in the README's example
  prompts) — all with zero custom code from this hive.
- Nothing that talks to ledgrrr, b00t, or Telnyx. That is explicitly
  deferred.

## Out of scope

- Any Gatekeeper Worker for `ledgerr-mcp` or `b00t-mcp` (→ sub-project 4).
- Production deployment to Cloudflare's actual edge (`wrangler deploy` —
  the README's "deploy to your Cloudflare account" path). This spec is
  dev-mode-only, matching the earlier scope decision.
- Any change to cloudflare-os's own code. This spec runs it as-is.

## Risks

cloudflare-os's own README describes it as **"early access"** — "this
repository is actually version 2, a complete rewrite... still has many
rough edges." Treat instability, undocumented setup steps, and possible
breaking changes between commits as expected, not a sign something in this
spec is wrong. No specific incident to report yet — flagging this
up front so it isn't mistaken for a packaging problem later.

## Testing

- Container builds and `pnpm run-local` starts without error.
- `curl http://localhost:8787` (or the equivalent podman-published port)
  returns the chat UI's HTML shell.
- Manually exercise one of the README's example prompts (e.g. "Make a tic
  tac toe game") to confirm the agent chat loop itself works end-to-end,
  independent of any hive-specific integration.

## References

- [cloudflare/cloudflare-os](https://github.com/cloudflare/cloudflare-os) — README, `wrangler.jsonc`, Gatekeeper description (checked 2026-08-10)
- Zero results searching the repo for "mcp" — confirmed no native MCP client support exists as of this date
