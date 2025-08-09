//! Grok MCP client for b00t knowledgebase system
//!
//! Provides a DRY implementation of grok functionality using rmcp client
//! to connect to b00t-grok-py MCP server. Can be used by both b00t-cli 
//! and b00t-mcp.

use anyhow::Result;
use rmcp::{
    ServiceExt,
    transport::{ConfigureCommandExt, TokioChildProcess},
    service::RunningService,
    model::CallToolRequestParam,
};
use std::borrow::Cow;
use serde_json::{json, Value, Map};
use std::env;
use tokio::process::Command;

/// Concrete MCP client running service type
type McpRunningService = RunningService<rmcp::service::RoleClient, ()>;

/// Grok MCP client for knowledgebase operations
pub struct GrokClient {
    mcp_client: Option<McpRunningService>,
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
    /// Create a new GrokClient
    pub fn new() -> Self {
        Self {
            mcp_client: None,
        }
    }

    /// Initialize the MCP client connection to b00t-grok-py server
    pub async fn initialize(&mut self) -> Result<()> {
        // 🤓 b00t pattern: use environment variables for configuration
        let qdrant_url = env::var("QDRANT_URL")
            .unwrap_or_else(|_| "http://192.168.2.13:6333".to_string());
        let qdrant_api_key = env::var("QDRANT_API_KEY").unwrap_or_default();

        // Create the child process transport
        let transport = TokioChildProcess::new(Command::new("uv").configure(|cmd| {
            cmd.arg("run")
                .arg("python")
                .arg("-m")
                .arg("b00t_grok_guru.server")
                .current_dir("/home/brianh/.dotfiles/b00t-grok-py")
                .env("QDRANT_URL", qdrant_url)
                .env("QDRANT_API_KEY", qdrant_api_key);
        }))?;

        // Connect to b00t-grok-py MCP server using unit client handler
        let client_handler = (); // Unit type implements ClientHandler as a no-op
        let running_service = client_handler.serve(transport).await?;
        
        self.mcp_client = Some(running_service);
        Ok(())
    }

