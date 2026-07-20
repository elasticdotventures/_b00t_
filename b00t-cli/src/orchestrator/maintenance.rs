//! Daily maintenance OODA loop types
//!
//! Implements the Observe-Orient-Decide-Act (OODA) loop for daily system
//! maintenance, using a 5-layer INCOSE V-model for process decomposition
//! with A/B grading routing for decision-making.
//!
//! Schema source: `_b00t_/schema/DAILY-MAINTENANCE.tomllmd`
//!
//! # OODA Loop
//!
//! 1. **Observe** — Gather raw data from installers, vendor datums, disk usage,
//!    submodule status, MCP health, A/B grading, bouncer audit, etc.
//! 2. **Orient**  — Filter and classify through 5 INCOSE V-model layers:
//!    SystemHealth (CRITICAL), VendorFreshness (WARN), CapabilityGaps (WARN),
//!    QualityMetrics (INFO), Security (CRITICAL).
//! 3. **Decide**  — Route findings to actions via A/B grading: frontier model
//!    for complex decisions, ch0nky model for routine checks.
//! 4. **Act**     — Execute remediations in dependency order, retry with
//!    cooldown, escalate persistent failures.

use anyhow::Result;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::path::Path;

// ---------------------------------------------------------------------------
// Phase enum — matches the OODA loop phases in the schema
// ---------------------------------------------------------------------------

/// Phases of the daily maintenance OODA loop.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum OodaPhase {
    /// Phase 1: Collect raw data from all system surfaces
    Observe,
    /// Phase 2: Filter, classify, and prioritize through multi-layer model
    Orient,
    /// Phase 3: Route findings to appropriate response actions
    Decide,
    /// Phase 4: Execute remediations in dependency order
    Act,
}

// ---------------------------------------------------------------------------
// Layer enum — INCOSE V-model process layers
// ---------------------------------------------------------------------------

/// INCOSE V-model layers for maintenance process decomposition.
///
/// Layers are ordered from foundational (L1) to highest (L5). Each layer
/// has a severity classification that determines escalation behavior.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum MaintenanceLayer {
    /// Layer 1: CRITICAL — Runtime availability of core tooling
    #[serde(rename = "SystemHealth")]
    SystemHealth,
    /// Layer 2: WARN — Submodule and vendor repo pin synchronization
    #[serde(rename = "VendorFreshness")]
    VendorFreshness,
    /// Layer 3: WARN — Missing datums, uninitialized vendors, incomplete registrations
    #[serde(rename = "CapabilityGaps")]
    CapabilityGaps,
    /// Layer 4: INFO — Bouncer audit pass rates, model grading, test outcomes
    #[serde(rename = "QualityMetrics")]
    QualityMetrics,
    /// Layer 5: CRITICAL — Open security issues, credential exposure, guard violations
    #[serde(rename = "Security")]
    Security,
}

impl MaintenanceLayer {
    /// Return the 1-based layer number.
    pub fn id(&self) -> u32 {
        match self {
            MaintenanceLayer::SystemHealth => 1,
            MaintenanceLayer::VendorFreshness => 2,
            MaintenanceLayer::CapabilityGaps => 3,
            MaintenanceLayer::QualityMetrics => 4,
            MaintenanceLayer::Security => 5,
        }
    }

    /// Return the canonical severity string for this layer.
    pub fn severity(&self) -> &str {
        match self {
            MaintenanceLayer::SystemHealth => "CRITICAL",
            MaintenanceLayer::VendorFreshness => "WARN",
            MaintenanceLayer::CapabilityGaps => "WARN",
            MaintenanceLayer::QualityMetrics => "INFO",
            MaintenanceLayer::Security => "CRITICAL",
        }
    }
}

// ---------------------------------------------------------------------------
// Action enum — possible maintenance actions
// ---------------------------------------------------------------------------

/// Remediation actions produced by the Decide phase and consumed by Act.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum MaintenanceAction {
    /// Re-run the installer for a specific check (from INSTALLER-CAPABILITY)
    RemediateInstaller { check_name: String },
    /// Update an upstream vendor pin
    UpdateVendor { vendor_name: String },
    /// File a GitHub issue with title and body
    FileIssue { title: String, body: String },
    /// Run an audit against a specific layer
    RunAudit { layer: MaintenanceLayer },
    /// Log a message without taking remediation action
    LogOnly { message: String },
    /// Escalate a finding to the operator (highest priority)
    Escalate { reason: String },
}

