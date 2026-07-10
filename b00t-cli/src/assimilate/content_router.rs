//! Content-type-aware router — detects type and dispatches to appropriate parser.
//!
//! Fills the explicit gap at grok.rs:164 ("URL fetching not yet implemented").
//! Domain-specific routing: arxiv.org → arxiv-mcp-server (not webfetch).

use crate::assimilate::domain_router;
use anyhow::{Result, anyhow, bail};
use std::path::Path;
use std::time::Duration;

/// Detected content type of a source.
#[derive(Debug, Clone, PartialEq)]
pub enum ContentType {
    Html,
    Pdf,
    Markdown,
    Text,
    Json,
    /// Image formats (PNG, JPEG, GIF, WebP, SVG)
    Image(String),      // MIME subtype e.g. "png"
    /// Audio formats (MP3, WAV, OGG, FLAC)
    Audio(String),      // MIME subtype e.g. "mpeg"
    /// Video formats (MP4, WebM, AVI)
    Video(String),      // MIME subtype e.g. "mp4"
    /// Generic binary (unknown or unclassified)
    Binary(String),     // file extension e.g. "bin"
    Unknown,
}

/// Parsed content from a URL or file.
#[derive(Debug, Clone)]
pub struct ParsedContent {
    pub text: String,
    pub content_type: ContentType,
    pub source_url: Option<String>,
}

/// Routes sources to appropriate parsers based on content-type detection.
pub struct ContentRouter {
    client: reqwest::Client,
}

