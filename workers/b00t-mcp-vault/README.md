# b00t-mcp-vault

A thin, stateless Cloudflare Worker exposing get/set access to the `mcp_key`
column on the real, already-deployed `b00t-agents` D1 database
(`86bb3c9d-309a-4d27-8856-38934dd316b1`).

Not deployed yet — written as real, ready-to-deploy source. Deployment
itself needs the operator's own `wrangler` login; the Cloudflare MCP
connector used to design this can read/create D1/KV/R2 resources but has
no Worker-deploy capability.

## Background

`b00t-agents`' `agents`/`roles` schema (NATS-JWT hive-agent identity) is
real, production, and predates this Worker — it was seeded with 5 roles
but had zero registered agents when discovered. The `mcp_key TEXT` column
was added via a live `ALTER TABLE` (additive, nullable — no existing rows
to affect) to extend that schema for per-agent MCP credential storage,
rather than building a new Durable-Object-based vault mirroring an
unrelated product (see `docs/superpowers/specs/2026-08-10-cloudflare-native-credential-vault-and-node-mirror-design.md`
in the parent repo for the full design history and rationale).

## API

Both routes require `Authorization: Bearer <VAULT_ADMIN_KEY>`.

- `GET /agents/:id/mcp_key` → `{"id": "...", "mcp_key": "..." | null}`, or 404 if the agent id doesn't exist.
- `PUT /agents/:id/mcp_key` with body `{"mcp_key": "..."}` → `{"id": "...", "updated": true}`, or 404/400.

`VAULT_ADMIN_KEY` is a single operator-held secret gating both routes —
not a per-user credential system. It authorizes *administration* of the
vault (which is itself where per-agent MCP keys live), not end-user access
to their own key.

## Deploy

```bash
cd workers/b00t-mcp-vault
pnpm install   # or npm install
wrangler secret put VAULT_ADMIN_KEY   # prompts for the value, stores it as a real Worker secret
wrangler deploy
```

## Local dev

```bash
wrangler dev
```

Uses the real `b00t-agents` D1 binding even in `wrangler dev` unless a
local/preview D1 override is configured — be aware `wrangler dev` without
`--local`/`--persist-to` flags can read/write the real production
database. Prefer testing against a specific known-inert agent id
(`agents` had zero rows as of 2026-08-11) rather than experimenting freely.
