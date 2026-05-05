use once_cell::sync::Lazy;
use std::sync::RwLock;

/// Typed cloud capability — agents query by this, not by provider name.
/// Minimizes agent context because "who does inference?" is one question,
/// not "does cloudflare have inference? does AWS have inference? ..."
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub enum Capability {
    /// LLM inference — chat, embeddings, image generation
    Inference,
    /// Object storage — S3-compatible (R2, S3, GCS)
    ObjectStorage,
    /// Key-value store — low-latency lookups (KV, Redis)
    KeyValue,
    /// Vector database — semantic search (Vectorize, Pinecone, pgvector)
    VectorDb,
    /// SQL database — structured queries (D1, Neon, PlanetScale)
    Sql,
    /// Message queue — async messaging (Queues, SQS, PubSub)
    MessageQueue,
    /// Secure tunnel — expose local services (Cloudflare Tunnel, ngrok)
    Tunnel,
    /// DNS management — zones, records, routing
    Dns,
    /// Secrets management — encrypted config (Secrets Store, Vault)
    Secrets,
    /// Edge compute — serverless functions (Workers, Lambda)
    EdgeCompute,
    /// Container orchestration — Docker/Firecracker containers
    Containers,
    /// Real-time communication — WebRTC, media streaming
    Realtime,
    /// AI agent framework — agent hosting (Agents SDK, Claude Code)
    AgentFramework,
}

impl Capability {
    /// All known capability variants — useful for building discovery UIs.
    pub fn all() -> Vec<Capability> {
        vec![
            Capability::Inference,
            Capability::ObjectStorage,
            Capability::KeyValue,
            Capability::VectorDb,
            Capability::Sql,
            Capability::MessageQueue,
            Capability::Tunnel,
            Capability::Dns,
            Capability::Secrets,
            Capability::EdgeCompute,
            Capability::Containers,
            Capability::Realtime,
            Capability::AgentFramework,
        ]
    }

    /// Human-readable label
    pub fn label(&self) -> &'static str {
        match self {
            Capability::Inference => "LLM Inference",
            Capability::ObjectStorage => "Object Storage",
            Capability::KeyValue => "Key-Value Store",
            Capability::VectorDb => "Vector Database",
            Capability::Sql => "SQL Database",
            Capability::MessageQueue => "Message Queue",
            Capability::Tunnel => "Secure Tunnel",
            Capability::Dns => "DNS Management",
            Capability::Secrets => "Secrets Management",
            Capability::EdgeCompute => "Edge Compute",
            Capability::Containers => "Container Orchestration",
            Capability::Realtime => "Real-time Communication",
            Capability::AgentFramework => "AI Agent Framework",
        }
    }
}

/// Provider metadata — what agents discover when querying by capability.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ProviderInfo {
    pub name: String,                  // e.g. "cloudflare"
    pub display_name: String,          // e.g. "Cloudflare"
    pub capabilities: Vec<Capability>, // what this provider offers
    pub endpoint: String,              // base API endpoint
    pub plan: String,                  // "free", "paid", "beta"
    pub priority: u8,                  // 0=primary, 255=fallback
    pub env_vars: Vec<String>,         // required env vars
    pub status: String,                // "active", "beta", "preview"
}

impl ProviderInfo {
    pub fn new(name: &str, display_name: &str) -> Self {
        Self {
            name: name.to_string(),
            display_name: display_name.to_string(),
            capabilities: Vec::new(),
            endpoint: String::new(),
            plan: "unknown".to_string(),
            priority: 100,
            env_vars: Vec::new(),
            status: "active".to_string(),
        }
    }
}

/// Global provider registry — agents discover cloud providers here.
/// Lazy-initialized, thread-safe, append-only.
pub struct ProviderRegistry {
    providers: RwLock<Vec<ProviderInfo>>,
}

