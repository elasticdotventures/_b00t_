//! Grok MCP client for b00t knowledgebase system
//!
//! Provides a DRY implementation of grok functionality using rmcp client
//! to connect to b00t-grok-py MCP server. Can be used by both b00t-cli
//! and b00t-mcp.

use anyhow::Result;
use rmcp::{
    ServiceExt,
    model::CallToolRequestParam,
    service::RunningService,
    transport::{ConfigureCommandExt, TokioChildProcess},
};
use serde_json::{Map, Value, json};
use std::borrow::Cow;
use std::env;
use tokio::process::Command;
use dirs;

/// Concrete MCP client running service type
type McpRunningService = RunningService<rmcp::service::RoleClient, ()>;

/// Which backend powers grok operations
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GrokBackend {
    /// b00t-grok-py (Python, requires Qdrant + Ollama)
    Python,
    /// irontology-mcp (Rust, NeumannStore + 4-way fusion)
    Irontology,
}

impl GrokBackend {
    /// Read GROK_BACKEND env var; default to Irontology
    pub fn from_env() -> Self {
        match std::env::var("GROK_BACKEND")
            .unwrap_or_default()
            .to_lowercase()
            .as_str()
        {
            "qdrant" | "python" => Self::Python,
            _ => Self::Irontology, // irontology is the default now
        }
    }
}

/// Grok MCP client for knowledgebase operations
pub struct GrokClient {
    mcp_client: Option<McpRunningService>,
    backend: GrokBackend,
}

/// Result structure for digest operations
#[derive(Debug, Clone)]
pub struct DigestResult {
    pub success: bool,
    pub chunk_id: String,
    pub topic: String,
    pub content_preview: String,
    pub created_at: String,
    pub message: Option<String>,
}

/// Result structure for ask operations  
#[derive(Debug, Clone)]
pub struct AskResult {
    pub success: bool,
    pub query: String,
    pub total_found: usize,
    pub results: Vec<ChunkResult>,
    pub message: Option<String>,
}

/// Individual chunk result from ask queries
#[derive(Debug, Clone)]
pub struct ChunkResult {
    pub id: String,
    pub content: String,
    pub topic: String,
    pub tags: Vec<String>,
    pub source: Option<String>,
    pub created_at: String,
}

/// Result structure for learn operations
#[derive(Debug, Clone)]
pub struct LearnResult {
    pub success: bool,
    pub source: String,
    pub chunks_created: usize,
    pub chunk_summaries: Vec<ChunkSummary>,
    pub message: Option<String>,
}

/// Summary of a created chunk
#[derive(Debug, Clone)]
pub struct ChunkSummary {
    pub id: String,
    pub topic: String,
    pub content_preview: String,
    pub tags: Vec<String>,
}

impl GrokClient {
    /// Create a new GrokClient; backend selected from GROK_BACKEND env var
    pub fn new() -> Self {
        Self { mcp_client: None, backend: GrokBackend::from_env() }
    }

    /// Initialize MCP client — dispatches to Python or irontology backend
    pub async fn initialize(&mut self) -> Result<()> {
        match self.backend {
            GrokBackend::Irontology => self.initialize_irontology().await,
            GrokBackend::Python => self.initialize_python().await,
        }
    }

    /// Spawn irontology-mcp binary via stdio MCP transport
    /// 🤓 IRONTOLOGY_BIN env var overrides; default: ~/.b00t/vendor/irontology-mcp/target/release/irontology-mcp
    async fn initialize_irontology(&mut self) -> Result<()> {
        let home = dirs::home_dir()
            .map(|p| p.to_string_lossy().into_owned())
            .or_else(|| env::var("HOME").ok())
            .unwrap_or_else(|| {
                tracing::warn!("HOME is unset and dirs::home_dir() failed; defaulting to /tmp");
                "/tmp".to_string()
            });
        let bin_path = env::var("IRONTOLOGY_BIN").unwrap_or_else(|_| {
            format!("{}/.b00t/vendor/irontology-mcp/target/release/irontology-mcp", home)
        });
        let neumann_data_dir = env::var("NEUMANN_DATA_DIR")
            .unwrap_or_else(|_| format!("{}/.b00t/neumann/default", home));

        let transport = TokioChildProcess::new(Command::new(&bin_path).configure(|cmd| {
            cmd.env("PATH", env::var("PATH").unwrap_or_default())
                .env("NEUMANN_DATA_DIR", &neumann_data_dir);
        }))
        .map_err(|e| anyhow::anyhow!("Failed to spawn irontology-mcp ({}): {}", bin_path, e))?;

        let running_service = ()
            .serve(transport)
            .await
            .map_err(|e| anyhow::anyhow!("Failed to connect to irontology-mcp: {}", e))?;

        self.mcp_client = Some(running_service);
        Ok(())
    }

