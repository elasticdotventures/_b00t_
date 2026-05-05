use super::provider::*;
use anyhow::Result;
use std::collections::HashMap;

/// Cloudflare provider — implements AbstractCloudProvider using the CF v4 API.
/// Credentials are resolved from env vars (CLOUDFLARE_API_TOKEN, etc.).
pub struct CloudflareProvider {
    account_id: String,
    zone_id: String,
    api_token: Option<String>,
    api_email: Option<String>,
    http_client: reqwest::blocking::Client,
    // Cache for service health checks
    health_cache: HashMap<String, ServiceHealth>,
}

impl CloudflareProvider {
    pub fn new() -> Self {
    let api_token = std::env::var("CLOUDFLARE_API_TOKEN").ok()
        .or_else(|| std::env::var("CLOUDFLARE_TOKEN_VALUE").ok());
    let api_email = std::env::var("CLOUDFLARE_EMAIL").ok();
    let account_id = std::env::var("CLOUDFLARE_ACCOUNT_ID")
        .unwrap_or_else(|_| "f00c391669432ae2a423c04a001dab2d".to_string());
    let zone_id = std::env::var("CLOUDFLARE_ZONE_ID")
        .unwrap_or_else(|_| "df9acd5c921098ee3a6bb9ecff5b500f".to_string());

        let http_client = reqwest::blocking::Client::builder()
            .timeout(std::time::Duration::from_secs(30))
            .build()
            .unwrap_or_default();

        Self {
            account_id,
            zone_id,
            api_token,
            api_email,
            http_client,
            health_cache: HashMap::new(),
        }
    }

    fn api_url(&self, path: &str) -> String {
        format!("https://api.cloudflare.com/client/v4{}", path)
    }

    fn auth_header(&self) -> Option<(String, String)> {
        self.api_token.as_ref().map(|t| {
            ("Authorization".to_string(), format!("Bearer {}", t))
        })
    }

    fn request(
        &self,
        method: reqwest::Method,
        path: &str,
    ) -> Result<reqwest::blocking::RequestBuilder> {
        let url = self.api_url(path);
        let mut req = self.http_client.request(method, &url);
        if let Some((key, value)) = self.auth_header() {
            req = req.header(&key, &value);
        }
        // Also support email+key auth
        if let (Some(email), Some(key)) = (&self.api_email, &self.api_token) {
            req = req.header("X-Auth-Email", email);
            req = req.header("X-Auth-Key", key);
        }
        Ok(req)
    }
}

impl AbstractCloudProvider for CloudflareProvider {
    fn provider_name(&self) -> &str {
        "cloudflare"
    }

    fn account_info(&self) -> Result<ProviderAccountInfo> {
        Ok(ProviderAccountInfo {
            account_id: self.account_id.clone(),
            zone_id: self.zone_id.clone(),
            plan: "workers-paid".to_string(),
            region: "global".to_string(),
            email: self.api_email.clone(),
        })
    }

    fn env_health(&self) -> Vec<EnvVarStatus> {
        vec![
            EnvVarStatus {
                name: "CLOUDFLARE_API_TOKEN".into(),
                detected: self.api_token.is_some(),
                hint: "CF API Token with worker/d1/r2/kv/vectorize permissions".into(),
            },
            EnvVarStatus {
                name: "CLOUDFLARE_ACCOUNT_ID".into(),
                detected: true,
                hint: format!("{} (hardcoded default)", self.account_id),
            },
            EnvVarStatus {
                name: "CLOUDFLARE_ZONE_ID".into(),
                detected: true,
                hint: format!("{} (hardcoded default)", self.zone_id),
            },
            EnvVarStatus {
                name: "CLOUDFLARE_EMAIL".into(),
                detected: self.api_email.is_some(),
                hint: "Account email for legacy auth".into(),
            },
            EnvVarStatus {
                name: "CLOUDFLARE_TUNNEL_TOKEN".into(),
                detected: std::env::var("CLOUDFLARE_TUNNEL_TOKEN").is_ok(),
                hint: "cloudflared tunnel token".into(),
            },
            EnvVarStatus {
                name: "CLOUDFLARE_R2_ACCESS_KEY".into(),
                detected: std::env::var("CLOUDFLARE_R2_ACCESS_KEY").is_ok(),
                hint: "R2 S3 access key".into(),
            },
            EnvVarStatus {
                name: "CLOUDFLARE_R2_SECRET_KEY".into(),
                detected: std::env::var("CLOUDFLARE_R2_SECRET_KEY").is_ok(),
                hint: "R2 S3 secret key".into(),
            },
            EnvVarStatus {
                name: "CF_ACCESS_CLIENT_ID".into(),
                detected: std::env::var("CF_ACCESS_CLIENT_ID").is_ok(),
                hint: "CF Access service token ID for MCP OAuth".into(),
            },
            EnvVarStatus {
                name: "CF_ACCESS_CLIENT_SECRET".into(),
                detected: std::env::var("CF_ACCESS_CLIENT_SECRET").is_ok(),
                hint: "CF Access service token secret".into(),
            },
        ]
    }

