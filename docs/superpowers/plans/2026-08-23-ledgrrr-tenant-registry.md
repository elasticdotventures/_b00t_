# ledgrrr Tenant/Org Registry Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build the tenant/org identity + access-control layer for the ledgrrr multi-tenant service — a D1 tenant registry routing to one Durable-Object-with-SQLite per tenant, with hierarchical (business-unit/directory/tag) nodes, transitive membership authorization, and a token issuance flow extending #1104.

**Architecture:** A Cloudflare Worker (`ledgrrr-tenant-registry`) fronts two tiers: a `tenants` table added to the existing production `b00t-agents` D1 (thin routing table, `tenant_id → root_do_id`), and a `TenantNode` Durable Object (one instance per tenant, SQLite storage) holding that tenant's `nodes`/`members` tables. Cross-tenant isolation is structural — the Worker never opens more than one DO per request.

**Tech Stack:** TypeScript, Cloudflare Workers + Durable Objects (SQLite storage), D1, `wrangler`, `vitest` + `@cloudflare/vitest-pool-workers` for local testing (no live Cloudflare account needed to run tests).

**Spec:** `docs/superpowers/specs/2026-08-23-ledgrrr-tenant-identity-design.md`

## Global Constraints

- D1 database: `b00t-agents` (binding `DB`, database_id `86bb3c9d-309a-4d27-8856-38934dd316b1`) — the `tenants` table is additive; never modify the existing `agents` table's columns.
- `compatibility_date` must be `2026-08-01` or later (needed for DO SQLite storage) — match the existing `telnyx-fax-handler`/`b00t-mcp-vault` workers' value for consistency.
- `@cloudflare/workers-types` must be pinned `^5.20260823.1` or later — the earlier `^4.20260801.0` pin no longer resolves (fixed this session in the sibling workers).
- Every DO/D1 test in this plan runs against **local** simulated storage via `@cloudflare/vitest-pool-workers` — none of Tasks 1–6 touch the real production `b00t-agents` D1. Applying the `tenants` table migration to the real database is a separate, explicit, final task (Task 7) that must be run with the operator's awareness, matching this session's established "additive-only, live D1 mutation" pattern (precedent: the `mcp_key` column add).
- Authorization logic lives entirely inside the `TenantNode` Durable Object — the Worker's `index.ts` never runs a SQL query against a tenant's `nodes`/`members` tables directly; it only ever calls the DO.

---

## Task 1: Project scaffolding + local D1 tenants-table migration

**Files:**
- Create: `workers/ledgrrr-tenant-registry/package.json`
- Create: `workers/ledgrrr-tenant-registry/tsconfig.json`
- Create: `workers/ledgrrr-tenant-registry/wrangler.jsonc`
- Create: `workers/ledgrrr-tenant-registry/.gitignore`
- Create: `workers/ledgrrr-tenant-registry/migrations/0001_create_tenants.sql`
- Create: `workers/ledgrrr-tenant-registry/vitest.config.ts`
- Create: `workers/ledgrrr-tenant-registry/test/env.d.ts`

**Interfaces:**
- Consumes: nothing (first task).
- Produces: the `wrangler.jsonc` D1 binding name `DB` and Durable Object binding name `TENANT_DO` (class `TenantNode`) that every later task's code and tests reference.

- [ ] **Step 1: Write `package.json`**

```json
{
  "name": "ledgrrr-tenant-registry",
  "private": true,
  "scripts": {
    "dev": "wrangler dev",
    "deploy": "wrangler deploy",
    "test": "vitest run"
  },
  "devDependencies": {
    "wrangler": "^4.15.2",
    "typescript": "^5.9.3",
    "@cloudflare/workers-types": "^5.20260823.1",
    "@cloudflare/vitest-pool-workers": "^0.8.19",
    "vitest": "^2.1.9"
  }
}
```

- [ ] **Step 2: Write `tsconfig.json`**

```json
{
  "compilerOptions": {
    "target": "ES2022",
    "lib": ["ES2022"],
    "module": "ES2022",
    "moduleResolution": "Bundler",
    "types": ["@cloudflare/workers-types", "@cloudflare/vitest-pool-workers"],
    "strict": true,
    "skipLibCheck": true,
    "noEmit": true
  },
  "include": ["src", "test"]
}
```

- [ ] **Step 3: Write `wrangler.jsonc`**

```jsonc
{
  "$schema": "node_modules/wrangler/config-schema.json",
  "name": "ledgrrr-tenant-registry",
  "main": "src/index.ts",
  "compatibility_date": "2026-08-01",
  "d1_databases": [
    {
      "binding": "DB",
      "database_name": "b00t-agents",
      "database_id": "86bb3c9d-309a-4d27-8856-38934dd316b1",
      "migrations_dir": "migrations"
    }
  ],
  "durable_objects": {
    "bindings": [
      { "name": "TENANT_DO", "class_name": "TenantNode" }
    ]
  },
  "migrations": [
    { "tag": "v1", "new_sqlite_classes": ["TenantNode"] }
  ]
}
```

- [ ] **Step 4: Write the D1 migration file `migrations/0001_create_tenants.sql`**

```sql
CREATE TABLE tenants (
    id TEXT PRIMARY KEY,
    kind TEXT NOT NULL CHECK (kind IN ('personal', 'organizational')),
    display_name TEXT NOT NULL,
    root_do_id TEXT NOT NULL,
    created_at TEXT NOT NULL
);
```

