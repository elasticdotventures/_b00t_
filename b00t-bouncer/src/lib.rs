// b00t-bouncer — Bouncer pattern gatekeeper for b00t hive agents
// Bouncer pattern: caller → bouncer (input gates) → implementation → bouncer (output gates) → caller

use serde::{Deserialize, Serialize};
use std::io::Write;
use std::path::PathBuf;

/// Bouncer gatekeeper system
/// Enforces input/output validation, constraint enforcement, and audit trail logging
pub struct Bouncer {
    pub config: BouncerConfig,
    audit_log: PathBuf,
}

/// Bouncer configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BouncerConfig {
    pub enabled: bool,
    pub audit_log: String,
    pub input_gates: InputGatesConfig,
    pub output_gates: OutputGatesConfig,
}

/// Input gate configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InputGatesConfig {
    pub sanitize: GateConfig,
    pub credential_check: GateConfig,
    pub permission_check: GateConfig,
    pub rate_limit: GateConfig,
}

/// Output gate configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OutputGatesConfig {
    pub contract_validation: GateConfig,
    pub security_scan: GateConfig,
    pub quality_check: GateConfig,
}

/// Individual gate configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GateConfig {
    pub enabled: bool,
    pub rules: Vec<String>,
}

/// Bouncer gate result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum GateResult {
    Pass,
    Fail { gate: String, reason: String },
    Warn { gate: String, reason: String },
}

/// Bouncer result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum BouncerResult {
    Pass,
    Fail { gate: String, reason: String },
    Warn { gate: String, reason: String },
}

impl Bouncer {
    /// Create a new bouncer with default configuration
    pub fn new() -> Self {
        Self {
            config: BouncerConfig::default(),
            audit_log: PathBuf::from(".b00t/bouncer-audit.jsonl"),
        }
    }

    /// Create a new bouncer with custom configuration
    pub fn with_config(config: BouncerConfig) -> Self {
        Self {
            config: config.clone(),
            audit_log: PathBuf::from(config.audit_log.clone()),
        }
    }

    /// Validate input through input gates
    pub fn validate_input(&self, input: &str) -> BouncerResult {
        if !self.config.enabled {
            return BouncerResult::Pass;
        }

        // Sanitize gate
        if self.config.input_gates.sanitize.enabled {
            if Self::sanitize(input).is_err() {
                return BouncerResult::Fail {
                    gate: "sanitize".to_string(),
                    reason: "input sanitization failed".to_string(),
                };
            }
        }

        // Credential check gate
        if self.config.input_gates.credential_check.enabled {
            if Self::check_credentials(input).is_err() {
                return BouncerResult::Fail {
                    gate: "credential-check".to_string(),
                    reason: "credential exposure detected".to_string(),
                };
            }
        }

        // Permission check gate
        if self.config.input_gates.permission_check.enabled {
            if Self::check_permissions(input).is_err() {
                return BouncerResult::Fail {
                    gate: "permission-check".to_string(),
                    reason: "permission check failed".to_string(),
                };
            }
        }

        // Rate limit gate
        if self.config.input_gates.rate_limit.enabled {
            if Self::check_rate_limit(input).is_err() {
                return BouncerResult::Fail {
                    gate: "rate-limit".to_string(),
                    reason: "rate limit exceeded".to_string(),
                };
            }
        }

        BouncerResult::Pass
    }

    /// Validate output through output gates
    pub fn validate_output(&self, output: &str) -> BouncerResult {
        if !self.config.enabled {
            return BouncerResult::Pass;
        }

        // Contract validation gate
        if self.config.output_gates.contract_validation.enabled {
            if Self::validate_contract(output).is_err() {
                return BouncerResult::Fail {
                    gate: "contract-validation".to_string(),
                    reason: "output contract validation failed".to_string(),
                };
            }
        }

        // Security scan gate
        if self.config.output_gates.security_scan.enabled {
            if Self::security_scan(output).is_err() {
                return BouncerResult::Fail {
                    gate: "security-scan".to_string(),
                    reason: "security scan detected issues".to_string(),
                };
            }
        }

        // Quality check gate
        if self.config.output_gates.quality_check.enabled {
            if Self::quality_check(output).is_err() {
                return BouncerResult::Fail {
                    gate: "quality-check".to_string(),
                    reason: "quality check failed".to_string(),
                };
            }
        }

        BouncerResult::Pass
    }