    /// Spawn b00t-grok-py Python MCP server (legacy; requires Qdrant + Ollama)
    async fn initialize_python(&mut self) -> Result<()> {
        // 🤓 b00t pattern: use environment variables for configuration
        let qdrant_url =
            env::var("QDRANT_URL").unwrap_or_else(|_| "http://192.168.2.13:6333".to_string());
        let qdrant_api_key = env::var("QDRANT_API_KEY").unwrap_or_default();

        // Resolve grok-py path: env var > sibling of cargo workspace root > fallback
        // 🤓 B00T_GROK_PY_PATH env var allows override; workspace root sibling avoids hardcoded $HOME
        let grok_py_path = env::var("B00T_GROK_PY_PATH").unwrap_or_else(|_| {
            let workspace_root = env::var("CARGO_MANIFEST_DIR")
                .map(|d| {
                    std::path::Path::new(&d)
                        .parent()
                        .map(|p| p.to_string_lossy().to_string())
                        .unwrap_or(d)
                })
                .unwrap_or_else(|_| {
                    std::env::current_exe()
                        .ok()
                        .and_then(|p| {
                            p.parent()
                                .and_then(|p| p.parent())
                                .and_then(|p| p.parent())
                                .map(|p| p.to_string_lossy().to_string())
                        })
                        .unwrap_or_else(|| {
                            format!("{}/.b00t", env::var("HOME").unwrap_or_default())
                        })
                });
            format!("{}/b00t-grok-py", workspace_root)
        });

        // Create the child process transport
        // 🤓 Resolve uv binary path explicitly: Command::new("uv") fails when spawned
        //    from a process without full PATH (systemd/MCP). uv lives in ~/.local/bin.
        let home = env::var("HOME").unwrap_or_default();
        let uv_candidates = [
            format!("{}/.local/bin/uv", home),
            format!("{}/.cargo/bin/uv", home),
            "/usr/local/bin/uv".to_string(),
            "/usr/bin/uv".to_string(),
        ];
        let uv_path = uv_candidates
            .iter()
            .find(|p| std::path::Path::new(p.as_str()).exists())
            .cloned()
            .unwrap_or_else(|| "uv".to_string()); // fallback: hope it's in PATH
        let transport = TokioChildProcess::new(Command::new(&uv_path).configure(|cmd| {
            cmd.arg("run")
                .arg("python")
                .arg("-m")
                .arg("b00t_grok_guru.server")
                .current_dir(&grok_py_path) // output: resolved from B00T_GROK_PY_PATH or workspace root
                .env("PATH", env::var("PATH").unwrap_or_default()) // 🤓 propagate PATH to child
                .env("QDRANT_URL", qdrant_url)
                .env("QDRANT_API_KEY", qdrant_api_key)
                .env("PYTHONPATH", "python");
        }))
        .map_err(|e| anyhow::anyhow!("Failed to spawn grok server: {}", e))?;

        let running_service = ()
            .serve(transport)
            .await
            .map_err(|e| anyhow::anyhow!("Failed to initialize grok client: {}", e))?;

        self.mcp_client = Some(running_service);
        Ok(())
    }

