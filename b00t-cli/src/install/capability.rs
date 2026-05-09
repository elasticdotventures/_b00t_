use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::path::Path;
use std::time::Instant;

// ── Core types ───────────────────────────────────────────────────────────────

/// A single capability check with remediation instructions.
///
/// Maps to each `[[b00t.check]]` entry in the INSTALLER-CAPABILITY datum.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CapabilityCheck {
    pub name: String,
    pub check_command: String,
    pub remediation: String,
    pub required: bool,
    #[serde(default)]
    pub vendor_datum: Option<String>,
}

/// Status of a capability check after execution.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum CheckStatus {
    Pass,
    Fail,
    Remediated,
    Escalated,
    Skipped,
}

/// Result of running a single capability check, including output and timing.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CheckResult {
    pub check: CapabilityCheck,
    pub status: CheckStatus,
    pub output: Option<String>,
    pub error: Option<String>,
    pub duration_ms: u64,
}

// ── Daily routine & remediation config ───────────────────────────────────────

/// Daily routine configuration from `[b00t.daily_routine]`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DailyRoutineConfig {
    pub enabled: bool,
    pub time: String,
    pub max_duration_minutes: u32,
    pub parallel: bool,
    pub sandbox: String,
    pub report_to: String,
}

impl Default for DailyRoutineConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            time: "06:00".to_string(),
            max_duration_minutes: 15,
            parallel: true,
            sandbox: "ledgrrr".to_string(),
            report_to: "gh-issue".to_string(),
        }
    }
}

/// Escalation rules from `[b00t.remediation]`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RemediationConfig {
    pub max_retries: u32,
    pub cooldown_seconds: u64,
    pub escalation: String,
    pub escalation_threshold: u32,
    pub escalation_label: String,
    pub escalation_assignee: String,
}

impl Default for RemediationConfig {
    fn default() -> Self {
        Self {
            max_retries: 2,
            cooldown_seconds: 300,
            escalation: "gh-issue".to_string(),
            escalation_threshold: 3,
            escalation_label: "installer-capability".to_string(),
            escalation_assignee: "@b00t".to_string(),
        }
    }
}

// ── TOML datum parsing types ─────────────────────────────────────────────────

/// Top-level wrapper for deserializing the INSTALLER-CAPABILITY TOML datum.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CapabilitySchema {
    pub b00t: CapabilitySchemaInner,
}

/// Inner `[b00t]` section.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CapabilitySchemaInner {
    pub name: String,
    #[serde(rename = "type")]
    pub schema_type: Option<String>,
    pub hint: Option<String>,
    pub version: Option<serde_json::Value>,
    #[serde(default)]
    pub check: Vec<CapabilityCheck>,
    pub daily_routine: Option<DailyRoutineConfig>,
    pub remediation: Option<RemediationConfig>,
}

// ── InstallerRegistry ─────────────────────────────────────────────────────────

/// Registry of all capability checks loaded from an INSTALLER-CAPABILITY datum.
#[derive(Debug, Clone)]
pub struct InstallerRegistry {
    pub checks: Vec<CapabilityCheck>,
    pub daily_routine: DailyRoutineConfig,
    pub remediation: RemediationConfig,
}

impl InstallerRegistry {
    /// Load checks from a schema datum TOML file.
    pub fn load<P: AsRef<Path>>(path: P) -> Result<Self> {
        let content = std::fs::read_to_string(path.as_ref())
            .with_context(|| {
                format!("Failed to read capability datum: {}", path.as_ref().display())
            })?;
        Self::from_toml_str(&content)
    }

    /// Parse checks from a TOML string.
    pub fn from_toml_str(toml_str: &str) -> Result<Self> {
        let schema: CapabilitySchema =
            toml::from_str(toml_str).context("Failed to parse capability datum TOML")?;
        Ok(Self {
            checks: schema.b00t.check,
            daily_routine: schema.b00t.daily_routine.unwrap_or_default(),
            remediation: schema.b00t.remediation.unwrap_or_default(),
        })
    }

