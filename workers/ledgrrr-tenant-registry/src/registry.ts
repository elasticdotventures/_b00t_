import type { TenantNode } from "./tenant-do";

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
  doNamespace: DurableObjectNamespace<TenantNode>,
  input: { kind: "personal" | "organizational"; displayName: string; ownerAgentId: string }
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

  const stub = doNamespace.get(doNamespace.idFromString(rootDoId));
  const rootNode = await stub.createNode({ parentId: null, kind: "business_unit", name: "root" });
  await stub.addMember(input.ownerAgentId, rootNode.id, "owner");

  return { id, kind: input.kind, displayName: input.displayName, rootDoId, createdAt };
}
