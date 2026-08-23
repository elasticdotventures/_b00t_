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
});
