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
#[cfg(feature = "ledgerr-events")]
use serde_json::json;

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
    pub control_events: Vec<ControlCodeEvent>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DualQueryItem {
    pub backend: String,
    pub content: String,
    pub topic: String,
    pub tags: Vec<String>,
    pub score: f32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ControlCodeEvent {
    pub action_code: String,
    pub severity: String,
    pub source: String,
    pub target: String,
    pub request: String,
    pub log_ref: String,
    pub reply: ControlReply,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ControlReply {
    Immediate,
    Queued { queue: String },
    Promise { promise_id: String },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ControlEventCapability {
    pub name: String,
    pub backend: String,
    pub active: bool,
    pub tags: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ControlEventReceipt {
    pub capability: ControlEventCapability,
    pub delivered: bool,
    pub reply: ControlReply,
    pub message: String,
}

pub trait ControlEventSink: Send + Sync {
    fn capability(&self) -> ControlEventCapability;
    fn emit(&self, event: &ControlCodeEvent) -> ControlEventReceipt;
}

#[derive(Debug, Default)]
pub struct StubControlEventSink;

impl ControlEventSink for StubControlEventSink {
    fn capability(&self) -> ControlEventCapability {
        ControlEventCapability {
            name: "control-event-sink".to_string(),
            backend: "stub".to_string(),
            active: false,
            tags: vec![
                "control-code".to_string(),
                "stub".to_string(),
                "minimal-init".to_string(),
            ],
        }
    }

    fn emit(&self, event: &ControlCodeEvent) -> ControlEventReceipt {
        ControlEventReceipt {
            capability: self.capability(),
            delivered: false,
            reply: event.reply.clone(),
            message: "ledgerr-events feature/config unavailable; event retained for local handling"
                .to_string(),
        }
    }
}

#[cfg(feature = "ledgerr-events")]
pub struct LedgerrControlEventSink {
    command: String,
    args: Vec<String>,
}

#[cfg(feature = "ledgerr-events")]
impl LedgerrControlEventSink {
    pub fn new(command: impl Into<String>, args: Vec<String>) -> Self {
        Self {
            command: command.into(),
            args,
        }
    }
}

#[cfg(feature = "ledgerr-events")]
impl ControlEventSink for LedgerrControlEventSink {
    fn capability(&self) -> ControlEventCapability {
        ControlEventCapability {
            name: "control-event-sink".to_string(),
            backend: "ledgerr-mcp".to_string(),
            active: true,
            tags: vec![
                "control-code".to_string(),
                "ledgerr".to_string(),
                "event-log".to_string(),
                "classification".to_string(),
                "visualization".to_string(),
            ],
        }
    }

    fn emit(&self, event: &ControlCodeEvent) -> ControlEventReceipt {
        use std::io::Write;
        use std::process::{Command, Stdio};

        let payload = json!({
            "action_code": event.action_code,
            "severity": event.severity,
            "source": event.source,
            "target": event.target,
            "request": event.request,
            "log_ref": event.log_ref,
            "reply": event.reply.clone(),
        });

        let mut child = match Command::new(&self.command)
            .args(self.args.iter())
            .stdin(Stdio::piped())
            .stdout(Stdio::null())
            .stderr(Stdio::piped())
            .spawn()
        {
            Ok(child) => child,
            Err(e) => {
                return ControlEventReceipt {
                    capability: self.capability(),
                    delivered: false,
                    reply: event.reply.clone(),
                    message: format!("ledgerr event command spawn failed: {e}"),
                };
            }
        };

        if let Some(stdin) = child.stdin.as_mut() {
            if let Err(e) = writeln!(stdin, "{payload}") {
                return ControlEventReceipt {
                    capability: self.capability(),
                    delivered: false,
                    reply: event.reply.clone(),
                    message: format!("ledgerr event command write failed: {e}"),
                };
            }
        }

        match child.wait_with_output() {
            Ok(output) if output.status.success() => ControlEventReceipt {
                capability: self.capability(),
                delivered: true,
                reply: event.reply.clone(),
                message: "delivered to ledgerr event command".to_string(),
            },
            Ok(output) => ControlEventReceipt {
                capability: self.capability(),
                delivered: false,
                reply: event.reply.clone(),
                message: format!(
                    "ledgerr event command failed: {}",
                    String::from_utf8_lossy(&output.stderr).trim()
                ),
            },
            Err(e) => ControlEventReceipt {
                capability: self.capability(),
                delivered: false,
                reply: event.reply.clone(),
                message: format!("ledgerr event command wait failed: {e}"),
            },
        }
    }
}

pub fn default_control_event_sink() -> Box<dyn ControlEventSink> {
    #[cfg(feature = "ledgerr-events")]
    {
        if let Ok(command) = std::env::var("B00T_LEDGERR_EVENT_COMMAND") {
            let args = std::env::var("B00T_LEDGERR_EVENT_ARGS")
                .ok()
                .map(|raw| raw.split_whitespace().map(str::to_string).collect())
                .unwrap_or_default();
            return Box::new(LedgerrControlEventSink::new(command, args));
        }
    }

    Box::<StubControlEventSink>::default()
}

impl ControlCodeEvent {
    fn from_backend_warning(source: &str, warning: &str) -> Option<Self> {
        let lower = warning.to_lowercase();
        let is_backend_error = lower.contains("failed")
            || lower.contains("error")
            || lower.contains("unavailable")
            || lower.contains("could not set lock")
            || lower.contains("conflicting lock");

        if !is_backend_error {
            return None;
        }

        Some(Self {
            action_code: "|e|".to_string(),
            severity: if lower.contains("could not set lock") || lower.contains("conflicting lock")
            {
                "degraded".to_string()
            } else {
                "warning".to_string()
            },
            source: source.to_string(),
            target: "ledgerr_review".to_string(),
            request: "inspect backend error log and recommend fallback/state-machine action"
                .to_string(),
            log_ref: stable_log_ref(source, warning),
            reply: ControlReply::Queued {
                queue: "b00t.control.errors".to_string(),
            },
        })
    }
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
                Some(iron) => match iron.query(query_str, topic, Some(max)).await {
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
                },
                None => {
                    warnings.push("Irontology unavailable".to_string());
                }
            }
        }

        // Deduplicate by content (exact duplication from both backends).
        // 🤓 dedup_by only removes *consecutive* equal elements; must sort by
        //    content first, dedup, then re-sort by score so cross-backend
        //    duplicates are actually adjacent during dedup.
        let before = items.len();
        items.sort_unstable_by(|a, b| a.content.cmp(&b.content));
        items.dedup_by(|a, b| a.content == b.content);
        if items.len() < before {
            tracing::debug!(
                "Deduped {} duplicate results across backends",
                before - items.len()
            );
        }
        items.sort_unstable_by(|a, b| {
            b.score
                .partial_cmp(&a.score)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        items.truncate(max);

        let total = items.len();
        let control_events = warnings
            .iter()
            .filter_map(|warning| {
                let source = warning
                    .split_once(':')
                    .map(|(source, _)| source)
                    .unwrap_or("grok");
                ControlCodeEvent::from_backend_warning(source, warning)
            })
            .collect();
        Ok(DualQueryResult {
            query: query_str.to_string(),
            topic: topic.map(str::to_string),
            total_found: total,
            items,
            raglite_ok,
            irontology_ok,
            warnings,
            control_events,
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
    let trimmed = s.trim_matches('_');
    if trimmed.is_empty() { "topic".to_string() } else { trimmed.to_string() }
}

fn stable_log_ref(source: &str, warning: &str) -> String {
    // 🤓 FNV-1a 64-bit: deterministic across Rust versions & processes.
    //    DefaultHasher is SipHash-1-3 with a *randomized* seed — NOT stable.
    const FNV_OFFSET: u64 = 14_695_981_039_346_656_037;
    const FNV_PRIME: u64 = 1_099_511_628_211;

    let mut h = FNV_OFFSET;
    for &b in source.as_bytes().iter().chain(b":").chain(warning.as_bytes()) {
        h ^= b as u64;
        h = h.wrapping_mul(FNV_PRIME);
    }
    format!("b00t:grok:error:{h:016x}")
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
        assert_eq!(
            GrokBackend::from_flag(Some("both")).unwrap(),
            GrokBackend::Both
        );
    }

    #[test]
    fn test_backend_from_flag_raglite() {
        for s in &["raglite", "raglight", "rag-light", "rag_light"] {
            assert_eq!(
                GrokBackend::from_flag(Some(s)).unwrap(),
                GrokBackend::Raglite
            );
        }
    }

    #[test]
    fn test_backend_from_flag_irontology() {
        for s in &["irontology", "iron"] {
            assert_eq!(
                GrokBackend::from_flag(Some(s)).unwrap(),
                GrokBackend::Irontology
            );
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

    #[test]
    fn test_control_event_for_raglite_duckdb_lock() {
        let warning = "RAGLight query: RAGLight query failed: IO Error: Could not set lock on file \"/home/brianh/.local/share/raglite/raglite.db\": Conflicting lock is held in /usr/bin/python3.12 (PID 53200)";
        let event = ControlCodeEvent::from_backend_warning("RAGLight query", warning)
            .expect("RAGLite lock must emit a control-code error event");

        assert_eq!(event.action_code, "|e|");
        assert_eq!(event.severity, "degraded");
        assert_eq!(event.target, "ledgerr_review");
        assert!(event.log_ref.starts_with("b00t:grok:error:"));
        assert_eq!(
            event.request,
            "inspect backend error log and recommend fallback/state-machine action"
        );
        assert_eq!(
            event.reply,
            ControlReply::Queued {
                queue: "b00t.control.errors".to_string()
            }
        );
    }

    #[test]
    fn test_stub_control_event_sink_reports_minimal_capability() {
        let warning = "Irontology query: Irontology unavailable";
        let event = ControlCodeEvent::from_backend_warning("Irontology query", warning).unwrap();
        let sink = StubControlEventSink;
        let receipt = sink.emit(&event);

        assert!(!receipt.delivered);
        assert_eq!(receipt.capability.backend, "stub");
        assert!(!receipt.capability.active);
        assert!(
            receipt
                .capability
                .tags
                .contains(&"minimal-init".to_string())
        );
        assert!(
            receipt
                .message
                .contains("ledgerr-events feature/config unavailable")
        );
    }

    #[cfg(feature = "ledgerr-events")]
    #[test]
    fn test_ledgerr_control_event_sink_reports_active_capability() {
        let sink = LedgerrControlEventSink::new("ledgerr-mcp-server", Vec::new());
        let capability = sink.capability();

        assert_eq!(capability.backend, "ledgerr-mcp");
        assert!(capability.active);
        assert!(capability.tags.contains(&"event-log".to_string()));
    }
}