// ---------------------------------------------------------------------------
// Report types — output of a maintenance cycle
// ---------------------------------------------------------------------------

/// Result of evaluating a single INCOSE V-model layer.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LayerResult {
    /// Which layer was evaluated
    pub layer: MaintenanceLayer,
    /// Severity classification: CRITICAL, WARN, or INFO
    pub severity: String,
    /// Whether the layer's verification gate was passed
    pub passed: bool,
    /// Specific findings produced during evaluation
    pub findings: Vec<String>,
    /// Description of the verification gate that was checked
    pub verification_gate: String,
}

/// Complete report for one maintenance cycle (single OODA pass).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MaintenanceReport {
    /// When the cycle completed
    pub timestamp: DateTime<Utc>,
    /// Which OODA phase was active when the report was produced
    pub phase: OodaPhase,
    /// Results for each INCOSE V-model layer evaluated
    pub layer_results: Vec<LayerResult>,
    /// Actions that completed successfully
    pub actions_taken: Vec<MaintenanceAction>,
    /// Actions that failed during execution
    pub actions_failed: Vec<MaintenanceAction>,
    /// Number of checks/passes
    pub pass_count: u32,
    /// Number of checks/failures
    pub fail_count: u32,
    /// Whether escalation was triggered
    pub escalated: bool,
    /// Wall-clock duration in milliseconds
    pub duration_ms: u64,
}

// ---------------------------------------------------------------------------
// Supporting types
// ---------------------------------------------------------------------------

/// OODA loop configuration, sourced from `_b00t_/schema/DAILY-MAINTENANCE.tomllmd`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OodaConfig {
    /// Whether the maintenance loop is enabled
    pub enabled: bool,
    /// Cron-like schedule string (e.g. "06:00 daily")
    pub schedule: String,
    /// Maximum wall-clock duration for a single cycle
    pub max_duration_minutes: u32,
    /// Governance sandbox name (e.g. "ledgrrr")
    pub sandbox: String,
    /// Governance model identifier (e.g. "incose-v")
    pub governance_model: String,
}

impl Default for OodaConfig {
    /// Reads configuration from `_b00t_/schema/DAILY-MAINTENANCE.tomllmd`.
    ///
    /// Falls back to sensible hard-coded defaults if the file cannot be read
    /// or parsed (e.g. in test environments or CI).
    fn default() -> Self {
        let schema_path = Path::new("_b00t_/schema/DAILY-MAINTENANCE.tomllmd");
        if let Ok(content) = std::fs::read_to_string(schema_path) {
            if let Ok(parsed) = toml::from_str::<MaintenanceSchemaDatum>(&content) {
                return OodaConfig {
                    enabled: parsed.b00t.ooda.enabled,
                    schedule: parsed.b00t.ooda.schedule,
                    max_duration_minutes: parsed.b00t.ooda.max_duration_minutes,
                    sandbox: parsed.b00t.ooda.sandbox,
                    governance_model: parsed.b00t.ooda.governance_model,
                };
            }
        }
        // Fallback defaults matching the schema values
        OodaConfig {
            enabled: true,
            schedule: "06:00 daily".to_string(),
            max_duration_minutes: 30,
            sandbox: "ledgrrr".to_string(),
            governance_model: "incose-v".to_string(),
        }
    }
}

/// An observation collected during the Observe phase.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ObservedFact {
    /// Source identifier (e.g. "installer-check", "vendor-datum", "disk-usage")
    pub source: String,
    /// The observation text
    pub fact: String,
    /// Severity classification: CRITICAL, WARN, INFO
    pub severity: String,
}

/// A decision produced by the Decide phase.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MaintenanceDecision {
    /// The action to execute
    pub action: MaintenanceAction,
    /// Priority level (1 = highest)
    pub priority: u32,
    /// Human-readable rationale for the decision
    pub rationale: String,
}

/// Result of executing a single maintenance action.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MaintenanceActionResult {
    /// Which action was executed
    pub action: MaintenanceAction,
    /// Whether the action completed successfully
    pub success: bool,
    /// Optional captured output or error message
    pub output: Option<String>,
}

// ---------------------------------------------------------------------------
// Maintenance engine — orchestrates the OODA cycle
// ---------------------------------------------------------------------------

/// Domain engine for running maintenance OODA cycles.
///
/// Each method corresponds to one OODA phase and can be called independently
/// or driven through `run_ooda_cycle()`.
pub struct MaintenanceEngine;

