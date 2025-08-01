//! Type-safe parameter structures for b00t-mcp tools following rmcp 0.3.2 patterns

use schemars::JsonSchema;
use serde::Deserialize;

/// Parameters for CLI tool detection
#[derive(Debug, Deserialize, JsonSchema)]
pub struct DetectParams {
    /// Name of the tool to detect
    pub tool: String,
    /// Enable verbose output
    #[serde(default)]
    pub verbose: bool,
}

/// Parameters for CLI tool desires
#[derive(Debug, Deserialize, JsonSchema)]
pub struct DesiresParams {
    /// Name of the tool to check desires for
    pub tool: String,
    /// Enable verbose output
    #[serde(default)]
    pub verbose: bool,
}

/// Parameters for CLI tool installation
#[derive(Debug, Deserialize, JsonSchema)]
pub struct InstallParams {
    /// Name of the tool to install
    pub tool: String,
    /// Specific version to install (optional)
    pub version: Option<String>,
    /// Force installation even if already present
    #[serde(default)]
    pub force: bool,
    /// Enable verbose output
    #[serde(default)]
    pub verbose: bool,
}

/// Parameters for CLI tool update
#[derive(Debug, Deserialize, JsonSchema)]
pub struct UpdateParams {
    /// Name of the tool to update
    pub tool: String,
    /// Target version (optional, defaults to latest)
    pub version: Option<String>,
    /// Force update even if already up to date
    #[serde(default)]
    pub force: bool,
    /// Enable verbose output
    #[serde(default)]
    pub verbose: bool,
}

/// Parameters for updating all CLI tools
#[derive(Debug, Deserialize, JsonSchema)]
pub struct UpParams {
    /// Enable verbose output
    #[serde(default)]
    pub verbose: bool,
    /// Dry run - show what would be updated
    #[serde(default)]
    pub dry_run: bool,
}

/// Parameters for MCP server addition
#[derive(Debug, Deserialize, JsonSchema)]
pub struct McpAddParams {
    /// Name of the MCP server
    pub name: String,
    /// Command to run the MCP server
    pub command: String,
    /// Arguments for the MCP server command
    #[serde(default)]
    pub args: Vec<String>,
    /// Environment variables for the MCP server
    #[serde(default)]
    pub env: std::collections::HashMap<String, String>,
}

/// Parameters for MCP server listing
#[derive(Debug, Deserialize, JsonSchema)]
pub struct McpListParams {
    /// Enable verbose output
    #[serde(default)]
    pub verbose: bool,
    /// Show only enabled servers
    #[serde(default)]
    pub enabled_only: bool,
}

/// Parameters for status command
#[derive(Debug, Deserialize, JsonSchema)]
pub struct StatusParams {
    /// Enable verbose output
    #[serde(default)]
    pub verbose: bool,
    /// Show detailed system information
    #[serde(default)]
    pub detailed: bool,
}

/// Parameters for session get command
#[derive(Debug, Deserialize, JsonSchema)]
pub struct SessionGetParams {
    /// Key to retrieve from session memory
    pub key: String,
}

/// Parameters for session set command
#[derive(Debug, Deserialize, JsonSchema)]
pub struct SessionSetParams {
    /// Key to set in session memory
    pub key: String,
    /// Value to store
    pub value: String,
}

/// Parameters for session increment command
#[derive(Debug, Deserialize, JsonSchema)]
pub struct SessionIncrParams {
    /// Key to increment in session memory
    pub key: String,
    /// Amount to increment by (default: 1)
    #[serde(default = "default_increment")]
    pub amount: i64,
}

fn default_increment() -> i64 {
    1
}

/// Parameters for learning topics
#[derive(Debug, Deserialize, JsonSchema)]
pub struct LearnParams {
    /// Topic to learn about
    pub topic: String,
    /// Show detailed information
    #[serde(default)]
    pub detailed: bool,
}

/// Parameters for AI provider listing
#[derive(Debug, Deserialize, JsonSchema)]
pub struct AiListParams {
    /// Enable verbose output
    #[serde(default)]
    pub verbose: bool,
}

/// Parameters for AI provider addition
#[derive(Debug, Deserialize, JsonSchema)]
pub struct AiAddParams {
    /// Name of the AI provider
    pub name: String,
    /// API key for the provider
    pub api_key: String,
    /// Base URL for the provider (optional)
    pub base_url: Option<String>,
    /// Model to use (optional)
    pub model: Option<String>,
}

/// Parameters for Kubernetes resource listing
#[derive(Debug, Deserialize, JsonSchema)]
pub struct K8sListParams {
    /// Kubernetes resource type (e.g., pods, services)
    pub resource: String,
    /// Namespace to query (optional, defaults to current namespace)
    pub namespace: Option<String>,
    /// Show all namespaces
    #[serde(default)]
    pub all_namespaces: bool,
}

/// Parameters for Kubernetes deployment
#[derive(Debug, Deserialize, JsonSchema)]
pub struct K8sDeployParams {
    /// Path to deployment manifest
    pub manifest: String,
    /// Namespace to deploy to (optional)
    pub namespace: Option<String>,
    /// Enable verbose output
    #[serde(default)]
    pub verbose: bool,
}

/// Parameters for identity commands (whoami)
#[derive(Debug, Deserialize, JsonSchema)]
pub struct WhoamiParams {
    /// Enable verbose output
    #[serde(default)]
    pub verbose: bool,
}

/// Parameters for IP address information
#[derive(Debug, Deserialize, JsonSchema)]
pub struct WhatismyIpParams {
    /// Show detailed IP information
    #[serde(default)]
    pub detailed: bool,
}

/// Parameters for hostname information
#[derive(Debug, Deserialize, JsonSchema)]
pub struct WhatismyHostnameParams {
    /// Show detailed hostname information
    #[serde(default)]
    pub detailed: bool,
}

/// Empty parameters for commands that don't need arguments
#[derive(Debug, Deserialize, JsonSchema)]
pub struct EmptyParams {}