    fn list_services(&self) -> Result<Vec<CloudService>> {
        Ok(vec![
            CloudService {
                name: "workers-ai".into(),
                kind: "inference".into(),
                status: "active".into(),
                endpoint: Some(format!(
                    "https://api.cloudflare.com/client/v4/accounts/{}/ai/run/{{model}}",
                    self.account_id
                )),
                plan: "paid".into(),
            },
            CloudService {
                name: "d1".into(),
                kind: "database".into(),
                status: "active".into(),
                endpoint: Some(format!(
                    "https://api.cloudflare.com/client/v4/accounts/{}/d1/database",
                    self.account_id
                )),
                plan: "paid".into(),
            },
            CloudService {
                name: "r2".into(),
                kind: "storage".into(),
                status: "active".into(),
                endpoint: Some(format!(
                    "https://{}.r2.cloudflarestorage.com",
                    self.account_id
                )),
                plan: "paid".into(),
            },
            CloudService {
                name: "kv".into(),
                kind: "key-value-store".into(),
                status: "active".into(),
                endpoint: Some(
                    "https://api.cloudflare.com/client/v4/accounts/{id}/storage/kv".into(),
                ),
                plan: "paid".into(),
            },
            CloudService {
                name: "vectorize".into(),
                kind: "vector-database".into(),
                status: "active".into(),
                endpoint: Some(format!(
                    "https://api.cloudflare.com/client/v4/accounts/{}/vectorize/v2",
                    self.account_id
                )),
                plan: "paid".into(),
            },
            CloudService {
                name: "queues".into(),
                kind: "message-queue".into(),
                status: "active".into(),
                endpoint: None,
                plan: "paid".into(),
            },
            CloudService {
                name: "tunnel".into(),
                kind: "network-tunnel".into(),
                status: "active".into(),
                endpoint: None,
                plan: "free".into(),
            },
            CloudService {
                name: "ai-gateway".into(),
                kind: "llm-proxy".into(),
                status: "active".into(),
                endpoint: None,
                plan: "paid".into(),
            },
            CloudService {
                name: "autorag".into(),
                kind: "rag-pipeline".into(),
                status: "beta".into(),
                endpoint: None,
                plan: "paid".into(),
            },
        ])
    }

    fn service_status(&self, service: &str) -> Result<ServiceHealth> {
        // Check cache first
        if let Some(cached) = self.health_cache.get(service) {
            return Ok(cached.clone());
        }

        let start = std::time::Instant::now();
        let available = self.api_token.is_some();
        let latency_ms = start.elapsed().as_millis() as u64;

        let health = ServiceHealth {
            name: service.to_string(),
            available,
            latency_ms,
            error: if !available {
                Some("CLOUDFLARE_API_TOKEN not set".to_string())
            } else {
                None
            },
        };
        Ok(health)
    }

    fn run_inference(&self, model: &str, input: &InferenceInput) -> Result<InferenceOutput> {
        let path = format!("/accounts/{}/ai/run/{}", self.account_id, model);
        let body = serde_json::json!({
            "messages": input.messages.iter().map(|m| serde_json::json!({
                "role": m.role,
                "content": m.content,
            })).collect::<Vec<_>>(),
            "max_tokens": input.max_tokens.unwrap_or(4096),
            "temperature": input.temperature.unwrap_or(0.7),
            "stream": input.stream,
        });

        let req = self
            .request(reqwest::Method::POST, &path)?
            .header("Content-Type", "application/json")
            .body(body.to_string());

        let resp = req.send()?;
        let json: serde_json::Value = resp.json()?;

        let text = json["result"]["response"]
            .as_str()
            .unwrap_or("")
            .to_string();
        let usage = TokenUsage {
            prompt_tokens: json["result"]["usage"]["prompt_tokens"]
                .as_u64()
                .unwrap_or(0),
            completion_tokens: json["result"]["usage"]["completion_tokens"]
                .as_u64()
                .unwrap_or(0),
        };

        Ok(InferenceOutput {
            text,
            usage,
            model: model.to_string(),
        })
    }

    fn kv_get(&self, namespace: &str, key: &str) -> Result<Option<String>> {
        let path = format!(
            "/accounts/{}/storage/kv/namespaces/{}/values/{}",
            self.account_id, namespace, key
        );
        let req = self.request(reqwest::Method::GET, &path)?;
        let resp = req.send()?;
        if resp.status().is_success() {
            Ok(Some(resp.text()?))
        } else if resp.status().as_u16() == 404 {
            Ok(None)
        } else {
            anyhow::bail!("KV GET failed: HTTP {}", resp.status())
        }
    }

    fn kv_put(
        &self,
        namespace: &str,
        key: &str,
        value: &str,
        _ttl_secs: Option<u64>,
    ) -> Result<()> {
        let path = format!(
            "/accounts/{}/storage/kv/namespaces/{}/values/{}",
            self.account_id, namespace, key
        );
        let req = self
            .request(reqwest::Method::PUT, &path)?
            .header("Content-Type", "text/plain")
            .body(value.to_string());
        req.send()?;
        Ok(())
    }

