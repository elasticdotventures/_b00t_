//! Utility functions for the k8s subsystem

use k8s_openapi::api::core::v1::Pod;
use std::collections::BTreeMap;
use tracing::{debug, info, warn};

use crate::k8s::{
    MANAGED_BY_LABEL, MANAGED_BY_VALUE,
    error::{Error, Result},
};

/// Utility functions for working with Kubernetes resources
pub struct K8sUtils;

impl K8sUtils {
    /// Validate a Kubernetes resource name
    pub fn validate_name(name: &str) -> Result<()> {
        if name.is_empty() {
            return Err(Error::generic("Resource name cannot be empty"));
        }

        if name.len() > 253 {
            return Err(Error::generic("Resource name cannot exceed 253 characters"));
        }

        // Check DNS-1123 subdomain compliance
        if !name
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-' || c == '.')
        {
            return Err(Error::generic(
                "Resource name must contain only lowercase alphanumeric characters, '-' or '.'",
            ));
        }

        if name.starts_with('-')
            || name.ends_with('-')
            || name.starts_with('.')
            || name.ends_with('.')
        {
            return Err(Error::generic(
                "Resource name cannot start or end with '-' or '.'",
            ));
        }

        Ok(())
    }

    /// Validate a Kubernetes label key
    pub fn validate_label_key(key: &str) -> Result<()> {
        if key.is_empty() {
            return Err(Error::generic("Label key cannot be empty"));
        }

        if key.len() > 63 {
            return Err(Error::generic("Label key cannot exceed 63 characters"));
        }

        // Check for valid prefix if present
        if let Some((prefix, name)) = key.split_once('/') {
            if prefix.len() > 253 {
                return Err(Error::generic(
                    "Label key prefix cannot exceed 253 characters",
                ));
            }
            Self::validate_name(prefix)?;
            return Self::validate_label_name(name);
        }

        Self::validate_label_name(key)
    }

    /// Validate a label name part (after prefix)
    fn validate_label_name(name: &str) -> Result<()> {
        if name.is_empty() {
            return Err(Error::generic("Label name cannot be empty"));
        }

        if !name
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_' || c == '.')
        {
            return Err(Error::generic(
                "Label name must contain only alphanumeric characters, '-', '_', or '.'",
            ));
        }

        if !name.chars().next().unwrap().is_ascii_alphanumeric() {
            return Err(Error::generic(
                "Label name must start with an alphanumeric character",
            ));
        }

        if !name.chars().last().unwrap().is_ascii_alphanumeric() {
            return Err(Error::generic(
                "Label name must end with an alphanumeric character",
            ));
        }

        Ok(())
    }

    /// Validate a Kubernetes label value
    pub fn validate_label_value(value: &str) -> Result<()> {
        if value.len() > 63 {
            return Err(Error::generic("Label value cannot exceed 63 characters"));
        }

        if value.is_empty() {
            return Ok(()); // Empty values are allowed
        }

        if !value
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_' || c == '.')
        {
            return Err(Error::generic(
                "Label value must contain only alphanumeric characters, '-', '_', or '.'",
            ));
        }

        if !value.chars().next().unwrap().is_ascii_alphanumeric() {
            return Err(Error::generic(
                "Label value must start with an alphanumeric character",
            ));
        }

        if !value.chars().last().unwrap().is_ascii_alphanumeric() {
            return Err(Error::generic(
                "Label value must end with an alphanumeric character",
            ));
        }

        Ok(())
    }

    /// Validate all labels in a map
    pub fn validate_labels(labels: &BTreeMap<String, String>) -> Result<()> {
        for (key, value) in labels {
            Self::validate_label_key(key)?;
            Self::validate_label_value(value)?;
        }
        Ok(())
    }

    /// Check if a pod is ready
    pub fn is_pod_ready(pod: &Pod) -> bool {
        if let Some(ref status) = pod.status {
            if let Some(ref conditions) = status.conditions {
                return conditions
                    .iter()
                    .any(|condition| condition.type_ == "Ready" && condition.status == "True");
            }
        }
        false
    }

    /// Check if a pod is running
    pub fn is_pod_running(pod: &Pod) -> bool {
        if let Some(ref status) = pod.status {
            if let Some(ref phase) = status.phase {
                return phase == "Running";
            }
        }
        false
    }