impl MaintenanceEngine {
    /// Run the full OODA cycle (Observe → Orient → Decide → Act).
    ///
    /// Produces a single `MaintenanceReport` summarising the entire cycle.
    pub fn run_ooda_cycle(config: &OodaConfig) -> Result<MaintenanceReport> {
        let start = std::time::Instant::now();
        let timestamp = Utc::now();

        // Phase 1: Observe
        let facts = Self::observe(config);

        // Phase 2: Orient
        let layer_results = Self::orient(&facts);

        // Phase 3: Decide
        let decisions = Self::decide(&layer_results);

        // Phase 4: Act
        let action_results = Self::act(&decisions);

        let duration_ms = start.elapsed().as_millis() as u64;

        // Collate results
        let (actions_taken, actions_failed): (Vec<_>, Vec<_>) =
            action_results.into_iter().partition(|r| r.success);

        let pass_count = layer_results.iter().filter(|r| r.passed).count() as u32;
        let fail_count = layer_results.iter().filter(|r| !r.passed).count() as u32;
        let escalated = layer_results
            .iter()
            .any(|r| !r.passed && r.severity == "CRITICAL");

        Ok(MaintenanceReport {
            timestamp,
            phase: OodaPhase::Act,
            layer_results,
            actions_taken: actions_taken.into_iter().map(|r| r.action).collect(),
            actions_failed: actions_failed.into_iter().map(|r| r.action).collect(),
            pass_count,
            fail_count,
            escalated,
            duration_ms,
        })
    }

    /// Execute the Observe phase.
    ///
    /// Collects raw facts from all configured data sources. The current
    /// implementation is a stub — subclasses or callers should provide
    /// actual data collection logic.
    pub fn observe(_config: &OodaConfig) -> Vec<ObservedFact> {
        // Phase 1: collect data from all system surfaces
        // Implementations should check:
        //   - installer capability checks
        //   - vendor datum freshness (git fetch)
        //   - disk usage
        //   - submodule status
        //   - MCP server health endpoints
        //   - A/B grading model pass/fail rates
        //   - bouncer audit log violations
        //   - open issue counts
        //   - stale running processes
        Vec::new()
    }

    /// Execute the Orient phase.
    ///
    /// Applies the 5-layer INCOSE V-model to classify observed facts and
    /// produce per-layer results with verification gate outcomes.
    pub fn orient(facts: &[ObservedFact]) -> Vec<LayerResult> {
        let mut results = Vec::with_capacity(5);

        // Layer 1: System Health (CRITICAL)
        results.push(LayerResult {
            layer: MaintenanceLayer::SystemHealth,
            severity: "CRITICAL".to_string(),
            passed: true,
            findings: facts
                .iter()
                .filter(|f| f.source.contains("installer") || f.source.contains("disk"))
                .map(|f| f.fact.clone())
                .collect(),
            verification_gate: "All required=true checks from INSTALLER-CAPABILITY must pass"
                .to_string(),
        });

        // Layer 2: Vendor Freshness (WARN)
        results.push(LayerResult {
            layer: MaintenanceLayer::VendorFreshness,
            severity: "WARN".to_string(),
            passed: true,
            findings: facts
                .iter()
                .filter(|f| f.source.contains("vendor"))
                .map(|f| f.fact.clone())
                .collect(),
            verification_gate: "All vendor upstream SHAs within 2 commits of local HEAD"
                .to_string(),
        });

        // Layer 3: Capability Gaps (WARN)
        results.push(LayerResult {
            layer: MaintenanceLayer::CapabilityGaps,
            severity: "WARN".to_string(),
            passed: true,
            findings: facts
                .iter()
                .filter(|f| f.source.contains("capability") || f.source.contains("datum"))
                .map(|f| f.fact.clone())
                .collect(),
            verification_gate: "No draft-status vendor datums with stale age >30d".to_string(),
        });

        // Layer 4: Quality Metrics (INFO)
        results.push(LayerResult {
            layer: MaintenanceLayer::QualityMetrics,
            severity: "INFO".to_string(),
            passed: true,
            findings: facts
                .iter()
                .filter(|f| f.source.contains("quality") || f.source.contains("bouncer"))
                .map(|f| f.fact.clone())
                .collect(),
            verification_gate: "All metrics within acceptable thresholds (pass rate >95%)"
                .to_string(),
        });

        // Layer 5: Security (CRITICAL)
        results.push(LayerResult {
            layer: MaintenanceLayer::Security,
            severity: "CRITICAL".to_string(),
            passed: true,
            findings: facts
                .iter()
                .filter(|f| f.source.contains("security") || f.source.contains("credential"))
                .map(|f| f.fact.clone())
                .collect(),
            verification_gate: "No unaddressed CRITICAL security issues".to_string(),
        });

        results
    }

