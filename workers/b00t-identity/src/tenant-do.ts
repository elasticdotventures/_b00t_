import { DurableObject } from "cloudflare:workers";

export interface AgentGrant {
  agentId: string;
  nodeId: string;
  r0le: string;
  shards: string[];
}

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
    this.ctx.storage.sql.exec(`
      CREATE TABLE IF NOT EXISTS agent_grants (
        agent_id TEXT NOT NULL,
        node_id TEXT NOT NULL REFERENCES nodes(id),
        r0le TEXT NOT NULL,
        shards_json TEXT NOT NULL DEFAULT '[]',
        PRIMARY KEY (agent_id, node_id)
      )
    `);
    this.ctx.storage.sql.exec(`CREATE INDEX IF NOT EXISTS idx_nodes_parent ON nodes(parent_id)`);
    this.ctx.storage.sql.exec(`CREATE INDEX IF NOT EXISTS idx_members_agent ON members(agent_id)`);
    this.ctx.storage.sql.exec(`
      CREATE TABLE IF NOT EXISTS _placeholder_leaf_balances (
        node_id TEXT PRIMARY KEY REFERENCES nodes(id),
        balance INTEGER NOT NULL
      )
    `);
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
          UNION
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

  async nodeGrantsShards(nodeId: string, requestedShards: string[]): Promise<boolean> {
    const row = this.ctx.storage.sql
      .exec("SELECT settings_json FROM nodes WHERE id = ?", nodeId)
      .toArray()[0] as { settings_json: string } | undefined;
    if (!row) return false;
    const settings = JSON.parse(row.settings_json) as { grantedShards?: string[] };
    const granted = new Set(settings.grantedShards ?? []);
    return requestedShards.every((shard) => granted.has(shard));
  }

  async setAgentGrant(agentId: string, nodeId: string, r0le: string, shards: string[]): Promise<void> {
    this.ctx.storage.sql.exec(
      `INSERT INTO agent_grants (agent_id, node_id, r0le, shards_json)
       VALUES (?, ?, ?, ?)
       ON CONFLICT(agent_id, node_id)
       DO UPDATE SET r0le = excluded.r0le, shards_json = excluded.shards_json`,
      agentId,
      nodeId,
      r0le,
      JSON.stringify(shards)
    );
  }

  async getAgentGrant(agentId: string, nodeId: string): Promise<AgentGrant | null> {
    const row = this.ctx.storage.sql
      .exec("SELECT r0le, shards_json FROM agent_grants WHERE agent_id = ? AND node_id = ?", agentId, nodeId)
      .toArray()[0] as { r0le: string; shards_json: string } | undefined;
    if (!row) return null;
    return { agentId, nodeId, r0le: row.r0le, shards: JSON.parse(row.shards_json) as string[] };
  }

  async revokeAgent(agentId: string, nodeId: string): Promise<{ revoked: boolean }> {
    const existed = this.ctx.storage.sql
      .exec("SELECT 1 FROM agent_grants WHERE agent_id = ? AND node_id = ?", agentId, nodeId)
      .toArray().length > 0;
    this.ctx.storage.sql.exec("DELETE FROM agent_grants WHERE agent_id = ? AND node_id = ?", agentId, nodeId);
    return { revoked: existed };
  }

  /** Per-agent grant wins; otherwise fall back to the node's own granted shards. */
  async agentGrantsShards(agentId: string, nodeId: string, requestedShards: string[]): Promise<boolean> {
    const grant = await this.getAgentGrant(agentId, nodeId);
    if (grant) {
      const granted = new Set(grant.shards);
      return requestedShards.every((s) => granted.has(s));
    }
    return this.nodeGrantsShards(nodeId, requestedShards);
  }

  /** Resolve authorization in one DO turn; the closest membership supplies the legacy role. */
  async agentAuthorization(agentId: string, nodeId: string): Promise<{
    r0le: string; shards: string[]; source: string;
  } | null> {
    const member = this.ctx.storage.sql.exec(`
      WITH RECURSIVE ancestors(id, parent_id, depth) AS (
        SELECT id, parent_id, 0 FROM nodes WHERE id = ?
        UNION ALL
        SELECT nodes.id, nodes.parent_id, ancestors.depth + 1
        FROM nodes JOIN ancestors ON nodes.id = ancestors.parent_id
      )
      SELECT members.role, members.node_id FROM ancestors
      JOIN members ON members.node_id = ancestors.id
      WHERE members.agent_id = ? ORDER BY ancestors.depth LIMIT 1
    `, nodeId, agentId).toArray()[0] as { role: string; node_id: string } | undefined;
    if (!member) return null;
    const grant = this.ctx.storage.sql.exec(
      "SELECT r0le, shards_json FROM agent_grants WHERE agent_id = ? AND node_id = ?",
      agentId, nodeId,
    ).toArray()[0] as { r0le: string; shards_json: string } | undefined;
    if (grant) return { r0le: grant.r0le, shards: JSON.parse(grant.shards_json), source: `grant:${nodeId}` };
    const node = this.ctx.storage.sql.exec("SELECT settings_json FROM nodes WHERE id = ?", nodeId)
      .toArray()[0] as { settings_json: string };
    const settings = JSON.parse(node.settings_json) as { grantedShards?: string[] };
    return { r0le: member.role, shards: settings.grantedShards ?? [], source: `member:${member.node_id}` };
  }

  /** Revalidate the exact node, role, scopes and grant source used at issuance. */
  async checkStillGranted(agentId: string, nodeId: string, r0le: string, scopes: string[], source: string): Promise<boolean> {
    if (!nodeId || !source || !Array.isArray(scopes)) return false;
    const current = await this.agentAuthorization(agentId, nodeId);
    return current !== null && current.source === source && current.r0le === r0le
      && scopes.every((scope) => current.shards.includes(scope));
  }

  /**
   * Admin revoke. With `nodeId` — drop the agent's grant + membership on that
   * node only. Without — drop every grant + membership the agent has in this
   * tenant. Returns whether anything was removed.
   */
  async revokeAgentEverywhere(agentId: string, nodeId?: string): Promise<{ revoked: boolean }> {
    let removed = 0;
    if (nodeId) {
      removed += this.ctx.storage.sql
        .exec("SELECT 1 FROM agent_grants WHERE agent_id = ? AND node_id = ?", agentId, nodeId)
        .toArray().length;
      removed += this.ctx.storage.sql
        .exec("SELECT 1 FROM members WHERE agent_id = ? AND node_id = ?", agentId, nodeId)
        .toArray().length;
      this.ctx.storage.sql.exec(
        "DELETE FROM agent_grants WHERE agent_id = ? AND node_id = ?",
        agentId,
        nodeId,
      );
      this.ctx.storage.sql.exec(
        "DELETE FROM members WHERE agent_id = ? AND node_id = ?",
        agentId,
        nodeId,
      );
    } else {
      removed += this.ctx.storage.sql
        .exec("SELECT 1 FROM agent_grants WHERE agent_id = ?", agentId)
        .toArray().length;
      removed += this.ctx.storage.sql
        .exec("SELECT 1 FROM members WHERE agent_id = ?", agentId)
        .toArray().length;
      this.ctx.storage.sql.exec("DELETE FROM agent_grants WHERE agent_id = ?", agentId);
      this.ctx.storage.sql.exec("DELETE FROM members WHERE agent_id = ?", agentId);
    }
    return { revoked: removed > 0 };
  }

  async deleteNode(nodeId: string): Promise<{ deleted: boolean; reason?: string }> {
    const children = this.ctx.storage.sql
      .exec("SELECT id FROM nodes WHERE parent_id = ?", nodeId)
      .toArray();
    if (children.length > 0) {
      return { deleted: false, reason: "node has children; delete or reparent them first" };
    }
    this.ctx.storage.transactionSync(() => {
      this.ctx.storage.sql.exec("DELETE FROM agent_grants WHERE node_id = ?", nodeId);
      this.ctx.storage.sql.exec("DELETE FROM members WHERE node_id = ?", nodeId);
      this.ctx.storage.sql.exec("DELETE FROM _placeholder_leaf_balances WHERE node_id = ?", nodeId);
      this.ctx.storage.sql.exec("DELETE FROM nodes WHERE id = ?", nodeId);
    });
    return { deleted: true };
  }

  async cakeRollup(nodeId: string): Promise<number> {
    const rows = this.ctx.storage.sql
      .exec(
        `
        WITH RECURSIVE descendants(id) AS (
          SELECT ? AS id
          UNION
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
}