- [ ] **Step 5: Write `.gitignore`**

```
.wrangler/
node_modules/
```

- [ ] **Step 6: Write `vitest.config.ts`**

```typescript
import { defineWorkersConfig } from "@cloudflare/vitest-pool-workers/config";

export default defineWorkersConfig({
  test: {
    poolOptions: {
      workers: {
        wrangler: { configPath: "./wrangler.jsonc" },
      },
    },
  },
});
```

- [ ] **Step 7: Write `test/env.d.ts`**

```typescript
declare module "cloudflare:test" {
  interface ProvidedEnv {
    DB: D1Database;
    TENANT_DO: DurableObjectNamespace<import("../src/tenant-do").TenantNode>;
  }
}
```

- [ ] **Step 8: Install dependencies and run the D1 migration locally**

Run: `cd workers/ledgrrr-tenant-registry && pnpm install && pnpm wrangler d1 migrations apply b00t-agents --local`
Expected: migration `0001_create_tenants.sql` applies with no errors, creating the local `tenants` table in the simulated D1 instance.

- [ ] **Step 9: Commit**

```bash
git add workers/ledgrrr-tenant-registry/package.json workers/ledgrrr-tenant-registry/tsconfig.json workers/ledgrrr-tenant-registry/wrangler.jsonc workers/ledgrrr-tenant-registry/.gitignore workers/ledgrrr-tenant-registry/migrations/0001_create_tenants.sql workers/ledgrrr-tenant-registry/vitest.config.ts workers/ledgrrr-tenant-registry/test/env.d.ts
git commit -m "chore(ledgrrr-tenant-registry): scaffold worker, D1 migration, test harness"
```

---

## Task 2: Tenant registry routes (create + lookup)

**Files:**
- Create: `workers/ledgrrr-tenant-registry/src/registry.ts`
- Create: `workers/ledgrrr-tenant-registry/src/index.ts`
- Test: `workers/ledgrrr-tenant-registry/test/registry.test.ts`