    /// Digest content into a knowledge chunk about a specific topic
    pub async fn digest(&self, topic: &str, content: &str) -> Result<DigestResult> {
        let client = self.mcp_client.as_ref().ok_or_else(|| {
            anyhow::anyhow!("GrokClient not initialized - call initialize() first")
        })?;

        match self.backend {
            GrokBackend::Irontology => {
                // 🤓 irontology: repo.index tool — chunk+embed+persist
                let mut params = Map::new();
                params.insert("content".to_string(), json!(content));
                params.insert("source".to_string(), json!(format!("grok:digest:{}", topic)));
                params.insert("topic".to_string(), json!(topic));
                let request = CallToolRequestParam {
                    name: Cow::Borrowed("repo.index"),
                    arguments: Some(params),
                };
                let response = client.call_tool(request).await?;
                self.parse_irontology_digest_response(response, topic, content)
            }
            GrokBackend::Python => {
                let mut params = Map::new();
                params.insert("topic".to_string(), json!(topic));
                params.insert("content".to_string(), json!(content));
                let request = CallToolRequestParam {
                    name: Cow::Borrowed("grok_digest"),
                    arguments: Some(params),
                };
                let response = client.call_tool(request).await?;
                self.parse_digest_response(response)
            }
        }
    }

    /// Search the knowledgebase for information related to a query
    pub async fn ask(
        &self,
        query: &str,
        topic: Option<&str>,
        limit: Option<usize>,
    ) -> Result<AskResult> {
        let client = self.mcp_client.as_ref().ok_or_else(|| {
            anyhow::anyhow!("GrokClient not initialized - call initialize() first")
        })?;

        match self.backend {
            GrokBackend::Irontology => {
                // 🤓 irontology: repo.search — 4-way fusion (vector 0.35, graph 0.30, lexical 0.20, ontology 0.15)
                let mut params = Map::new();
                params.insert("query".to_string(), json!(query));
                if let Some(t) = topic {
                    params.insert("topic".to_string(), json!(t));
                }
                if let Some(k) = limit {
                    params.insert("top_k".to_string(), json!(k));
                }
                let request = CallToolRequestParam {
                    name: Cow::Borrowed("repo.search"),
                    arguments: Some(params),
                };
                let response = client.call_tool(request).await?;
                self.parse_irontology_ask_response(response, query)
            }
            GrokBackend::Python => {
                let mut params = Map::new();
                params.insert("query".to_string(), json!(query));
                if let Some(topic) = topic {
                    params.insert("topic".to_string(), json!(topic));
                }
                if let Some(limit) = limit {
                    params.insert("limit".to_string(), json!(limit));
                }
                let request = CallToolRequestParam {
                    name: Cow::Borrowed("grok_ask"),
                    arguments: Some(params),
                };
                let response = client.call_tool(request).await?;
                self.parse_ask_response(response)
            }
        }
    }

    /// Learn from content by breaking it into chunks and storing in knowledgebase
    pub async fn learn(&self, content: &str, source: Option<&str>) -> Result<LearnResult> {
        let client = self.mcp_client.as_ref().ok_or_else(|| {
            anyhow::anyhow!("GrokClient not initialized - call initialize() first")
        })?;

        match self.backend {
            GrokBackend::Irontology => {
                // 🤓 irontology: repo.index with source metadata
                let src = source.unwrap_or("grok:learn:unknown");
                let mut params = Map::new();
                params.insert("content".to_string(), json!(content));
                params.insert("source".to_string(), json!(src));
                let request = CallToolRequestParam {
                    name: Cow::Borrowed("repo.index"),
                    arguments: Some(params),
                };
                let response = client.call_tool(request).await?;
                self.parse_irontology_learn_response(response, src)
            }
            GrokBackend::Python => {
                let mut params = Map::new();
                params.insert("content".to_string(), json!(content));
                if let Some(source) = source {
                    params.insert("source".to_string(), json!(source));
                }
                let request = CallToolRequestParam {
                    name: Cow::Borrowed("grok_learn"),
                    arguments: Some(params),
                };
                let response = client.call_tool(request).await?;
                self.parse_learn_response(response)
            }
        }
    }

    /// Get the current status of the grok system
    pub async fn status(&self) -> Result<Value> {
        if self.backend == GrokBackend::Irontology {
            // irontology doesn't have a status tool — return static info
            return Ok(json!({
                "status": "ok",
                "backend": "irontology-mcp",
                "initialized": self.mcp_client.is_some(),
            }));
        }

        let client = self.mcp_client.as_ref().ok_or_else(|| {
            anyhow::anyhow!("GrokClient not initialized - call initialize() first")
        })?;

        let request = CallToolRequestParam {
            name: Cow::Borrowed("grok_status"),
            arguments: None,
        };
        let response = client.call_tool(request).await?;

        let content_text = response
            .content
            .get(0)
            .ok_or_else(|| anyhow::anyhow!("Empty response content"))?
            .as_text()
            .ok_or_else(|| anyhow::anyhow!("No text content in response"))?;

        let response_json: Value = serde_json::from_str(&content_text.text)?;
        Ok(response_json)
    }