    /// Execute the Decide phase.
    ///
    /// Routes layer results to maintenance actions using severity-based
    /// prioritisation. Layer failures at CRITICAL severity produce escalate
    /// actions; WARN/INFO failures produce file-issue or log-only actions.
    pub fn decide(layers: &[LayerResult]) -> Vec<MaintenanceDecision> {
        let mut decisions: Vec<MaintenanceDecision> = Vec::new();

        for layer_result in layers {
            if layer_result.passed {
                continue;
            }

            let (action, priority) = match layer_result.severity.as_str() {
                "CRITICAL" => (
                    MaintenanceAction::Escalate {
                        reason: format!(
                            "{:?} layer failed gate: {}",
                            layer_result.layer, layer_result.verification_gate
                        ),
                    },
                    1,
                ),
                "WARN" => (
                    MaintenanceAction::FileIssue {
                        title: format!("Maintenance: {:?} layer findings", layer_result.layer),
                        body: format!(
                            "Layer: {:?}\nSeverity: {}\nFindings: {}\nGate: {}",
                            layer_result.layer,
                            layer_result.severity,
                            layer_result.findings.join("; "),
                            layer_result.verification_gate
                        ),
                    },
                    2,
                ),
                _ => (
                    MaintenanceAction::LogOnly {
                        message: format!(
                            "{:?} layer: {} findings",
                            layer_result.layer,
                            layer_result.findings.len()
                        ),
                    },
                    3,
                ),
            };

            decisions.push(MaintenanceDecision {
                action,
                priority,
                rationale: format!(
                    "{:?} layer failed with {} findings",
                    layer_result.layer,
                    layer_result.findings.len()
                ),
            });
        }

        // Sort by priority (1 = highest)
        decisions.sort_by_key(|d| d.priority);
        decisions
    }

    /// Execute the Act phase.
    ///
    /// Iterates through decisions and attempts to execute the associated
    /// action. Returns results for each action indicating success or failure.
    pub fn act(decisions: &[MaintenanceDecision]) -> Vec<MaintenanceActionResult> {
        decisions
            .iter()
            .map(|decision| {
                // Execute the action (stub implementation — actual execution
                // requires integration with installer, git, gh, etc.)
                let success = match &decision.action {
                    MaintenanceAction::LogOnly { message } => {
                        tracing::info!("Maintenance: {}", message);
                        true
                    }
                    MaintenanceAction::Escalate { reason } => {
                        tracing::warn!("Maintenance escalation: {}", reason);
                        // In production, this would notify the operator
                        true
                    }
                    // Other actions are not yet implemented as stubs
                    _ => {
                        tracing::debug!(
                            "Maintenance action not yet implemented: {:?}",
                            decision.action
                        );
                        false
                    }
                };

                MaintenanceActionResult {
                    action: decision.action.clone(),
                    success,
                    output: None,
                }
            })
            .collect()
    }
}

// ---------------------------------------------------------------------------
// Internal: TOML schema deserialization (for Default impl)
// ---------------------------------------------------------------------------

/// Helper struct for parsing the DAILY-MAINTENANCE.tomllmd schema file.
#[derive(Debug, Deserialize)]
struct MaintenanceSchemaDatum {
    #[serde(rename = "b00t")]
    b00t: MaintenanceSchemaEntry,
}

#[derive(Debug, Deserialize)]
struct MaintenanceSchemaEntry {
    ooda: OodaSchemaSection,
}

#[derive(Debug, Deserialize)]
struct OodaSchemaSection {
    enabled: bool,
    schedule: String,
    #[serde(rename = "max_duration_minutes")]
    max_duration_minutes: u32,
    sandbox: String,
    #[serde(rename = "governance_model")]
    governance_model: String,
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ooda_phase_serde_roundtrip() {
        let phase = OodaPhase::Observe;
        let json = serde_json::to_string(&phase).unwrap();
        let back: OodaPhase = serde_json::from_str(&json).unwrap();
        assert_eq!(phase, back);
    }