**Interfaces:**
- Consumes: `env.DB` (D1Database, from Task 1's `wrangler.jsonc`), `env.TENANT_DO` (DurableObjectNamespace, from Task 1).
- Produces: `createTenant(db: D1Database, doNamespace: DurableObjectNamespace, input: { kind: "personal" | "organizational"; displayName: string }): Promise<Tenant>` and `lookupTenant(db: D1Database, tenantId: string): Promise<Tenant | null>`, where `Tenant = { id: string; kind: "personal" | "organizational"; displayName: string; rootDoId: string; createdAt: string }`. Later tasks (3–6) consume `lookupTenant` to resolve a `tenant_id` to a DO stub.

- [ ] **Step 1: Write the failing test for `lookupTenant` returning null on unknown id**

```typescript
import { describe, it, expect } from "vitest";
import { env } from "cloudflare:test";
import { lookupTenant } from "../src/registry";

describe("lookupTenant", () => {
  it("returns null for an unknown tenant id", async () => {
    const result = await lookupTenant(env.DB, "nonexistent-id");
    expect(result).toBeNull();
  });
});
```

- [ ] **Step 2: Run test to verify it fails**

Run: `pnpm test -- registry.test.ts`
Expected: FAIL — `lookupTenant` is not defined (module `../src/registry` doesn't exist yet).

- [ ] **Step 3: Write `src/registry.ts`**

```typescript
export interface Tenant {
  id: string;
  kind: "personal" | "organizational";
  displayName: string;
  rootDoId: string;
  createdAt: string;
}

interface TenantRow {
  id: string;
  kind: string;
  display_name: string;
  root_do_id: string;
  created_at: string;
}

function rowToTenant(row: TenantRow): Tenant {
  return {
    id: row.id,
    kind: row.kind as "personal" | "organizational",
    displayName: row.display_name,
    rootDoId: row.root_do_id,
    createdAt: row.created_at,
  };
}

export async function lookupTenant(db: D1Database, tenantId: string): Promise<Tenant | null> {
  const row = await db
    .prepare("SELECT id, kind, display_name, root_do_id, created_at FROM tenants WHERE id = ?")
    .bind(tenantId)
    .first<TenantRow>();
  return row ? rowToTenant(row) : null;
}

export async function createTenant(
  db: D1Database,
  doNamespace: DurableObjectNamespace,
  input: { kind: "personal" | "organizational"; displayName: string }
): Promise<Tenant> {
  const id = crypto.randomUUID();
  const doId = doNamespace.newUniqueId();
  const rootDoId = doId.toString();
  const createdAt = new Date().toISOString();

  await db
    .prepare(
      "INSERT INTO tenants (id, kind, display_name, root_do_id, created_at) VALUES (?, ?, ?, ?, ?)"
    )
    .bind(id, input.kind, input.displayName, rootDoId, createdAt)
    .run();

  return { id, kind: input.kind, displayName: input.displayName, rootDoId, createdAt };
}
```

- [ ] **Step 4: Run test to verify it passes**

Run: `pnpm test -- registry.test.ts`
Expected: PASS

- [ ] **Step 5: Write the failing test for `createTenant` + round-trip `lookupTenant`**

```typescript
it("creates a tenant and looks it up by id", async () => {
  const { createTenant, lookupTenant } = await import("../src/registry");
  const created = await createTenant(env.DB, env.TENANT_DO, {
    kind: "organizational",
    displayName: "Acme Corp",
  });
  expect(created.kind).toBe("organizational");
  expect(created.displayName).toBe("Acme Corp");
  expect(created.rootDoId).toBeTruthy();

  const found = await lookupTenant(env.DB, created.id);
  expect(found).toEqual(created);
});
```

- [ ] **Step 6: Run test to verify it passes (implementation already covers this)**

Run: `pnpm test -- registry.test.ts`
Expected: PASS (both tests green — 2 passed)

- [ ] **Step 7: Write `src/index.ts` (Worker fetch handler wiring the routes)**

```typescript
import { createTenant, lookupTenant } from "./registry";
export { TenantNode } from "./tenant-do";

export interface Env {
  DB: D1Database;
  TENANT_DO: DurableObjectNamespace;
}

export default {
  async fetch(request: Request, env: Env): Promise<Response> {
    const url = new URL(request.url);

    if (request.method === "POST" && url.pathname === "/tenants") {
      const body = await request.json<{ kind?: string; displayName?: string }>();
      if (body.kind !== "personal" && body.kind !== "organizational") {
        return new Response(JSON.stringify({ error: "kind must be 'personal' or 'organizational'" }), {
          status: 400,
          headers: { "Content-Type": "application/json" },
        });
      }
      if (!body.displayName) {
        return new Response(JSON.stringify({ error: "displayName is required" }), {
          status: 400,
          headers: { "Content-Type": "application/json" },
        });
      }
      const tenant = await createTenant(env.DB, env.TENANT_DO, {
        kind: body.kind,
        displayName: body.displayName,
      });
      return new Response(JSON.stringify(tenant), {
        status: 201,
        headers: { "Content-Type": "application/json" },
      });
    }

    const tenantMatch = url.pathname.match(/^\/tenants\/([^/]+)$/);
    if (request.method === "GET" && tenantMatch) {
      const tenant = await lookupTenant(env.DB, decodeURIComponent(tenantMatch[1]));
      if (!tenant) {
        return new Response(JSON.stringify({ error: "tenant not found" }), {
          status: 404,
          headers: { "Content-Type": "application/json" },
        });
      }
      return new Response(JSON.stringify(tenant), {
        status: 200,
        headers: { "Content-Type": "application/json" },
      });
    }

    return new Response("Not found", { status: 404 });
  },
};
```

Note: `export { TenantNode } from "./tenant-do"` is required here even though `tenant-do.ts` doesn't exist until Task 3 — Task 3's first step creates a minimal stub so this import resolves. If running this task in isolation before Task 3, create an empty stub first (see Task 3 Step 1).

- [ ] **Step 8: Commit**

```bash
git add workers/ledgrrr-tenant-registry/src/registry.ts workers/ledgrrr-tenant-registry/src/index.ts workers/ledgrrr-tenant-registry/test/registry.test.ts
git commit -m "feat(ledgrrr-tenant-registry): tenant registry create/lookup routes"
```

---

## Task 3: TenantNode Durable Object — schema init + node/member CRUD

**Files:**
- Create: `workers/ledgrrr-tenant-registry/src/tenant-do.ts`
- Test: `workers/ledgrrr-tenant-registry/test/tenant-do.test.ts`

**Interfaces:**
- Consumes: nothing external (DO manages its own SQLite storage via `this.ctx.storage.sql`).
- Produces: class `TenantNode extends DurableObject` with methods `createNode(input: { parentId: string | null; kind: "business_unit" | "directory" | "tag"; name: string; settingsJson?: string }): Promise<Node>`, `addMember(agentId: string, nodeId: string, role: string): Promise<void>`, where `Node = { id: string; parentId: string | null; kind: string; name: string; settingsJson: string }`. Tasks 4–6 consume these plus the DO's internal SQLite schema (`nodes`, `members` tables) directly.

- [ ] **Step 1: Write the failing test for schema auto-initialization**

```typescript
import { describe, it, expect } from "vitest";
import { env, runInDurableObject } from "cloudflare:test";

describe("TenantNode schema", () => {
  it("initializes nodes and members tables on first access", async () => {
    const id = env.TENANT_DO.newUniqueId();
    const stub = env.TENANT_DO.get(id);
    await runInDurableObject(stub, async (instance) => {
      const tables = instance.ctx.storage.sql
        .exec("SELECT name FROM sqlite_master WHERE type = 'table' ORDER BY name")
        .toArray()
        .map((row: any) => row.name);
      expect(tables).toContain("nodes");
      expect(tables).toContain("members");
    });
  });
});
```

- [ ] **Step 2: Run test to verify it fails**

Run: `pnpm test -- tenant-do.test.ts`
Expected: FAIL — `../src/tenant-do` module doesn't exist.

- [ ] **Step 3: Write `src/tenant-do.ts` (schema init + CRUD)**

```typescript
import { DurableObject } from "cloudflare:workers";

export interface TenantNodeRow {
  id: string;
  parentId: string | null;
  kind: "business_unit" | "directory" | "tag";
  name: string;
  settingsJson: string;
}

export class TenantNode extends DurableObject {
  constructor(ctx: DurableObjectState, env: unknown) {
    super(ctx, env);
    this.ctx.storage.sql.exec(`
      CREATE TABLE IF NOT EXISTS nodes (
        id TEXT PRIMARY KEY,
        parent_id TEXT REFERENCES nodes(id),
        kind TEXT NOT NULL CHECK (kind IN ('business_unit', 'directory', 'tag')),
        name TEXT NOT NULL,
        settings_json TEXT NOT NULL DEFAULT '{}'
      )
    `);
    this.ctx.storage.sql.exec(`
      CREATE TABLE IF NOT EXISTS members (
        agent_id TEXT NOT NULL,
        node_id TEXT NOT NULL REFERENCES nodes(id),
        role TEXT NOT NULL,
        PRIMARY KEY (agent_id, node_id)
      )
    `);
    this.ctx.storage.sql.exec(`CREATE INDEX IF NOT EXISTS idx_nodes_parent ON nodes(parent_id)`);
    this.ctx.storage.sql.exec(`CREATE INDEX IF NOT EXISTS idx_members_agent ON members(agent_id)`);
  }

  async createNode(input: {
    parentId: string | null;
    kind: "business_unit" | "directory" | "tag";
    name: string;
    settingsJson?: string;
  }): Promise<TenantNodeRow> {
    const id = crypto.randomUUID();
    const settingsJson = input.settingsJson ?? "{}";
    this.ctx.storage.sql.exec(
      "INSERT INTO nodes (id, parent_id, kind, name, settings_json) VALUES (?, ?, ?, ?, ?)",
      id,
      input.parentId,
      input.kind,
      input.name,
      settingsJson
    );
    return { id, parentId: input.parentId, kind: input.kind, name: input.name, settingsJson };
  }

  async addMember(agentId: string, nodeId: string, role: string): Promise<void> {
    this.ctx.storage.sql.exec(
      "INSERT INTO members (agent_id, node_id, role) VALUES (?, ?, ?)",
      agentId,
      nodeId,
      role
    );
  }
}
```

- [ ] **Step 4: Run test to verify it passes**

Run: `pnpm test -- tenant-do.test.ts`
Expected: PASS

- [ ] **Step 5: Write the failing test for `createNode` + `addMember`**

```typescript
it("creates a node and adds a member to it", async () => {
  const id = env.TENANT_DO.newUniqueId();
  const stub = env.TENANT_DO.get(id);
  await runInDurableObject(stub, async (instance) => {
    const node = await instance.createNode({ parentId: null, kind: "business_unit", name: "Engineering" });
    expect(node.kind).toBe("business_unit");
    expect(node.name).toBe("Engineering");

    await instance.addMember("agent-1", node.id, "member");

    const members = instance.ctx.storage.sql
      .exec("SELECT agent_id, node_id, role FROM members WHERE node_id = ?", node.id)
      .toArray();
    expect(members).toHaveLength(1);
    expect(members[0]).toMatchObject({ agent_id: "agent-1", node_id: node.id, role: "member" });
  });
});
```

- [ ] **Step 6: Run test to verify it passes**

Run: `pnpm test -- tenant-do.test.ts`
Expected: PASS (2 passed)

- [ ] **Step 7: Commit**

```bash
git add workers/ledgrrr-tenant-registry/src/tenant-do.ts workers/ledgrrr-tenant-registry/test/tenant-do.test.ts
git commit -m "feat(ledgrrr-tenant-registry): TenantNode DO schema init + node/member CRUD"
```

---

## Task 4: Transitive membership authorization (recursive CTE)

**Files:**
- Modify: `workers/ledgrrr-tenant-registry/src/tenant-do.ts`
- Modify: `workers/ledgrrr-tenant-registry/test/tenant-do.test.ts`

**Interfaces:**
- Consumes: `nodes`/`members` tables from Task 3.
- Produces: `hasMembershipPath(agentId: string, targetNodeId: string): Promise<boolean>` on `TenantNode` — Task 5 (`GrantsAccess`) and Task 6 (token issuance) call this first.

- [ ] **Step 1: Write the failing test for direct membership**

```typescript
it("hasMembershipPath: true for direct membership", async () => {
  const id = env.TENANT_DO.newUniqueId();
  const stub = env.TENANT_DO.get(id);
  await runInDurableObject(stub, async (instance) => {
    const node = await instance.createNode({ parentId: null, kind: "business_unit", name: "Eng" });
    await instance.addMember("agent-1", node.id, "member");
    const result = await instance.hasMembershipPath("agent-1", node.id);
    expect(result).toBe(true);
  });
});
```

- [ ] **Step 2: Run test to verify it fails**

Run: `pnpm test -- tenant-do.test.ts`
Expected: FAIL — `hasMembershipPath` is not a function.

- [ ] **Step 3: Add `hasMembershipPath` to `src/tenant-do.ts`**

Add this method inside the `TenantNode` class, after `addMember`:

```typescript
  async hasMembershipPath(agentId: string, targetNodeId: string): Promise<boolean> {
    const rows = this.ctx.storage.sql
      .exec(
        `
        WITH RECURSIVE ancestors(id) AS (
          SELECT ? AS id
          UNION ALL
          SELECT nodes.parent_id FROM nodes JOIN ancestors ON nodes.id = ancestors.id
          WHERE nodes.parent_id IS NOT NULL
        )
        SELECT 1 FROM members
        WHERE members.agent_id = ?
          AND members.node_id IN (SELECT id FROM ancestors)
        `,
        targetNodeId,
        agentId
      )
      .toArray();
    return rows.length > 0;
  }
```

- [ ] **Step 4: Run test to verify it passes**

Run: `pnpm test -- tenant-do.test.ts`
Expected: PASS

- [ ] **Step 5: Write the failing test for transitive membership via an ancestor**

```typescript
it("hasMembershipPath: true via membership in an ancestor node", async () => {
  const id = env.TENANT_DO.newUniqueId();
  const stub = env.TENANT_DO.get(id);
  await runInDurableObject(stub, async (instance) => {
    const parent = await instance.createNode({ parentId: null, kind: "business_unit", name: "Eng" });
    const child = await instance.createNode({ parentId: parent.id, kind: "business_unit", name: "Backend" });
    await instance.addMember("agent-1", parent.id, "member");

    const result = await instance.hasMembershipPath("agent-1", child.id);
    expect(result).toBe(true);
  });
});

it("hasMembershipPath: false for an agent with no path to the node", async () => {
  const id = env.TENANT_DO.newUniqueId();
  const stub = env.TENANT_DO.get(id);
  await runInDurableObject(stub, async (instance) => {
    const node = await instance.createNode({ parentId: null, kind: "business_unit", name: "Eng" });
    const result = await instance.hasMembershipPath("agent-nobody", node.id);
    expect(result).toBe(false);
  });
});
```

- [ ] **Step 6: Run test to verify it passes**

Run: `pnpm test -- tenant-do.test.ts`
Expected: PASS (4 passed)

- [ ] **Step 7: Commit**

```bash
git add workers/ledgrrr-tenant-registry/src/tenant-do.ts workers/ledgrrr-tenant-registry/test/tenant-do.test.ts
git commit -m "feat(ledgrrr-tenant-registry): transitive membership check via recursive CTE"
```

---

## Task 5: GrantsAccess check + orphaned-node-deletion refusal

**Files:**
- Modify: `workers/ledgrrr-tenant-registry/src/tenant-do.ts`
- Modify: `workers/ledgrrr-tenant-registry/test/tenant-do.test.ts`

**Interfaces:**
- Consumes: `hasMembershipPath` from Task 4.
- Produces: `nodeGrantsShards(nodeId: string, requestedShards: string[]): Promise<boolean>` and `deleteNode(nodeId: string): Promise<{ deleted: boolean; reason?: string }>` on `TenantNode` — Task 6 calls `nodeGrantsShards`.

- [ ] **Step 1: Write the failing test for `nodeGrantsShards`**

```typescript
it("nodeGrantsShards: true when all requested shards are declared in settings_json", async () => {
  const id = env.TENANT_DO.newUniqueId();
  const stub = env.TENANT_DO.get(id);
  await runInDurableObject(stub, async (instance) => {
    const node = await instance.createNode({
      parentId: null,
      kind: "business_unit",
      name: "Eng",
      settingsJson: JSON.stringify({ grantedShards: ["project", "datum"] }),
    });
    expect(await instance.nodeGrantsShards(node.id, ["project"])).toBe(true);
    expect(await instance.nodeGrantsShards(node.id, ["project", "datum"])).toBe(true);
    expect(await instance.nodeGrantsShards(node.id, ["agent"])).toBe(false);
  });
});
```

- [ ] **Step 2: Run test to verify it fails**

Run: `pnpm test -- tenant-do.test.ts`
Expected: FAIL — `nodeGrantsShards` is not a function.

- [ ] **Step 3: Add `nodeGrantsShards` and `deleteNode` to `src/tenant-do.ts`**

Add these methods inside the `TenantNode` class, after `hasMembershipPath`:

```typescript
  async nodeGrantsShards(nodeId: string, requestedShards: string[]): Promise<boolean> {
    const row = this.ctx.storage.sql
      .exec("SELECT settings_json FROM nodes WHERE id = ?", nodeId)
      .toArray()[0] as { settings_json: string } | undefined;
    if (!row) return false;
    const settings = JSON.parse(row.settings_json) as { grantedShards?: string[] };
    const granted = new Set(settings.grantedShards ?? []);
    return requestedShards.every((shard) => granted.has(shard));
  }

  async deleteNode(nodeId: string): Promise<{ deleted: boolean; reason?: string }> {
    const children = this.ctx.storage.sql
      .exec("SELECT id FROM nodes WHERE parent_id = ?", nodeId)
      .toArray();
    if (children.length > 0) {
      return { deleted: false, reason: "node has children; delete or reparent them first" };
    }
    this.ctx.storage.sql.exec("DELETE FROM members WHERE node_id = ?", nodeId);
    this.ctx.storage.sql.exec("DELETE FROM nodes WHERE id = ?", nodeId);
    return { deleted: true };
  }
```

- [ ] **Step 4: Run test to verify it passes**

Run: `pnpm test -- tenant-do.test.ts`
Expected: PASS

- [ ] **Step 5: Write the failing test for orphaned-node-deletion refusal**

```typescript
it("deleteNode refuses to delete a node that still has children", async () => {
  const id = env.TENANT_DO.newUniqueId();
  const stub = env.TENANT_DO.get(id);
  await runInDurableObject(stub, async (instance) => {
    const parent = await instance.createNode({ parentId: null, kind: "business_unit", name: "Eng" });
    await instance.createNode({ parentId: parent.id, kind: "business_unit", name: "Backend" });

    const result = await instance.deleteNode(parent.id);
    expect(result.deleted).toBe(false);
    expect(result.reason).toMatch(/children/);
  });
});

it("deleteNode succeeds for a childless node", async () => {
  const id = env.TENANT_DO.newUniqueId();
  const stub = env.TENANT_DO.get(id);
  await runInDurableObject(stub, async (instance) => {
    const node = await instance.createNode({ parentId: null, kind: "business_unit", name: "Solo" });
    const result = await instance.deleteNode(node.id);
    expect(result.deleted).toBe(true);
  });
});
```

- [ ] **Step 6: Run test to verify it passes**

Run: `pnpm test -- tenant-do.test.ts`
Expected: PASS (7 passed)

- [ ] **Step 7: Commit**

```bash
git add workers/ledgrrr-tenant-registry/src/tenant-do.ts workers/ledgrrr-tenant-registry/test/tenant-do.test.ts
git commit -m "feat(ledgrrr-tenant-registry): GrantsAccess check + orphaned-node deletion refusal"
```

---

## Task 6: Token issuance flow + cross-tenant isolation test + cake-rollup query shape

**Files:**
- Create: `workers/ledgrrr-tenant-registry/src/token.ts`
- Modify: `workers/ledgrrr-tenant-registry/src/tenant-do.ts`
- Modify: `workers/ledgrrr-tenant-registry/src/index.ts`
- Create: `workers/ledgrrr-tenant-registry/test/token.test.ts`
- Create: `workers/ledgrrr-tenant-registry/test/isolation.test.ts`

**Interfaces:**
- Consumes: `lookupTenant` (Task 2), `hasMembershipPath` + `nodeGrantsShards` (Tasks 4–5), `env.DB` + `env.TENANT_DO`.
- Produces: `issueToken(env: Env, input: { tenantId: string; agentId: string; nodeId: string; requestedShards: string[] }): Promise<{ token: string } | { error: string }>` — the terminal deliverable this whole plan builds toward.

- [ ] **Step 1: Write the failing test for token issuance succeeding on a valid path**

```typescript
import { describe, it, expect } from "vitest";
import { env } from "cloudflare:test";
import { createTenant } from "../src/registry";
import { issueToken } from "../src/token";

describe("issueToken", () => {
  it("issues a token when the agent has membership and the node grants the requested shards", async () => {
    const tenant = await createTenant(env.DB, env.TENANT_DO, { kind: "organizational", displayName: "Acme" });
    const stub = env.TENANT_DO.get(env.TENANT_DO.idFromString(tenant.rootDoId));
    const node = await stub.createNode({
      parentId: null,
      kind: "business_unit",
      name: "Eng",
      settingsJson: JSON.stringify({ grantedShards: ["project"] }),
    });
    await stub.addMember("agent-1", node.id, "member");

    const result = await issueToken(env, {
      tenantId: tenant.id,
      agentId: "agent-1",
      nodeId: node.id,
      requestedShards: ["project"],
    });

    expect("token" in result).toBe(true);
  });
});
```

- [ ] **Step 2: Run test to verify it fails**

Run: `pnpm test -- token.test.ts`
Expected: FAIL — `../src/token` module doesn't exist.

- [ ] **Step 3: Write `src/token.ts`**

```typescript
import { lookupTenant } from "./registry";

export interface Env {
  DB: D1Database;
  TENANT_DO: DurableObjectNamespace;
}

export interface IssueTokenInput {
  tenantId: string;
  agentId: string;
  nodeId: string;
  requestedShards: string[];
}

export type IssueTokenResult = { token: string } | { error: string };

export async function issueToken(env: Env, input: IssueTokenInput): Promise<IssueTokenResult> {
  const tenant = await lookupTenant(env.DB, input.tenantId);
  if (!tenant) {
    return { error: "tenant not found" };
  }

  const stub = env.TENANT_DO.get(env.TENANT_DO.idFromString(tenant.rootDoId));

  const hasMembership = await stub.hasMembershipPath(input.agentId, input.nodeId);
  if (!hasMembership) {
    return { error: "unauthorized" };
  }

  const grantsShards = await stub.nodeGrantsShards(input.nodeId, input.requestedShards);
  if (!grantsShards) {
    return { error: "unauthorized" };
  }

  const payload = {
    tenantId: input.tenantId,
    rootDoId: tenant.rootDoId,
    nodeId: input.nodeId,
    shards: input.requestedShards,
    issuedAt: new Date().toISOString(),
  };
  const token = btoa(JSON.stringify(payload));
  return { token };
}
```

Note: `stub.createNode`/`stub.addMember`/`stub.hasMembershipPath`/`stub.nodeGrantsShards` calls on a DO stub (as opposed to `runInDurableObject`'s direct instance access used in Tasks 3–5's tests) work because Durable Object classes expose their public methods as RPC-callable across the stub boundary automatically — this is standard Cloudflare DO behavior, not something this plan's code needs to wire up separately.

- [ ] **Step 4: Run test to verify it passes**

Run: `pnpm test -- token.test.ts`
Expected: PASS

- [ ] **Step 5: Write the failing tests for the two rejection paths**

```typescript
it("rejects when the agent has no membership path", async () => {
  const tenant = await createTenant(env.DB, env.TENANT_DO, { kind: "organizational", displayName: "Acme2" });
  const stub = env.TENANT_DO.get(env.TENANT_DO.idFromString(tenant.rootDoId));
  const node = await stub.createNode({
    parentId: null,
    kind: "business_unit",
    name: "Eng",
    settingsJson: JSON.stringify({ grantedShards: ["project"] }),
  });

  const result = await issueToken(env, {
    tenantId: tenant.id,
    agentId: "agent-nobody",
    nodeId: node.id,
    requestedShards: ["project"],
  });

  expect(result).toEqual({ error: "unauthorized" });
});

it("rejects when the node does not grant a requested shard, even with valid membership", async () => {
  const tenant = await createTenant(env.DB, env.TENANT_DO, { kind: "organizational", displayName: "Acme3" });
  const stub = env.TENANT_DO.get(env.TENANT_DO.idFromString(tenant.rootDoId));
  const node = await stub.createNode({
    parentId: null,
    kind: "business_unit",
    name: "Eng",
    settingsJson: JSON.stringify({ grantedShards: ["project"] }),
  });
  await stub.addMember("agent-1", node.id, "member");

  const result = await issueToken(env, {
    tenantId: tenant.id,
    agentId: "agent-1",
    nodeId: node.id,
    requestedShards: ["agent"],
  });

  expect(result).toEqual({ error: "unauthorized" });
});

it("rejects for an unknown tenant before ever opening a Durable Object", async () => {
  const result = await issueToken(env, {
    tenantId: "does-not-exist",
    agentId: "agent-1",
    nodeId: "irrelevant",
    requestedShards: ["project"],
  });
  expect(result).toEqual({ error: "tenant not found" });
});
```

- [ ] **Step 6: Run test to verify it passes**

Run: `pnpm test -- token.test.ts`
Expected: PASS (4 passed)

- [ ] **Step 7: Write the cross-tenant isolation structural test**

```typescript
// test/isolation.test.ts
import { describe, it, expect } from "vitest";
import { env } from "cloudflare:test";
import { createTenant } from "../src/registry";
import { issueToken } from "../src/token";

describe("cross-tenant isolation", () => {
  it("a token scoped to tenant A's node cannot be satisfied against tenant B's membership", async () => {
    const tenantA = await createTenant(env.DB, env.TENANT_DO, { kind: "organizational", displayName: "A" });
    const tenantB = await createTenant(env.DB, env.TENANT_DO, { kind: "organizational", displayName: "B" });

    const stubA = env.TENANT_DO.get(env.TENANT_DO.idFromString(tenantA.rootDoId));
    const nodeA = await stubA.createNode({
      parentId: null,
      kind: "business_unit",
      name: "A-Eng",
      settingsJson: JSON.stringify({ grantedShards: ["project"] }),
    });

    const stubB = env.TENANT_DO.get(env.TENANT_DO.idFromString(tenantB.rootDoId));
    // agent-1 is a member in tenant B, NOT tenant A
    const nodeB = await stubB.createNode({ parentId: null, kind: "business_unit", name: "B-Eng" });
    await stubB.addMember("agent-1", nodeB.id, "member");

    // Attempting to get a token for tenant A's node, using an agent who is
    // only a member in tenant B, must fail — issueToken always resolves
    // tenantId -> rootDoId from the registry itself, so there is no
    // parameter combination that lets a caller address tenant B's DO while
    // claiming tenant A's tenantId.
    const result = await issueToken(env, {
      tenantId: tenantA.id,
      agentId: "agent-1",
      nodeId: nodeA.id,
      requestedShards: ["project"],
    });
    expect(result).toEqual({ error: "unauthorized" });
  });
});
```

- [ ] **Step 8: Run test to verify it passes**

Run: `pnpm test -- isolation.test.ts`
Expected: PASS

- [ ] **Step 9: Write the cake-rollup query-shape test (interface only, placeholder data)**

Add this method to `src/tenant-do.ts`, inside the `TenantNode` class, after `deleteNode`. This is the interface sub-project 2 (cake ledger per-org) will build its real transaction schema against — the spec commits only to the query *shape* (sum over a node and all its descendants), using a placeholder `_placeholder_leaf_balances` table populated solely by this test.

```typescript
  async cakeRollup(nodeId: string): Promise<number> {
    this.ctx.storage.sql.exec(`
      CREATE TABLE IF NOT EXISTS _placeholder_leaf_balances (
        node_id TEXT PRIMARY KEY REFERENCES nodes(id),
        balance INTEGER NOT NULL
      )
    `);
    const rows = this.ctx.storage.sql
      .exec(
        `
        WITH RECURSIVE descendants(id) AS (
          SELECT ? AS id
          UNION ALL
          SELECT nodes.id FROM nodes JOIN descendants ON nodes.parent_id = descendants.id
        )
        SELECT COALESCE(SUM(balance), 0) AS total
        FROM _placeholder_leaf_balances
        WHERE node_id IN (SELECT id FROM descendants)
        `,
        nodeId
      )
      .toArray()[0] as { total: number };
    return rows.total;
  }
```

```typescript
// test/tenant-do.test.ts — add this test
it("cakeRollup sums placeholder leaf balances across a node and its descendants", async () => {
  const id = env.TENANT_DO.newUniqueId();
  const stub = env.TENANT_DO.get(id);
  await runInDurableObject(stub, async (instance) => {
    const parent = await instance.createNode({ parentId: null, kind: "business_unit", name: "Eng" });
    const child = await instance.createNode({ parentId: parent.id, kind: "business_unit", name: "Backend" });

    instance.ctx.storage.sql.exec(
      "INSERT INTO _placeholder_leaf_balances (node_id, balance) VALUES (?, ?), (?, ?)",
      parent.id, 10, child.id, 25
    );

    const total = await instance.cakeRollup(parent.id);
    expect(total).toBe(35);
  });
});
```

- [ ] **Step 10: Run test to verify it passes**

Run: `pnpm test -- tenant-do.test.ts`
Expected: PASS (8 passed)

- [ ] **Step 11: Wire the token-issuance route into `src/index.ts`**

Add this route inside the `fetch` handler, after the existing `/tenants/:id` GET route:

```typescript
    if (request.method === "POST" && url.pathname === "/tokens") {
      const body = await request.json<{
        tenantId?: string;
        agentId?: string;
        nodeId?: string;
        requestedShards?: string[];
      }>();
      if (!body.tenantId || !body.agentId || !body.nodeId || !Array.isArray(body.requestedShards)) {
        return new Response(
          JSON.stringify({ error: "tenantId, agentId, nodeId, and requestedShards[] are required" }),
          { status: 400, headers: { "Content-Type": "application/json" } }
        );
      }
      const result = await issueToken(env, {
        tenantId: body.tenantId,
        agentId: body.agentId,
        nodeId: body.nodeId,
        requestedShards: body.requestedShards,
      });
      if ("error" in result) {
        return new Response(JSON.stringify(result), {
          status: result.error === "tenant not found" ? 404 : 403,
          headers: { "Content-Type": "application/json" },
        });
      }
      return new Response(JSON.stringify(result), {
        status: 200,
        headers: { "Content-Type": "application/json" },
      });
    }
```

Add `import { issueToken } from "./token";` to the top of `src/index.ts`.

- [ ] **Step 12: Run the full test suite**

Run: `pnpm test`
Expected: all tests across `registry.test.ts`, `tenant-do.test.ts`, `token.test.ts`, `isolation.test.ts` pass.

- [ ] **Step 13: Commit**

```bash
git add workers/ledgrrr-tenant-registry/src/token.ts workers/ledgrrr-tenant-registry/src/tenant-do.ts workers/ledgrrr-tenant-registry/src/index.ts workers/ledgrrr-tenant-registry/test/token.test.ts workers/ledgrrr-tenant-registry/test/isolation.test.ts workers/ledgrrr-tenant-registry/test/tenant-do.test.ts
git commit -m "feat(ledgrrr-tenant-registry): token issuance flow, cross-tenant isolation test, cake-rollup query shape"
```

---

## Task 7: Apply the `tenants` table migration to the real production D1 (live action)

**Files:** none created/modified — this task runs the migration written in Task 1 against the real Cloudflare account.

**Interfaces:**
- Consumes: `migrations/0001_create_tenants.sql` (Task 1).
- Produces: the real, live `tenants` table in production `b00t-agents` D1.

This is the one task in this plan that touches live infrastructure. Per this session's established pattern for live D1 mutations (precedent: the `mcp_key` column add to the `agents` table), confirm with the operator before running, even though the migration is purely additive (`CREATE TABLE`, no existing table touched).

- [ ] **Step 1: Confirm with the operator that this live migration should proceed now**

Do not run Step 2 without explicit go-ahead — this writes to the real production database.

- [ ] **Step 2: Apply the migration to the real D1**

Run: `cd workers/ledgrrr-tenant-registry && pnpm wrangler d1 migrations apply b00t-agents --remote`
Expected: migration `0001_create_tenants.sql` applies with no errors against the real `b00t-agents` database.

- [ ] **Step 3: Verify the table exists in production**

Run: `pnpm wrangler d1 execute b00t-agents --remote --command "SELECT name FROM sqlite_master WHERE type='table' AND name='tenants'"`
Expected: one row returned, `name: tenants`.

- [ ] **Step 4: Deploy the Worker**

Run: `pnpm wrangler deploy`
Expected: `ledgrrr-tenant-registry` deploys successfully, printing its `*.workers.dev` URL.

- [ ] **Step 5: Smoke-test the deployed Worker**

Run: `curl -X POST https://ledgrrr-tenant-registry.<subdomain>.workers.dev/tenants -H "Content-Type: application/json" -d '{"kind":"personal","displayName":"smoke-test"}'`
Expected: `201` with a JSON body containing `id`, `kind: "personal"`, `displayName: "smoke-test"`, `rootDoId`, `createdAt`.

- [ ] **Step 6: Commit** (no file changes expected, but run in case `wrangler deploy` updated a lockfile)

```bash
git status --short
# if anything changed (e.g. pnpm-lock.yaml):
git add -A
git commit -m "chore(ledgrrr-tenant-registry): deploy to production"
```