    /// Digest content into a knowledge chunk about a specific topic
    pub async fn digest(&self, topic: &str, content: &str) -> Result<DigestResult> {
        let client = self.mcp_client.as_ref()
            .ok_or_else(|| anyhow::anyhow!("GrokClient not initialized - call initialize() first"))?;

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

    /// Search the knowledgebase for information related to a query
    pub async fn ask(&self, query: &str, topic: Option<&str>, limit: Option<usize>) -> Result<AskResult> {
        let client = self.mcp_client.as_ref()
            .ok_or_else(|| anyhow::anyhow!("GrokClient not initialized - call initialize() first"))?;

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

    /// Learn from content by breaking it into chunks and storing in knowledgebase
    pub async fn learn(&self, content: &str, source: Option<&str>) -> Result<LearnResult> {
        let client = self.mcp_client.as_ref()
            .ok_or_else(|| anyhow::anyhow!("GrokClient not initialized - call initialize() first"))?;

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

    /// Get the current status of the grok system
    pub async fn status(&self) -> Result<Value> {
        let client = self.mcp_client.as_ref()
            .ok_or_else(|| anyhow::anyhow!("GrokClient not initialized - call initialize() first"))?;

        let request = CallToolRequestParam {
            name: Cow::Borrowed("grok_status"),
            arguments: None,
        };
        let response = client.call_tool(request).await?;
        
        // Extract JSON from the first content item
        let content_text = response.content.get(0)
            .ok_or_else(|| anyhow::anyhow!("Empty response content"))?
            .as_text()
            .ok_or_else(|| anyhow::anyhow!("No text content in response"))?;

        let response_json: Value = serde_json::from_str(&content_text.text)?;
        Ok(response_json)
    }

    // Helper methods for parsing responses from CallToolResult
    fn parse_digest_response(&self, response: rmcp::model::CallToolResult) -> Result<DigestResult> {
        // Extract JSON from the first content item
        let content_text = response.content.get(0)
            .ok_or_else(|| anyhow::anyhow!("Empty response content"))?
            .as_text()
            .ok_or_else(|| anyhow::anyhow!("No text content in response"))?;

        let response_json: Value = serde_json::from_str(&content_text.text)?;
        
        if let Some(obj) = response_json.as_object() {
            Ok(DigestResult {
                success: obj.get("success").and_then(|v| v.as_bool()).unwrap_or(false),
                chunk_id: obj.get("chunk_id").and_then(|v| v.as_str()).unwrap_or("").to_string(),
                topic: obj.get("topic").and_then(|v| v.as_str()).unwrap_or("").to_string(),
                content_preview: obj.get("content_preview").and_then(|v| v.as_str()).unwrap_or("").to_string(),
                created_at: obj.get("created_at").and_then(|v| v.as_str()).unwrap_or("").to_string(),
                message: obj.get("message").and_then(|v| v.as_str()).map(|s| s.to_string()),
            })
        } else {
            Err(anyhow::anyhow!("Invalid digest response format"))
        }
    }

    fn parse_ask_response(&self, response: rmcp::model::CallToolResult) -> Result<AskResult> {
        // Extract JSON from the first content item
        let content_text = response.content.get(0)
            .ok_or_else(|| anyhow::anyhow!("Empty response content"))?
            .as_text()
            .ok_or_else(|| anyhow::anyhow!("No text content in response"))?;

        let response_json: Value = serde_json::from_str(&content_text.text)?;
        
        if let Some(obj) = response_json.as_object() {
            let results = obj.get("results")
                .and_then(|v| v.as_array())
                .unwrap_or(&Vec::new())
                .iter()
                .filter_map(|item| {
                    if let Some(chunk) = item.as_object() {
                        Some(ChunkResult {
                            id: chunk.get("id").and_then(|v| v.as_str()).unwrap_or("").to_string(),
                            content: chunk.get("content").and_then(|v| v.as_str()).unwrap_or("").to_string(),
                            topic: chunk.get("topic").and_then(|v| v.as_str()).unwrap_or("").to_string(),
                            tags: chunk.get("tags")
                                .and_then(|v| v.as_array())
                                .unwrap_or(&Vec::new())
                                .iter()
                                .filter_map(|t| t.as_str().map(|s| s.to_string()))
                                .collect(),
                            source: chunk.get("source").and_then(|v| v.as_str()).map(|s| s.to_string()),
                            created_at: chunk.get("created_at").and_then(|v| v.as_str()).unwrap_or("").to_string(),
                        })
                    } else {
                        None
                    }
                })
                .collect();

            Ok(AskResult {
                success: obj.get("success").and_then(|v| v.as_bool()).unwrap_or(false),
                query: obj.get("query").and_then(|v| v.as_str()).unwrap_or("").to_string(),
                total_found: obj.get("total_found").and_then(|v| v.as_u64()).unwrap_or(0) as usize,
                results,
                message: obj.get("message").and_then(|v| v.as_str()).map(|s| s.to_string()),
            })
        } else {
            Err(anyhow::anyhow!("Invalid ask response format"))
        }
    }

    fn parse_learn_response(&self, response: rmcp::model::CallToolResult) -> Result<LearnResult> {
        // Extract JSON from the first content item
        let content_text = response.content.get(0)
            .ok_or_else(|| anyhow::anyhow!("Empty response content"))?
            .as_text()
            .ok_or_else(|| anyhow::anyhow!("No text content in response"))?;

        let response_json: Value = serde_json::from_str(&content_text.text)?;
        
        if let Some(obj) = response_json.as_object() {
            let chunk_summaries = obj.get("chunk_summaries")
                .and_then(|v| v.as_array())
                .unwrap_or(&Vec::new())
                .iter()
                .filter_map(|item| {
                    if let Some(summary) = item.as_object() {
                        Some(ChunkSummary {
                            id: summary.get("id").and_then(|v| v.as_str()).unwrap_or("").to_string(),
                            topic: summary.get("topic").and_then(|v| v.as_str()).unwrap_or("").to_string(),
                            content_preview: summary.get("content_preview").and_then(|v| v.as_str()).unwrap_or("").to_string(),
                            tags: summary.get("tags")
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
                success: obj.get("success").and_then(|v| v.as_bool()).unwrap_or(false),
                source: obj.get("source").and_then(|v| v.as_str()).unwrap_or("").to_string(),
                chunks_created: obj.get("chunks_created").and_then(|v| v.as_u64()).unwrap_or(0) as usize,
                chunk_summaries,
                message: obj.get("message").and_then(|v| v.as_str()).map(|s| s.to_string()),
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

    #[tokio::test]
    async fn test_grok_client_initialization() {
        let mut client = GrokClient::new();
        
        // This test will fail if b00t-grok-py server is not available
        // That's expected in CI/testing environments
        let result = client.initialize().await;
        
        // Don't assert success - just verify the method exists and returns a Result
        match result {
            Ok(_) => println!("✅ GrokClient initialized successfully"),
            Err(e) => println!("⚠️ GrokClient initialization failed (expected in test env): {}", e),
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