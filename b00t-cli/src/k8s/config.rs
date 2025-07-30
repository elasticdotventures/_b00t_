//! Configuration management for the k8s subsystem

use crate::k8s::error::{Error, Result};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// Configuration for the k8s subsystem
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct K8sConfig {
    /// Default namespace for operations
    pub namespace: String,

    /// Kubeconfig file path (optional, uses default if None)
    pub kubeconfig_path: Option<PathBuf>,

    /// Context to use from kubeconfig
    pub context: Option<String>,

    /// Timeout for k8s operations in seconds
    pub timeout_seconds: u64,

    /// Whether to auto-create namespace if it doesn't exist
    pub auto_create_namespace: bool,

    /// Labels to apply to all b00t-managed resources
    pub default_labels: std::collections::HashMap<String, String>,

    /// Pod resource limits and requests
    pub resource_defaults: ResourceDefaults,

    /// Translation engine settings
    pub translation: TranslationConfig,
}

/// Default resource limits and requests for pods
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResourceDefaults {
    /// Default CPU request
    pub cpu_request: Option<String>,

    /// Default memory request
    pub memory_request: Option<String>,

    /// Default CPU limit
    pub cpu_limit: Option<String>,

    /// Default memory limit
    pub memory_limit: Option<String>,
}

/// Configuration for the translation engine
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TranslationConfig {
    /// Whether to enable LLM-powered smart translations
    pub enable_llm_translation: bool,

    /// Default image pull policy
    pub default_image_pull_policy: String,

    /// Default restart policy for pods
    pub default_restart_policy: String,

    /// Whether to generate services for exposed ports
    pub auto_generate_services: bool,

    /// Default service type for generated services
    pub default_service_type: String,
}

impl Default for K8sConfig {
    fn default() -> Self {
        let mut default_labels = std::collections::HashMap::new();
        default_labels.insert(
            super::MANAGED_BY_LABEL.to_string(),
            super::MANAGED_BY_VALUE.to_string(),
        );
        default_labels.insert("b00t.version".to_string(), super::VERSION.to_string());

        Self {
            namespace: super::DEFAULT_NAMESPACE.to_string(),
            kubeconfig_path: None,
            context: None,
            timeout_seconds: 60,
            auto_create_namespace: true,
            default_labels,
            resource_defaults: ResourceDefaults::default(),
            translation: TranslationConfig::default(),
        }
    }
}

impl Default for ResourceDefaults {
    fn default() -> Self {
        Self {
            cpu_request: Some("100m".to_string()),
            memory_request: Some("128Mi".to_string()),
            cpu_limit: Some("500m".to_string()),
            memory_limit: Some("512Mi".to_string()),
        }
    }
}

impl Default for TranslationConfig {
    fn default() -> Self {
        Self {
            enable_llm_translation: false,
            default_image_pull_policy: "IfNotPresent".to_string(),
            default_restart_policy: "Always".to_string(),
            auto_generate_services: true,
            default_service_type: "ClusterIP".to_string(),
        }
    }
}

impl K8sConfig {
    /// Load configuration from a TOML file
    pub fn from_file<P: AsRef<std::path::Path>>(path: P) -> Result<Self> {
        let content = std::fs::read_to_string(path)
            .map_err(|e| Error::config(format!("Failed to read config file: {}", e)))?;

        let config: K8sConfig = toml::from_str(&content)
            .map_err(|e| Error::config(format!("Failed to parse config file: {}", e)))?;

        Ok(config)
    }

    /// Save configuration to a TOML file
    pub fn to_file<P: AsRef<std::path::Path>>(&self, path: P) -> Result<()> {
        let content = toml::to_string_pretty(self)
            .map_err(|e| Error::config(format!("Failed to serialize config: {}", e)))?;

        std::fs::write(path, content)
            .map_err(|e| Error::config(format!("Failed to write config file: {}", e)))?;

        Ok(())
    }

    /// Get the effective kubeconfig path (respects KUBECONFIG env var)
    pub fn effective_kubeconfig_path(&self) -> Option<PathBuf> {
        // Priority: explicit config > KUBECONFIG env > default
        if let Some(ref path) = self.kubeconfig_path {
            return Some(path.clone());
        }

        if let Ok(kubeconfig) = std::env::var("KUBECONFIG") {
            return Some(PathBuf::from(kubeconfig));
        }

        // Default kubeconfig location
        if let Ok(home) = std::env::var("HOME") {
            Some(PathBuf::from(home).join(".kube").join("config"))
        } else {
            None
        }
    }

    /// Validate the configuration
    pub fn validate(&self) -> Result<()> {
        if self.namespace.is_empty() {
            return Err(Error::config("Namespace cannot be empty"));
        }

        if self.timeout_seconds == 0 {
            return Err(Error::config("Timeout must be greater than 0"));
        }

        // Validate resource defaults
        if let Some(ref cpu_request) = self.resource_defaults.cpu_request {
            if cpu_request.is_empty() {
                return Err(Error::config("CPU request cannot be empty"));
            }
        }

        if let Some(ref memory_request) = self.resource_defaults.memory_request {
            if memory_request.is_empty() {
                return Err(Error::config("Memory request cannot be empty"));
            }
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::NamedTempFile;

    #[test]
    fn test_default_config() {
        let config = K8sConfig::default();
        assert_eq!(config.namespace, "default");
        assert_eq!(config.timeout_seconds, 60);
        assert!(config.auto_create_namespace);
        assert!(
            config
                .default_labels
                .contains_key("app.kubernetes.io/managed-by")
        );
    }

    #[test]
    fn test_config_validation() {
        let mut config = K8sConfig::default();
        assert!(config.validate().is_ok());

        config.namespace = String::new();
        assert!(config.validate().is_err());

        config.namespace = "test".to_string();
        config.timeout_seconds = 0;
        assert!(config.validate().is_err());
    }

    #[test]
    fn test_config_serialization() {
        let config = K8sConfig::default();
        let temp_file = NamedTempFile::new().unwrap();

        // Test saving
        assert!(config.to_file(temp_file.path()).is_ok());

        // Test loading
        let loaded_config = K8sConfig::from_file(temp_file.path()).unwrap();
        assert_eq!(config.namespace, loaded_config.namespace);
        assert_eq!(config.timeout_seconds, loaded_config.timeout_seconds);
    }

    #[test]
    fn test_effective_kubeconfig_path() {
        let config = K8sConfig::default();

        // Should return default path when no explicit config
        let path = config.effective_kubeconfig_path();
        assert!(path.is_some());
        assert!(path.unwrap().to_string_lossy().contains(".kube/config"));
    }
}
