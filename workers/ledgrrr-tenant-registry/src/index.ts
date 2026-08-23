import { createTenant, lookupTenant } from "./registry";
export { TenantNode } from "./tenant-do";

export interface Env {
  DB: D1Database;
  TENANT_DO: DurableObjectNamespace;
}

export default {
  async fetch(request: Request, env: Env): Promise<Response> {
    const url = new URL(request.url);

    if (request.method === "POST" && url.pathname === "/tenants") {
      const body = await request.json<{ kind?: string; displayName?: string }>();
      if (body.kind !== "personal" && body.kind !== "organizational") {
        return new Response(JSON.stringify({ error: "kind must be 'personal' or 'organizational'" }), {
          status: 400,
          headers: { "Content-Type": "application/json" },
        });
      }
      if (!body.displayName) {
        return new Response(JSON.stringify({ error: "displayName is required" }), {
          status: 400,
          headers: { "Content-Type": "application/json" },
        });
      }
      const tenant = await createTenant(env.DB, env.TENANT_DO, {
        kind: body.kind,
        displayName: body.displayName,
      });
      return new Response(JSON.stringify(tenant), {
        status: 201,
        headers: { "Content-Type": "application/json" },
      });
    }

    const tenantMatch = url.pathname.match(/^\/tenants\/([^/]+)$/);
    if (request.method === "GET" && tenantMatch) {
      const tenant = await lookupTenant(env.DB, decodeURIComponent(tenantMatch[1]));
      if (!tenant) {
        return new Response(JSON.stringify({ error: "tenant not found" }), {
          status: 404,
          headers: { "Content-Type": "application/json" },
        });
      }
      return new Response(JSON.stringify(tenant), {
        status: 200,
        headers: { "Content-Type": "application/json" },
      });
    }

    return new Response("Not found", { status: 404 });
  },
};