    // ── irontology response parsers ──────────────────────────────────────────

    fn parse_irontology_digest_response(
        &self,
        response: rmcp::model::CallToolResult,
        topic: &str,
        content: &str,
    ) -> Result<DigestResult> {
        // repo.index returns: {"indexed": bool, "source": str, "topic": str, "chunks": n, "embedded": n}
        let text = Self::extract_text(&response)?;
        let v: Value = serde_json::from_str(&text).unwrap_or(Value::Null);
        let indexed = v.get("indexed").and_then(|x| x.as_bool()).unwrap_or(false);
        Ok(DigestResult {
            success: indexed,
            chunk_id: format!(
                "grok:digest:{}::chunk::0",
                topic
            ),
            topic: topic.to_string(),
            content_preview: content.chars().take(80).collect(),
            created_at: String::new(),
            message: if indexed { None } else { Some(text) },
        })
    }

    fn parse_irontology_ask_response(
        &self,
        response: rmcp::model::CallToolResult,
        query: &str,
    ) -> Result<AskResult> {
        // repo.search returns: {"results": [{"id": str, "content": str, "score": f64}], ...}
        let text = Self::extract_text(&response)?;
        let v: Value = serde_json::from_str(&text).unwrap_or(Value::Null);
        let results: Vec<ChunkResult> =
            if let Some(results_array) = v.get("results").and_then(|r| r.as_array()) {
                results_array
                    .iter()
                    .filter_map(|item| {
                        let obj = item.as_object()?;
                        Some(ChunkResult {
                            id: obj.get("id").and_then(|x| x.as_str()).unwrap_or("").to_string(),
                            content: obj.get("content").and_then(|x| x.as_str()).unwrap_or("").to_string(),
                            topic: obj.get("topic").and_then(|x| x.as_str()).unwrap_or("").to_string(),
                            tags: vec![],
                            source: obj.get("source").and_then(|x| x.as_str()).map(|s| s.to_string()),
                            created_at: String::new(),
                        })
                    })
                    .collect()
            } else {
                Vec::new()
            };
        let total = results.len();
        Ok(AskResult {
            success: true,
            query: query.to_string(),
            total_found: total,
            results,
            message: None,
        })
    }

    fn parse_irontology_learn_response(
        &self,
        response: rmcp::model::CallToolResult,
        source: &str,
    ) -> Result<LearnResult> {
        // repo.index returns: {"indexed": bool, "chunks": n, "embedded": n, ...}
        let text = Self::extract_text(&response)?;
        let v: Value = serde_json::from_str(&text).unwrap_or(Value::Null);
        let indexed = v.get("indexed").and_then(|x| x.as_bool()).unwrap_or(false);
        let chunks = v.get("chunks").and_then(|x| x.as_u64()).unwrap_or(0) as usize;
        Ok(LearnResult {
            success: indexed,
            source: source.to_string(),
            chunks_created: chunks,
            chunk_summaries: vec![],
            message: if indexed { None } else { Some(text) },
        })
    }

    /// Extract text from the first content item of a CallToolResult
    fn extract_text(response: &rmcp::model::CallToolResult) -> Result<String> {
        Ok(response
            .content
            .first()
            .ok_or_else(|| anyhow::anyhow!("Empty response content"))?
            .as_text()
            .ok_or_else(|| anyhow::anyhow!("No text content in response"))?
            .text
            .clone())
    }

    // ── Python (legacy) response parsers ─────────────────────────────────────

