declare module "cloudflare:test" {
  interface ProvidedEnv {
    DB: D1Database;
    TENANT_DO: DurableObjectNamespace<import("../src/tenant-do").TenantNode>;
  }
}
