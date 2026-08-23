import { describe, it, expect, beforeAll } from "vitest";
import { env } from "cloudflare:test";
import { createTenant } from "../src/registry";
import { issueToken } from "../src/token";

describe("cross-tenant isolation", () => {
  beforeAll(async () => {
    // Create the tenants table (migration equivalent) — each test file gets
    // isolated D1 storage under vitest-pool-workers, mirroring registry.test.ts.
    await env.DB.exec(
      "CREATE TABLE IF NOT EXISTS tenants (id TEXT PRIMARY KEY, kind TEXT NOT NULL CHECK (kind IN ('personal', 'organizational')), display_name TEXT NOT NULL, root_do_id TEXT NOT NULL, created_at TEXT NOT NULL)"
    );
  });

  it("a token scoped to tenant A's node cannot be satisfied against tenant B's membership", async () => {
    const tenantA = await createTenant(env.DB, env.TENANT_DO, {
      kind: "organizational",
      displayName: "A",
      ownerAgentId: "agent-owner-a",
    });
    const tenantB = await createTenant(env.DB, env.TENANT_DO, {
      kind: "organizational",
      displayName: "B",
      ownerAgentId: "agent-owner-b",
    });

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
