use anyhow::Result;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderAccountInfo {
    pub account_id: String,
    pub zone_id: String,
    pub plan: String,
    pub region: String,
    pub email: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EnvVarStatus {
    pub name: String,
    pub detected: bool,
    pub hint: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CloudService {
    pub name: String,
    pub kind: String,
    pub status: String,
    pub endpoint: Option<String>,
    pub plan: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServiceHealth {
    pub name: String,
    pub available: bool,
    pub latency_ms: u64,
    pub error: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatMessage {
    pub role: String,
    pub content: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InferenceInput {
    pub messages: Vec<ChatMessage>,
    pub max_tokens: Option<u64>,
    pub temperature: Option<f64>,
    pub stream: bool,
}

impl Default for InferenceInput {
    fn default() -> Self {
        Self {
            messages: Vec::new(),
            max_tokens: Some(4096),
            temperature: Some(0.7),
            stream: false,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TokenUsage {
    pub prompt_tokens: u64,
    pub completion_tokens: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InferenceOutput {
    pub text: String,
    pub usage: TokenUsage,
    pub model: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct R2Object {
    pub key: String,
    pub size: u64,
    pub etag: String,
    pub last_modified: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VectorMatch {
    pub id: String,
    pub score: f64,
    pub metadata: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TunnelInfo {
    pub id: String,
    pub name: String,
    pub status: String,
    pub connections: u32,
}

/// Abstract cloud provider interface — b00t agents dispatch through this trait.
/// Each cloud provider (Cloudflare, AWS, GCP, etc.) implements this trait.
pub trait AbstractCloudProvider: Send + Sync {
    fn provider_name(&self) -> &str;
    fn account_info(&self) -> Result<ProviderAccountInfo>;
    fn env_health(&self) -> Vec<EnvVarStatus>;
    fn list_services(&self) -> Result<Vec<CloudService>>;
    fn service_status(&self, service: &str) -> Result<ServiceHealth>;
    fn run_inference(&self, model: &str, input: &InferenceInput) -> Result<InferenceOutput>;
    fn kv_get(&self, namespace: &str, key: &str) -> Result<Option<String>>;
    fn kv_put(&self, namespace: &str, key: &str, value: &str, ttl_secs: Option<u64>) -> Result<()>;
    fn d1_query(&self, sql: &str, params: &[serde_json::Value]) -> Result<Vec<serde_json::Value>>;
    fn r2_list(&self, bucket: &str, prefix: Option<&str>) -> Result<Vec<R2Object>>;
    fn vectorize_search(&self, index: &str, vector: &[f32], top_k: usize) -> Result<Vec<VectorMatch>>;
    fn tunnel_list(&self) -> Result<Vec<TunnelInfo>>;
}
