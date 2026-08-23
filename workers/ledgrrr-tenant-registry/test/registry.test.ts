import { describe, it, expect, beforeAll } from "vitest";
import { env } from "cloudflare:test";
import { lookupTenant } from "../src/registry";

describe("lookupTenant", () => {
  beforeAll(async () => {
    // Create the tenants table (migration equivalent) — each test file gets
    // isolated D1 storage under vitest-pool-workers, mirroring bootstrap.test.ts.
    await env.DB.exec(
      "CREATE TABLE IF NOT EXISTS tenants (id TEXT PRIMARY KEY, kind TEXT NOT NULL CHECK (kind IN ('personal', 'organizational')), display_name TEXT NOT NULL, root_do_id TEXT NOT NULL, created_at TEXT NOT NULL)"
    );
  });

  it("returns null for an unknown tenant id", async () => {
    const result = await lookupTenant(env.DB, "nonexistent-id");
    expect(result).toBeNull();
  });

  it("creates a tenant and looks it up by id", async () => {
    const { createTenant, lookupTenant } = await import("../src/registry");
    const created = await createTenant(env.DB, env.TENANT_DO, {
      kind: "organizational",
      displayName: "Acme Corp",
    });
    expect(created.kind).toBe("organizational");
    expect(created.displayName).toBe("Acme Corp");
    expect(created.rootDoId).toBeTruthy();

    const found = await lookupTenant(env.DB, created.id);
    expect(found).toEqual(created);
  });
});
