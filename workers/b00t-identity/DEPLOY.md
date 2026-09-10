# Deploying `b00t-identity`

The SP1 product-identity plane: RS256 JWT issuance + JWKS + tenant registry.

The source directory is `b00t-identity`, but Wrangler intentionally retains the
deployed Worker name `ledgrrr-tenant-registry`. Its local `TenantNode` binding
therefore continues using the existing Durable Object namespace and D1
`root_do_id` values. Do not override `--name` on deploy. Renaming the deployed
Worker requires a separately planned namespace transfer; changing only `name`
would provision unrelated storage. See [Cloudflare namespace migrations](https://developers.cloudflare.com/durable-objects/reference/durable-objects-migrations/).

## 1. Provision secrets (idempotent — reuses `~/.env` values)

```bash
cd workers
just -f cf-workers.just provision-secret b00t-identity JWT_PRIVATE_KEY_PEM 0   # paste a PKCS#8 PEM
just -f cf-workers.just provision-secret b00t-identity JWT_KID 16
just -f cf-workers.just provision-secret b00t-identity REGISTRY_ADMIN_KEY 48
```

Plain vars (not secrets) — set in the Cloudflare dashboard or a `[vars]` block:

| var | value |
|---|---|
| `LEDGRRR_MODE` | `mock` until ledgrrr #175 ships, then `http` |
| `LEDGRRR_BASE_URL` | ledgrrr's base URL (only read when `LEDGRRR_MODE=http`) |

`JWT_PRIVATE_KEY_PEM` is a PKCS#8 RSA private key; its public half is published
at `GET /.well-known/jwks.json` keyed by `JWT_KID`. The **same key/kid** signs
datum packages (`$B00T_DATUM_SIGNING_KEY_PEM` / `$B00T_DATUM_SIGNING_KID` on the
CLI side, SP2-04).

## 2. Apply D1 migrations to production

`b00t-agents` is a **shared, `prevent_destroy`** database — migrations are
additive-only.

```bash
just -f cf-workers.just migrate-remote b00t-identity b00t-agents
```

Applies `migrations/0001_create_tenants.sql` (if not already) and
`migrations/0002_tenant_slug.sql`.

## 3. Deploy

```bash
just -f cf-workers.just deploy b00t-identity
```

Routes (from `wrangler.jsonc`):
- `b00t.promptexecution.com/.well-known/*` — public JWKS
- `b00t.promptexecution.com/identity/*` — `/identity/tenants`, `/identity/tokens`,
  `/identity/verify`, `DELETE /identity/tenants/:id/agents/:id` (the worker
  strips the `/identity` prefix internally)

Clients use the origin as `B00T_IDENTITY_URL` (no `/identity` suffix).
`HttpTokenSource::with_issuance_credential` and CLI environment variable
`B00T_IDENTITY_ISSUANCE_TOKEN` supply the `REGISTRY_ADMIN_KEY` bearer required
by `/identity/tokens`. JWKS remains at `/.well-known/jwks.json` without auth.
Issuance checks the stored agent grant's role, or the closest membership's
role for legacy node grants. A requested role must match that authorization.
Tokens include their node and grant source; `/identity/verify` rechecks that
authorization, including role and scopes. Older tokens lacking this context
must be reissued.

## 4. Seed the first-party tenants

```bash
cd workers/b00t-identity
pnpm exec tsx scripts/seed.ts https://b00t.promptexecution.com "$REGISTRY_ADMIN_KEY"
```

## Terraform note

`terraform/b00t/` in `PromptExecution/infrastructure` owns the Cloudflare
substrate. `wrangler deploy` above publishes the **Worker code + its route
bindings**; a future `terraform/b00t/identity-worker.tf` should *adopt* the
Worker's D1 / DO bindings via `import` + `ignore_changes` /
`keep_bindings = true` (the same pattern `b00t_api` uses), never re-declare
them — the D1 database has `prevent_destroy`.
