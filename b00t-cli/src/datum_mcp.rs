use crate::traits::*;
use crate::{BootDatum, check_command_available, get_config};
use anyhow::Result;
use serde::{Deserialize, Serialize};

// MCP-specific multi-method structures

/// Stdio-based MCP server method configuration (MCP nomenclature)
/// 
/// Defines how to execute an MCP server via stdio transport (command line interface).
/// Multiple stdio methods can be specified with different priorities.
/// 
/// # Examples
/// 
/// ```toml
/// [[b00t.stdio]]
/// command = "npx"
/// args = ["-y", "@modelcontextprotocol/server-filesystem"]
/// priority = 0
/// requires = ["node"]
/// transport = "stdio"
/// 
/// [[b00t.stdio]]
/// command = "uvx"  
/// args = ["mcp-server-filesystem"]
/// priority = 1
/// requires = ["python"]
/// transport = "stdio"
/// ```
#[derive(Deserialize, Serialize, Debug, Clone, PartialEq)]
pub struct McpStdioMethod {
    pub command: String,
    pub args: Vec<String>,
    #[serde(default)]
    pub priority: u8,
    #[serde(default)]
    pub requires: Vec<String>,
    #[serde(default)]
    pub env: std::collections::HashMap<String, String>,
    #[serde(default = "default_stdio_transport")]
    pub transport: String,
}

/// HTTP stream-based MCP server method configuration (MCP nomenclature)
/// 
/// Defines how to connect to an MCP server via HTTP stream transport.
/// 
/// # Examples
/// 
/// ```toml
/// [b00t.httpstream]
/// url = "https://mcp-server.example.com"
/// priority = 0
/// requires = ["internet"]
/// requires_internet = true
/// requires_auth = false
/// transport = "httpstream"
/// ```
#[derive(Deserialize, Serialize, Debug, Clone, PartialEq)]
pub struct McpHttpStreamMethod {
    pub url: String,
    #[serde(default)]
    pub priority: u8,
    #[serde(default)]
    pub requires: Vec<String>,
    #[serde(default = "default_true")]
    pub requires_internet: bool,
    #[serde(default)]
    pub requires_auth: bool,
    #[serde(default = "default_httpstream_transport")]
    pub transport: String,
}

fn default_true() -> bool {
    true
}

fn default_stdio_transport() -> String {
    "stdio".to_string()
}

fn default_httpstream_transport() -> String {
    "httpstream".to_string()
}

/// Multi-method MCP server configuration datum
/// 
/// Manages MCP server configurations with support for multiple deployment methods.
/// Automatically selects the best available method based on priority and system requirements.
/// 
/// # Examples
/// 
/// ```rust
/// use b00t_cli::datum_mcp::McpDatum;
/// 
/// // Load MCP server configuration
/// let mcp = McpDatum::from_config("filesystem", "~/.dotfiles/_b00t_").unwrap();
/// 
/// // Select best available method
/// if let Some(method) = mcp.select_best_method() {
///     println!("Selected method: {:?}", method);
/// }
/// ```
pub struct McpDatum {
    pub datum: BootDatum,
}

impl McpDatum {
    pub fn from_config(name: &str, path: &str) -> Result<Self> {
        let (config, _filename) = get_config(name, path).map_err(|e| anyhow::anyhow!("{}", e))?;
        Ok(McpDatum { datum: config.b00t })
    }

    // Helper to parse stdio methods from raw data
    fn parse_stdio_methods(&self) -> Vec<McpStdioMethod> {
        if let Some(mcp) = &self.datum.mcp {
            if let Some(stdio_data) = &mcp.stdio {
                stdio_data.iter()
                    .filter_map(|raw| serde_json::from_value(serde_json::Value::Object(
                        raw.iter().map(|(k, v)| (k.clone(), v.clone())).collect()
                    )).ok())
                    .collect()
            } else {
                Vec::new()
            }
        } else {
            Vec::new()
        }
    }

    // Helper to parse HTTP stream method from raw data  
    fn parse_httpstream_method(&self) -> Option<McpHttpStreamMethod> {
        if let Some(mcp) = &self.datum.mcp {
            if let Some(httpstream_data) = &mcp.httpstream {
                serde_json::from_value(serde_json::Value::Object(
                    httpstream_data.iter().map(|(k, v)| (k.clone(), v.clone())).collect()
                )).ok()
            } else {
                None
            }
        } else {
            None
        }
    }

    /// Select the best available method based on priority and availability
    pub fn select_best_method(&self) -> Option<McpSelectedMethod> {
        let mut candidates = Vec::new();

        // Add HTTP stream method if available
        if let Some(httpstream) = self.parse_httpstream_method() {
            if self.check_httpstream_requirements(&httpstream) {
                candidates.push(McpSelectedMethod::HttpStream(httpstream));
            }
        }

        // Add stdio methods if available
        let stdio_methods = self.parse_stdio_methods();
        for stdio in stdio_methods {
            if self.check_stdio_requirements(&stdio) {
                candidates.push(McpSelectedMethod::Stdio(stdio));
            }
        }

        // Return None if no multi-methods available (legacy format no longer supported)
        if candidates.is_empty() {
            return None;
        }

        // Sort by priority (lower number = higher priority)
        candidates.sort_by_key(|method| method.priority());
        candidates.into_iter().next()
    }

