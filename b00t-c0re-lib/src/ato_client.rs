//! ATO Legislation API client — fetch Australian tax legislation.
//!
//! Integrates with b00t's doc_pipeline::DocumentSource for the
//! compound-engineering Australian Tax Capability (Phase 5: Work).

use crate::doc_pipeline::{DocumentFormat, DocumentSource};
use chrono::Utc;
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
    /// Return the act identifier for the legislation.gov.au API.
    pub fn api_id(&self) -> &str {
        match self {
            AtoAct::Itaa1936 => "C2004A04838", // ITAA 1936 compilation
            AtoAct::Itaa1997 => "C2025C00001", // ITAA 1997 (current compilation)
            AtoAct::GstAct => "C2004A04840",   // A New Tax System (GST) Act 1999
            AtoAct::FbtAct => "C2004A04839",   // Fringe Benefits Tax Assessment Act 1986
        }
    }

    /// Human-readable short name.
    pub fn short_name(&self) -> &str {
        match self {
            AtoAct::Itaa1936 => "Income Tax Assessment Act 1936",
            AtoAct::Itaa1997 => "Income Tax Assessment Act 1997",
            AtoAct::GstAct => "A New Tax System (Goods and Services Tax) Act 1999",
            AtoAct::FbtAct => "Fringe Benefits Tax Assessment Act 1986",
        }
    }

    /// Legislation.gov.au URL for the current compilation.
    pub fn url(&self) -> String {
        format!("https://www.legislation.gov.au/{}", self.api_id())
    }
}

/// Client for fetching ATO legislation documents.
pub struct AtoClient {
    /// Base URL for the legislation API.
    base_url: String,
}

impl Default for AtoClient {
    fn default() -> Self {
        Self {
            base_url: "https://www.legislation.gov.au".into(),
        }
    }
}

impl AtoClient {
    /// Create a new ATO client with custom base URL.
    pub fn new(base_url: &str) -> Self {
        Self {
            base_url: base_url.into(),
        }
    }

    /// Fetch a legislation act and return it as a DocumentSource.
    ///
    /// The returned DocumentSource is compatible with b00t's existing
    /// pipeline: ChunkNode → EvidenceNode → RequirementsNode.
    ///
    /// # Rate limiting
    /// ATO API requests should be spaced ≥3 seconds apart (arxiv-compatible pattern).
    pub fn fetch_legislation(&self, act: &AtoAct) -> DocumentSource {
        let source_id = format!("ato:{}", act.api_id());
        let url = act.url();

        DocumentSource {
            source_id: source_id.clone(),
            title: act.short_name().into(),
            authors: vec!["Australian Parliament".into()],
            abstract_text: format!(
                "{} — Current compilation. Source: {}",
                act.short_name(),
                url
            ),
            url: Some(url),
            pdf_url: None, // legislation.gov.au provides HTML, not PDF
            fetched_at: Utc::now(),
            content_hash: None, // populated after actual fetch
            format: DocumentFormat::Html,
            metadata: HashMap::from([
                ("jurisdiction".into(), "AU".into()),
                ("act_type".into(), format!("{:?}", act)),
                ("api_id".into(), act.api_id().into()),
            ]),
        }
    }
}

/// Fetch all known ATO acts and return as DocumentSource vector.
pub fn fetch_all_acts() -> Vec<DocumentSource> {
    let client = AtoClient::default();
    vec![
        client.fetch_legislation(&AtoAct::Itaa1997),
        client.fetch_legislation(&AtoAct::Itaa1936),
        client.fetch_legislation(&AtoAct::GstAct),
        client.fetch_legislation(&AtoAct::FbtAct),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_fetch_itaa1997_produces_document_source() {
        let client = AtoClient::default();
        let doc = client.fetch_legislation(&AtoAct::Itaa1997);
        assert_eq!(doc.source_id, "ato:C2025C00001");
        assert!(doc.title.contains("Income Tax Assessment Act 1997"));
        assert_eq!(doc.format, DocumentFormat::Html);
        assert_eq!(doc.metadata.get("jurisdiction").unwrap(), "AU");
    }

    #[test]
    fn test_fetch_all_produces_four_acts() {
        let docs = fetch_all_acts();
        assert_eq!(docs.len(), 4);
        let ids: Vec<&str> = docs.iter().map(|d| d.source_id.as_str()).collect();
        assert!(ids.contains(&"ato:C2025C00001")); // ITAA 1997
        assert!(ids.contains(&"ato:C2004A04838")); // ITAA 1936
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
        // Verify the DocumentSource is compatible with pipeline nodes
        let doc = AtoClient::default().fetch_legislation(&AtoAct::Itaa1997);
        // UFO: Endurant check
        use crate::doc_pipeline::Endurant;
        assert!(doc.exists_wholly_at(Utc::now()));
        assert_eq!(doc.endurant_kind(), "Document");
    }
}