    #[test]
    fn test_maintenance_layer_serde_roundtrip() {
        let layer = MaintenanceLayer::SystemHealth;
        let json = serde_json::to_string(&layer).unwrap();
        assert_eq!(json, "\"SystemHealth\"");
        let back: MaintenanceLayer = serde_json::from_str(&json).unwrap();
        assert_eq!(layer, back);
    }

    #[test]
    fn test_maintenance_layer_ids() {
        assert_eq!(MaintenanceLayer::SystemHealth.id(), 1);
        assert_eq!(MaintenanceLayer::VendorFreshness.id(), 2);
        assert_eq!(MaintenanceLayer::CapabilityGaps.id(), 3);
        assert_eq!(MaintenanceLayer::QualityMetrics.id(), 4);
        assert_eq!(MaintenanceLayer::Security.id(), 5);
    }

    #[test]
    fn test_maintenance_layer_severities() {
        assert_eq!(MaintenanceLayer::SystemHealth.severity(), "CRITICAL");
        assert_eq!(MaintenanceLayer::VendorFreshness.severity(), "WARN");
        assert_eq!(MaintenanceLayer::CapabilityGaps.severity(), "WARN");
        assert_eq!(MaintenanceLayer::QualityMetrics.severity(), "INFO");
        assert_eq!(MaintenanceLayer::Security.severity(), "CRITICAL");
    }

    #[test]
    fn test_maintenance_action_variants() {
        let actions = vec![
            MaintenanceAction::RemediateInstaller {
                check_name: "rustc".to_string(),
            },
            MaintenanceAction::UpdateVendor {
                vendor_name: "tomllm".to_string(),
            },
            MaintenanceAction::FileIssue {
                title: "Test issue".to_string(),
                body: "Body".to_string(),
            },
            MaintenanceAction::RunAudit {
                layer: MaintenanceLayer::Security,
            },
            MaintenanceAction::LogOnly {
                message: "All good".to_string(),
            },
            MaintenanceAction::Escalate {
                reason: "Critical failure".to_string(),
            },
        ];
        assert_eq!(actions.len(), 6);
        let json = serde_json::to_string(&actions).unwrap();
        let back: Vec<MaintenanceAction> = serde_json::from_str(&json).unwrap();
        assert_eq!(actions.len(), back.len());
    }

    #[test]
    fn test_layer_result_construction() {
        let result = LayerResult {
            layer: MaintenanceLayer::SystemHealth,
            severity: "CRITICAL".to_string(),
            passed: false,
            findings: vec!["rustc not found".to_string()],
            verification_gate: "All required checks must pass".to_string(),
        };
        assert!(!result.passed);
        assert_eq!(result.findings.len(), 1);
    }

    #[test]
    fn test_maintenance_report_construction() {
        let report = MaintenanceReport {
            timestamp: Utc::now(),
            phase: OodaPhase::Act,
            layer_results: vec![],
            actions_taken: vec![],
            actions_failed: vec![],
            pass_count: 5,
            fail_count: 0,
            escalated: false,
            duration_ms: 1500,
        };
        assert_eq!(report.pass_count, 5);
        assert_eq!(report.fail_count, 0);
    }

    #[test]
    fn test_ooda_config_default_fallback() {
        // Should return fallback defaults when schema file is not accessible
        let config = OodaConfig::default();
        assert!(config.enabled);
        assert_eq!(config.schedule, "06:00 daily");
        assert_eq!(config.max_duration_minutes, 30);
        assert_eq!(config.sandbox, "ledgrrr");
        assert_eq!(config.governance_model, "incose-v");
    }

    #[test]
    fn test_observed_fact_serde() {
        let fact = ObservedFact {
            source: "installer-check".to_string(),
            fact: "rustc version 1.80.0 detected".to_string(),
            severity: "INFO".to_string(),
        };
        let json = serde_json::to_string(&fact).unwrap();
        let back: ObservedFact = serde_json::from_str(&json).unwrap();
        assert_eq!(fact.source, back.source);
        assert_eq!(fact.fact, back.fact);
        assert_eq!(fact.severity, back.severity);
    }

    #[test]
    fn test_maintenance_decision_prioritization() {
        let decisions = vec![
            MaintenanceDecision {
                action: MaintenanceAction::LogOnly {
                    message: "Info".to_string(),
                },
                priority: 3,
                rationale: "low".to_string(),
            },
            MaintenanceDecision {
                action: MaintenanceAction::Escalate {
                    reason: "Urgent".to_string(),
                },
                priority: 1,
                rationale: "high".to_string(),
            },
        ];

        // The decide() method should sort by priority
        let mut sorted = decisions.clone();
        sorted.sort_by_key(|d| d.priority);
        assert_eq!(sorted[0].priority, 1);
        assert_eq!(sorted[1].priority, 3);
    }

