use once_cell::sync::Lazy;
use std::sync::RwLock;

/// Typed cloud capability — agents query by this, not by provider name.
/// Minimizes agent context because "who does inference?" is one question,
/// not "does cloudflare have inference? does AWS have inference? ..."
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub enum Capability {
    Inference,        // LLM inference — chat, embeddings, image gen
    ObjectStorage,    // S3-compatible (R2, S3, GCS)
    KeyValue,         // Low-latency lookups (KV, Redis)
    VectorDb,         // Semantic search (Vectorize, Pinecone)
    Sql,              // SQL database (D1, Neon, PlanetScale)
    MessageQueue,     // Async messaging (Queues, SQS)
    Tunnel,           // Secure tunnel (Cloudflare Tunnel, ngrok)
    Dns,              // DNS management
    Secrets,          // Secrets management (Secrets Store, Vault)
    EdgeCompute,      // Serverless functions (Workers, Lambda)
    Containers,       // Container orchestration
    Realtime,         // Real-time communication (WebRTC)
    AgentFramework,   // AI agent framework (Agents SDK)
}

impl Capability {
    pub fn all() -> Vec<Capability> {
        vec![
            Capability::Inference, Capability::ObjectStorage, Capability::KeyValue,
            Capability::VectorDb, Capability::Sql, Capability::MessageQueue,
            Capability::Tunnel, Capability::Dns, Capability::Secrets,
            Capability::EdgeCompute, Capability::Containers,
            Capability::Realtime, Capability::AgentFramework,
        ]
    }

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

/// Provider metadata — agents discover this when querying by capability.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ProviderInfo {
    pub name: String,
    pub display_name: String,
    pub capabilities: Vec<Capability>,
    pub endpoint: String,
    pub plan: String,
    pub priority: u8,
    pub env_vars: Vec<String>,
    pub status: String,
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
/// Lazy-initialized, thread-safe. Agents query by capability, never by name.
pub struct ProviderRegistry {
    providers: RwLock<Vec<ProviderInfo>>,
}

impl ProviderRegistry {
    pub fn global() -> &'static Self {
        static REGISTRY: Lazy<ProviderRegistry> = Lazy::new(|| {
            let reg = ProviderRegistry { providers: RwLock::new(Vec::new()) };
            reg.register_cloudflare();
            reg
        });
        &REGISTRY
    }

    pub fn register(&self, info: ProviderInfo) {
        if let Ok(mut providers) = self.providers.write() {
            if let Some(pos) = providers.iter().position(|p| p.name == info.name) {
                providers[pos] = info;
            } else {
                providers.push(info);
            }
        }
    }

    pub fn find_by_capability(&self, capability: Capability) -> Vec<ProviderInfo> {
        self.providers.read().ok().map(|p|
            p.iter().filter(|info| info.capabilities.contains(&capability)).cloned().collect()
        ).unwrap_or_default()
    }

    pub fn find_by_name(&self, name: &str) -> Option<ProviderInfo> {
        self.providers.read().ok()?.iter().find(|info| info.name == name).cloned()
    }

    pub fn all(&self) -> Vec<ProviderInfo> {
        self.providers.read().ok().map(|p| p.clone()).unwrap_or_default()
    }

    pub fn load_from_datum(&self, path: &std::path::Path) -> anyhow::Result<()> {
        let content = std::fs::read_to_string(path)?;
        let table: toml::Table = content.parse()?;
        if let Some(services) = table.get("cloudflare").and_then(|c| c.get("services")).and_then(|s| s.as_table()) {
            for (_name, cfg) in services {
                let kind = cfg.get("kind").and_then(|k| k.as_str()).unwrap_or("");
                let cap = match kind {
                    "inference" | "llm-proxy" | "rag-pipeline" => Some(Capability::Inference),
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
                    _ => None,
                };
                if let Some(cap) = cap {
                    if let Ok(mut providers) = self.providers.write() {
                        if let Some(existing) = providers.iter_mut().find(|p| p.name == "cloudflare") {
                            if !existing.capabilities.contains(&cap) {
                                existing.capabilities.push(cap);
                            }
                        }
                    }
                }
            }
        }
        Ok(())
    }

    fn register_cloudflare(&self) {
        self.register(ProviderInfo {
            name: "cloudflare".into(),
            display_name: "Cloudflare".into(),
            capabilities: vec![
                Capability::Inference, Capability::ObjectStorage, Capability::KeyValue,
                Capability::VectorDb, Capability::Sql, Capability::MessageQueue,
                Capability::Tunnel, Capability::Dns, Capability::Secrets,
                Capability::EdgeCompute, Capability::Containers,
                Capability::Realtime, Capability::AgentFramework,
            ],
            endpoint: "https://api.cloudflare.com/client/v4".into(),
            plan: "workers-paid".into(),
            priority: 10,
            env_vars: vec!["CLOUDFLARE_API_TOKEN".into(), "CLOUDFLARE_ACCOUNT_ID".into(), "CLOUDFLARE_ZONE_ID".into()],
            status: "active".into(),
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_registry_global() {
        let reg = ProviderRegistry::global();
        assert!(reg.all().iter().any(|p| p.name == "cloudflare"));
    }

    #[test]
    fn test_find_by_capability() {
        let reg = ProviderRegistry::global();
        let inf = reg.find_by_capability(Capability::Inference);
        assert!(!inf.is_empty());
        assert_eq!(inf[0].name, "cloudflare");
    }

    #[test]
    fn test_find_by_name() {
        let reg = ProviderRegistry::global();
        assert!(reg.find_by_name("cloudflare").is_some());
    }

    #[test]
    fn test_load_from_datum() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("test.datum");
        std::fs::write(&path, r#"[cloudflare.services.workers_ai]
kind = "inference"
[cloudflare.services.r2]
kind = "storage"
"#).unwrap();
        let reg = ProviderRegistry::global();
        reg.load_from_datum(&path).unwrap();
        assert!(reg.find_by_capability(Capability::Inference).iter().any(|p| p.name == "cloudflare"));
    }

    #[test]
    fn test_capability_all_includes_all() {
        let all = Capability::all();
        assert_eq!(all.len(), 13);
        assert!(all.contains(&Capability::Dns));
    }
}