    /// Check if a pod is managed by b00t
    pub fn is_b00t_managed(pod: &Pod) -> bool {
        if let Some(ref labels) = pod.metadata.labels {
            return labels
                .get(MANAGED_BY_LABEL)
                .map(|v| v == MANAGED_BY_VALUE)
                .unwrap_or(false);
        }
        false
    }

    /// Extract the app name from a pod's labels
    pub fn extract_app_name(pod: &Pod) -> Option<String> {
        if let Some(ref labels) = pod.metadata.labels {
            labels
                .get("app.kubernetes.io/name")
                .cloned()
                .or_else(|| labels.get("app").cloned())
        } else {
            None
        }
    }

    /// Get pod restart count
    pub fn get_restart_count(pod: &Pod) -> i32 {
        if let Some(ref status) = pod.status {
            if let Some(ref container_statuses) = status.container_statuses {
                return container_statuses.iter().map(|cs| cs.restart_count).sum();
            }
        }
        0
    }

    /// Check if pod has any failed containers
    pub fn has_failed_containers(pod: &Pod) -> bool {
        if let Some(ref status) = pod.status {
            if let Some(ref container_statuses) = status.container_statuses {
                return container_statuses.iter().any(|cs| {
                    if let Some(ref state) = cs.state {
                        if let Some(ref terminated) = state.terminated {
                            return terminated.exit_code != 0;
                        }
                        if let Some(ref waiting) = state.waiting {
                            return waiting
                                .reason
                                .as_ref()
                                .map(|r| r.contains("Error") || r.contains("Failed"))
                                .unwrap_or(false);
                        }
                    }
                    false
                });
            }
        }
        false
    }

    /// Get container resource usage (if available)
    pub fn get_resource_usage(_pod: &Pod) -> Option<ResourceUsage> {
        // This would require metrics-server to be available
        // For now, return None - can be implemented later with metrics API
        debug!("Resource usage metrics not yet implemented");
        None
    }

    /// Format pod status for display
    pub fn format_pod_status(pod: &Pod) -> String {
        if let Some(ref status) = pod.status {
            if let Some(ref phase) = status.phase {
                let restart_count = Self::get_restart_count(pod);
                if restart_count > 0 {
                    format!("{} (restarts: {})", phase, restart_count)
                } else {
                    phase.clone()
                }
            } else {
                "Unknown".to_string()
            }
        } else {
            "Pending".to_string()
        }
    }

    /// Wait for a condition with exponential backoff
    pub async fn wait_for_condition<F, Fut>(
        mut condition: F,
        initial_delay_ms: u64,
        max_delay_ms: u64,
        max_attempts: u32,
    ) -> Result<()>
    where
        F: FnMut() -> Fut,
        Fut: std::future::Future<Output = Result<bool>>,
    {
        use tokio::time::{Duration, sleep};

        let mut delay = initial_delay_ms;
        let mut attempts_made = 0;

        for attempt in 1..=max_attempts {
            attempts_made = attempt;
            debug!("Checking condition (attempt {}/{})", attempt, max_attempts);

            match condition().await {
                Ok(true) => {
                    info!("Condition satisfied after {} attempts", attempt);
                    return Ok(());
                }
                Ok(false) => {
                    if attempt < max_attempts {
                        debug!("Condition not met, waiting {}ms before retry", delay);
                        sleep(Duration::from_millis(delay)).await;
                        delay = (delay * 2).min(max_delay_ms); // Exponential backoff with cap
                    }
                }
                Err(e) => {
                    warn!("Error checking condition (attempt {}): {}", attempt, e);
                    if attempt < max_attempts {
                        sleep(Duration::from_millis(delay)).await;
                        delay = (delay * 2).min(max_delay_ms);
                    } else {
                        return Err(e);
                    }
                }
            }
        }

        Err(Error::Generic {
            message: format!("Condition check timed out after {} attempts", attempts_made),
        })
    }

    /// Generate a sanitized resource name from input
    pub fn sanitize_resource_name(input: &str) -> String {
        let mut sanitized = input
            .to_lowercase()
            .chars()
            .map(|c| {
                if c.is_ascii_alphanumeric() || c == '-' {
                    c
                } else {
                    '-'
                }
            })
            .collect::<String>();

        // Remove leading/trailing dashes
        sanitized = sanitized.trim_matches('-').to_string();

        // Ensure it doesn't exceed length limits
        if sanitized.len() > 63 {
            sanitized.truncate(63);
            sanitized = sanitized.trim_end_matches('-').to_string();
        }

        // Ensure it's not empty
        if sanitized.is_empty() {
            sanitized = "resource".to_string();
        }

        sanitized
    }