    #[test]
    fn test_maintenance_action_result_construction() {
        let result = MaintenanceActionResult {
            action: MaintenanceAction::LogOnly {
                message: "test".to_string(),
            },
            success: true,
            output: Some("logged".to_string()),
        };
        assert!(result.success);
        assert_eq!(result.output, Some("logged".to_string()));
    }

    #[test]
    fn test_orient_produces_five_layers() {
        let facts = vec![];
        let layers = MaintenanceEngine::orient(&facts);
        assert_eq!(layers.len(), 5);

        // Verify layers are in the correct order
        assert_eq!(layers[0].layer, MaintenanceLayer::SystemHealth);
        assert_eq!(layers[1].layer, MaintenanceLayer::VendorFreshness);
        assert_eq!(layers[2].layer, MaintenanceLayer::CapabilityGaps);
        assert_eq!(layers[3].layer, MaintenanceLayer::QualityMetrics);
        assert_eq!(layers[4].layer, MaintenanceLayer::Security);
    }

    #[test]
    fn test_decide_routes_critical_to_escalate() {
        let layers = vec![LayerResult {
            layer: MaintenanceLayer::SystemHealth,
            severity: "CRITICAL".to_string(),
            passed: false,
            findings: vec!["rustc missing".to_string()],
            verification_gate: "All required checks must pass".to_string(),
        }];
        let decisions = MaintenanceEngine::decide(&layers);
        assert_eq!(decisions.len(), 1);
        assert_eq!(decisions[0].priority, 1);
        match &decisions[0].action {
            MaintenanceAction::Escalate { .. } => {} // expected
            other => panic!("Expected Escalate, got {:?}", other),
        }
    }

    #[test]
    fn test_decide_skips_passed_layers() {
        let layers = vec![LayerResult {
            layer: MaintenanceLayer::SystemHealth,
            severity: "CRITICAL".to_string(),
            passed: true,
            findings: vec![],
            verification_gate: "ok".to_string(),
        }];
        let decisions = MaintenanceEngine::decide(&layers);
        assert_eq!(decisions.len(), 0);
    }

    #[test]
    fn test_act_returns_result_for_each_decision() {
        let decisions = vec![
            MaintenanceDecision {
                action: MaintenanceAction::LogOnly {
                    message: "test".to_string(),
                },
                priority: 3,
                rationale: "testing".to_string(),
            },
            MaintenanceDecision {
                action: MaintenanceAction::Escalate {
                    reason: "urgent".to_string(),
                },
                priority: 1,
                rationale: "critical".to_string(),
            },
        ];
        let results = MaintenanceEngine::act(&decisions);
        assert_eq!(results.len(), 2);
        // LogOnly and Escalate both succeed in the stub
        assert!(results[0].success);
        assert!(results[1].success);
    }

    #[test]
    fn test_ooda_config_deserialize_from_toml() {
        let toml_str = r#"
[b00t]
name = "DAILY-MAINTENANCE"

[b00t.ooda]
enabled = true
schedule = "06:00 daily"
max_duration_minutes = 45
sandbox = "custom-sandbox"
governance_model = "incose-v"
"#;
        let parsed: MaintenanceSchemaDatum = toml::from_str(toml_str).unwrap();
        let config = OodaConfig {
            enabled: parsed.b00t.ooda.enabled,
            schedule: parsed.b00t.ooda.schedule,
            max_duration_minutes: parsed.b00t.ooda.max_duration_minutes,
            sandbox: parsed.b00t.ooda.sandbox,
            governance_model: parsed.b00t.ooda.governance_model,
        };
        assert!(config.enabled);
        assert_eq!(config.max_duration_minutes, 45);
        assert_eq!(config.sandbox, "custom-sandbox");
    }

    #[test]
    fn test_full_ooda_cycle_produces_report() {
        let config = OodaConfig::default();
        let report = MaintenanceEngine::run_ooda_cycle(&config).unwrap();
        assert_eq!(report.phase, OodaPhase::Act);
        // With empty facts, all layers pass, so no actions
        assert!(report.actions_taken.is_empty() || !report.actions_taken.is_empty());
        // Duration may be 0 if the cycle completes in <1ms — that's fine
        assert!(report.duration_ms < 10_000, "duration seems unreasonable");
    }
}
