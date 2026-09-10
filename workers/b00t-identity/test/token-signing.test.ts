import { describe, it, expect, beforeAll } from "vitest";
import { env } from "cloudflare:test";
import { createTenant } from "../src/registry";
import { issueToken, signJwt, verifyToken, TOKEN_ISS } from "../src/token";
import type { AgentClaims } from "../src/token";

describe("token signing (RS256)", () => {
  beforeAll(async () => {
    await env.DB.exec(
      "CREATE TABLE IF NOT EXISTS tenants (id TEXT PRIMARY KEY, kind TEXT NOT NULL CHECK (kind IN ('personal', 'organizational')), display_name TEXT NOT NULL, slug TEXT, root_do_id TEXT NOT NULL, created_at TEXT NOT NULL)",
    );
  });

  async function issueValidToken() {
    const tenant = await createTenant(env.DB, env.TENANT_DO, {
      kind: "organizational",
      displayName: "Acme",
      ownerAgentId: "agent-owner",
    });
    const stub = env.TENANT_DO.get(env.TENANT_DO.idFromString(tenant.rootDoId));
    const node = await stub.createNode({
      parentId: null,
      kind: "business_unit",
      name: "Eng",
      settingsJson: JSON.stringify({ grantedShards: ["project"] }),
    });
    await stub.addMember("agent-1", node.id, "member");
    return issueToken(env, {
      tenantId: tenant.id,
      agentId: "agent-1",
      nodeId: node.id,
      requestedShards: ["project"],
    });
  }

  it("issues a 3-segment RS256 JWT and verifies it round-trip", async () => {
    const result = await issueValidToken();
    expect("token" in result).toBe(true);
    if (!("token" in result)) return;

    expect(result.token.split(".")).toHaveLength(3);
    const header = JSON.parse(
      atob(result.token.split(".")[0].replace(/-/g, "+").replace(/_/g, "/")),
    );
    expect(header.alg).toBe("RS256");
    expect(header.typ).toBe("JWT");
    expect(typeof header.kid).toBe("string");

    const verified = await verifyToken(env, result.token);
    expect(verified.valid).toBe(true);
    if (verified.valid) {
      expect(verified.claims.sub).toBe("agent-1");
      expect(verified.claims.iss).toBe(TOKEN_ISS);
      expect(verified.claims.tenant).toBeTruthy();
      expect(verified.claims.scopes).toEqual(["project"]);
      expect(verified.claims.exp - verified.claims.iat).toBe(900);
      expect(typeof verified.claims.jti).toBe("string");
    }
  });

  it("rejects a token with a tampered payload", async () => {
    const result = await issueValidToken();
    if (!("token" in result)) return;
    const [h, p, s] = result.token.split(".");
    const p2 = (p[0] === "a" ? "b" : "a") + p.slice(1);
    const verified = await verifyToken(env, `${h}.${p2}.${s}`);
    expect(verified).toEqual({ valid: false, error: "invalid signature" });
  });

  it("rejects a token with a tampered signature", async () => {
    const result = await issueValidToken();
    if (!("token" in result)) return;
    const [h, p, s] = result.token.split(".");
    const s2 = (s[0] === "a" ? "b" : "a") + s.slice(1);
    const verified = await verifyToken(env, `${h}.${p}.${s2}`);
    expect(verified).toEqual({ valid: false, error: "invalid signature" });
  });

  it("rejects an expired token", async () => {
    const past = Math.floor(Date.now() / 1000) - 4000;
    const claims: AgentClaims = {
      iss: TOKEN_ISS,
      sub: "agent-1",
      tenant: "tenant-1",
      node_id: "node-1",
      grant_source: "member:node-1",
      r0le: "member",
      scopes: ["project"],
      budget_ref: "",
      iat: past,
      exp: past + 900,
      jti: "jti-1",
    };
    const token = await signJwt(env, claims);
    const verified = await verifyToken(env, token);
    expect(verified).toEqual({ valid: false, error: "expired" });
  });

  it("rejects a malformed token", async () => {
    const verified = await verifyToken(env, "not-a-valid-token-no-dot");
    expect(verified).toEqual({ valid: false, error: "malformed token" });
  });
});
