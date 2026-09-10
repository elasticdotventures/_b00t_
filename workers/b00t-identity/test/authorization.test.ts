import { beforeAll, expect, it, vi } from "vitest";
import { env, runInDurableObject } from "cloudflare:test";
import fixture from "./fixtures/authorization.json";
import { createTenant } from "../src/registry";
import { issueToken, verifyToken } from "../src/token";
import worker from "../src/index";

beforeAll(async () => {
  await env.DB.exec("CREATE TABLE IF NOT EXISTS tenants (id TEXT PRIMARY KEY, kind TEXT NOT NULL, display_name TEXT NOT NULL, slug TEXT, root_do_id TEXT NOT NULL, created_at TEXT NOT NULL)");
});

async function setup() {
  const tenant = await createTenant(env.DB, env.TENANT_DO, {
    ...fixture.tenant, kind: "organizational",
  });
  const stub = env.TENANT_DO.get(env.TENANT_DO.idFromString(tenant.rootDoId));
  const node = await stub.createNode({ ...fixture.node, kind: "business_unit" });
  await stub.addMember(fixture.agent, node.id, "member");
  await stub.setAgentGrant(fixture.agent, node.id, fixture.role, fixture.scopes);
  const input = { tenantId: tenant.id, agentId: fixture.agent, nodeId: node.id, requestedShards: fixture.scopes };
  return { tenant, stub, node, input };
}

async function verify(token: string) {
  return worker.fetch(new Request("https://x/identity/verify", {
    method: "POST",
    headers: { Authorization: `Bearer ${env.REGISTRY_ADMIN_KEY}`, "Content-Type": "application/json" },
    body: JSON.stringify({ token }),
  }), env);
}

it("rejects a requested role outside the stored grant before spending", async () => {
  const { input } = await setup();
  const authorizeSpend = vi.fn(async () => ({ ok: true, budget_remaining: 1 }));
  expect(await issueToken(env, { ...input, r0le: fixture.unauthorizedRole }, { authorizeSpend }))
    .toEqual({ error: "unauthorized" });
  expect(authorizeSpend).not.toHaveBeenCalled();
});

it("derives the role from the stored grant when omitted", async () => {
  const { input } = await setup();
  const result = await issueToken(env, input);
  if (!("token" in result)) throw new Error(JSON.stringify(result));
  const verified = await verifyToken(env, result.token);
  expect(verified.valid && verified.claims.r0le).toBe(fixture.role);
});

it("revokes a node-issued token even while sibling membership remains", async () => {
  const { input, stub, tenant, node } = await setup();
  const sibling = await stub.createNode({ ...fixture.node, kind: "business_unit" });
  await stub.addMember(fixture.agent, sibling.id, "member");
  const result = await issueToken(env, { ...input, r0le: fixture.role });
  if (!("token" in result)) throw new Error(JSON.stringify(result));
  expect((await verify(result.token)).status).toBe(200);
  const revoked = await worker.fetch(new Request(
    `https://x/identity/tenants/${tenant.id}/agents/${fixture.agent}?nodeId=${node.id}`,
    { method: "DELETE", headers: { Authorization: `Bearer ${env.REGISTRY_ADMIN_KEY}` } },
  ), env);
  expect(revoked.status).toBe(200);
  expect((await verify(result.token)).status).toBe(401);
});

it.each(["role", "scopes", "grant"])("rejects tokens after their %s changes", async (change) => {
  const { input, stub, node } = await setup();
  const result = await issueToken(env, { ...input, r0le: fixture.role });
  if (!("token" in result)) throw new Error(JSON.stringify(result));
  if (change === "grant") await stub.revokeAgent(fixture.agent, node.id);
  else await stub.setAgentGrant(fixture.agent, node.id,
    change === "role" ? "reviewer" : fixture.role,
    change === "scopes" ? [] : fixture.scopes);
  expect((await verify(result.token)).status).toBe(401);
});

it("deletes leaf grants, members and balances atomically with the node", async () => {
  const { stub, node } = await setup();
  await runInDurableObject(stub, async (_instance, state) => {
    state.storage.sql.exec("INSERT INTO _placeholder_leaf_balances (node_id, balance) VALUES (?, ?)", node.id, 1);
  });
  expect(await stub.deleteNode(node.id)).toEqual({ deleted: true });
  expect(await stub.getAgentGrant(fixture.agent, node.id)).toBeNull();
  expect(await stub.hasMembershipPath(fixture.agent, node.id)).toBe(false);
});
