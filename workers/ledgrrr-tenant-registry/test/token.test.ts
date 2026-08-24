import { describe, it, expect, beforeAll } from "vitest";
import { env } from "cloudflare:test";
import { createTenant } from "../src/registry";
import { issueToken, verifyToken } from "../src/token";

describe("issueToken", () => {
  beforeAll(async () => {
    // Create the tenants table (migration equivalent) — each test file gets
    // isolated D1 storage under vitest-pool-workers, mirroring registry.test.ts.
    await env.DB.exec(
      "CREATE TABLE IF NOT EXISTS tenants (id TEXT PRIMARY KEY, kind TEXT NOT NULL CHECK (kind IN ('personal', 'organizational')), display_name TEXT NOT NULL, root_do_id TEXT NOT NULL, created_at TEXT NOT NULL)"
    );
  });

  it("issues a token when the agent has membership and the node grants the requested shards", async () => {
    const tenant = await createTenant(env.DB, env.TENANT_DO, {
      kind: "organizational",
      displayName: "Acme",
      ownerAgentId: "agent-owner",
    });
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
    if ("token" in result) {
      const verified = await verifyToken(env, result.token);
      expect(verified.valid).toBe(true);
      if (verified.valid) {
        expect(verified.payload.agentId).toBe("agent-1");
      }
    }
  });

  it("rejects when the agent has no membership path", async () => {
    const tenant = await createTenant(env.DB, env.TENANT_DO, {
      kind: "organizational",
      displayName: "Acme2",
      ownerAgentId: "agent-owner",
    });
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
    const tenant = await createTenant(env.DB, env.TENANT_DO, {
      kind: "organizational",
      displayName: "Acme3",
      ownerAgentId: "agent-owner",
    });
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
});
