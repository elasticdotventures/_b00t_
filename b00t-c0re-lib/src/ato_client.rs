//! ATO Legislation API client — fetch Australian tax legislation.
//!
//! Integrates with b00t's doc_pipeline::DocumentSource for the
//! compound-engineering Australian Tax Capability (Phase 5: Work).

use crate::doc_pipeline::{DocumentFormat, DocumentSource};
use chrono::Utc;
use sha2::{Digest, Sha256};
use std::collections::HashMap;

/// Known Australian tax legislation acts.
#[derive(Debug, Clone)]
pub enum AtoAct {
    Itaa1936,
    Itaa1997,
    GstAct,
    FbtAct,
}

impl AtoAct {
    pub fn api_id(&self) -> &str {
        match self {
            AtoAct::Itaa1936 => "C2004A04838",
            AtoAct::Itaa1997 => "C2025C00001",
            AtoAct::GstAct => "C2004A04840",
            AtoAct::FbtAct => "C2004A04839",
        }
    }

    pub fn short_name(&self) -> &str {
        match self {
            AtoAct::Itaa1936 => "Income Tax Assessment Act 1936",
            AtoAct::Itaa1997 => "Income Tax Assessment Act 1997",
            AtoAct::GstAct => "A New Tax System (Goods and Services Tax) Act 1999",
            AtoAct::FbtAct => "Fringe Benefits Tax Assessment Act 1986",
        }
    }

    pub fn url(&self) -> String {
        format!("https://www.legislation.gov.au/{}", self.api_id())
    }
}

pub struct AtoClient {
    base_url: String,
}

impl Default for AtoClient {
    fn default() -> Self {
        Self { base_url: "https://www.legislation.gov.au".into() }
    }
}

impl AtoClient {
    pub fn new(base_url: &str) -> Self {
        Self { base_url: base_url.into() }
    }

    /// Fetch legislation HTML and return as DocumentSource.
    /// Populates abstract_text with preamble + section count, content_hash with SHA-256 of raw HTML.
    /// Rate-limit: caller is responsible for ≥3s between requests.
    pub fn fetch_legislation(&self, act: &AtoAct) -> Result<DocumentSource, reqwest::Error> {
        let url = format!("{}/{}", self.base_url, act.api_id());
        let html = reqwest::blocking::get(&url)?.text()?;

        let hash = hex::encode(Sha256::digest(html.as_bytes()));

        // Extract preamble text for abstract_text (first <p> inside main content)
        let preamble = extract_preamble(&html)
            .unwrap_or_else(|| format!("{} — {}", act.short_name(), url));

        Ok(DocumentSource {
            source_id: format!("ato:{}", act.api_id()),
            title: act.short_name().into(),
            authors: vec!["Australian Parliament".into()],
            abstract_text: preamble,
            url: Some(url),
            pdf_url: None,
            fetched_at: Utc::now(),
            content_hash: Some(hash),
            format: DocumentFormat::Html,
            metadata: HashMap::from([
                ("jurisdiction".into(), "AU".into()),
                ("act_type".into(), format!("{:?}", act)),
                ("api_id".into(), act.api_id().into()),
            ]),
        })
    }

    /// Build DocumentSource from metadata only (no HTTP) — for tests and offline use.
    pub fn source_stub(&self, act: &AtoAct) -> DocumentSource {
        DocumentSource {
            source_id: format!("ato:{}", act.api_id()),
            title: act.short_name().into(),
            authors: vec!["Australian Parliament".into()],
            abstract_text: format!("{} — Current compilation.", act.short_name()),
            url: Some(act.url()),
            pdf_url: None,
            fetched_at: Utc::now(),
            content_hash: None,
            format: DocumentFormat::Html,
            metadata: HashMap::from([
                ("jurisdiction".into(), "AU".into()),
                ("act_type".into(), format!("{:?}", act)),
                ("api_id".into(), act.api_id().into()),
                ("stub".into(), "true".into()),
            ]),
        }
    }
}

/// Extract preamble text from legislation HTML (first substantive paragraph).
fn extract_preamble(html: &str) -> Option<String> {
    use scraper::{Html, Selector};
    let doc = Html::parse_document(html);
    // legislation.gov.au wraps content in .legis-body or .document-main
    let candidates = [
        ".legis-body p",
        ".document-main p",
        "article p",
        "main p",
        "p",
    ];
    for sel_str in &candidates {
        if let Ok(sel) = Selector::parse(sel_str) {
            if let Some(el) = doc.select(&sel).next() {
                let text: String = el.text().collect::<Vec<_>>().join(" ").trim().to_string();
                if text.len() > 20 {
                    return Some(text);
                }
            }
        }
    }
    None
}

/// Fetch all known ATO acts (stubs, no HTTP) — for use in pipeline scaffolding.
pub fn stub_all_acts() -> Vec<DocumentSource> {
    let client = AtoClient::default();
    [AtoAct::Itaa1997, AtoAct::Itaa1936, AtoAct::GstAct, AtoAct::FbtAct]
        .iter()
        .map(|a| client.source_stub(a))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_stub_itaa1997_produces_document_source() {
        let client = AtoClient::default();
        let doc = client.source_stub(&AtoAct::Itaa1997);
        assert_eq!(doc.source_id, "ato:C2025C00001");
        assert!(doc.title.contains("Income Tax Assessment Act 1997"));
        assert_eq!(doc.format, DocumentFormat::Html);
        assert_eq!(doc.metadata.get("jurisdiction").unwrap(), "AU");
        assert_eq!(doc.metadata.get("stub").unwrap(), "true");
    }

    #[test]
    fn test_stub_all_produces_four_acts() {
        let docs = stub_all_acts();
        assert_eq!(docs.len(), 4);
        let ids: Vec<&str> = docs.iter().map(|d| d.source_id.as_str()).collect();
        assert!(ids.contains(&"ato:C2025C00001"));
        assert!(ids.contains(&"ato:C2004A04838"));
    }

    #[test]
    fn test_act_urls_are_valid() {
        for act in &[AtoAct::Itaa1997, AtoAct::Itaa1936, AtoAct::GstAct, AtoAct::FbtAct] {
            let url = act.url();
            assert!(url.starts_with("https://www.legislation.gov.au/"));
            assert!(url.contains(act.api_id()));
        }
    }

    #[test]
    fn test_document_source_integrates_with_pipeline() {
        let doc = AtoClient::default().source_stub(&AtoAct::Itaa1997);
        use crate::doc_pipeline::Endurant;
        assert!(doc.exists_wholly_at(Utc::now()));
        assert_eq!(doc.endurant_kind(), "Document");
    }

    #[test]
    fn test_extract_preamble_from_mock_html() {
        let html = r#"<html><body><main><p>This is the preamble text of the act.</p></main></body></html>"#;
        let result = extract_preamble(html);
        assert!(result.is_some());
        assert!(result.unwrap().contains("preamble text"));
    }
}