    /// Get all required checks.
    pub fn required_checks(&self) -> Vec<&CapabilityCheck> {
        self.checks.iter().filter(|c| c.required).collect()
    }

    /// Get all optional (non-required) checks.
    pub fn optional_checks(&self) -> Vec<&CapabilityCheck> {
        self.checks.iter().filter(|c| !c.required).collect()
    }

    /// Total number of checks registered.
    pub fn total(&self) -> usize {
        self.checks.len()
    }
}

// ── InstallerEngine ──────────────────────────────────────────────────────────

/// Engine that runs capability checks, performs remediation, and collects
/// results for dashboard display.
pub struct InstallerEngine;

impl InstallerEngine {
    /// Run a single capability check by executing `check_command` in a shell.
    pub fn run_check(check: &CapabilityCheck) -> CheckResult {
        let start = Instant::now();

        let output = std::process::Command::new("sh")
            .arg("-c")
            .arg(&check.check_command)
            .output();

        let duration_ms = start.elapsed().as_millis() as u64;

        match output {
            Ok(out) => {
                let stdout = String::from_utf8_lossy(&out.stdout).to_string();
                let stderr = String::from_utf8_lossy(&out.stderr).to_string();
                let combined = if stderr.is_empty() {
                    stdout
                } else {
                    format!("{stdout}{stderr}")
                };

                if out.status.success() {
                    CheckResult {
                        check: check.clone(),
                        status: CheckStatus::Pass,
                        output: Some(combined),
                        error: None,
                        duration_ms,
                    }
                } else {
                    CheckResult {
                        check: check.clone(),
                        status: CheckStatus::Fail,
                        output: Some(combined),
                        error: Some(format!(
                            "Exit code: {}",
                            out.status.code().unwrap_or(-1)
                        )),
                        duration_ms,
                    }
                }
            }
            Err(e) => CheckResult {
                check: check.clone(),
                status: CheckStatus::Fail,
                output: None,
                error: Some(format!("Failed to execute check: {e}")),
                duration_ms,
            },
        }
    }

    /// Run all checks and return results.
    pub fn run_all(registry: &InstallerRegistry) -> Vec<CheckResult> {
        registry.checks.iter().map(|c| Self::run_check(c)).collect()
    }

    /// Run only required checks.
    pub fn run_required(registry: &InstallerRegistry) -> Vec<CheckResult> {
        registry
            .checks
            .iter()
            .filter(|c| c.required)
            .map(|c| Self::run_check(c))
            .collect()
    }

    /// Attempt remediation for a failed check, then re-run the check to verify.
    pub fn remediate(
        check: &CapabilityCheck,
        _result: &CheckResult,
        config: &RemediationConfig,
    ) -> CheckResult {
        let start = Instant::now();

        let output = std::process::Command::new("sh")
            .arg("-c")
            .arg(&check.remediation)
            .output();

        let duration_ms = start.elapsed().as_millis() as u64;

        match output {
            Ok(out) => {
                let stdout = String::from_utf8_lossy(&out.stdout).to_string();
                let stderr = String::from_utf8_lossy(&out.stderr).to_string();
                let combined = if stderr.is_empty() {
                    stdout
                } else {
                    format!("{stdout}{stderr}")
                };

                if out.status.success() {
                    // Re-run the check to verify remediation worked
                    Self::run_check(check)
                } else {
                    CheckResult {
                        check: check.clone(),
                        status: CheckStatus::Escalated,
                        output: Some(combined),
                        error: Some(format!(
                            "Remediation failed. Exit code: {}. Max retries: {}",
                            out.status.code().unwrap_or(-1),
                            config.max_retries,
                        )),
                        duration_ms,
                    }
                }
            }
            Err(e) => CheckResult {
                check: check.clone(),
                status: CheckStatus::Escalated,
                output: None,
                error: Some(format!("Failed to execute remediation: {e}")),
                duration_ms,
            },
        }
    }