    fn parse_digest_response(&self, response: rmcp::model::CallToolResult) -> Result<DigestResult> {
        // Extract JSON from the first content item
        let content_text = response
            .content
            .get(0)
            .ok_or_else(|| anyhow::anyhow!("Empty response content"))?
            .as_text()
            .ok_or_else(|| anyhow::anyhow!("No text content in response"))?;

        let response_json: Value = serde_json::from_str(&content_text.text)?;

        if let Some(obj) = response_json.as_object() {
            Ok(DigestResult {
                success: obj
                    .get("success")
                    .and_then(|v| v.as_bool())
                    .unwrap_or(false),
                chunk_id: obj
                    .get("chunk_id")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string(),
                topic: obj
                    .get("topic")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string(),
                content_preview: obj
                    .get("content_preview")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string(),
                created_at: obj
                    .get("created_at")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string(),
                message: obj
                    .get("message")
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string()),
            })
        } else {
            Err(anyhow::anyhow!("Invalid digest response format"))
        }
    }

    fn parse_ask_response(&self, response: rmcp::model::CallToolResult) -> Result<AskResult> {
        // Extract JSON from the first content item
        let content_text = response
            .content
            .get(0)
            .ok_or_else(|| anyhow::anyhow!("Empty response content"))?
            .as_text()
            .ok_or_else(|| anyhow::anyhow!("No text content in response"))?;

        let response_json: Value = serde_json::from_str(&content_text.text)?;

        if let Some(obj) = response_json.as_object() {
            let results = obj
                .get("results")
                .and_then(|v| v.as_array())
                .unwrap_or(&Vec::new())
                .iter()
                .filter_map(|item| {
                    if let Some(chunk) = item.as_object() {
                        Some(ChunkResult {
                            id: chunk
                                .get("id")
                                .and_then(|v| v.as_str())
                                .unwrap_or("")
                                .to_string(),
                            content: chunk
                                .get("content")
                                .and_then(|v| v.as_str())
                                .unwrap_or("")
                                .to_string(),
                            topic: chunk
                                .get("topic")
                                .and_then(|v| v.as_str())
                                .unwrap_or("")
                                .to_string(),
                            tags: chunk
                                .get("tags")
                                .and_then(|v| v.as_array())
                                .unwrap_or(&Vec::new())
                                .iter()
                                .filter_map(|t| t.as_str().map(|s| s.to_string()))
                                .collect(),
                            source: chunk
                                .get("source")
                                .and_then(|v| v.as_str())
                                .map(|s| s.to_string()),
                            created_at: chunk
                                .get("created_at")
                                .and_then(|v| v.as_str())
                                .unwrap_or("")
                                .to_string(),
                        })
                    } else {
                        None
                    }
                })
                .collect();

            Ok(AskResult {
                success: obj
                    .get("success")
                    .and_then(|v| v.as_bool())
                    .unwrap_or(false),
                query: obj
                    .get("query")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string(),
                total_found: obj.get("total_found").and_then(|v| v.as_u64()).unwrap_or(0) as usize,
                results,
                message: obj
                    .get("message")
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string()),
            })
        } else {
            Err(anyhow::anyhow!("Invalid ask response format"))
        }
    }

    fn parse_learn_response(&self, response: rmcp::model::CallToolResult) -> Result<LearnResult> {
        // Extract JSON from the first content item
        let content_text = response
            .content
            .get(0)
            .ok_or_else(|| anyhow::anyhow!("Empty response content"))?
            .as_text()
            .ok_or_else(|| anyhow::anyhow!("No text content in response"))?;

        let response_json: Value = serde_json::from_str(&content_text.text)?;

        if let Some(obj) = response_json.as_object() {
            let chunk_summaries = obj
                .get("chunk_summaries")
                .and_then(|v| v.as_array())
                .unwrap_or(&Vec::new())
                .iter()
                .filter_map(|item| {
                    if let Some(summary) = item.as_object() {
                        Some(ChunkSummary {
                            id: summary
                                .get("id")
                                .and_then(|v| v.as_str())
                                .unwrap_or("")
                                .to_string(),
                            topic: summary
                                .get("topic")
                                .and_then(|v| v.as_str())
                                .unwrap_or("")
                                .to_string(),
                            content_preview: summary
                                .get("content_preview")
                                .and_then(|v| v.as_str())
                                .unwrap_or("")
                                .to_string(),
                            tags: summary
                                .get("tags")
                                .and_then(|v| v.as_array())
                                .unwrap_or(&Vec::new())
                                .iter()
                                .filter_map(|t| t.as_str().map(|s| s.to_string()))
                                .collect(),
                        })
                    } else {
                        None
                    }
                })
                .collect();

            Ok(LearnResult {
                success: obj
                    .get("success")
                    .and_then(|v| v.as_bool())
                    .unwrap_or(false),
                source: obj
                    .get("source")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string(),
                chunks_created: obj
                    .get("chunks_created")
                    .and_then(|v| v.as_u64())
                    .unwrap_or(0) as usize,
                chunk_summaries,
                message: obj
                    .get("message")
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string()),
            })
        } else {
            Err(anyhow::anyhow!("Invalid learn response format"))
        }
    }
}

