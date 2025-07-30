//! Error handling for the b00t k8s subsystem using snafu

use snafu::Snafu;

/// Result type alias for the k8s subsystem
pub type Result<T, E = Error> = std::result::Result<T, E>;

/// Comprehensive error types for k8s operations
#[derive(Debug, Snafu)]
#[snafu(visibility(pub))]
pub enum Error {
    /// Kubernetes client errors from kube-rs
    #[snafu(display("Kubernetes client error: {}", source))]
    KubeClient { source: kube::Error },

    /// Configuration-related errors
    #[snafu(display("Configuration error: {}", message))]
    Configuration { message: String },

    /// Resource operation errors
    #[snafu(display("Resource error for {}: {}", resource_name, message))]
    Resource {
        resource_name: String,
        message: String,
    },

    /// Pod deployment specific errors
    #[snafu(display("Pod deployment failed for '{}': {}", pod_name, reason))]
    PodDeployment { pod_name: String, reason: String },

    /// Translation engine errors
    #[snafu(display("Translation error from {} to k8s: {}", source_type, message))]
    Translation {
        source_type: String,
        message: String,
    },

    /// Cluster connectivity errors
    #[snafu(display("Failed to connect to cluster: {}", details))]
    ClusterConnection { details: String },

    /// Namespace operation errors
    #[snafu(display("Namespace '{}' error: {}", namespace, message))]
    Namespace { namespace: String, message: String },

    /// YAML/JSON serialization errors
    #[snafu(display("Serialization error: {}", source))]
    Serialization { source: serde_yaml::Error },

    /// I/O errors (file operations, etc.)
    #[snafu(display("I/O error: {}", source))]
    Io { source: std::io::Error },

    /// Chart operations (helm integration)
    #[snafu(display("Chart operation failed for '{}': {}", chart_name, message))]
    Chart { chart_name: String, message: String },

    /// MCP server deployment errors
    #[snafu(display("MCP server '{}' deployment error: {}", server_name, message))]
    McpDeployment {
        server_name: String,
        message: String,
    },

    /// Resource validation errors
    #[snafu(display("Resource validation failed: {}", details))]
    Validation { details: String },

    /// Timeout errors for long-running operations
    #[snafu(display("Operation timed out after {}s: {}", timeout_seconds, operation))]
    Timeout {
        timeout_seconds: u64,
        operation: String,
    },

    /// Generic errors for cases not covered above
    #[snafu(display("k8s subsystem error: {}", message))]
    Generic { message: String },
}

/// Convenience functions for creating specific error types
impl Error {
    /// Create a configuration error
    pub fn config(message: impl Into<String>) -> Self {
        Error::Configuration {
            message: message.into(),
        }
    }

    /// Create a resource error
    pub fn resource(resource_name: impl Into<String>, message: impl Into<String>) -> Self {
        Error::Resource {
            resource_name: resource_name.into(),
            message: message.into(),
        }
    }

    /// Create a translation error
    pub fn translation(source_type: impl Into<String>, message: impl Into<String>) -> Self {
        Error::Translation {
            source_type: source_type.into(),
            message: message.into(),
        }
    }

    /// Create a generic error
    pub fn generic(message: impl Into<String>) -> Self {
        Error::Generic {
            message: message.into(),
        }
    }

    /// Create a validation error
    pub fn validation(details: impl Into<String>) -> Self {
        Error::Validation {
            details: details.into(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_error_creation() {
        let config_err = Error::config("test config error");
        assert!(matches!(config_err, Error::Configuration { .. }));

        let resource_err = Error::resource("test-pod", "failed to create");
        assert!(matches!(resource_err, Error::Resource { .. }));

        let translation_err = Error::translation("dockerfile", "invalid syntax");
        assert!(matches!(translation_err, Error::Translation { .. }));

        let generic_err = Error::generic("something went wrong");
        assert!(matches!(generic_err, Error::Generic { .. }));
    }

    #[test]
    fn test_error_display() {
        let err = Error::config("test message");
        assert!(err.to_string().contains("Configuration error"));
        assert!(err.to_string().contains("test message"));
    }
}
