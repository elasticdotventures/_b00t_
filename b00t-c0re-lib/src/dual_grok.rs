//! Dual-backend grok client — fan-out across raglite + irontology
//!
//! # Default behavior
//! `GrokBackend::Both` fans out `tokio::join!` to raglite AND irontology.
//! Each backend can fail independently — partial results surfaced with warnings.
//! This makes grok resilient: if raglite Python subprocess is unavailable,
//! irontology still serves; vice versa.
//!
//! # Backend selection
//! CLI flag `--rag` maps to `GrokBackend`:
//!   absent            → `Both` (default — fan-out)
//!   `--rag`           → `Both` (backward compat shorthand)
//!   `--rag=raglite`   → `Raglite`
//!   `--rag=irontology`→ `Irontology`
//!   `--rag=both`      → `Both`
//!
//! 🤓 Legacy `--rag=raglight` (old spelling) still maps to Raglite for compat

use anyhow::Result;
use serde::{Deserialize, Serialize};

use crate::{
    irontology_bridge::IrontologyBridgeClient,
    rag::{DocumentSource, LoaderType, RagLightConfig, RagLightManager},
};

// ── Backend selector ─────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum GrokBackend {
    /// RAGLight Python subprocess only
    Raglite,
    /// Irontology NeumannStore only
    Irontology,
    /// Fan-out to both (default)
    Both,
}

impl GrokBackend {
    /// Parse from `--rag` flag value. Returns `Both` for absent/empty.
    pub fn from_flag(raw: Option<&str>) -> Result<Self> {
        match raw {
            None | Some("both") | Some("") => Ok(Self::Both),
            Some("raglite") | Some("raglight") | Some("rag-light") | Some("rag_light") => {
                Ok(Self::Raglite)
            }
            Some("irontology") | Some("iron") => Ok(Self::Irontology),
            Some(other) => Err(anyhow::anyhow!(
                "Unknown --rag backend '{}'. Valid: raglite, irontology, both",
                other
            )),
        }
    }

    pub fn display_name(self) -> &'static str {
        match self {
            Self::Raglite => "RAGLight",
            Self::Irontology => "Irontology",
            Self::Both => "RAGLight+Irontology",
        }
    }
}