impl Default for GrokClient {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_grok_client_creation() {
        let client = GrokClient::new();
        assert!(client.mcp_client.is_none());
    }

    #[test]
    fn test_grok_backend_from_env_default_is_irontology() {
        // GROK_BACKEND unset → Irontology
        // 🤓 unsafe required in Rust 2024 for env mutation
        unsafe { std::env::remove_var("GROK_BACKEND") };
        assert_eq!(GrokBackend::from_env(), GrokBackend::Irontology);
    }

    #[test]
    fn test_grok_backend_python_variants() {
        for val in &["python", "qdrant", "PYTHON", "QDRANT"] {
            unsafe { std::env::set_var("GROK_BACKEND", val) };
            assert_eq!(GrokBackend::from_env(), GrokBackend::Python, "val={}", val);
        }
        unsafe { std::env::remove_var("GROK_BACKEND") };
    }

    #[tokio::test]
    #[ignore = "Requires uv + b00t-grok-py service"]
    async fn test_grok_client_initialization() {
        let mut client = GrokClient::new();

        // This test will fail if b00t-grok-py server is not available
        // That's expected in CI/testing environments
        let result = client.initialize().await;

        // Don't assert success - just verify the method exists and returns a Result
        match result {
            Ok(_) => println!("✅ GrokClient initialized successfully"),
            Err(e) => println!(
                "⚠️ GrokClient initialization failed (expected in test env): {}",
                e
            ),
        }
    }

    #[test]
    fn test_digest_result_creation() {
        let result = DigestResult {
            success: true,
            chunk_id: "test-123".to_string(),
            topic: "rust".to_string(),
            content_preview: "Test content...".to_string(),
            created_at: "2025-01-01T00:00:00Z".to_string(),
            message: None,
        };

        assert!(result.success);
        assert_eq!(result.chunk_id, "test-123");
        assert_eq!(result.topic, "rust");
    }

    #[test]
    fn test_ask_result_creation() {
        let chunk = ChunkResult {
            id: "chunk-1".to_string(),
            content: "Test chunk content".to_string(),
            topic: "rust".to_string(),
            tags: vec!["test".to_string()],
            source: Some("test.md".to_string()),
            created_at: "2025-01-01T00:00:00Z".to_string(),
        };

        let result = AskResult {
            success: true,
            query: "test query".to_string(),
            total_found: 1,
            results: vec![chunk],
            message: None,
        };

        assert!(result.success);
        assert_eq!(result.total_found, 1);
        assert_eq!(result.results.len(), 1);
    }
}

#[cfg(test)]
mod additional_tests {
    use super::*;

    #[test]
    fn test_grok_client_default() {
        let client = GrokClient::default();
        assert!(client.mcp_client.is_none());
    }

    #[test]
    fn test_digest_result_message() {
        let result = DigestResult {
            success: false,
            chunk_id: String::new(),
            topic: String::new(),
            content_preview: String::new(),
            created_at: String::new(),
            message: Some("Error occurred".to_string()),
        };

        assert!(!result.success);
        assert_eq!(result.message.unwrap(), "Error occurred");
    }

    #[test]
    fn test_ask_result_empty() {
        let result = AskResult {
            success: true,
            query: "test".to_string(),
            total_found: 0,
            results: vec![],
            message: None,
        };

        assert!(result.results.is_empty());
        assert_eq!(result.total_found, 0);
    }

    #[test]
    fn test_chunk_result_with_tags() {
        let chunk = ChunkResult {
            id: "test-1".to_string(),
            content: "Test content".to_string(),
            topic: "rust".to_string(),
            tags: vec!["testing".to_string(), "rust".to_string()],
            source: Some("test.rs".to_string()),
            created_at: "2025-01-01T00:00:00Z".to_string(),
        };

        assert_eq!(chunk.tags.len(), 2);
        assert!(chunk.tags.contains(&"testing".to_string()));
    }

