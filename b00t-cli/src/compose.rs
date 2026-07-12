use serde::{Deserialize, Serialize};

/// Measured composition metric — one `{metric=..., value=...}` entry in
/// `[b00t.compose]` `measured`. Emitted as a `b00t:measured` triple with
/// object "metric=value" (e.g. "context_savings_record_lesson=94%").
#[derive(Deserialize, Serialize, Debug, Clone, PartialEq, Default)]
#[serde(default)]
pub struct MeasuredMetric {
    pub metric: String,
    pub value: String,
}

/// Composition knowledge — `[b00t.compose]` table.
/// Makes capability composition graph-visible: datum_triples emits
/// b00t:composes_with / b00t:audits / b00t:supersedes / b00t:measured
/// triples from these fields (previously comment-prose only).
#[derive(Deserialize, Serialize, Debug, Clone, PartialEq, Default)]
#[serde(default)]
pub struct ComposeConfig {
    /// Datums this capability composes with into a larger capability.
    pub composes_with: Option<Vec<String>>,
    /// Datums whose output this capability audits/verifies.
    pub audits: Option<Vec<String>>,
    /// Datums (or approaches) this capability makes obsolete.
    pub supersedes: Option<Vec<String>>,
    /// Measured evidence for the composition (e.g. token-savings metrics).
    pub measured: Option<Vec<MeasuredMetric>>,
}
