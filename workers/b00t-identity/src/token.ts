import { lookupTenant } from "./registry";
import type { TenantNode } from "./tenant-do";
import { loadSigningKey, loadVerifyKey } from "./jwks";
import { authorizeSpend } from "./ledgrrr";
import type { AuthorizeSpendResult } from "./ledgrrr";

export interface Env {
  DB: D1Database;
  TENANT_DO: DurableObjectNamespace<TenantNode>;
  JWT_PRIVATE_KEY_PEM: string;
  JWT_KID: string;
  LEDGRRR_MODE?: string;
  LEDGRRR_BASE_URL?: string;
}

export interface IssueTokenInput {
  tenantId: string;
  agentId: string;
  nodeId: string;
  requestedShards: string[];
  r0le?: string; // SP1-04 threads this from the route; defaults to "member"
}

export type IssueTokenResult =
  | { token: string; budget_remaining: number }
  | { error: "tenant not found" | "unauthorized" | "budget_exceeded" };

/** RS256 JWT claims (SP1-02). `exp - iat` is always 900. */
export interface AgentClaims {
  iss: string;
  sub: string; // agent id
  tenant: string;
  r0le: string;
  scopes: string[];
  budget_ref: string;
  iat: number; // seconds since epoch
  exp: number; // seconds since epoch
  jti: string;
}

export const TOKEN_ISS = "https://b00t.promptexecution.com";
const TOKEN_TTL_SECONDS = 900;

function bufferToBase64Url(buffer: ArrayBufferLike): string {
  const bytes = new Uint8Array(buffer);
  let binary = "";
  for (const byte of bytes) binary += String.fromCharCode(byte);
  return btoa(binary).replace(/\+/g, "-").replace(/\//g, "_").replace(/=+$/, "");
}

function base64UrlToBuffer(base64url: string): ArrayBuffer {
  const base64 = base64url.replace(/-/g, "+").replace(/_/g, "/");
  const padded = base64 + "=".repeat((4 - (base64.length % 4)) % 4);
  const binary = atob(padded);
  const bytes = new Uint8Array(binary.length);
  for (let i = 0; i < binary.length; i++) bytes[i] = binary.charCodeAt(i);
  return bytes.buffer;
}

function stringToBase64Url(str: string): string {
  return bufferToBase64Url(new TextEncoder().encode(str).buffer);
}

function base64UrlToString(base64url: string): string {
  return new TextDecoder().decode(base64UrlToBuffer(base64url));
}

/** Sign claims as a 3-part RS256 JWS. Header carries `kid`. */
export async function signJwt(env: Env, claims: AgentClaims): Promise<string> {
  const header = { alg: "RS256", typ: "JWT", kid: env.JWT_KID };
  const signingInput = `${stringToBase64Url(JSON.stringify(header))}.${stringToBase64Url(
    JSON.stringify(claims),
  )}`;
  const key = await loadSigningKey(env);
  const sig = await crypto.subtle.sign(
    "RSASSA-PKCS1-v1_5",
    key,
    new TextEncoder().encode(signingInput),
  );
  return `${signingInput}.${bufferToBase64Url(sig)}`;
}

export type VerifyTokenResult =
  | { valid: true; claims: AgentClaims }
  | { valid: false; error: string };

/** Verify a 3-part RS256 JWS and its `exp`. */
export async function verifyJwt(env: Env, token: string): Promise<VerifyTokenResult> {
  const parts = token.split(".");
  if (parts.length !== 3) return { valid: false, error: "malformed token" };
  const [headerPart, payloadPart, signaturePart] = parts;

  let header: { alg?: string; kid?: string };
  try {
    header = JSON.parse(base64UrlToString(headerPart));
  } catch {
    return { valid: false, error: "malformed token" };
  }
  if (header.alg !== "RS256") return { valid: false, error: "invalid signature" };

  const key = await loadVerifyKey(env);
  const ok = await crypto.subtle.verify(
    "RSASSA-PKCS1-v1_5",
    key,
    base64UrlToBuffer(signaturePart),
    new TextEncoder().encode(`${headerPart}.${payloadPart}`),
  );
  if (!ok) return { valid: false, error: "invalid signature" };

  let claims: AgentClaims;
  try {
    claims = JSON.parse(base64UrlToString(payloadPart));
  } catch {
    return { valid: false, error: "malformed token" };
  }
  if (typeof claims.exp !== "number" || claims.exp * 1000 < Date.now()) {
    return { valid: false, error: "expired" };
  }
  return { valid: true, claims };
}

/** Back-compat alias used by the /verify route and tests. */
export const verifyToken = verifyJwt;

/**
 * Mint an agent token. Strict order:
 *   1. tenant must exist            -> `tenant not found`  (route: 404)
 *   2. agent must have a membership path to the node        (route: 403)
 *   3. the node/agent must grant every requested shard      (route: 403)
 *   4. ledgrrr must authorize the spend -> `budget_exceeded` (route: 402)
 * Only then is the JWT signed. `budget_ref` on the claims is the idempotency
 * key handed to ledgrrr.
 */
export async function issueToken(
  env: Env,
  input: IssueTokenInput,
  deps: { authorizeSpend?: typeof authorizeSpend } = {},
): Promise<IssueTokenResult> {
  const authorize = deps.authorizeSpend ?? authorizeSpend;

  const tenant = await lookupTenant(env.DB, input.tenantId);
  if (!tenant) {
    return { error: "tenant not found" };
  }

  const stub = env.TENANT_DO.get(env.TENANT_DO.idFromString(tenant.rootDoId));

  const hasMembership = await stub.hasMembershipPath(input.agentId, input.nodeId);
  if (!hasMembership) {
    return { error: "unauthorized" };
  }

  const grantsShards = await stub.agentGrantsShards(
    input.agentId,
    input.nodeId,
    input.requestedShards,
  );
  if (!grantsShards) {
    return { error: "unauthorized" };
  }

  const iat = Math.floor(Date.now() / 1000);
  const jti = crypto.randomUUID();

  const budget: AuthorizeSpendResult = await authorize(env, {
    tenant: input.tenantId,
    agent: input.agentId,
    cost: 1, // flat per-issuance cost for now
    ref: jti,
  });
  if (!budget.ok) {
    return { error: "budget_exceeded" };
  }

  const claims: AgentClaims = {
    iss: TOKEN_ISS,
    sub: input.agentId,
    tenant: input.tenantId,
    r0le: input.r0le ?? "member",
    scopes: input.requestedShards,
    budget_ref: jti,
    iat,
    exp: iat + TOKEN_TTL_SECONDS,
    jti,
  };
  const token = await signJwt(env, claims);
  return { token, budget_remaining: budget.budget_remaining };
}