impl ContentRouter {
    pub fn new() -> Self {
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(30))
            .redirect(reqwest::redirect::Policy::limited(5))
            .user_agent("b00t-assimilate/0.1")
            .build()
            .expect("failed to build HTTP client");
        Self { client }
    }

    /// Route a source (URL or file path) to the appropriate parser.
    /// 🤓 Domain-specific MCP routing: arxiv.org → arxiv-mcp-server (not webfetch)
    pub async fn route(&self, source: &str) -> Result<ParsedContent> {
        if source.starts_with("http://") || source.starts_with("https://") {
            // Check for institutional MCP handlers first
            if let Some((handler, resource_id)) = domain_router::extract_mcp_resource_id(source) {
                return self.route_via_mcp(&handler, &resource_id).await;
            }
            self.route_url(source).await
        } else if Path::new(source).exists() {
            self.route_file(source)
        } else {
            bail!("source '{source}' is neither a valid URL nor an existing file")
        }
    }

    /// Route through an MCP server for institutional domains.
    /// E.g., arxiv.org URLs → download_paper, huggingface.co → paper_search.
    /// HF_TOKEN env var is passed as Authorization header when available.
    async fn route_via_mcp(
        &self,
        handler: &domain_router::McpDomainHandler,
        resource_id: &str,
    ) -> Result<ParsedContent> {
        let mcp_url = match handler.mcp_server.as_str() {
            "arxiv-mcp-server" => std::env::var("ARXIV_MCP_URL")
                .unwrap_or_else(|_| "http://127.0.0.1:8000/mcp".to_string()),
            "huggingface" => "https://huggingface.co/mcp".to_string(),
            _ => bail!("unknown MCP server: {}", handler.mcp_server),
        };

        let mut request = self.client.post(&mcp_url);

        // Pass HF_TOKEN as Authorization header for HuggingFace
        if handler.mcp_server == "huggingface" {
            if let Ok(token) = std::env::var("HF_TOKEN") {
                request = request.header("Authorization", format!("Bearer {}", token));
            }
        }

        // Initialize MCP session
        let init_body = serde_json::json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "initialize",
            "params": {
                "protocolVersion": "2024-11-05",
                "capabilities": {},
                "clientInfo": {"name": "b00t", "version": "0.1"}
            }
        });

        let init_resp = request
            .try_clone()
            .unwrap()
            .header("Content-Type", "application/json")
            .header("Accept", "application/json, text/event-stream")
            .json(&init_body)
            .send()
            .await
            .map_err(|e| anyhow!("{} MCP init failed: {e}", handler.domain_label))?;

        let session_id = init_resp
            .headers()
            .get("Mcp-Session-Id")
            .and_then(|v| v.to_str().ok())
            .map(String::from)
            .ok_or_else(|| anyhow!("{} MCP: no session ID in init response", handler.domain_label))?;

        // Send initialized notification
        // 🤓 Some MCP servers require this before tools/call
        let _notify = request
            .try_clone()
            .unwrap()
            .header("Content-Type", "application/json")
            .header("Accept", "application/json, text/event-stream")
            .header("Mcp-Session-Id", &session_id)
            .json(&serde_json::json!({
                "jsonrpc": "2.0",
                "method": "notifications/initialized"
            }))
            .send()
            .await;

        // Call the fetch tool
        let call_body = serde_json::json!({
            "jsonrpc": "2.0",
            "id": 3,
            "method": "tools/call",
            "params": {
                "name": handler.fetch_tool,
                "arguments": { &handler.fetch_arg: resource_id }
            }
        });

        let call_resp = request
            .header("Content-Type", "application/json")
            .header("Accept", "application/json, text/event-stream")
            .header("Mcp-Session-Id", &session_id)
            .json(&call_body)
            .send()
            .await
            .map_err(|e| anyhow!("{} MCP fetch failed: {e}", handler.domain_label))?;

        let call_json: serde_json::Value = call_resp
            .json()
            .await
            .map_err(|e| anyhow!("{} MCP response parse failed: {e}", handler.domain_label))?;

        let text = call_json
            .pointer("/result/content/0/text")
            .and_then(|v| v.as_str())
            .unwrap_or_default()
            .to_string();

        Ok(ParsedContent {
            text,
            content_type: ContentType::Markdown,
            source_url: Some(format!("{}://{}", handler.domain_label.to_lowercase(), resource_id)),
        })
    }

    /// Fetch a URL and parse based on Content-Type header + extension.
    async fn route_url(&self, url: &str) -> Result<ParsedContent> {
        let resp = self
            .client
            .get(url)
            .send()
            .await
            .map_err(|e| anyhow!("fetch failed for '{url}': {e}"))?;

        if !resp.status().is_success() {
            bail!("HTTP {} for {url}", resp.status());
        }

        let content_type_header = resp
            .headers()
            .get(reqwest::header::CONTENT_TYPE)
            .and_then(|v| v.to_str().ok())
            .unwrap_or("")
            .to_string();

        let content_type = Self::detect_from_header(&content_type_header)
            .or_else(|| Self::detect_from_extension(&url))
            .unwrap_or(ContentType::Unknown);

        let bytes = resp
            .bytes()
            .await
            .map_err(|e| anyhow!("failed to read response body: {e}"))?;

        let text = match content_type {
            ContentType::Html => Self::strip_html(&String::from_utf8_lossy(&bytes)),
            ContentType::Pdf => {
                // PDFs need external extraction — defer to pdf_extractor
                bail!("PDF detected — use pdf_extractor::extract_pdf_url instead")
            }
            ContentType::Json => String::from_utf8_lossy(&bytes).to_string(),
            _ => String::from_utf8_lossy(&bytes).to_string(),
        };

        Ok(ParsedContent {
            text,
            content_type,
            source_url: Some(url.to_string()),
        })
    }

    /// Read a local file and parse based on extension.
    fn route_file(&self, path: &str) -> Result<ParsedContent> {
        let content_type = Self::detect_from_extension(path).unwrap_or(ContentType::Text);

        match content_type {
            ContentType::Pdf => {
                bail!("PDF detected — use pdf_extractor::extract_pdf_file instead")
            }
            ContentType::Html => {
                let raw = std::fs::read_to_string(path)
                    .map_err(|e| anyhow!("failed to read '{path}': {e}"))?;
                Ok(ParsedContent {
                    text: Self::strip_html(&raw),
                    content_type,
                    source_url: None,
                })
            }
            _ => {
                let text = std::fs::read_to_string(path)
                    .map_err(|e| anyhow!("failed to read '{path}': {e}"))?;
                Ok(ParsedContent {
                    text,
                    content_type,
                    source_url: None,
                })
            }
        }
    }

    /// Detect content type from HTTP Content-Type header.
    fn detect_from_header(header: &str) -> Option<ContentType> {
        let lower = header.to_lowercase();
        if lower.contains("text/html") || lower.contains("application/xhtml") {
            Some(ContentType::Html)
        } else if lower.contains("application/pdf") {
            Some(ContentType::Pdf)
        } else if lower.contains("text/markdown") || lower.contains("text/x-markdown") {
            Some(ContentType::Markdown)
        } else if lower.contains("application/json") {
            Some(ContentType::Json)
        } else if lower.contains("text/plain") {
            Some(ContentType::Text)
        } else if lower.starts_with("image/") {
            let subtype = lower.strip_prefix("image/").unwrap_or("unknown");
            Some(ContentType::Image(subtype.to_string()))
        } else if lower.starts_with("audio/") {
            let subtype = lower.strip_prefix("audio/").unwrap_or("unknown");
            Some(ContentType::Audio(subtype.to_string()))
        } else if lower.starts_with("video/") {
            let subtype = lower.strip_prefix("video/").unwrap_or("unknown");
            Some(ContentType::Video(subtype.to_string()))
        } else {
            None
        }
    }

    /// Detect content type from file extension.
    fn detect_from_extension(path: &str) -> Option<ContentType> {
        let lower = path.to_lowercase();
        if lower.ends_with(".html") || lower.ends_with(".htm") {
            Some(ContentType::Html)
        } else if lower.ends_with(".pdf") {
            Some(ContentType::Pdf)
        } else if lower.ends_with(".md") || lower.ends_with(".markdown") {
            Some(ContentType::Markdown)
        } else if lower.ends_with(".json") {
            Some(ContentType::Json)
        } else if lower.ends_with(".txt") || lower.ends_with(".text") {
            Some(ContentType::Text)
        } else if lower.ends_with(".png") {
            Some(ContentType::Image("png".into()))
        } else if lower.ends_with(".jpg") || lower.ends_with(".jpeg") {
            Some(ContentType::Image("jpeg".into()))
        } else if lower.ends_with(".gif") {
            Some(ContentType::Image("gif".into()))
        } else if lower.ends_with(".webp") {
            Some(ContentType::Image("webp".into()))
        } else if lower.ends_with(".svg") {
            Some(ContentType::Image("svg".into()))
        } else if lower.ends_with(".mp3") {
            Some(ContentType::Audio("mpeg".into()))
        } else if lower.ends_with(".wav") {
            Some(ContentType::Audio("wav".into()))
        } else if lower.ends_with(".ogg") {
            Some(ContentType::Audio("ogg".into()))
        } else if lower.ends_with(".mp4") {
            Some(ContentType::Video("mp4".into()))
        } else if lower.ends_with(".webm") {
            Some(ContentType::Video("webm".into()))
        } else if lower.ends_with(".bin") || lower.ends_with(".dat") {
            Some(ContentType::Binary(lower.rsplit('.').next().unwrap_or("bin").into()))
        } else {
            None
        }
    }

    /// Lightweight HTML tag stripping — removes tags, keeps text content.
    /// For complex HTML, the sm0l model can be used for semantic extraction.
    fn strip_html(html: &str) -> String {
        // Remove script/style blocks entirely (separate patterns — Rust regex has no backrefs)
        let script_re = regex::Regex::new(r"(?is)<script[^>]*>.*?</script>").unwrap();
        let style_re = regex::Regex::new(r"(?is)<style[^>]*>.*?</style>").unwrap();
        let without_scripts = script_re.replace_all(html, "");
        let without_scripts = style_re.replace_all(&without_scripts, "");
        // Remove all remaining tags
        let without_tags = regex::Regex::new(r"<[^>]+>")
            .unwrap()
            .replace_all(&without_scripts, "");
        // Decode common HTML entities
        without_tags
            .replace("&amp;", "&")
            .replace("&lt;", "<")
            .replace("&gt;", ">")
            .replace("&quot;", "\"")
            .replace("&#39;", "'")
            .replace("&nbsp;", " ")
            // Collapse whitespace
            .split_whitespace()
            .collect::<Vec<_>>()
            .join(" ")
    }
}