impl ProviderRegistry {
    /// Get the global singleton instance.
    pub fn global() -> &'static Self {
        static REGISTRY: Lazy<ProviderRegistry> = Lazy::new(|| {
            let reg = ProviderRegistry {
                providers: RwLock::new(Vec::new()),
            };
            // Register built-in providers
            reg.register_cloudflare();
            reg
        });
        &REGISTRY
    }

    /// Register a provider in the registry.
    pub fn register(&self, info: ProviderInfo) {
        if let Ok(mut providers) = self.providers.write() {
            // Upsert by name
            if let Some(pos) = providers.iter().position(|p| p.name == info.name) {
                providers[pos] = info;
            } else {
                providers.push(info);
            }
        }
    }

    /// Find all providers offering a specific capability.
    pub fn find_by_capability(&self, capability: Capability) -> Vec<ProviderInfo> {
        self.providers
            .read()
            .ok()
            .map(|p| {
                p.iter()
                    .filter(|info| info.capabilities.contains(&capability))
                    .cloned()
                    .collect()
            })
            .unwrap_or_default()
    }

    /// Find a provider by name.
    pub fn find_by_name(&self, name: &str) -> Option<ProviderInfo> {
        self.providers
            .read()
            .ok()
            .map(|p| {
                p.iter()
                    .find(|info| info.name == name)
                    .cloned()
            })
            .flatten()
    }

    /// List all registered providers.
    pub fn all(&self) -> Vec<ProviderInfo> {
        self.providers
            .read()
            .ok()
            .map(|p| p.clone())
            .unwrap_or_default()
    }

    /// Load providers from a .datum TOML file — enables datum-driven discovery.
    /// Looks for [cloudflare.services] sections to auto-register capabilities.
    pub fn load_from_datum(&self, path: &std::path::Path) -> anyhow::Result<()> {
        let content = std::fs::read_to_string(path)?;
        let table: toml::Table = content.parse()?;

        // Extract services from [cloudflare.services.*]
        if let Some(services) = table
            .get("cloudflare")
            .and_then(|c| c.get("services"))
            .and_then(|s| s.as_table())
        {
            for (_svc_name, svc_config) in services {
                let _kind = svc_config
                    .get("kind")
                    .and_then(|k| k.as_str())
                    .unwrap_or("unknown");
                let _status = svc_config
                    .get("status")
                    .and_then(|s| s.as_str())
                    .unwrap_or("unknown");
                let _plan = svc_config
                    .get("plan")
                    .and_then(|p| p.as_str())
                    .unwrap_or("unknown");
                let _endpoint = svc_config
                    .get("endpoint")
                    .and_then(|e| e.as_str())
                    .unwrap_or("");

                // Map service kind → Capability
                let capability = match _kind {
                    "inference" => Some(Capability::Inference),
                    "storage" | "s3-compatible" => Some(Capability::ObjectStorage),
                    "key-value-store" => Some(Capability::KeyValue),
                    "vector-database" => Some(Capability::VectorDb),
                    "database" | "sqlite" => Some(Capability::Sql),
                    "message-queue" => Some(Capability::MessageQueue),
                    "network-tunnel" => Some(Capability::Tunnel),
                    "secret-management" => Some(Capability::Secrets),
                    "container-orchestration" => Some(Capability::Containers),
                    "real-time-communication" => Some(Capability::Realtime),
                    "agent-framework" => Some(Capability::AgentFramework),
                    "llm-proxy" | "rag-pipeline" => Some(Capability::Inference),
                    _ => None,
                };

                if let Some(cap) = capability {
                    // Find or create provider entry
                    let mut providers = self
                        .providers
                        .write()
                        .map_err(|e| anyhow::anyhow!("lock: {}", e))?;
                    if let Some(existing) = providers.iter_mut().find(|p| p.name == "cloudflare") {
                        if !existing.capabilities.contains(&cap) {
                            existing.capabilities.push(cap);
                        }
                    }
                }
            }
        }

        Ok(())
    }

    /// Register the built-in Cloudflare provider with known capabilities.
    fn register_cloudflare(&self) {
        let info = ProviderInfo {
            name: "cloudflare".to_string(),
            display_name: "Cloudflare".to_string(),
            capabilities: vec![
                Capability::Inference,
                Capability::ObjectStorage,
                Capability::KeyValue,
                Capability::VectorDb,
                Capability::Sql,
                Capability::MessageQueue,
                Capability::Tunnel,
                Capability::Dns,
                Capability::Secrets,
                Capability::EdgeCompute,
                Capability::Containers,
                Capability::Realtime,
                Capability::AgentFramework,
            ],
            endpoint: "https://api.cloudflare.com/client/v4".to_string(),
            plan: "workers-paid".to_string(),
            priority: 10,
            env_vars: vec![
                "CLOUDFLARE_API_TOKEN".into(),
                "CLOUDFLARE_ACCOUNT_ID".into(),
                "CLOUDFLARE_ZONE_ID".into(),
            ],
            status: "active".to_string(),
        };
        self.register(info);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_registry_global() {
        let reg = ProviderRegistry::global();
        let all = reg.all();
        assert!(!all.is_empty());
        assert!(all.iter().any(|p| p.name == "cloudflare"));
    }

    #[test]
    fn test_find_by_capability() {
        let reg = ProviderRegistry::global();
        let inference = reg.find_by_capability(Capability::Inference);
        assert!(!inference.is_empty());
        assert_eq!(inference[0].name, "cloudflare");
    }

    #[test]
    fn test_find_by_name() {
        let reg = ProviderRegistry::global();
        let cf = reg.find_by_name("cloudflare");
        assert!(cf.is_some());
        assert_eq!(cf.unwrap().display_name, "Cloudflare");
    }

    #[test]
    fn test_load_from_datum() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("test.datum");
        std::fs::write(&path, r#"[cloudflare.services.workers_ai]
kind = "inference"
status = "active"
plan = "paid"

[cloudflare.services.r2]
kind = "storage"
status = "active"
plan = "paid"

[cloudflare.services.vectorize]
kind = "vector-database"
status = "active"
plan = "paid"
"#).unwrap();

        let reg = ProviderRegistry::global();
        reg.load_from_datum(&path).unwrap();
        let inference = reg.find_by_capability(Capability::Inference);
        assert!(inference.iter().any(|p| p.name == "cloudflare"));
    }
}