    /// Log audit entry
    pub fn log_audit(&self, gate: &str, decision: &str, reason: &str) -> Result<(), anyhow::Error> {
        let entry = serde_json::json!({
            "timestamp": chrono::Utc::now().to_rfc3339(),
            "gate": gate,
            "decision": decision,
            "reason": reason,
        });

        let log_line = serde_json::to_string(&entry)?;

        // Append to audit log
        let mut file = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.audit_log)?;
        writeln!(file, "{}", log_line)?;

        Ok(())
    }

    /// Input gate implementations
    fn sanitize(input: &str) -> Result<(), anyhow::Error> {
        // Check for shell injection patterns
        let injection_patterns = [";", "|", "&", "`", "$(", ")", "{", "}", "'", "\"", "\\"];

        for pattern in &injection_patterns {
            if input.contains(pattern) {
                return Err(anyhow::anyhow!(
                    "shell injection pattern detected: {}",
                    pattern
                ));
            }
        }

        Ok(())
    }

    fn check_credentials(input: &str) -> Result<(), anyhow::Error> {
        // Check for credential exposure
        let credential_patterns = [
            "password",
            "secret",
            "token",
            "api_key",
            "apikey",
            "credential",
        ];

        for pattern in &credential_patterns {
            if input.to_lowercase().contains(pattern) {
                return Err(anyhow::anyhow!(
                    "potential credential exposure: {}",
                    pattern
                ));
            }
        }

        Ok(())
    }

    fn check_permissions(input: &str) -> Result<(), anyhow::Error> {
        // Check for permission violations
        let restricted_actions = ["rm -rf /", "sudo", "chmod 777", "chown root"];

        for action in &restricted_actions {
            if input.contains(action) {
                return Err(anyhow::anyhow!("restricted action detected: {}", action));
            }
        }

        Ok(())
    }

    fn check_rate_limit(input: &str) -> Result<(), anyhow::Error> {
        // Simple rate limit check (in production, this would check actual rate limits)
        // For now, just verify input is not empty
        if input.is_empty() {
            return Err(anyhow::anyhow!("empty input"));
        }

        Ok(())
    }

    /// Output gate implementations
    fn validate_contract(output: &str) -> Result<(), anyhow::Error> {
        // Validate output contract (basic check)
        if output.is_empty() {
            return Err(anyhow::anyhow!("empty output"));
        }

        Ok(())
    }

    fn security_scan(output: &str) -> Result<(), anyhow::Error> {
        // Security scan for output
        let security_patterns = ["password", "secret", "token", "api_key", "credential"];

        for pattern in &security_patterns {
            if output.to_lowercase().contains(pattern) {
                return Err(anyhow::anyhow!(
                    "potential credential in output: {}",
                    pattern
                ));
            }
        }

        Ok(())
    }

    fn quality_check(output: &str) -> Result<(), anyhow::Error> {
        // Quality check (basic check)
        if output.len() < 10 {
            return Err(anyhow::anyhow!("output too short"));
        }

        Ok(())
    }
}