impl Default for ContentRouter {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_detect_from_extension() {
        assert_eq!(
            ContentRouter::detect_from_extension("doc.pdf"),
            Some(ContentType::Pdf)
        );
        assert_eq!(
            ContentRouter::detect_from_extension("page.html"),
            Some(ContentType::Html)
        );
        assert_eq!(
            ContentRouter::detect_from_extension("README.md"),
            Some(ContentType::Markdown)
        );
        assert_eq!(
            ContentRouter::detect_from_extension("data.json"),
            Some(ContentType::Json)
        );
        assert_eq!(
            ContentRouter::detect_from_extension("notes.txt"),
            Some(ContentType::Text)
        );
        assert_eq!(
            ContentRouter::detect_from_extension("unknown.xyz"),
            None
        );
    }

    #[test]
    fn test_detect_from_header() {
        assert_eq!(
            ContentRouter::detect_from_header("text/html; charset=utf-8"),
            Some(ContentType::Html)
        );
        assert_eq!(
            ContentRouter::detect_from_header("application/pdf"),
            Some(ContentType::Pdf)
        );
    }

    #[test]
    fn test_strip_html() {
        let html = r#"<html><body><script>alert(1)</script><h1>Title</h1><p>Hello &amp; world</p></body></html>"#;
        let text = ContentRouter::strip_html(html);
        assert!(text.contains("Title"));
        assert!(text.contains("Hello & world"));
        assert!(!text.contains("alert"));
        assert!(!text.contains("<"));
    }

    #[tokio::test]
    async fn test_route_file_markdown() {
        let router = ContentRouter::new();
        let result = router.route("Cargo.toml").await;
        assert!(result.is_ok());
        let parsed = result.unwrap();
        assert!(parsed.text.contains("[package]"));
    }
}
