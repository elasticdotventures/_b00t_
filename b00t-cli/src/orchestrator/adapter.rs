// OrchestratorAdapter trait and core types
// Defines the interface for translating b00t datums to orchestrator-specific formats

use anyhow::Result;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::str::FromStr;

use crate::datum_stack::{JobDatum, StackDatum};

/// Supported orchestrators
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Orchestrator {
    #[serde(rename = "kubernetes")]
    Kubernetes,
    #[serde(rename = "docker-compose")]
    DockerCompose,
    #[serde(rename = "nomad")]
    Nomad,
    #[serde(rename = "direct")]
    Direct,
}

impl FromStr for Orchestrator {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self> {
        match s.to_lowercase().as_str() {
            "kubernetes" | "k8s" | "k0s" => Ok(Orchestrator::Kubernetes),
            "docker-compose" | "compose" | "docker" => Ok(Orchestrator::DockerCompose),
            "nomad" => Ok(Orchestrator::Nomad),
            "direct" | "local" | "systemd" => Ok(Orchestrator::Direct),
            _ => anyhow::bail!("Unknown orchestrator: {}", s),
        }
    }
}

impl std::fmt::Display for Orchestrator {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Orchestrator::Kubernetes => write!(f, "kubernetes"),
            Orchestrator::DockerCompose => write!(f, "docker-compose"),
            Orchestrator::Nomad => write!(f, "nomad"),
            Orchestrator::Direct => write!(f, "direct"),
        }
    }
}

/// MCP tool call command
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpCommand {
    pub server: String,   // MCP server name (e.g., "kubernetes-mcp")
    pub tool: String,     // Tool name (e.g., "kubectl_apply")
    pub arguments: Value, // JSON arguments for the tool
}

/// Output from adapter translation
#[derive(Debug, Clone)]
pub struct AdapterOutput {
    pub orchestrator: Orchestrator,
    pub manifests: Vec<String>, // Generated manifests (YAML, HCL, scripts, etc.)
    pub mcp_commands: Vec<McpCommand>, // MCP tool calls for execution
    pub metadata: AdapterMetadata, // Additional metadata
}

/// Metadata about the adapter output
#[derive(Debug, Clone, Default)]
pub struct AdapterMetadata {
    pub warnings: Vec<String>,
    pub dependencies: Vec<String>,
    pub service_endpoints: Vec<ServiceEndpoint>,
}

/// Service endpoint information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServiceEndpoint {
    pub name: String,
    pub port: u16,
    pub protocol: String,
    pub url: String, // Orchestrator-specific URL
}

/// Abstract container specification (from datum)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContainerSpec {
    pub image: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub command: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub args: Option<Vec<String>>,
}

/// Abstract dependency specification
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Dependencies {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub requires_stacks: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub wait_for_services: Option<Vec<String>>, // Format: "service:port"
}

/// Trait for orchestrator-specific adapters
pub trait OrchestratorAdapter: Send + Sync {
    /// Translate a Job datum to orchestrator-specific format
    fn translate_job(&self, job: &JobDatum) -> Result<AdapterOutput>;

    /// Translate a Stack datum to orchestrator-specific format
    fn translate_stack(&self, stack: &StackDatum) -> Result<AdapterOutput>;

    /// Get the orchestrator type
    fn orchestrator(&self) -> Orchestrator;

    /// Check if orchestrator is available
    fn is_available(&self) -> bool;

    /// Get human-readable name
    fn name(&self) -> &str {
        match self.orchestrator() {
            Orchestrator::Kubernetes => "Kubernetes (k8s/k0s)",
            Orchestrator::DockerCompose => "Docker Compose",
            Orchestrator::Nomad => "HashiCorp Nomad",
            Orchestrator::Direct => "Direct Execution",
        }
    }
}

/// Helper to create adapter from orchestrator type
pub fn create_adapter(orchestrator: Orchestrator) -> Result<Box<dyn OrchestratorAdapter>> {
    match orchestrator {
        Orchestrator::Kubernetes => {
            Ok(Box::new(crate::orchestrator::k8s_adapter::K8sAdapter::new()))
        }
        _ => anyhow::bail!("Adapter for {} not yet implemented", orchestrator),
    }
}
