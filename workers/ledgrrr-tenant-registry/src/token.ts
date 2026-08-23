import { lookupTenant } from "./registry";
import type { TenantNode } from "./tenant-do";

export interface Env {
  DB: D1Database;
  TENANT_DO: DurableObjectNamespace<TenantNode>;
}

export interface IssueTokenInput {
  tenantId: string;
  agentId: string;
  nodeId: string;
  requestedShards: string[];
}

export type IssueTokenResult = { token: string } | { error: string };

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

  const payload = {
    tenantId: input.tenantId,
    agentId: input.agentId,
    rootDoId: tenant.rootDoId,
    nodeId: input.nodeId,
    shards: input.requestedShards,
    issuedAt: new Date().toISOString(),
  };
  const token = btoa(JSON.stringify(payload));
  return { token };
}
