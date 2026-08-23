import { DurableObject } from "cloudflare:workers";

export interface TenantNodeRow {
  id: string;
  parentId: string | null;
  kind: "business_unit" | "directory" | "tag";
  name: string;
  settingsJson: string;
}

export class TenantNode extends DurableObject {
  constructor(ctx: DurableObjectState, env: Cloudflare.Env) {
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
}
