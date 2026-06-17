//! Content-type-aware router — detects type and dispatches to appropriate parser.
//!
//! Fills the explicit gap at grok.rs:164 ("URL fetching not yet implemented").

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
    client: reqwest::blocking::Client,
}

impl ContentRouter {
    pub fn new() -> Self {
        let client = reqwest::blocking::Client::builder()
            .timeout(Duration::from_secs(30))
            .redirect(reqwest::redirect::Policy::limited(5))
            .user_agent("b00t-assimilate/0.1")
            .build()
            .expect("failed to build HTTP client");
        Self { client }
    }

    /// Route a source (URL or file path) to the appropriate parser.
    pub fn route(&self, source: &str) -> Result<ParsedContent> {
        if source.starts_with("http://") || source.starts_with("https://") {
            self.route_url(source)
        } else if Path::new(source).exists() {
            self.route_file(source)
        } else {
            bail!("source '{source}' is neither a valid URL nor an existing file")
        }
    }

    /// Fetch a URL and parse based on Content-Type header + extension.
    fn route_url(&self, url: &str) -> Result<ParsedContent> {
        let resp = self
            .client
            .get(url)
            .send()
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
            .or_else(|| Self::detect_from_extension(url))
            .unwrap_or(ContentType::Unknown);

        let bytes = resp
            .bytes()
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

    #[test]
    fn test_route_file_markdown() {
        let router = ContentRouter::new();
        let result = router.route("Cargo.toml");
        assert!(result.is_ok());
        let parsed = result.unwrap();
        assert!(parsed.text.contains("[package]"));
    }
}
