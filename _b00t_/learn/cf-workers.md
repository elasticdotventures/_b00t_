# Cloudflare Workers — secret provisioning + deploy

Shared `just` recipes for every Cloudflare Worker under `workers/*/`
(`b00t-mcp-vault`, `telnyx-fax-handler`, `ledgrrr-tenant-registry`, and any
future one). Module: `workers/cf-workers.just`, declared as `mod cf-workers`
in the root `justfile`. Recipes are thin command surfaces; the actual
stateful logic lives in `workers/scripts/*.sh`.

## Recipes

```bash
just cf-workers::provision-secret <worker> <VAR_NAME> [length=32]
just cf-workers::deploy <worker>
just cf-workers::migrate-remote <worker> <db>
```

## `provision-secret` — idempotent by design

```bash
just cf-workers::provision-secret ledgrrr-tenant-registry TOKEN_SIGNING_KEY
```

- If `VAR_NAME` already exists in `~/.env`, its existing value is reused —
  the script never regenerates or appends a duplicate line for a var
  that's already present.
- If absent, generates `openssl rand -hex <length>` (default 32 bytes),
  appends `VAR_NAME="<value>"` to `~/.env`, then runs
  `wrangler secret put VAR_NAME` against `workers/<worker>/`.
- Reads `~/.env` by sourcing `~/.bash_profile` (its quote-stripping `.env`
  loader — see the "env loader quote-stripping" fix, PR #1126) rather than
  re-parsing `~/.env` itself. One parser, not two — if that loader is ever
  touched again, this script inherits the fix for free instead of drifting
  out of sync with a second implementation.

## `deploy` — install + wrangler deploy

```bash
just cf-workers::deploy ledgrrr-tenant-registry
```

Runs `pnpm install && wrangler deploy` in `workers/<worker>/`, with
`CLOUDFLARE_ACCOUNT_ID` defaulted to the real b00t Cloudflare account
(`f00c391669432ae2a423c04a001dab2d`) if not already set in the environment.

## `migrate-remote` — live production D1 migration

```bash
just cf-workers::migrate-remote ledgrrr-tenant-registry b00t-agents
```

⚠️ This runs `wrangler d1 migrations apply <db> --remote` against the REAL
production database. Confirm with the operator before running — this is
not a dry-run or local-only recipe.

## Why this module exists

Every Worker in `workers/*/` needs the same two things before it can serve
real traffic: a secret provisioned into its Cloudflare environment, and a
deploy. Before this module, each Worker's secret was set with a one-off
hand-typed `printf '%s' "$KEY" | wrangler secret put KEY` command — correct,
but not memoized, not idempotent, and easy to accidentally re-run in a way
that appends a duplicate `~/.env` line (the exact class of bug found and
fixed in PR #1126's `.env` loader). This module makes the safe version the
default, and the recipe is the documentation.

<!-- b00t:map v1
summary: Shared just recipes for Cloudflare Worker secret provisioning + deploy across workers/*/
tags: cloudflare, workers, wrangler, secrets, just, justfile
tier: sm0l
cmds: just cf-workers::provision-secret <worker> <VAR>, just cf-workers::deploy <worker>, just cf-workers::migrate-remote <worker> <db>
complexity: 3
-->
