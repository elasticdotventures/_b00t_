-- SP1-06: URL-safe tenant handle. Additive-only (shared prod D1 `b00t-agents`).
ALTER TABLE tenants ADD COLUMN slug TEXT;
CREATE UNIQUE INDEX IF NOT EXISTS idx_tenants_slug ON tenants(slug);
