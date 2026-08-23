import { describe, it, expect, beforeAll } from "vitest";
import { env } from "cloudflare:test";

describe("Worker pool bootstrap", () => {
  beforeAll(async () => {
    // Create the tenants table (migration equivalent)
    await env.DB.exec("CREATE TABLE IF NOT EXISTS tenants (id TEXT PRIMARY KEY, kind TEXT NOT NULL CHECK (kind IN ('personal', 'organizational')), display_name TEXT NOT NULL, root_do_id TEXT NOT NULL, created_at TEXT NOT NULL)");
  });

  it("D1 binding resolves and the tenants table can be created and queried", async () => {
    const result = await env.DB.prepare(
      "SELECT name FROM sqlite_master WHERE type = 'table' AND name = 'tenants'"
    ).first();
    expect(result).toEqual({ name: "tenants" });
  });

  it("D1 binding can insert and retrieve from tenants table", async () => {
    const now = new Date().toISOString();
    await env.DB.prepare(
      "INSERT INTO tenants (id, kind, display_name, root_do_id, created_at) VALUES (?, ?, ?, ?, ?)"
    ).bind("test-tenant-1", "personal", "Test Tenant", "root-do-123", now).run();

    const result = await env.DB.prepare(
      "SELECT id, kind FROM tenants WHERE id = ?"
    ).bind("test-tenant-1").first();

    expect(result).toEqual({ id: "test-tenant-1", kind: "personal" });
  });

  it("Durable Object binding resolves to a usable stub", async () => {
    const id = env.TENANT_DO.newUniqueId();
    const stub = env.TENANT_DO.get(id);
    expect(stub).toBeDefined();
    expect(typeof stub.fetch).toBe("function");
  });
});
