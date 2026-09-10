import { createTenant, lookupTenant, SlugTakenError } from "./registry";
import { TenantNode } from "./tenant-do";
import { issueToken, verifyToken } from "./token";
import { jwks, jwksResponse } from "./jwks";
export { TenantNode } from "./tenant-do";

export interface Env {
  DB: D1Database;
  TENANT_DO: DurableObjectNamespace<TenantNode>;
  REGISTRY_ADMIN_KEY: string;
  TOKEN_SIGNING_KEY: string;
  JWT_PRIVATE_KEY_PEM: string;
  JWT_KID: string;
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
    const url = new URL(request.url);

    // Public — no auth. The JWT trust root.
    if (request.method === "GET" && url.pathname === "/.well-known/jwks.json") {
      return jwksResponse(await jwks(env));
    }

    const selfServe =
      request.method === "POST" && url.pathname === "/tenants";
    if (!selfServe && !isAuthorized(request, env)) return unauthorized();

    if (request.method === "POST" && url.pathname === "/tenants") {
      const body = await request.json<{
        kind?: string;
        displayName?: string;
        slug?: string;
        ownerAgentId?: string;
      }>();
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
      if (typeof body.ownerAgentId !== "string" || !body.ownerAgentId) {
        return new Response(JSON.stringify({ error: "ownerAgentId is required" }), {
          status: 400,
          headers: { "Content-Type": "application/json" },
        });
      }
      if (typeof body.slug !== "string" || !body.slug.trim()) {
        return new Response(JSON.stringify({ error: "slug is required" }), {
          status: 400,
          headers: { "Content-Type": "application/json" },
        });
      }
      let tenant;
      try {
        tenant = await createTenant(env.DB, env.TENANT_DO, {
          kind: body.kind,
          displayName: body.displayName,
          ownerAgentId: body.ownerAgentId,
          slug: body.slug,
        });
      } catch (e) {
        if (e instanceof SlugTakenError) {
          return new Response(JSON.stringify({ error: "slug taken" }), {
            status: 409,
            headers: { "Content-Type": "application/json" },
          });
        }
        throw e;
      }
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

    if (request.method === "POST" && url.pathname === "/verify") {
      const body = await request.json<{ token?: string }>();
      if (typeof body.token !== "string") {
        return new Response(JSON.stringify({ error: "token is required" }), {
          status: 400,
          headers: { "Content-Type": "application/json" },
        });
      }
      const result = await verifyToken(env, body.token);
      if (!result.valid) {
        return new Response(JSON.stringify({ error: result.error }), {
          status: 401,
          headers: { "Content-Type": "application/json" },
        });
      }
      return new Response(JSON.stringify(result.claims), {
        status: 200,
        headers: { "Content-Type": "application/json" },
      });
    }

    return new Response("Not found", { status: 404 });
  },
};
