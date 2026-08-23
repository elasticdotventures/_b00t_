import { lookupTenant } from "./registry";
import type { TenantNode } from "./tenant-do";

export interface Env {
  DB: D1Database;
  TENANT_DO: DurableObjectNamespace<TenantNode>;
  TOKEN_SIGNING_KEY: string;
}

export interface IssueTokenInput {
  tenantId: string;
  agentId: string;
  nodeId: string;
  requestedShards: string[];
}

export type IssueTokenResult = { token: string } | { error: string };

export interface TokenPayload {
  tenantId: string;
  agentId: string;
  rootDoId: string;
  nodeId: string;
  shards: string[];
  issuedAt: string;
  expiresAt: string;
}

const TOKEN_TTL_MS = 60 * 60 * 1000; // 1 hour

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

async function getSigningKey(secret: string): Promise<CryptoKey> {
  return crypto.subtle.importKey(
    "raw",
    new TextEncoder().encode(secret),
    { name: "HMAC", hash: "SHA-256" },
    false,
    ["sign", "verify"]
  );
}

async function signData(secret: string, data: string): Promise<ArrayBuffer> {
  const key = await getSigningKey(secret);
  return crypto.subtle.sign("HMAC", key, new TextEncoder().encode(data));
}

async function verifySignature(secret: string, data: string, signature: ArrayBuffer): Promise<boolean> {
  const key = await getSigningKey(secret);
  return crypto.subtle.verify("HMAC", key, signature, new TextEncoder().encode(data));
}

export async function signPayload(secret: string, payload: TokenPayload): Promise<string> {
  const payloadPart = stringToBase64Url(JSON.stringify(payload));
  const signature = await signData(secret, payloadPart);
  const signaturePart = bufferToBase64Url(signature);
  return `${payloadPart}.${signaturePart}`;
}

export type VerifyTokenResult = { valid: true; payload: TokenPayload } | { valid: false; error: string };

export async function verifyToken(env: Env, token: string): Promise<VerifyTokenResult> {
  const parts = token.split(".");
  if (parts.length !== 2) {
    return { valid: false, error: "malformed token" };
  }
  const [payloadPart, signaturePart] = parts;

  const signature = base64UrlToBuffer(signaturePart);
  const signatureValid = await verifySignature(env.TOKEN_SIGNING_KEY, payloadPart, signature);
  if (!signatureValid) {
    return { valid: false, error: "invalid signature" };
  }

  let payload: TokenPayload;
  try {
    payload = JSON.parse(base64UrlToString(payloadPart));
  } catch {
    return { valid: false, error: "malformed payload" };
  }

  if (new Date(payload.expiresAt).getTime() < Date.now()) {
    return { valid: false, error: "expired" };
  }

  return { valid: true, payload };
}

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

  const now = Date.now();
  const payload: TokenPayload = {
    tenantId: input.tenantId,
    agentId: input.agentId,
    rootDoId: tenant.rootDoId,
    nodeId: input.nodeId,
    shards: input.requestedShards,
    issuedAt: new Date(now).toISOString(),
    expiresAt: new Date(now + TOKEN_TTL_MS).toISOString(),
  };
  const token = await signPayload(env.TOKEN_SIGNING_KEY, payload);
  return { token };
}
