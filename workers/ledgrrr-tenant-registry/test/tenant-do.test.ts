import { describe, it, expect } from "vitest";
import { env, runInDurableObject } from "cloudflare:test";

describe("TenantNode schema", () => {
  it("initializes nodes and members tables on first access", async () => {
    const id = env.TENANT_DO.newUniqueId();
    const stub = env.TENANT_DO.get(id);
    await runInDurableObject(stub, async (_instance, state) => {
      const tables = state.storage.sql
        .exec("SELECT name FROM sqlite_master WHERE type = 'table' ORDER BY name")
        .toArray()
        .map((row: any) => row.name);
      expect(tables).toContain("nodes");
      expect(tables).toContain("members");
    });
  });

  it("creates a node and adds a member to it", async () => {
    const id = env.TENANT_DO.newUniqueId();
    const stub = env.TENANT_DO.get(id);
    await runInDurableObject(stub, async (instance, state) => {
      const node = await instance.createNode({ parentId: null, kind: "business_unit", name: "Engineering" });
      expect(node.kind).toBe("business_unit");
      expect(node.name).toBe("Engineering");

      await instance.addMember("agent-1", node.id, "member");

      const members = state.storage.sql
        .exec("SELECT agent_id, node_id, role FROM members WHERE node_id = ?", node.id)
        .toArray();
      expect(members).toHaveLength(1);
      expect(members[0]).toMatchObject({ agent_id: "agent-1", node_id: node.id, role: "member" });
    });
  });

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

  it("hasMembershipPath: false for membership on a different sibling branch", async () => {
    const id = env.TENANT_DO.newUniqueId();
    const stub = env.TENANT_DO.get(id);
    await runInDurableObject(stub, async (instance) => {
      const parent = await instance.createNode({ parentId: null, kind: "business_unit", name: "Eng" });
      const branchA = await instance.createNode({ parentId: parent.id, kind: "business_unit", name: "Backend" });
      const branchB = await instance.createNode({ parentId: parent.id, kind: "business_unit", name: "Frontend" });
      await instance.addMember("agent-1", branchA.id, "member");

      const result = await instance.hasMembershipPath("agent-1", branchB.id);
      expect(result).toBe(false);
    });
  });
});
