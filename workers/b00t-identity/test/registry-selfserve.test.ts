import { describe, it, expect, beforeAll } from "vitest";
import { env } from "cloudflare:test";
import worker from "../src/index";
import { normalizeSlug } from "../src/registry";

const CTX = {} as ExecutionContext;

describe("self-serve POST /tenants", () => {
  beforeAll(async () => {
    await env.DB.exec(
      "CREATE TABLE IF NOT EXISTS tenants (id TEXT PRIMARY KEY, kind TEXT NOT NULL CHECK (kind IN ('personal', 'organizational')), display_name TEXT NOT NULL, slug TEXT, root_do_id TEXT NOT NULL, created_at TEXT NOT NULL)",
    );
  });

  it("normalizeSlug lowercases and dashes", () => {
    expect(normalizeSlug("  Prompt Execution! ")).toBe("prompt-execution");
  });

  it("creates a tenant with no admin key, then rejects a duplicate slug with 409", async () => {
    const body = JSON.stringify({
      kind: "organizational",
      displayName: "Acme Co",
      slug: "acme-co",
      ownerAgentId: "agent/owner",
    });
    const mk = () =>
      new Request("https://x/tenants", {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body,
      });

    const first = await worker.fetch(mk(), env, CTX);
    expect(first.status).toBe(201);
    const created = await first.json<{ slug: string; id: string }>();
    expect(created.slug).toBe("acme-co");

    const dup = await worker.fetch(mk(), env, CTX);
    expect(dup.status).toBe(409);

    // GET by slug resolves
    const got = await worker.fetch(
      new Request("https://x/tenants/acme-co", {
        headers: { Authorization: "Bearer " + (env.REGISTRY_ADMIN_KEY ?? "") },
      }),
      env,
      CTX,
    );
    expect(got.status).toBe(200);
  });

  it("still 400s without a slug", async () => {
    const res = await worker.fetch(
      new Request("https://x/tenants", {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({ kind: "personal", displayName: "No Slug", ownerAgentId: "a" }),
      }),
      env,
      CTX,
    );
    expect(res.status).toBe(400);
  });
});