impl Default for BouncerConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            audit_log: ".b00t/bouncer-audit.jsonl".to_string(),
            input_gates: InputGatesConfig {
                sanitize: GateConfig {
                    enabled: true,
                    rules: vec![
                        "no-shell-injection".to_string(),
                        "no-credential-exposure".to_string(),
                    ],
                },
                credential_check: GateConfig {
                    enabled: true,
                    rules: vec!["check-env".to_string()],
                },
                permission_check: GateConfig {
                    enabled: true,
                    rules: vec!["check-role".to_string()],
                },
                rate_limit: GateConfig {
                    enabled: true,
                    rules: vec!["max-concurrent-10".to_string()],
                },
            },
            output_gates: OutputGatesConfig {
                contract_validation: GateConfig {
                    enabled: true,
                    rules: vec!["validate-schema".to_string()],
                },
                security_scan: GateConfig {
                    enabled: true,
                    rules: vec!["check-secrets".to_string()],
                },
                quality_check: GateConfig {
                    enabled: true,
                    rules: vec!["min-quality-0.8".to_string()],
                },
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_bouncer_new() {
        let bouncer = Bouncer::new();
        assert!(bouncer.config.enabled);
    }

    #[test]
    fn test_bouncer_with_config() {
        let config = BouncerConfig {
            enabled: false,
            ..Default::default()
        };
        let bouncer = Bouncer::with_config(config);
        assert!(!bouncer.config.enabled);
    }

    #[test]
    fn test_input_gates_sanitize() {
        let bouncer = Bouncer::new();

        // Valid input
        let result = bouncer.validate_input("hello world");
        assert!(matches!(result, BouncerResult::Pass));

        // Invalid input (shell injection)
        let result = bouncer.validate_input("hello; rm -rf /");
        assert!(matches!(result, BouncerResult::Fail { .. }));
    }

    #[test]
    fn test_input_gates_credential_check() {
        let bouncer = Bouncer::new();

        // Valid input
        let result = bouncer.validate_input("hello world");
        assert!(matches!(result, BouncerResult::Pass));

        // Invalid input (credential exposure)
        let result = bouncer.validate_input("password=secret123");
        assert!(matches!(result, BouncerResult::Fail { .. }));
    }

    #[test]
    fn test_input_gates_permission_check() {
        let bouncer = Bouncer::new();

        // Valid input
        let result = bouncer.validate_input("hello world");
        assert!(matches!(result, BouncerResult::Pass));

        // Invalid input (restricted action)
        let result = bouncer.validate_input("rm -rf /");
        assert!(matches!(result, BouncerResult::Fail { .. }));
    }

    #[test]
    fn test_input_gates_rate_limit() {
        let bouncer = Bouncer::new();

        // Valid input
        let result = bouncer.validate_input("hello world");
        assert!(matches!(result, BouncerResult::Pass));

        // Invalid input (empty)
        let result = bouncer.validate_input("");
        assert!(matches!(result, BouncerResult::Fail { .. }));
    }

    #[test]
    fn test_output_gates_contract_validation() {
        let bouncer = Bouncer::new();

        // Valid output
        let result = bouncer.validate_output("hello world this is a valid output");
        assert!(matches!(result, BouncerResult::Pass));

        // Invalid output (empty)
        let result = bouncer.validate_output("");
        assert!(matches!(result, BouncerResult::Fail { .. }));
    }

    #[test]
    fn test_output_gates_security_scan() {
        let bouncer = Bouncer::new();

        // Valid output
        let result = bouncer.validate_output("hello world this is a valid output");
        assert!(matches!(result, BouncerResult::Pass));

        // Invalid output (credential in output)
        let result = bouncer.validate_output("password=secret123");
        assert!(matches!(result, BouncerResult::Fail { .. }));
    }

    #[test]
    fn test_output_gates_quality_check() {
        let bouncer = Bouncer::new();

        // Valid output
        let result = bouncer.validate_output("hello world this is a valid output");
        assert!(matches!(result, BouncerResult::Pass));

        // Invalid output (too short)
        let result = bouncer.validate_output("hi");
        assert!(matches!(result, BouncerResult::Fail { .. }));
    }

    #[test]
    fn test_bouncer_disabled() {
        let config = BouncerConfig {
            enabled: false,
            ..Default::default()
        };
        let bouncer = Bouncer::with_config(config);

        // Should pass even with invalid input when disabled
        let result = bouncer.validate_input("rm -rf /");
        assert!(matches!(result, BouncerResult::Pass));

        let result = bouncer.validate_output("");
        assert!(matches!(result, BouncerResult::Pass));
    }
}
