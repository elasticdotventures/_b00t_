import { describe, it, expect, beforeAll } from "vitest";
import { env } from "cloudflare:test";
import worker from "../src/index";
import { createTenant } from "../src/registry";
import { issueToken } from "../src/token";

const CTX = {} as ExecutionContext;

describe("SP1-07 revocation", () => {
  beforeAll(async () => {
    await env.DB.exec(
      "CREATE TABLE IF NOT EXISTS tenants (id TEXT PRIMARY KEY, kind TEXT NOT NULL CHECK (kind IN ('personal', 'organizational')), display_name TEXT NOT NULL, slug TEXT, root_do_id TEXT NOT NULL, created_at TEXT NOT NULL)",
    );
  });

  async function mintFor(agentId: string) {
    const tenant = await createTenant(env.DB, env.TENANT_DO, {
      kind: "organizational",
      displayName: "AcmeRevoke-" + agentId,
      ownerAgentId: "agent-owner",
    });
    const stub = env.TENANT_DO.get(env.TENANT_DO.idFromString(tenant.rootDoId));
    const node = await stub.createNode({
      parentId: null,
      kind: "business_unit",
      name: "Eng",
      settingsJson: JSON.stringify({ grantedShards: ["project"] }),
    });
    await stub.addMember(agentId, node.id, "member");
    const res = await issueToken(env, {
      tenantId: tenant.id,
      agentId,
      nodeId: node.id,
      requestedShards: ["project"],
    });
    if (!("token" in res)) throw new Error("mint failed");
    return { token: res.token, tenantId: tenant.id, agentId };
  }

  it("verifies a live token, then 401 'revoked' after the agent is deleted", async () => {
    const { token, tenantId, agentId } = await mintFor("agent-live");

    const ok = await worker.fetch(
      new Request("https://x/verify", {
        method: "POST",
        headers: { "Content-Type": "application/json", Authorization: "Bearer " + (env.REGISTRY_ADMIN_KEY ?? "") },
        body: JSON.stringify({ token }),
      }),
      env,
      CTX,
    );
    expect(ok.status).toBe(200);

    const del = await worker.fetch(
      new Request(`https://x/tenants/${tenantId}/agents/${agentId}`, {
        method: "DELETE",
        headers: { Authorization: "Bearer " + (env.REGISTRY_ADMIN_KEY ?? "") },
      }),
      env,
      CTX,
    );
    expect(del.status).toBe(200);
    expect(await del.json()).toEqual({ revoked: true });

    const after = await worker.fetch(
      new Request("https://x/verify", {
        method: "POST",
        headers: { "Content-Type": "application/json", Authorization: "Bearer " + (env.REGISTRY_ADMIN_KEY ?? "") },
        body: JSON.stringify({ token }),
      }),
      env,
      CTX,
    );
    expect(after.status).toBe(401);
    expect(await after.json()).toEqual({ error: "revoked" });
  });

  it("DELETE on an unknown tenant is 404", async () => {
    const res = await worker.fetch(
      new Request("https://x/tenants/does-not-exist/agents/whoever", {
        method: "DELETE",
        headers: { Authorization: "Bearer " + (env.REGISTRY_ADMIN_KEY ?? "") },
      }),
      env,
      CTX,
    );
    expect(res.status).toBe(404);
  });
});