    /// Merge two label maps, with the second taking precedence
    pub fn merge_labels(
        base: &BTreeMap<String, String>,
        override_labels: &BTreeMap<String, String>,
    ) -> BTreeMap<String, String> {
        let mut result = base.clone();
        result.extend(override_labels.clone());
        result
    }
}

/// Resource usage information (placeholder for future metrics integration)
#[derive(Debug, Clone)]
pub struct ResourceUsage {
    pub cpu_usage: Option<String>,
    pub memory_usage: Option<String>,
    pub cpu_percentage: Option<f64>,
    pub memory_percentage: Option<f64>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_name_validation() {
        // Valid names
        assert!(K8sUtils::validate_name("my-app").is_ok());
        assert!(K8sUtils::validate_name("app123").is_ok());
        assert!(K8sUtils::validate_name("my-app.example.com").is_ok());

        // Invalid names
        assert!(K8sUtils::validate_name("").is_err());
        assert!(K8sUtils::validate_name("My-App").is_err()); // uppercase
        assert!(K8sUtils::validate_name("-my-app").is_err()); // starts with dash
        assert!(K8sUtils::validate_name("my-app-").is_err()); // ends with dash
        assert!(K8sUtils::validate_name("my_app").is_err()); // underscore not allowed in names
    }

    #[test]
    fn test_label_validation() {
        // Valid label keys
        assert!(K8sUtils::validate_label_key("app").is_ok());
        assert!(K8sUtils::validate_label_key("app.kubernetes.io/name").is_ok());
        assert!(K8sUtils::validate_label_key("example.com/app").is_ok());

        // Invalid label keys
        assert!(K8sUtils::validate_label_key("").is_err());
        assert!(K8sUtils::validate_label_key("app/").is_err());
        assert!(K8sUtils::validate_label_key("/app").is_err());

        // Valid label values
        assert!(K8sUtils::validate_label_value("my-app").is_ok());
        assert!(K8sUtils::validate_label_value("").is_ok()); // empty is ok
        assert!(K8sUtils::validate_label_value("app_123").is_ok());

        // Invalid label values
        assert!(K8sUtils::validate_label_value("-my-app").is_err()); // starts with dash
        assert!(K8sUtils::validate_label_value("my-app-").is_err()); // ends with dash
    }

    #[test]
    fn test_sanitize_resource_name() {
        assert_eq!(K8sUtils::sanitize_resource_name("My App!"), "my-app");
        assert_eq!(K8sUtils::sanitize_resource_name("web_server"), "web-server");
        assert_eq!(K8sUtils::sanitize_resource_name("---"), "resource");
        assert_eq!(K8sUtils::sanitize_resource_name("app123"), "app123");
    }

    #[test]
    fn test_merge_labels() {
        let mut base = BTreeMap::new();
        base.insert("app".to_string(), "my-app".to_string());
        base.insert("version".to_string(), "1.0".to_string());

        let mut overrides = BTreeMap::new();
        overrides.insert("version".to_string(), "2.0".to_string());
        overrides.insert("env".to_string(), "prod".to_string());

        let result = K8sUtils::merge_labels(&base, &overrides);

        assert_eq!(result.get("app"), Some(&"my-app".to_string()));
        assert_eq!(result.get("version"), Some(&"2.0".to_string())); // overridden
        assert_eq!(result.get("env"), Some(&"prod".to_string())); // added
    }

    #[test]
    fn test_is_b00t_managed() {
        // Create a mock pod with b00t labels
        let mut labels = BTreeMap::new();
        labels.insert(MANAGED_BY_LABEL.to_string(), MANAGED_BY_VALUE.to_string());

        let pod = Pod {
            metadata: k8s_openapi::apimachinery::pkg::apis::meta::v1::ObjectMeta {
                labels: Some(labels),
                ..Default::default()
            },
            ..Default::default()
        };

        assert!(K8sUtils::is_b00t_managed(&pod));

        // Test pod without labels
        let pod_no_labels = Pod::default();
        assert!(!K8sUtils::is_b00t_managed(&pod_no_labels));
    }
}