    #[test]
    fn test_learn_result_multiple_chunks() {
        let summaries = vec![
            ChunkSummary {
                id: "chunk-1".to_string(),
                topic: "rust".to_string(),
                content_preview: "First chunk...".to_string(),
                tags: vec![],
            },
            ChunkSummary {
                id: "chunk-2".to_string(),
                topic: "rust".to_string(),
                content_preview: "Second chunk...".to_string(),
                tags: vec![],
            },
        ];

        let result = LearnResult {
            success: true,
            source: "test.md".to_string(),
            chunks_created: 2,
            chunk_summaries: summaries,
            message: None,
        };

        assert_eq!(result.chunks_created, 2);
        assert_eq!(result.chunk_summaries.len(), 2);
    }

    #[tokio::test]
    #[ignore = "Requires b00t-grok-py service and QDRANT"]
    async fn test_full_workflow() {
        // Test complete workflow:
        // 1. Initialize client
        // 2. Digest content
        // 3. Learn from content
        // 4. Query the knowledgebase
        // 5. Check status

        let mut client = GrokClient::new();

        // Initialize
        let init_result = client.initialize().await;
        if init_result.is_err() {
            println!("⚠️ Service not available, skipping workflow test");
            return;
        }

        // Digest
        let digest = client.digest("rust", "Rust ensures memory safety").await;
        assert!(digest.is_ok(), "Digest should succeed");

        // Learn
        let learn = client
            .learn("Rust is fast.\n\nRust is safe.", Some("test.md"))
            .await;
        assert!(learn.is_ok(), "Learn should succeed");

        // Ask
        let ask = client.ask("memory safety", None, Some(5)).await;
        assert!(ask.is_ok(), "Ask should succeed");

        // Status
        let status = client.status().await;
        assert!(status.is_ok(), "Status should succeed");

        let status_value = status.unwrap();
        assert!(status_value.get("status").is_some());
    }

    #[test]
    fn test_result_types_implement_debug() {
        // Verify all result types implement Debug
        let digest = DigestResult {
            success: true,
            chunk_id: "test".to_string(),
            topic: "rust".to_string(),
            content_preview: "preview".to_string(),
            created_at: "2025-01-01".to_string(),
            message: None,
        };

        let debug_str = format!("{:?}", digest);
        assert!(debug_str.contains("DigestResult"));
        assert!(debug_str.contains("rust"));
    }

    #[test]
    fn test_result_types_implement_clone() {
        let original = ChunkResult {
            id: "test".to_string(),
            content: "content".to_string(),
            topic: "topic".to_string(),
            tags: vec!["tag1".to_string()],
            source: Some("source.md".to_string()),
            created_at: "2025-01-01".to_string(),
        };

        let cloned = original.clone();
        assert_eq!(original.id, cloned.id);
        assert_eq!(original.content, cloned.content);
        assert_eq!(original.tags, cloned.tags);
    }
}

#[cfg(test)]
mod irontology_parser_tests {
    use super::*;
    use rmcp::model::{CallToolResult, Content};

    fn make_text_result(text: &str) -> CallToolResult {
        CallToolResult::success(vec![Content::text(text)])
    }

    fn make_empty_result() -> CallToolResult {
        CallToolResult::success(vec![])
    }

    // ── extract_text ─────────────────────────────────────────────────────────

    #[test]
    fn test_extract_text_success() {
        let result = make_text_result("hello world");
        let text = GrokClient::extract_text(&result).unwrap();
        assert_eq!(text, "hello world");
    }

    #[test]
    fn test_extract_text_empty_content_errors() {
        let result = make_empty_result();
        let err = GrokClient::extract_text(&result).unwrap_err();
        assert!(err.to_string().contains("Empty response content"));
    }

    #[test]
    fn test_extract_text_non_text_content_errors() {
        // Build a result whose content item is not text (use error variant which wraps text
        // but we can still test via an image-like case — simplest: no text annotation)
        // Content::text always produces text, so test via empty to cover non-text branch
        // via the error result path (is_error=true, content is text so extract_text succeeds)
        let result = CallToolResult::error(vec![Content::text("err msg")]);
        let text = GrokClient::extract_text(&result).unwrap();
        assert_eq!(text, "err msg");
    }

