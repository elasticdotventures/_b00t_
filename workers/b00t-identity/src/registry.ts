import type { TenantNode } from "./tenant-do";

export interface Tenant {
  id: string;
  kind: "personal" | "organizational";
  displayName: string;
  slug: string | null;
  rootDoId: string;
  createdAt: string;
}

export interface CreatedTenant extends Tenant {
  rootNodeId: string;
}

interface TenantRow {
  id: string;
  kind: string;
  display_name: string;
  slug: string | null;
  root_do_id: string;
  created_at: string;
}

function rowToTenant(row: TenantRow): Tenant {
  return {
    id: row.id,
    kind: row.kind as "personal" | "organizational",
    displayName: row.display_name,
    slug: row.slug ?? null,
    rootDoId: row.root_do_id,
    createdAt: row.created_at,
  };
}

/** Lowercase, dash-separated, alnum only. */
export function normalizeSlug(raw: string): string {
  return raw
    .toLowerCase()
    .trim()
    .replace(/[^a-z0-9]+/g, "-")
    .replace(/^-+|-+$/g, "");
}

export async function lookupTenant(db: D1Database, idOrSlug: string): Promise<Tenant | null> {
  const row = await db
    .prepare(
      "SELECT id, kind, display_name, slug, root_do_id, created_at FROM tenants WHERE id = ? OR slug = ?",
    )
    .bind(idOrSlug, idOrSlug)
    .first<TenantRow>();
  return row ? rowToTenant(row) : null;
}

export class SlugTakenError extends Error {
  constructor(public slug: string) {
    super(`slug '${slug}' is already taken`);
    this.name = "SlugTakenError";
  }
}

export async function createTenant(
  db: D1Database,
  doNamespace: DurableObjectNamespace<TenantNode>,
  input: {
    kind: "personal" | "organizational";
    displayName: string;
    ownerAgentId: string;
    slug?: string;
  },
): Promise<CreatedTenant> {
  const id = crypto.randomUUID();
  const doId = doNamespace.newUniqueId();
  const rootDoId = doId.toString();
  const createdAt = new Date().toISOString();
  const slug = input.slug ? normalizeSlug(input.slug) : null;

  if (slug) {
    const clash = await db.prepare("SELECT 1 FROM tenants WHERE slug = ?").bind(slug).first();
    if (clash) throw new SlugTakenError(slug);
  }

  await db
    .prepare(
      "INSERT INTO tenants (id, kind, display_name, slug, root_do_id, created_at) VALUES (?, ?, ?, ?, ?, ?)",
    )
    .bind(id, input.kind, input.displayName, slug, rootDoId, createdAt)
    .run();

  const stub = doNamespace.get(doNamespace.idFromString(rootDoId));
  const rootNode = await stub.createNode({ parentId: null, kind: "business_unit", name: "root" });
  await stub.addMember(input.ownerAgentId, rootNode.id, "owner");

  return {
    id,
    kind: input.kind,
    displayName: input.displayName,
    slug,
    rootDoId,
    createdAt,
    rootNodeId: rootNode.id,
  };
}
