import { describe, it, expect, beforeAll } from "vitest";
import { env } from "cloudflare:test";
import { createTenant } from "../src/registry";
import { issueToken, signPayload, verifyToken } from "../src/token";
import type { TokenPayload } from "../src/token";

describe("token signing", () => {
  beforeAll(async () => {
    // Create the tenants table (migration equivalent) — each test file gets
    // isolated D1 storage under vitest-pool-workers, mirroring token.test.ts.
    await env.DB.exec(
      "CREATE TABLE IF NOT EXISTS tenants (id TEXT PRIMARY KEY, kind TEXT NOT NULL CHECK (kind IN ('personal', 'organizational')), display_name TEXT NOT NULL, root_do_id TEXT NOT NULL, created_at TEXT NOT NULL)"
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

  it("issues a token and verifies it round-trip", async () => {
    const result = await issueValidToken();
    expect("token" in result).toBe(true);
    if (!("token" in result)) return;

    const verified = await verifyToken(env, result.token);
    expect(verified.valid).toBe(true);
    if (verified.valid) {
      expect(verified.payload.agentId).toBe("agent-1");
      expect(verified.payload.tenantId).toBeTruthy();
      expect(verified.payload.nodeId).toBeTruthy();
      expect(verified.payload.shards).toEqual(["project"]);
    }
  });

  it("rejects a token with a tampered payload", async () => {
    const result = await issueValidToken();
    expect("token" in result).toBe(true);
    if (!("token" in result)) return;

    const [payloadPart, signaturePart] = result.token.split(".");
    const corruptedChar = payloadPart[0] === "a" ? "b" : "a";
    const tamperedPayload = corruptedChar + payloadPart.slice(1);
    const tamperedToken = `${tamperedPayload}.${signaturePart}`;

    const verified = await verifyToken(env, tamperedToken);
    expect(verified).toEqual({ valid: false, error: "invalid signature" });
  });

  it("rejects a token with a tampered signature", async () => {
    const result = await issueValidToken();
    expect("token" in result).toBe(true);
    if (!("token" in result)) return;

    const [payloadPart, signaturePart] = result.token.split(".");
    const corruptedChar = signaturePart[0] === "a" ? "b" : "a";
    const tamperedSignature = corruptedChar + signaturePart.slice(1);
    const tamperedToken = `${payloadPart}.${tamperedSignature}`;

    const verified = await verifyToken(env, tamperedToken);
    expect(verified).toEqual({ valid: false, error: "invalid signature" });
  });

  it("rejects an expired token", async () => {
    const payload: TokenPayload = {
      tenantId: "tenant-1",
      agentId: "agent-1",
      rootDoId: "root-do-1",
      nodeId: "node-1",
      shards: ["project"],
      issuedAt: new Date(Date.now() - 2 * 60 * 60 * 1000).toISOString(),
      expiresAt: new Date(Date.now() - 60 * 60 * 1000).toISOString(),
    };
    const token = await signPayload(env.TOKEN_SIGNING_KEY, payload);

    const verified = await verifyToken(env, token);
    expect(verified).toEqual({ valid: false, error: "expired" });
  });

  it("rejects a malformed token", async () => {
    const verified = await verifyToken(env, "not-a-valid-token-no-dot");
    expect(verified).toEqual({ valid: false, error: "malformed token" });
  });
});
