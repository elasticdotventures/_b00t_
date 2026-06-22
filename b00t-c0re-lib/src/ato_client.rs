//! ATO legislation client
//!
//! Fetches legislation documents from the Australian Taxation Office
//! and returns them as DocumentSource for RAG ingestion.

use crate::rag::{DocumentSource, LoaderType};
use anyhow::{Context, Result};

/// An ATO legislation act
#[derive(Debug, Clone)]
pub struct AtoAct {
    /// Human-readable name of the act
    pub name: String,
    /// URL to the legislation document
    pub url: String,
}

impl AtoAct {
    /// Create a new ATO act reference
    pub fn new(name: impl Into<String>, url: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            url: url.into(),
        }
    }

    /// Get the URL for this act
    pub fn url(&self) -> &str {
        &self.url
    }
}

/// Fetch the legislation document for an ATO act
///
/// Performs an HTTP GET to the act's URL, checks for a successful response,
/// and returns the document as a DocumentSource with the content stored
/// in metadata.
pub fn fetch_legislation(act: &AtoAct) -> Result<DocumentSource> {
    let response = reqwest::blocking::get(act.url())
        .context(format!("Failed to fetch legislation from {}", act.url()))?;

    if !response.status().is_success() {
        anyhow::bail!(
            "HTTP {} fetching legislation from {}",
            response.status(),
            act.url()
        );
    }

    let body = response
        .text()
        .context("Failed to read legislation response body")?;

    let mut metadata = std::collections::HashMap::new();
    metadata.insert("content".to_string(), body);
    metadata.insert("act_name".to_string(), act.name.clone());

    Ok(DocumentSource {
        source: act.url().to_string(),
        loader_type: Some(LoaderType::Url),
        topic: format!("legislation-{}", act.name.to_lowercase().replace(' ', "-")),
        metadata: Some(metadata),
    })
}
