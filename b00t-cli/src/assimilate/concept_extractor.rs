//! Concept + link extraction via sm0l model.
//!
//! Uses the model registry to resolve the sm0l tier endpoint,
//! sends a structured prompt, and parses JSON response.

use anyhow::{Result, anyhow, bail};
use serde::{Deserialize, Serialize};
use std::time::Duration;

/// A single extracted concept.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Concept {
    pub name: String,
    pub description: String,
    #[serde(default)]
    pub confidence: f32,
    #[serde(default)]
    pub is_advanced: bool,
}

/// An extracted hyperlink.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExtractedLink {
    pub url: String,
    #[serde(default)]
    pub anchor_text: String,
}

/// Result of concept extraction.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConceptExtraction {
    pub concepts: Vec<Concept>,
    pub links: Vec<ExtractedLink>,
}

/// Extracts concepts and links from text using a sm0l model.
pub struct ConceptExtractor {
    client: reqwest::blocking::Client,
}

const EXTRACTION_PROMPT_TEMPLATE: &str = r#"Analyze the following content and extract structured information.

Return ONLY valid JSON (no markdown fences, no commentary) with this schema:
{{
  "concepts": [
    {{"name": "<term>", "description": "<1-sentence explanation>", "confidence": 0.0-1.0, "is_advanced": false}}
  ],
  "links": [
    {{"url": "<absolute URL>", "anchor_text": "<link text>"}}
  ]
}}

Rules:
- Extract 5-20 primary concepts (is_advanced: false) — key terms, entities, technologies.
- Extract 0-10 advanced concepts (is_advanced: true) — relationships, patterns, abstractions.
- Extract links as absolute URLs (resolve relative URLs against the source).
- Confidence: 1.0 = explicitly defined, 0.7 = clearly referenced, 0.3 = peripheral.
- Limit to the most important concepts; quality over quantity.

Content:
---
{content}
---"#;

/// Truncate content to fit within sm0l context window (~4K tokens ≈ 16K chars).
const MAX_CONTENT_CHARS: usize = 16_000;

impl ConceptExtractor {
    pub fn new() -> Self {
        let client = reqwest::blocking::Client::builder()
            .timeout(Duration::from_secs(60))
            .build()
            .expect("failed to build HTTP client");
        Self { client }
    }

    /// Extract concepts and links from text.
    pub fn extract(&self, text: &str) -> Result<ConceptExtraction> {
        let (base_url, model) = self.resolve_endpoint()?;

        let truncated = if text.len() > MAX_CONTENT_CHARS {
            eprintln!("⚠️  content truncated to {MAX_CONTENT_CHARS} chars for sm0l context");
            &text[..MAX_CONTENT_CHARS]
        } else {
            text
        };

        let prompt = EXTRACTION_PROMPT_TEMPLATE.replace("{content}", truncated);

        let body = serde_json::json!({
            "model": model,
            "messages": [{"role": "user", "content": prompt}],
            "max_tokens": 1024,
            "temperature": 0.1,
        });

        let url = format!(
            "{}/v1/chat/completions",
            base_url.trim_end_matches('/').trim_end_matches("/v1")
        );

        let resp = self
            .client
            .post(&url)
            .json(&body)
            .send()
            .map_err(|e| anyhow!("sm0l request failed: {e}"))?;

        if !resp.status().is_success() {
            bail!("sm0l HTTP {}: {}", resp.status(), resp.text().unwrap_or_default());
        }

        let resp_json: serde_json::Value =
            resp.json().map_err(|e| anyhow!("failed to parse sm0l response: {e}"))?;

        let content = resp_json["choices"][0]["message"]["content"]
            .as_str()
            .ok_or_else(|| anyhow!("sm0l response missing content"))?;

        self.parse_response(content)
    }

    /// Resolve sm0l tier endpoint from model registry.
    /// Falls back to ch0nky tier, then localhost:8001.
    fn resolve_endpoint(&self) -> Result<(String, String)> {
        // Try sm0l tier first
        if let Some((base, model)) = crate::model_registry::resolve_tier_endpoint("sm0l") {
            return Ok((base, model));
        }
        // Fall back to ch0nky tier
        if let Some((base, model)) = crate::model_registry::resolve_tier_endpoint("ch0nky") {
            eprintln!("⚠️  no sm0l-tier model, using ch0nky");
            return Ok((base, model));
        }
        // Last resort: localhost
        eprintln!("⚠️  no registry model, trying localhost:8001");
        Ok((
            "http://localhost:8001".to_string(),
            std::env::var("B00T_SM0L_MODEL").unwrap_or_else(|_| "ch0nky".to_string()),
        ))
    }

    /// Parse the sm0l model's JSON response into ConceptExtraction.
    fn parse_response(&self, content: &str) -> Result<ConceptExtraction> {
        // Strip markdown code fences if present
        let cleaned = content
            .trim()
            .strip_prefix("```json")
            .or_else(|| content.trim().strip_prefix("```"))
            .unwrap_or(content)
            .trim()
            .strip_suffix("```")
            .unwrap_or(content)
            .trim();

        // Try direct parse
        if let Ok(extraction) = serde_json::from_str::<ConceptExtraction>(cleaned) {
            return Ok(extraction);
        }

        // Try to find JSON object within the response
        let start = cleaned.find('{');
        let end = cleaned.rfind('}');
        if let (Some(s), Some(e)) = (start, end) {
            if let Ok(extraction) = serde_json::from_str::<ConceptExtraction>(&cleaned[s..=e]) {
                return Ok(extraction);
            }
        }

        // Fallback: empty extraction
        eprintln!("⚠️  failed to parse sm0l concept JSON — returning empty");
        Ok(ConceptExtraction {
            concepts: vec![],
            links: vec![],
        })
    }
}

impl Default for ConceptExtractor {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_clean_json() {
        let json = r#"{"concepts":[{"name":"Rust","description":"Systems language","confidence":0.9,"is_advanced":false}],"links":[]}"#;
        let extractor = ConceptExtractor::new();
        let result = extractor.parse_response(json).unwrap();
        assert_eq!(result.concepts.len(), 1);
        assert_eq!(result.concepts[0].name, "Rust");
        assert_eq!(result.concepts[0].confidence, 0.9);
    }

    #[test]
    fn test_parse_markdown_fenced_json() {
        let json = r#"```json
{"concepts":[{"name":"TRIZ","description":"Inventive problem solving","confidence":0.8,"is_advanced":true}],"links":[{"url":"https://example.com","anchor_text":"ref"}]}
```"#;
        let extractor = ConceptExtractor::new();
        let result = extractor.parse_response(json).unwrap();
        assert_eq!(result.concepts.len(), 1);
        assert!(result.concepts[0].is_advanced);
        assert_eq!(result.links.len(), 1);
    }

    #[test]
    fn test_parse_with_preamble() {
        let json = r#"Here are the concepts:
{"concepts":[{"name":"Ockham","description":"Simplest explanation","confidence":0.7,"is_advanced":false}],"links":[]}
Hope this helps!"#;
        let extractor = ConceptExtractor::new();
        let result = extractor.parse_response(json).unwrap();
        assert_eq!(result.concepts.len(), 1);
    }

    #[test]
    fn test_parse_empty_response() {
        let json = "I cannot analyze this";
        let extractor = ConceptExtractor::new();
        let result = extractor.parse_response(json).unwrap();
        assert!(result.concepts.is_empty());
        assert!(result.links.is_empty());
    }
}