    fn check_httpstream_requirements(&self, httpstream: &McpHttpStreamMethod) -> bool {
        // Check if internet is required and available
        if httpstream.requires_internet {
            // TODO: Add actual internet connectivity check
            // For now, assume internet is available
        }
        
        // Check other requirements
        self.evaluate_method_constraints(&httpstream.requires)
    }

    fn check_stdio_requirements(&self, stdio: &McpStdioMethod) -> bool {
        // Check if command is available
        if !check_command_available(&stdio.command) {
            return false;
        }
        
        // Check method-specific constraints
        self.evaluate_method_constraints(&stdio.requires)
    }

    fn evaluate_method_constraints(&self, requires: &[String]) -> bool {
        if requires.is_empty() {
            return true;
        }

        requires.iter().all(|constraint| {
            match constraint.as_str() {
                "node" => check_command_available("node") || check_command_available("npx"),
                "python" => check_command_available("python") || check_command_available("python3") || check_command_available("uvx"),
                "docker" => check_command_available("docker"),
                "internet" => true, // TODO: Add actual internet check
                constraint if constraint.starts_with("CMD:") => {
                    check_command_available(&constraint[4..])
                }
                _ => true, // Unknown constraints default to true
            }
        })
    }
}

#[derive(Debug, Clone)]
pub enum McpSelectedMethod {
    HttpStream(McpHttpStreamMethod),
    Stdio(McpStdioMethod),
}

impl McpSelectedMethod {
    pub fn priority(&self) -> u8 {
        match self {
            McpSelectedMethod::HttpStream(httpstream) => httpstream.priority,
            McpSelectedMethod::Stdio(stdio) => stdio.priority,
        }
    }

    pub fn command(&self) -> Option<&str> {
        match self {
            McpSelectedMethod::HttpStream(_) => None,
            McpSelectedMethod::Stdio(stdio) => Some(&stdio.command),
        }
    }

    pub fn args(&self) -> Option<&[String]> {
        match self {
            McpSelectedMethod::HttpStream(_) => None,
            McpSelectedMethod::Stdio(stdio) => Some(&stdio.args),
        }
    }

    pub fn is_httpstream(&self) -> bool {
        matches!(self, McpSelectedMethod::HttpStream(_))
    }

    pub fn url(&self) -> Option<&str> {
        match self {
            McpSelectedMethod::HttpStream(httpstream) => Some(&httpstream.url),
            McpSelectedMethod::Stdio(_) => None,
        }
    }
}

impl TryFrom<(&str, &str)> for McpDatum {
    type Error = anyhow::Error;

    fn try_from((name, path): (&str, &str)) -> Result<Self, Self::Error> {
        Self::from_config(name, path)
    }
}

impl DatumChecker for McpDatum {
    fn is_installed(&self) -> bool {
        // Check if any method is available
        self.select_best_method().is_some()
    }

    fn current_version(&self) -> Option<String> {
        // MCP servers don't typically have semantic versions
        // Return the selected method info if available
        if let Some(method) = self.select_best_method() {
            match method {
                McpSelectedMethod::HttpStream(httpstream) => Some(format!("HTTP Stream: {}", httpstream.url)),
                McpSelectedMethod::Stdio(stdio) => Some(format!("{} available", stdio.command)),
            }
        } else {
            None
        }
    }

    fn desired_version(&self) -> Option<String> {
        self.datum.desires.clone()
    }

    fn version_status(&self) -> VersionStatus {
        if DatumChecker::is_installed(self) {
            VersionStatus::Unknown // MCP servers are just available/not available
        } else {
            VersionStatus::Missing
        }
    }
}

impl StatusProvider for McpDatum {
    fn name(&self) -> &str {
        &self.datum.name
    }

    fn subsystem(&self) -> &str {
        "mcp"
    }

    fn hint(&self) -> &str {
        &self.datum.hint
    }

    fn is_disabled(&self) -> bool {
        false // MCP servers are never disabled by default
    }
}

impl FilterLogic for McpDatum {
    fn is_available(&self) -> bool {
        !DatumChecker::is_installed(self) && self.prerequisites_satisfied()
    }

    fn prerequisites_satisfied(&self) -> bool {
        // Check if require constraints are satisfied
        if let Some(require) = &self.datum.require {
            self.evaluate_constraints(require)
        } else {
            // Default: check if all ENV vars are satisfied
            self.evaluate_constraints(&[])
        }
    }

    fn evaluate_constraints(&self, require: &[String]) -> bool {
        self.evaluate_constraints_default(require)
    }
}

impl ConstraintEvaluator for McpDatum {
    fn datum(&self) -> &BootDatum {
        &self.datum
    }
}

impl DatumProvider for McpDatum {
    fn datum(&self) -> &BootDatum {
        &self.datum
    }
}