    /// Run the full OODA loop: check all, remediate failures, return results.
    pub fn run_ooda(registry: &InstallerRegistry) -> Vec<CheckResult> {
        let mut results = Vec::new();

        for check in &registry.checks {
            let result = Self::run_check(check);
            let final_result = if result.status == CheckStatus::Fail {
                Self::remediate(check, &result, &registry.remediation)
            } else {
                result
            };
            results.push(final_result);
        }

        results
    }
}

// ── CapabilityDashboard ──────────────────────────────────────────────────────

/// Dashboard-friendly summary of capability check results.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CapabilityDashboard {
    pub total_checks: usize,
    pub passed: usize,
    pub failed: usize,
    pub remediated: usize,
    pub escalated: usize,
    pub skipped: usize,
    pub timestamp: DateTime<Utc>,
    pub results: Vec<CheckResult>,
}

impl CapabilityDashboard {
    /// Build a dashboard from a vector of check results.
    pub fn from_results(results: Vec<CheckResult>) -> Self {
        let total_checks = results.len();
        let passed = results.iter().filter(|r| r.status == CheckStatus::Pass).count();
        let failed = results.iter().filter(|r| r.status == CheckStatus::Fail).count();
        let remediated = results
            .iter()
            .filter(|r| r.status == CheckStatus::Remediated)
            .count();
        let escalated = results
            .iter()
            .filter(|r| r.status == CheckStatus::Escalated)
            .count();
        let skipped = results.iter().filter(|r| r.status == CheckStatus::Skipped).count();

        Self {
            total_checks,
            passed,
            failed,
            remediated,
            escalated,
            skipped,
            timestamp: Utc::now(),
            results,
        }
    }

    /// Check if all required checks passed or were remediated.
    pub fn all_required_passed(&self) -> bool {
        self.results
            .iter()
            .filter(|r| r.check.required)
            .all(|r| r.status == CheckStatus::Pass || r.status == CheckStatus::Remediated)
    }

    /// Overall pass rate as a percentage (passed + remediated / total).
    pub fn pass_rate(&self) -> f64 {
        if self.total_checks == 0 {
            return 100.0;
        }
        let ok = (self.passed + self.remediated) as f64;
        (ok / self.total_checks as f64) * 100.0
    }

    /// Count of checks that were escalated after failed remediation.
    pub fn escalation_count(&self) -> usize {
        self.escalated
    }
}

// ── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_check_status_partial_eq() {
        assert_eq!(CheckStatus::Pass, CheckStatus::Pass);
        assert_eq!(CheckStatus::Fail, CheckStatus::Fail);
        assert_ne!(CheckStatus::Pass, CheckStatus::Fail);
    }

    #[test]
    fn test_daily_routine_default() {
        let config = DailyRoutineConfig::default();
        assert!(!config.enabled);
        assert_eq!(config.time, "06:00");
        assert_eq!(config.max_duration_minutes, 15);
        assert!(config.parallel);
        assert_eq!(config.sandbox, "ledgrrr");
        assert_eq!(config.report_to, "gh-issue");
    }

    #[test]
    fn test_remediation_config_default() {
        let config = RemediationConfig::default();
        assert_eq!(config.max_retries, 2);
        assert_eq!(config.cooldown_seconds, 300);
        assert_eq!(config.escalation, "gh-issue");
        assert_eq!(config.escalation_threshold, 3);
        assert_eq!(config.escalation_label, "installer-capability");
        assert_eq!(config.escalation_assignee, "@b00t");
    }

    #[test]
    fn test_parse_datum_from_str() {
        let toml_str = r#"
[b00t]
name = "INSTALLER-CAPABILITY"
type = "schema"

[[b00t.check]]
name = "rustc"
check_command = "rustc --version"
remediation = "curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh"
required = true
vendor_datum = ""

[[b00t.check]]
name = "python3"
check_command = "python3 --version"
remediation = "echo 'Install Python'"
required = true

[b00t.daily_routine]
enabled = true
time = "06:00"
max_duration_minutes = 15
parallel = true
sandbox = "ledgrrr"
report_to = "gh-issue"

[b00t.remediation]
max_retries = 2
cooldown_seconds = 300
escalation = "gh-issue"
escalation_threshold = 3
escalation_label = "installer-capability"
escalation_assignee = "@b00t"
"#;

        let registry = InstallerRegistry::from_toml_str(toml_str).unwrap();
        assert_eq!(registry.total(), 2);
        assert_eq!(registry.checks[0].name, "rustc");
        assert!(registry.checks[0].required);
        assert_eq!(registry.checks[0].vendor_datum, Some("".to_string()));
        assert_eq!(registry.checks[1].vendor_datum, None);
        assert!(registry.daily_routine.enabled);
        assert_eq!(registry.remediation.max_retries, 2);
    }

    #[test]
    fn test_required_checks_filter() {
        let toml_str = r#"
[b00t]
name = "test"

[[b00t.check]]
name = "required1"
check_command = "true"
remediation = "true"
required = true
vendor_datum = ""

[[b00t.check]]
name = "optional1"
check_command = "true"
remediation = "true"
required = false
vendor_datum = ""

[[b00t.check]]
name = "required2"
check_command = "true"
remediation = "true"
required = true
vendor_datum = ""
"#;

        let registry = InstallerRegistry::from_toml_str(toml_str).unwrap();
        assert_eq!(registry.required_checks().len(), 2);
        assert_eq!(registry.optional_checks().len(), 1);
    }

    #[test]
    fn test_dashboard_from_results() {
        let check = CapabilityCheck {
            name: "test".to_string(),
            check_command: "true".to_string(),
            remediation: "true".to_string(),
            required: true,
            vendor_datum: None,
        };

        let results = vec![
            CheckResult {
                check: check.clone(),
                status: CheckStatus::Pass,
                output: Some("ok".to_string()),
                error: None,
                duration_ms: 10,
            },
            CheckResult {
                check: check.clone(),
                status: CheckStatus::Fail,
                output: None,
                error: Some("error".to_string()),
                duration_ms: 5,
            },
            CheckResult {
                check,
                status: CheckStatus::Remediated,
                output: Some("fixed".to_string()),
                error: None,
                duration_ms: 100,
            },
        ];

        let dashboard = CapabilityDashboard::from_results(results);
        assert_eq!(dashboard.total_checks, 3);
        assert_eq!(dashboard.passed, 1);
        assert_eq!(dashboard.failed, 1);
        assert_eq!(dashboard.remediated, 1);
        assert_eq!(dashboard.escalated, 0);
        assert!(!dashboard.all_required_passed()); // one still failed
    }

    #[test]
    fn test_vendor_datum_optional() {
        let toml_str = r#"
[b00t]
name = "test"

[[b00t.check]]
name = "no_vendor"
check_command = "true"
remediation = "true"
required = false
"#;

        let registry = InstallerRegistry::from_toml_str(toml_str).unwrap();
        assert_eq!(registry.checks[0].vendor_datum, None);
    }

    #[test]
    fn test_empty_checks_rejected() {
        let toml_str = r#"
[b00t]
name = "test"

[[b00t.check]]
"#;

        let result = InstallerRegistry::from_toml_str(toml_str);
        assert!(result.is_err());
    }

    #[test]
    fn test_run_check_success() {
        let check = CapabilityCheck {
            name: "true".to_string(),
            check_command: "true".to_string(),
            remediation: "true".to_string(),
            required: false,
            vendor_datum: None,
        };

        let result = InstallerEngine::run_check(&check);
        assert_eq!(result.status, CheckStatus::Pass);
        assert!(result.duration_ms > 0);
    }

    #[test]
    fn test_run_check_failure() {
        let check = CapabilityCheck {
            name: "false".to_string(),
            check_command: "false".to_string(),
            remediation: "true".to_string(),
            required: false,
            vendor_datum: None,
        };

        let result = InstallerEngine::run_check(&check);
        assert_eq!(result.status, CheckStatus::Fail);
        assert!(result.error.is_some());
    }

    #[test]
    fn test_remediate_escalates_on_failure() {
        let check = CapabilityCheck {
            name: "unfixable".to_string(),
            check_command: "false".to_string(),
            remediation: "false".to_string(), // remediation also fails
            required: false,
            vendor_datum: None,
        };

        let fail_result = InstallerEngine::run_check(&check);
        assert_eq!(fail_result.status, CheckStatus::Fail);

        let config = RemediationConfig::default();
        let escalated = InstallerEngine::remediate(&check, &fail_result, &config);
        assert_eq!(escalated.status, CheckStatus::Escalated);
    }

    #[test]
    fn test_run_all_returns_results_for_all_checks() {
        let toml_str = r#"
[b00t]
name = "test"

[[b00t.check]]
name = "true_check"
check_command = "true"
remediation = "true"
required = false

[[b00t.check]]
name = "false_check"
check_command = "false"
remediation = "true"
required = false
"#;

        let registry = InstallerRegistry::from_toml_str(toml_str).unwrap();
        let results = InstallerEngine::run_all(&registry);
        assert_eq!(results.len(), 2);
        assert_eq!(results[0].status, CheckStatus::Pass);
        assert_eq!(results[1].status, CheckStatus::Fail);
    }

    #[test]
    fn test_dashboard_pass_rate() {
        let check = CapabilityCheck {
            name: "test".to_string(),
            check_command: "true".to_string(),
            remediation: "true".to_string(),
            required: false,
            vendor_datum: None,
        };

        let results = vec![
            CheckResult {
                check: check.clone(),
                status: CheckStatus::Pass,
                output: None,
                error: None,
                duration_ms: 0,
            },
            CheckResult {
                check: check.clone(),
                status: CheckStatus::Pass,
                output: None,
                error: None,
                duration_ms: 0,
            },
            CheckResult {
                check,
                status: CheckStatus::Fail,
                output: None,
                error: None,
                duration_ms: 0,
            },
        ];

        let dashboard = CapabilityDashboard::from_results(results);
        let rate = dashboard.pass_rate();
        assert!((rate - 66.666_666_666_666_67).abs() < 0.001);
    }

    #[test]
    fn test_empty_dashboard_pass_rate() {
        let dashboard = CapabilityDashboard::from_results(vec![]);
        assert_eq!(dashboard.pass_rate(), 100.0);
    }

    #[test]
    fn test_ooda_loop_fixable() {
        let toml_str = r#"
[b00t]
name = "test"

[b00t.remediation]
max_retries = 2
cooldown_seconds = 300
escalation = "gh-issue"
escalation_threshold = 3
escalation_label = "installer-capability"
escalation_assignee = "@b00t"

[[b00t.check]]
name = "passing_check"
check_command = "true"
remediation = "true"
required = true

[[b00t.check]]
name = "fixable_check"
check_command = "true"
remediation = "true"
required = true
"#;

        let registry = InstallerRegistry::from_toml_str(toml_str).unwrap();
        let results = InstallerEngine::run_ooda(&registry);
        assert_eq!(results.len(), 2);
        assert_eq!(results[0].status, CheckStatus::Pass);
        assert_eq!(results[1].status, CheckStatus::Pass);
    }

    #[test]
    fn test_ooda_loop_escalates_when_remediation_fails() {
        let toml_str = r#"
[b00t]
name = "test"

[b00t.remediation]
max_retries = 2
cooldown_seconds = 300
escalation = "gh-issue"
escalation_threshold = 3
escalation_label = "installer-capability"
escalation_assignee = "@b00t"

[[b00t.check]]
name = "passing_check"
check_command = "true"
remediation = "true"
required = true

[[b00t.check]]
name = "unfixable_check"
check_command = "false"
remediation = "false"
required = true
"#;

        let registry = InstallerRegistry::from_toml_str(toml_str).unwrap();
        let results = InstallerEngine::run_ooda(&registry);
        assert_eq!(results.len(), 2);
        assert_eq!(results[0].status, CheckStatus::Pass);
        // unfixable: fails first, remediation (false) fails → Escalated
        assert_eq!(results[1].status, CheckStatus::Escalated);
    }
}
