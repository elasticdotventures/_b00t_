import { createTenant, lookupTenant } from "./registry";
import { TenantNode } from "./tenant-do";
import { issueToken } from "./token";
export { TenantNode } from "./tenant-do";

export interface Env {
  DB: D1Database;
  TENANT_DO: DurableObjectNamespace<TenantNode>;
  REGISTRY_ADMIN_KEY: string;
}

function isAuthorized(request: Request, env: Env): boolean {
  const auth = request.headers.get("Authorization");
  if (!auth || !auth.startsWith("Bearer ")) return false;
  const token = auth.slice("Bearer ".length);
  return token === env.REGISTRY_ADMIN_KEY;
}

function unauthorized(): Response {
  return new Response(JSON.stringify({ error: "unauthorized" }), {
    status: 401,
    headers: { "Content-Type": "application/json" },
  });
}

export default {
  async fetch(request: Request, env: Env): Promise<Response> {
    if (!isAuthorized(request, env)) return unauthorized();

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

    if (request.method === "POST" && url.pathname === "/tokens") {
      const body = await request.json<{
        tenantId?: string;
        agentId?: string;
        nodeId?: string;
        requestedShards?: string[];
      }>();
      if (!body.tenantId || !body.agentId || !body.nodeId || !Array.isArray(body.requestedShards)) {
        return new Response(
          JSON.stringify({ error: "tenantId, agentId, nodeId, and requestedShards[] are required" }),
          { status: 400, headers: { "Content-Type": "application/json" } }
        );
      }
      const result = await issueToken(env, {
        tenantId: body.tenantId,
        agentId: body.agentId,
        nodeId: body.nodeId,
        requestedShards: body.requestedShards,
      });
      if ("error" in result) {
        return new Response(JSON.stringify(result), {
          status: result.error === "tenant not found" ? 404 : 403,
          headers: { "Content-Type": "application/json" },
        });
      }
      return new Response(JSON.stringify(result), {
        status: 200,
        headers: { "Content-Type": "application/json" },
      });
    }

    return new Response("Not found", { status: 404 });
  },
};
