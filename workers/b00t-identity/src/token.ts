import { lookupTenant } from "./registry";
import type { TenantNode } from "./tenant-do";
import { loadSigningKey, loadVerifyKey } from "./jwks";

export interface Env {
  DB: D1Database;
  TENANT_DO: DurableObjectNamespace<TenantNode>;
  JWT_PRIVATE_KEY_PEM: string;
  JWT_KID: string;
}

export interface IssueTokenInput {
  tenantId: string;
  agentId: string;
  nodeId: string;
  requestedShards: string[];
  r0le?: string; // SP1-04 threads this from the route; defaults to "member"
}

export type IssueTokenResult = { token: string } | { error: string };

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

export async function issueToken(env: Env, input: IssueTokenInput): Promise<IssueTokenResult> {
  const tenant = await lookupTenant(env.DB, input.tenantId);
  if (!tenant) {
    return { error: "tenant not found" };
  }

  const stub = env.TENANT_DO.get(env.TENANT_DO.idFromString(tenant.rootDoId));

  const hasMembership = await stub.hasMembershipPath(input.agentId, input.nodeId);
  if (!hasMembership) {
    return { error: "unauthorized" };
  }

  const grantsShards = await stub.nodeGrantsShards(input.nodeId, input.requestedShards);
  if (!grantsShards) {
    return { error: "unauthorized" };
  }

  const iat = Math.floor(Date.now() / 1000);
  const claims: AgentClaims = {
    iss: TOKEN_ISS,
    sub: input.agentId,
    tenant: input.tenantId,
    r0le: input.r0le ?? "member",
    scopes: input.requestedShards,
    budget_ref: "", // SP1-04: set from the ledgrrr authorize-spend response
    iat,
    exp: iat + TOKEN_TTL_SECONDS,
    jti: crypto.randomUUID(),
  };
  const token = await signJwt(env, claims);
  return { token };
}
