declare module "cloudflare:test" {
  interface ProvidedEnv {
    DB: D1Database;
    REGISTRY_ADMIN_KEY: string;
    TENANT_DO: DurableObjectNamespace<import("../src/tenant-do").TenantNode>;
    TOKEN_SIGNING_KEY: string;
    JWT_PRIVATE_KEY_PEM: string;
    JWT_KID: string;
  }
}
