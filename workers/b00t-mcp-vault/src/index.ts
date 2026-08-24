// b00t-mcp-vault: a thin, stateless Worker exposing get/set access to the
// mcp_key column on the real, already-deployed b00t-agents D1 database
// (agents/roles schema, NATS-JWT-based hive-agent identity).
//
// Not a general-purpose API: exactly two routes, both requiring the same
// bearer-token auth. All state lives in D1 (agents table) — this Worker
// holds nothing itself, matching the "b00t-cli wrangler stateless MCP-proxy"
// pattern used elsewhere in this mission.

export interface Env {
  DB: D1Database;
  VAULT_ADMIN_KEY: string;
}

function unauthorized(): Response {
  return new Response(JSON.stringify({ error: "unauthorized" }), {
    status: 401,
    headers: { "Content-Type": "application/json" },
  });
}

function notFound(message = "not found"): Response {
  return new Response(JSON.stringify({ error: message }), {
    status: 404,
    headers: { "Content-Type": "application/json" },
  });
}

function isAuthorized(request: Request, env: Env): boolean {
  const auth = request.headers.get("Authorization");
  if (!auth || !auth.startsWith("Bearer ")) return false;
  const token = auth.slice("Bearer ".length);
  // Not constant-time; VAULT_ADMIN_KEY is a single operator-held secret,
  // not a per-user credential, so timing attacks aren't the threat model
  // here. Revisit if this ever gates per-user tokens instead.
  return token === env.VAULT_ADMIN_KEY;
}

async function getMcpKey(env: Env, agentId: string): Promise<Response> {
  const row = await env.DB.prepare(
    "SELECT mcp_key FROM agents WHERE id = ?"
  )
    .bind(agentId)
    .first<{ mcp_key: string | null }>();

  if (row === null) return notFound(`no agent with id ${agentId}`);

  return new Response(JSON.stringify({ id: agentId, mcp_key: row.mcp_key }), {
    status: 200,
    headers: { "Content-Type": "application/json" },
  });
}

async function putMcpKey(request: Request, env: Env, agentId: string): Promise<Response> {
  let body: { mcp_key?: string };
  try {
    body = await request.json();
  } catch {
    return new Response(JSON.stringify({ error: "invalid JSON body" }), {
      status: 400,
      headers: { "Content-Type": "application/json" },
    });
  }

  if (typeof body.mcp_key !== "string" || body.mcp_key.length === 0) {
    return new Response(
      JSON.stringify({ error: "mcp_key must be a non-empty string" }),
      { status: 400, headers: { "Content-Type": "application/json" } }
    );
  }

  const result = await env.DB.prepare(
    "UPDATE agents SET mcp_key = ? WHERE id = ?"
  )
    .bind(body.mcp_key, agentId)
    .run();

  if (result.meta.changes === 0) return notFound(`no agent with id ${agentId}`);

  return new Response(JSON.stringify({ id: agentId, updated: true }), {
    status: 200,
    headers: { "Content-Type": "application/json" },
  });
}

export default {
  async fetch(request: Request, env: Env): Promise<Response> {
    if (!isAuthorized(request, env)) return unauthorized();

    const url = new URL(request.url);
    const match = url.pathname.match(/^\/agents\/([^/]+)\/mcp_key$/);
    if (!match) return notFound();

    const agentId = decodeURIComponent(match[1]);

    if (request.method === "GET") return getMcpKey(env, agentId);
    if (request.method === "PUT") return putMcpKey(request, env, agentId);

    return new Response(JSON.stringify({ error: "method not allowed" }), {
      status: 405,
      headers: { "Content-Type": "application/json", Allow: "GET, PUT" },
    });
  },
};