    fn d1_query(
        &self,
        _sql: &str,
        _params: &[serde_json::Value],
    ) -> Result<Vec<serde_json::Value>> {
        anyhow::bail!("D1 query requires a database ID — use service-specific subcommand")
    }

    fn r2_list(&self, _bucket: &str, _prefix: Option<&str>) -> Result<Vec<R2Object>> {
        anyhow::bail!("R2 list requires S3 credentials — use aws-sdk-s3 directly")
    }

    fn vectorize_search(
        &self,
        _index: &str,
        _vector: &[f32],
        _top_k: usize,
    ) -> Result<Vec<VectorMatch>> {
        anyhow::bail!(
            "Vectorize search requires index configuration — use service-specific endpoint"
        )
    }

    fn tunnel_list(&self) -> Result<Vec<TunnelInfo>> {
        let path = format!("/accounts/{}/cfd_tunnel", self.account_id);
        let req = self.request(reqwest::Method::GET, &path)?;
        let resp = req.send()?;
        if !resp.status().is_success() {
            anyhow::bail!("tunnel_list failed: HTTP {}", resp.status())
        }
        let json: serde_json::Value = resp.json()?;
        let tunnels = json["result"]
            .as_array()
            .map(|arr| {
                arr.iter()
                    .map(|t| TunnelInfo {
                        id: t["id"].as_str().unwrap_or("").to_string(),
                        name: t["name"].as_str().unwrap_or("").to_string(),
                        status: t["status"].as_str().unwrap_or("unknown").to_string(),
                        connections: t["connections_count"].as_u64().unwrap_or(0) as u32,
                    })
                    .collect()
            })
            .unwrap_or_default();
        Ok(tunnels)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_provider_name() {
        let p = CloudflareProvider::new();
        assert_eq!(p.provider_name(), "cloudflare");
    }

    #[test]
    fn test_env_health_returns_all_vars() {
        let p = CloudflareProvider::new();
        let health = p.env_health();
        assert!(health.len() >= 8);
        assert!(health.iter().any(|v| v.name == "CLOUDFLARE_API_TOKEN"));
    }

    #[test]
    fn test_list_services_returns_known_services() {
        let p = CloudflareProvider::new();
        let services = p.list_services().unwrap();
        assert!(services.iter().any(|s| s.name == "workers-ai"));
        assert!(services.iter().any(|s| s.name == "d1"));
        assert!(services.iter().any(|s| s.name == "r2"));
        assert!(services.iter().any(|s| s.name == "tunnel"));
    }

    #[test]
    fn test_account_info_returns_defaults() {
        let p = CloudflareProvider::new();
        let info = p.account_info().unwrap();
        assert_eq!(info.account_id, "f00c391669432ae2a423c04a001dab2d");
        assert_eq!(info.zone_id, "df9acd5c921098ee3a6bb9ecff5b500f");
    }

    #[test]
    fn test_account_info_has_plan_and_region() {
        let p = CloudflareProvider::new();
        let info = p.account_info().unwrap();
        assert_eq!(info.plan, "workers-paid");
        assert_eq!(info.region, "global");
    }

    #[test]
    fn test_service_status_without_token() {
        let p = CloudflareProvider::new();
        let status = p.service_status("workers-ai").unwrap();
        assert_eq!(status.name, "workers-ai");
        // Without CLOUDFLARE_API_TOKEN, available should be false
        assert!(!status.available);
    }

    #[test]
    fn test_default_inference_input() {
        let input = InferenceInput::default();
        assert_eq!(input.max_tokens, Some(4096));
        assert_eq!(input.temperature, Some(0.7));
        assert!(!input.stream);
        assert!(input.messages.is_empty());
    }

    #[test]
    fn test_tunnel_list_returns_error_without_creds() {
        // This will fail at HTTP level without credentials, but shouldn't panic
        let p = CloudflareProvider::new();
        let result = p.tunnel_list();
        // We expect an error since no API token is set
        assert!(result.is_err());
    }

    #[test]
    fn test_inference_input_custom() {
        let input = InferenceInput {
            messages: vec![ChatMessage {
                role: "user".into(),
                content: "Hello".into(),
            }],
            max_tokens: Some(100),
            temperature: Some(0.5),
            stream: false,
        };
        assert_eq!(input.messages.len(), 1);
        assert_eq!(input.messages[0].role, "user");
        assert_eq!(input.messages[0].content, "Hello");
    }

    #[test]
    fn test_d1_query_bails() {
        let p = CloudflareProvider::new();
        let result = p.d1_query("SELECT 1", &[]);
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(err.contains("D1 query requires a database ID"));
    }

    #[test]
    fn test_r2_list_bails() {
        let p = CloudflareProvider::new();
        let result = p.r2_list("my-bucket", None);
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(err.contains("R2 list requires S3 credentials"));
    }

    #[test]
    fn test_vectorize_search_bails() {
        let p = CloudflareProvider::new();
        let result = p.vectorize_search("my-index", &[0.1, 0.2], 5);
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(err.contains("Vectorize search requires index configuration"));
    }
}