// ── Merged result types ───────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DualIngestResult {
    pub topic: String,
    pub backend: String,
    /// raglite job_id (if raglite was used)
    pub raglite_job_id: Option<String>,
    /// irontology subject prefix (if irontology was used)
    pub irontology_subject: Option<String>,
    pub raglite_ok: bool,
    pub irontology_ok: bool,
    pub warnings: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DualQueryResult {
    pub query: String,
    pub topic: Option<String>,
    pub total_found: usize,
    pub items: Vec<DualQueryItem>,
    pub raglite_ok: bool,
    pub irontology_ok: bool,
    pub warnings: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DualQueryItem {
    pub backend: String,
    pub content: String,
    pub topic: String,
    pub tags: Vec<String>,
    pub score: f32,
}

// ── Dual client ───────────────────────────────────────────────────────────────

/// Fan-out grok client dispatching to raglite and/or irontology
pub struct DualGrokClient {
    // 🤓 both backends are lazily initialized; failure on init is non-fatal
    iron: Option<IrontologyBridgeClient>,
}

impl DualGrokClient {
    /// Create with irontology namespace = "b00t-grok"
    pub fn new() -> Self {
        let iron = IrontologyBridgeClient::new("b00t-grok")
            .map_err(|e| {
                tracing::warn!("IrontologyBridgeClient init failed (non-fatal): {}", e);
                e
            })
            .ok();
        Self { iron }
    }

    /// Ingest content to the specified backend(s)
    ///
    /// Returns a `DualIngestResult`. Partial failure is surfaced as warnings,
    /// not as a hard error, so the operation is resilient to one backend being down.
    pub async fn ingest(
        &mut self,
        topic: &str,
        content: &str,
        backend: GrokBackend,
    ) -> Result<DualIngestResult> {
        let mut result = DualIngestResult {
            topic: topic.to_string(),
            backend: backend.display_name().to_string(),
            raglite_job_id: None,
            irontology_subject: None,
            raglite_ok: false,
            irontology_ok: false,
            warnings: Vec::new(),
        };

        // ── Raglite path ────────────────────────────────────────────────────
        if matches!(backend, GrokBackend::Raglite | GrokBackend::Both) {
            match self.raglite_ingest(topic, content).await {
                Ok(job_id) => {
                    result.raglite_job_id = Some(job_id);
                    result.raglite_ok = true;
                }
                Err(e) => {
                    result.warnings.push(format!("RAGLight ingest: {}", e));
                }
            }
        }

        // ── Irontology path ─────────────────────────────────────────────────
        if matches!(backend, GrokBackend::Irontology | GrokBackend::Both) {
            match &self.iron {
                Some(iron) => {
                    let datum = crate::irontology_bridge::DatumNode::new(topic, "Concept", content);
                    match iron.ingest(&datum).await {
                        Ok(r) => {
                            result.irontology_subject = Some(r.subject_prefix);
                            result.irontology_ok = true;
                        }
                        Err(e) => {
                            result.warnings.push(format!("Irontology ingest: {}", e));
                        }
                    }
                }
                None => {
                    result.warnings.push(
                        "Irontology unavailable (init failed — check ~/.b00t/neumann/)".to_string(),
                    );
                }
            }
        }

        Ok(result)
    }

    /// Query across the specified backend(s), merging and deduplicating results
    pub async fn query(
        &self,
        query_str: &str,
        topic: Option<&str>,
        limit: Option<usize>,
        backend: GrokBackend,
    ) -> Result<DualQueryResult> {
        let mut items: Vec<DualQueryItem> = Vec::new();
        let mut warnings: Vec<String> = Vec::new();
        let mut raglite_ok = false;
        let mut irontology_ok = false;
        let max = limit.unwrap_or(10);

        // ── Raglite query ───────────────────────────────────────────────────
        if matches!(backend, GrokBackend::Raglite | GrokBackend::Both) {
            if let Some(t) = topic {
                match self.raglite_query(query_str, t, Some(max)).await {
                    Ok(raw) => {
                        raglite_ok = true;
                        // raw is JSON string from raglight Python
                        items.push(DualQueryItem {
                            backend: "raglite".to_string(),
                            content: raw,
                            topic: t.to_string(),
                            tags: Vec::new(),
                            score: 1.0,
                        });
                    }
                    Err(e) => warnings.push(format!("RAGLight query: {}", e)),
                }
            } else {
                warnings.push("RAGLight query requires --topic".to_string());
            }
        }

        // ── Irontology query ────────────────────────────────────────────────
        if matches!(backend, GrokBackend::Irontology | GrokBackend::Both) {
            match &self.iron {
                Some(iron) => {
                    match iron.query(query_str, topic, Some(max)).await {
                        Ok(iron_items) => {
                            irontology_ok = true;
                            for item in iron_items {
                                items.push(DualQueryItem {
                                    backend: "irontology".to_string(),
                                    content: item.content,
                                    topic: item.topic,
                                    tags: item.tags,
                                    score: item.score,
                                });
                            }
                        }
                        Err(e) => warnings.push(format!("Irontology query: {}", e)),
                    }
                }
                None => {
                    warnings.push("Irontology unavailable".to_string());
                }
            }
        }

        // Deduplicate by content hash (exact duplication from both backends)
        let before = items.len();
        items.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap_or(std::cmp::Ordering::Equal));
        items.dedup_by(|a, b| a.content == b.content);
        if items.len() < before {
            tracing::debug!("Deduped {} duplicate results across backends", before - items.len());
        }
        items.truncate(max);

        let total = items.len();
        Ok(DualQueryResult {
            query: query_str.to_string(),
            topic: topic.map(|t| t.to_string()),
            total_found: total,
            items,
            raglite_ok,
            irontology_ok,
            warnings,
        })
    }

    // ── raglite helpers (delegate to RagLightManager) ──────────────────────

    async fn raglite_ingest(&mut self, topic: &str, content: &str) -> Result<String> {
        let config = RagLightConfig::default();
        let mut manager = RagLightManager::new(config)?;

        // Store inline content to tmp file (matches existing pattern in grok.rs)
        let tmp_dir = dirs::home_dir()
            .ok_or_else(|| anyhow::anyhow!("no home dir"))?
            .join(".b00t/raglight/uploads");
        std::fs::create_dir_all(&tmp_dir)?;

        let filename = format!("{}-{}.txt", sanitize_topic(topic), uuid::Uuid::new_v4());
        let path = tmp_dir.join(&filename);
        std::fs::write(&path, content)?;

        let doc = DocumentSource {
            source: path.to_string_lossy().to_string(),
            loader_type: Some(LoaderType::Text),
            topic: topic.to_string(),
            metadata: None,
        };
        let job_id = manager.add_document(doc).await?;
        Ok(job_id)
    }

    async fn raglite_query(
        &self,
        query: &str,
        topic: &str,
        limit: Option<usize>,
    ) -> Result<String> {
        let config = RagLightConfig::default();
        let manager = RagLightManager::new(config)?;
        manager.query(topic, query, limit).await
    }
}

impl Default for DualGrokClient {
    fn default() -> Self {
        Self::new()
    }
}

fn sanitize_topic(input: &str) -> String {
    let s: String = input
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '_' })
        .collect();
    let s = s.trim_matches('_').to_string();
    if s.is_empty() { "topic".to_string() } else { s }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_backend_from_flag_absent() {
        assert_eq!(GrokBackend::from_flag(None).unwrap(), GrokBackend::Both);
    }

    #[test]
    fn test_backend_from_flag_both() {
        assert_eq!(GrokBackend::from_flag(Some("both")).unwrap(), GrokBackend::Both);
    }

    #[test]
    fn test_backend_from_flag_raglite() {
        for s in &["raglite", "raglight", "rag-light", "rag_light"] {
            assert_eq!(GrokBackend::from_flag(Some(s)).unwrap(), GrokBackend::Raglite);
        }
    }

    #[test]
    fn test_backend_from_flag_irontology() {
        for s in &["irontology", "iron"] {
            assert_eq!(GrokBackend::from_flag(Some(s)).unwrap(), GrokBackend::Irontology);
        }
    }

    #[test]
    fn test_backend_from_flag_invalid() {
        assert!(GrokBackend::from_flag(Some("qdrant")).is_err());
    }

    #[test]
    fn test_dual_grok_client_new_does_not_panic() {
        let _ = DualGrokClient::new();
    }
}