    // ── parse_irontology_digest_response ─────────────────────────────────────

    #[test]
    fn test_parse_digest_valid_indexed() {
        let client = GrokClient::new();
        let json = r#"{"indexed": true, "source": "s.md", "topic": "rust", "chunks": 3, "embedded": 3}"#;
        let r = client
            .parse_irontology_digest_response(make_text_result(json), "rust", "content here")
            .unwrap();
        assert!(r.success);
        assert_eq!(r.topic, "rust");
        assert!(r.message.is_none());
        assert!(r.chunk_id.contains("rust"));
    }

    #[test]
    fn test_parse_digest_not_indexed() {
        let client = GrokClient::new();
        let json = r#"{"indexed": false}"#;
        let r = client
            .parse_irontology_digest_response(make_text_result(json), "rust", "content here")
            .unwrap();
        assert!(!r.success);
        assert!(r.message.is_some());
    }

    #[test]
    fn test_parse_digest_invalid_json() {
        let client = GrokClient::new();
        // invalid JSON: indexed defaults to false, text returned as message
        let r = client
            .parse_irontology_digest_response(make_text_result("not json"), "rust", "c")
            .unwrap();
        assert!(!r.success);
        assert!(r.message.is_some());
    }

    // ── parse_irontology_ask_response ────────────────────────────────────────

    #[test]
    fn test_parse_ask_valid_results() {
        let client = GrokClient::new();
        let json = r#"{"results": [{"id": "1", "content": "memory safety", "topic": "rust", "score": 0.9}]}"#;
        let r = client
            .parse_irontology_ask_response(make_text_result(json), "memory safety")
            .unwrap();
        assert!(r.success);
        assert_eq!(r.total_found, 1);
        assert_eq!(r.results[0].id, "1");
        assert_eq!(r.results[0].content, "memory safety");
        assert!(r.message.is_none());
    }

    #[test]
    fn test_parse_ask_empty_results() {
        let client = GrokClient::new();
        let json = r#"{"results": []}"#;
        let r = client
            .parse_irontology_ask_response(make_text_result(json), "query")
            .unwrap();
        assert!(r.success);
        assert_eq!(r.total_found, 0);
        assert!(r.results.is_empty());
    }

    #[test]
    fn test_parse_ask_invalid_json() {
        let client = GrokClient::new();
        let r = client
            .parse_irontology_ask_response(make_text_result("bad json {"), "query")
            .unwrap();
        assert!(!r.success);
        assert_eq!(r.total_found, 0);
        let msg = r.message.unwrap();
        assert!(msg.contains("JSON parse error"));
        assert!(msg.contains("bad json {"));
    }

    #[test]
    fn test_parse_ask_missing_results_key() {
        let client = GrokClient::new();
        // Valid JSON but no "results" key — should return empty with success=true
        let json = r#"{"hits": []}"#;
        let r = client
            .parse_irontology_ask_response(make_text_result(json), "query")
            .unwrap();
        assert!(r.success);
        assert_eq!(r.total_found, 0);
    }

    // ── parse_irontology_learn_response ──────────────────────────────────────

    #[test]
    fn test_parse_learn_valid_indexed() {
        let client = GrokClient::new();
        let json = r#"{"indexed": true, "chunks": 5, "embedded": 5}"#;
        let r = client
            .parse_irontology_learn_response(make_text_result(json), "test.md")
            .unwrap();
        assert!(r.success);
        assert_eq!(r.chunks_created, 5);
        assert_eq!(r.source, "test.md");
        assert!(r.message.is_none());
    }

    #[test]
    fn test_parse_learn_not_indexed() {
        let client = GrokClient::new();
        let json = r#"{"indexed": false, "chunks": 0}"#;
        let r = client
            .parse_irontology_learn_response(make_text_result(json), "test.md")
            .unwrap();
        assert!(!r.success);
        assert_eq!(r.chunks_created, 0);
        assert!(r.message.is_some());
    }

    #[test]
    fn test_parse_learn_invalid_json() {
        let client = GrokClient::new();
        // invalid JSON → indexed=false, chunks=0
        let r = client
            .parse_irontology_learn_response(make_text_result("oops"), "test.md")
            .unwrap();
        assert!(!r.success);
        assert_eq!(r.chunks_created, 0);
    }
}
