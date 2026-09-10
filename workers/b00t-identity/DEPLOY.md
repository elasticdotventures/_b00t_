# Deploying `b00t-identity`

The SP1 product-identity plane: RS256 JWT issuance + JWKS + tenant registry.

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
