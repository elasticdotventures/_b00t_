# ── Cloudflare Workers — secret provisioning + deploy ────────────────────────
# Shared recipes for every Worker under workers/*/ (b00t-mcp-vault,
# telnyx-fax-handler, ledgrrr-tenant-registry, ...). Thin command surfaces —
# stateful logic lives in workers/scripts/*.sh. See `b00t learn cf-workers`.

# Provision (or reuse) a secret in ~/.env and push it to a Worker.
# Idempotent: reuses an existing ~/.env value rather than regenerating one —
# never appends a duplicate line for a var that's already present.
provision-secret worker var length="32":
    #!/usr/bin/env bash
    set -euo pipefail
    "{{source_directory()}}/scripts/provision-secret.sh" {{worker}} {{var}} --length {{length}}

# Deploy a Worker (its secrets must already be provisioned).
deploy worker:
    #!/usr/bin/env bash
    set -euo pipefail
    export CLOUDFLARE_ACCOUNT_ID="${CLOUDFLARE_ACCOUNT_ID:-f00c391669432ae2a423c04a001dab2d}"
    cd "{{source_directory()}}/{{worker}}"
    pnpm install
    wrangler deploy

# Apply a Worker's D1 migrations to the REAL production database.
# Live action — confirm with the operator before running.
migrate-remote worker db:
    #!/usr/bin/env bash
    set -euo pipefail
    export CLOUDFLARE_ACCOUNT_ID="${CLOUDFLARE_ACCOUNT_ID:-f00c391669432ae2a423c04a001dab2d}"
    cd "{{source_directory()}}/{{worker}}"
    wrangler d1 migrations apply {{db}} --remote
