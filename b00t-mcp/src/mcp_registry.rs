//! b00t MCP Registry Implementation
//!
//! Provides a local MCP registry that can:
//! - Register and discover MCP servers locally
//! - Proxy to the official MCP registry (modelcontextprotocol/registry)
//! - Auto-discover tools from registered servers
//! - Act as both an MCP server AND a registry

use anyhow::{Result, Context};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
use chrono::{DateTime, Utc};
use tracing::{info, warn, debug};

/// MCP server registration entry in b00t registry
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpServerRegistration {
    /// Unique server identifier (e.g., "io.github.username/server-name")
    pub id: String,
    /// Human-readable server name
    pub name: String,
    /// Server description
    pub description: String,
    /// Server version
    pub version: String,
    /// Server homepage URL
    pub homepage: Option<String>,
    /// Server documentation URL
    pub documentation: Option<String>,
    /// Server license
    pub license: Option<String>,
    /// Tags for categorization
    pub tags: Vec<String>,
    /// Server configuration
    pub config: McpServerConfig,
    /// Registration metadata
    pub metadata: RegistrationMetadata,
}

/// MCP server configuration (compatible with MCP registry format)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpServerConfig {
    /// Server command
    pub command: String,
    /// Command arguments
    #[serde(default)]
    pub args: Vec<String>,
    /// Environment variables
    #[serde(skip_serializing_if = "Option::is_none")]
    pub env: Option<HashMap<String, String>>,
    /// Working directory
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cwd: Option<String>,
    /// Server transport type
    #[serde(default = "default_transport")]
    pub transport: ServerTransport,
}

fn default_transport() -> ServerTransport {
    ServerTransport::Stdio
}

/// MCP server transport types
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ServerTransport {
    /// Standard input/output (default)
    Stdio,
    /// HTTP streaming
    #[serde(rename = "http-stream")]
    HttpStream,
    /// WebSocket
    Websocket,
}

/// Registration metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RegistrationMetadata {
    /// Registration timestamp
    pub registered_at: DateTime<Utc>,
    /// Last updated timestamp
    pub updated_at: DateTime<Utc>,
    /// Source of registration (local, official-registry, discovered)
    pub source: RegistrationSource,
    /// Health check status
    pub health_status: HealthStatus,
    /// Last health check timestamp
    pub last_health_check: Option<DateTime<Utc>>,
    /// Dependencies required by this MCP server
    #[serde(default)]
    pub dependencies: Vec<Dependency>,
    /// Installation status
    #[serde(default)]
    pub installation_status: InstallationStatus,
}

/// Dependency required by an MCP server
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Dependency {
    /// Dependency type (docker, node, python, etc.)
    pub dep_type: DependencyType,
    /// Minimum version required (optional)
    pub min_version: Option<String>,
    /// Whether this dependency is currently installed
    pub installed: bool,
    /// Installation command/method
    pub install_method: Option<String>,
}

/// Type of dependency
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum DependencyType {
    /// Docker container runtime
    Docker,
    /// Node.js runtime
    Node,
    /// npm package manager
    Npm,
    /// Python runtime
    Python,
    /// pip package manager
    Pip,
    /// Rust toolchain
    Rust,
    /// Generic system package
    System(String),
}

/// Installation status of an MCP server
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum InstallationStatus {
    /// Not yet installed
    #[default]
    NotInstalled,
    /// Installation in progress
    Installing,
    /// Successfully installed
    Installed,
    /// Installation failed
    Failed(String),
}

/// Registration source
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum RegistrationSource {
    /// Manually registered locally
    Local,
    /// Synced from official MCP registry
    OfficialRegistry,
    /// Auto-discovered from system
    Discovered,
    /// Imported from configuration file
    Imported,
}

/// Health status
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum HealthStatus {
    /// Server is healthy
    Healthy,
    /// Server status unknown
    Unknown,
    /// Server is unhealthy
    Unhealthy,
}

/// b00t MCP Registry manager
pub struct McpRegistry {
    /// Registered servers
    servers: HashMap<String, McpServerRegistration>,
    /// Registry storage path
    storage_path: PathBuf,
    /// Enable sync with official registry
    enable_official_sync: bool,
}

impl McpRegistry {
    /// Create new MCP registry
    pub fn new(storage_path: PathBuf) -> Result<Self> {
        let mut registry = Self {
            servers: HashMap::new(),
            storage_path,
            enable_official_sync: true,
        };

        registry.load()?;
        Ok(registry)
    }

    /// Load registry from storage
    fn load(&mut self) -> Result<()> {
        if !self.storage_path.exists() {
            debug!("Registry storage not found, creating new registry");
            return Ok(());
        }

        let data = std::fs::read_to_string(&self.storage_path)
            .context("Failed to read registry storage")?;

        self.servers = serde_json::from_str(&data)
            .context("Failed to parse registry storage")?;

        info!("📂 Loaded {} servers from registry", self.servers.len());
        Ok(())
    }

    /// Save registry to storage
    fn save(&self) -> Result<()> {
        // Ensure parent directory exists
        if let Some(parent) = self.storage_path.parent() {
            std::fs::create_dir_all(parent)
                .context("Failed to create registry directory")?;
        }

        let data = serde_json::to_string_pretty(&self.servers)
            .context("Failed to serialize registry")?;

        std::fs::write(&self.storage_path, data)
            .context("Failed to write registry storage")?;

        debug!("💾 Saved registry with {} servers", self.servers.len());
        Ok(())
    }

    /// Register an MCP server
    pub fn register(&mut self, registration: McpServerRegistration) -> Result<()> {
        let server_id = registration.id.clone();

        info!("📝 Registering MCP server: {}", server_id);

        // Validate registration
        self.validate_registration(&registration)?;

        self.servers.insert(server_id.clone(), registration);
        self.save()?;

        info!("✅ Successfully registered MCP server: {}", server_id);
        Ok(())
    }

    /// Unregister an MCP server
    pub fn unregister(&mut self, server_id: &str) -> Result<()> {
        if self.servers.remove(server_id).is_some() {
            self.save()?;
            info!("🗑️  Unregistered MCP server: {}", server_id);
            Ok(())
        } else {
            Err(anyhow::anyhow!("Server '{}' not found in registry", server_id))
        }
    }

    /// Get server registration
    pub fn get(&self, server_id: &str) -> Option<&McpServerRegistration> {
        self.servers.get(server_id)
    }

    /// List all registered servers
    pub fn list(&self) -> Vec<&McpServerRegistration> {
        self.servers.values().collect()
    }

    /// Search servers by tag
    pub fn search_by_tag(&self, tag: &str) -> Vec<&McpServerRegistration> {
        self.servers
            .values()
            .filter(|s| s.tags.iter().any(|t| t.contains(tag)))
            .collect()
    }

    /// Search servers by keyword
    pub fn search(&self, keyword: &str) -> Vec<&McpServerRegistration> {
        let keyword_lower = keyword.to_lowercase();
        self.servers
            .values()
            .filter(|s| {
                s.name.to_lowercase().contains(&keyword_lower) ||
                s.description.to_lowercase().contains(&keyword_lower) ||
                s.tags.iter().any(|t| t.to_lowercase().contains(&keyword_lower))
            })
            .collect()
    }

    /// Update server health status
    pub fn update_health(&mut self, server_id: &str, status: HealthStatus) -> Result<()> {
        if let Some(registration) = self.servers.get_mut(server_id) {
            registration.metadata.health_status = status;
            registration.metadata.last_health_check = Some(Utc::now());
            self.save()?;
            Ok(())
        } else {
            Err(anyhow::anyhow!("Server '{}' not found", server_id))
        }
    }

    /// Validate registration
    fn validate_registration(&self, registration: &McpServerRegistration) -> Result<()> {
        // Validate server ID format
        if registration.id.is_empty() {
            return Err(anyhow::anyhow!("Server ID cannot be empty"));
        }

        // Validate command exists
        if registration.config.command.is_empty() {
            return Err(anyhow::anyhow!("Server command cannot be empty"));
        }

        Ok(())
    }

    /// Export registry to MCP registry format
    pub fn export_to_mcp_format(&self) -> Result<String> {
        #[derive(Serialize)]
        struct McpRegistryExport {
            version: String,
            servers: Vec<McpServerRegistration>,
        }

        let export = McpRegistryExport {
            version: "1.0.0".to_string(),
            servers: self.servers.values().cloned().collect(),
        };

        serde_json::to_string_pretty(&export)
            .context("Failed to export registry")
    }

    /// Import from MCP registry format
    pub fn import_from_mcp_format(&mut self, json: &str) -> Result<usize> {
        #[derive(Deserialize)]
        struct McpRegistryImport {
            servers: Vec<McpServerRegistration>,
        }

        let import: McpRegistryImport = serde_json::from_str(json)
            .context("Failed to parse import data")?;

        let mut imported_count = 0;
        for mut server in import.servers {
            server.metadata.source = RegistrationSource::Imported;
            server.metadata.updated_at = Utc::now();
            self.servers.insert(server.id.clone(), server);
            imported_count += 1;
        }

        if imported_count > 0 {
            self.save()?;
        }

        Ok(imported_count)
    }

    /// Sync with official MCP registry
    pub async fn sync_official_registry(&mut self) -> Result<usize> {
        if !self.enable_official_sync {
            return Ok(0);
        }

        info!("🔄 Syncing with official MCP registry");

        // Fetch servers from official registry API
        let official_servers = self.fetch_official_servers().await?;
        let mut synced_count = 0;

        for server in official_servers {
            // Only add if not already registered locally
            if !self.servers.contains_key(&server.id) {
                info!("📥 Adding server from official registry: {}", server.id);
                self.servers.insert(server.id.clone(), server);
                synced_count += 1;
            }
        }

        if synced_count > 0 {
            self.save()?;
        }

        info!("✅ Synced {} servers from official registry", synced_count);
        Ok(synced_count)
    }

    /// Fetch servers from official MCP registry
    async fn fetch_official_servers(&self) -> Result<Vec<McpServerRegistration>> {
        // 🤓 This would query the official MCP registry API
        // For now, return empty vec - implementation depends on registry API availability
        info!("🌐 Fetching from official MCP registry (not yet implemented)");
        Ok(Vec::new())
    }

    /// Auto-discover MCP servers from system
    pub async fn auto_discover(&mut self) -> Result<usize> {
        info!("🔍 Auto-discovering MCP servers from system");

        let mut discovered_count = 0;

        // Check common MCP server locations
        let discovery_paths = vec![
            dirs::home_dir().map(|h| h.join(".local/share/mcp/servers")),
            dirs::home_dir().map(|h| h.join(".config/mcp/servers")),
            Some(PathBuf::from("/usr/local/share/mcp/servers")),
        ];

        for path in discovery_paths.into_iter().flatten() {
            if let Ok(discovered) = self.discover_from_path(&path).await {
                discovered_count += discovered;
            }
        }

        if discovered_count > 0 {
            self.save()?;
            info!("✅ Discovered {} MCP servers", discovered_count);
        }

        Ok(discovered_count)
    }

    /// Discover servers from a specific path
    async fn discover_from_path(&mut self, _path: &PathBuf) -> Result<usize> {
        // 🤓 Implementation would scan path for MCP server configurations
        Ok(0)
    }

    /// Install dependencies for an MCP server
    pub async fn install_dependencies(&mut self, server_id: &str) -> Result<()> {
        // Clone dependencies to avoid borrow conflicts
        let dependencies = {
            let registration = self.servers.get_mut(server_id)
                .ok_or_else(|| anyhow::anyhow!("Server '{}' not found", server_id))?;

            info!("📦 Installing dependencies for {}", server_id);
            registration.metadata.installation_status = InstallationStatus::Installing;
            registration.metadata.dependencies.clone()
        };
        self.save()?;

        // Check and install each dependency
        let mut installed_deps = Vec::new();
        for mut dep in dependencies {
            if dep.installed {
                debug!("✅ Dependency {:?} already installed", dep.dep_type);
                installed_deps.push(dep);
                continue;
            }

            info!("🔧 Installing dependency: {:?}", dep.dep_type);
            match self.install_dependency(&dep).await {
                Ok(()) => {
                    dep.installed = true;
                    info!("✅ Successfully installed {:?}", dep.dep_type);
                    installed_deps.push(dep);
                }
                Err(e) => {
                    let error_msg = format!("Failed to install {:?}: {}", dep.dep_type, e);
                    warn!("⚠️  {}", error_msg);
                    let reg = self.servers.get_mut(server_id).unwrap();
                    reg.metadata.installation_status = InstallationStatus::Failed(error_msg);
                    self.save()?;
                    return Err(e);
                }
            }
        }

        // Update installation status
        let registration = self.servers.get_mut(server_id).unwrap();
        registration.metadata.dependencies = installed_deps;
        registration.metadata.installation_status = InstallationStatus::Installed;
        registration.metadata.updated_at = Utc::now();
        self.save()?;

        info!("✅ All dependencies installed for {}", server_id);
        Ok(())
    }

    /// Install a single dependency
    async fn install_dependency(&self, dep: &Dependency) -> Result<()> {
        match &dep.dep_type {
            DependencyType::Docker => self.install_docker().await,
            DependencyType::Node => self.install_node(&dep.min_version).await,
            DependencyType::Npm => self.install_npm().await,
            DependencyType::Python => self.install_python(&dep.min_version).await,
            DependencyType::Pip => self.install_pip().await,
            DependencyType::Rust => self.install_rust().await,
            DependencyType::System(package) => self.install_system_package(package).await,
        }
    }

    /// Check if dependency is installed
    pub async fn check_dependency(&self, dep_type: &DependencyType) -> Result<bool> {
        match dep_type {
            DependencyType::Docker => {
                let output = tokio::process::Command::new("docker")
                    .arg("--version")
                    .output()
                    .await?;
                Ok(output.status.success())
            }
            DependencyType::Node => {
                let output = tokio::process::Command::new("node")
                    .arg("--version")
                    .output()
                    .await?;
                Ok(output.status.success())
            }
            DependencyType::Npm => {
                let output = tokio::process::Command::new("npm")
                    .arg("--version")
                    .output()
                    .await?;
                Ok(output.status.success())
            }
            DependencyType::Python => {
                let output = tokio::process::Command::new("python3")
                    .arg("--version")
                    .output()
                    .await?;
                Ok(output.status.success())
            }
            DependencyType::Pip => {
                let output = tokio::process::Command::new("pip3")
                    .arg("--version")
                    .output()
                    .await?;
                Ok(output.status.success())
            }
            DependencyType::Rust => {
                let output = tokio::process::Command::new("rustc")
                    .arg("--version")
                    .output()
                    .await?;
                Ok(output.status.success())
            }
            DependencyType::System(package) => {
                let output = tokio::process::Command::new("which")
                    .arg(package)
                    .output()
                    .await?;
                Ok(output.status.success())
            }
        }
    }

    /// Install Docker using b00t cli
    async fn install_docker(&self) -> Result<()> {
        info!("🐳 Installing Docker via b00t cli");
        let output = tokio::process::Command::new("b00t-cli")
            .args(&["cli", "install", "docker"])
            .output()
            .await
            .context("Failed to run b00t-cli install docker")?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(anyhow::anyhow!("Docker installation failed: {}", stderr));
        }

        Ok(())
    }

    /// Install Node.js using b00t cli
    async fn install_node(&self, _min_version: &Option<String>) -> Result<()> {
        info!("📦 Installing Node.js via b00t cli");
        let output = tokio::process::Command::new("b00t-cli")
            .args(&["cli", "install", "node"])
            .output()
            .await
            .context("Failed to run b00t-cli install node")?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(anyhow::anyhow!("Node.js installation failed: {}", stderr));
        }

        Ok(())
    }

    /// Install npm (usually comes with Node.js)
    async fn install_npm(&self) -> Result<()> {
        // npm typically comes with Node.js, so install node
        self.install_node(&None).await
    }

    /// Install Python using b00t cli
    async fn install_python(&self, _min_version: &Option<String>) -> Result<()> {
        info!("🐍 Installing Python via b00t cli");
        let output = tokio::process::Command::new("b00t-cli")
            .args(&["cli", "install", "python"])
            .output()
            .await
            .context("Failed to run b00t-cli install python")?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(anyhow::anyhow!("Python installation failed: {}", stderr));
        }

        Ok(())
    }

    /// Install pip (usually comes with Python)
    async fn install_pip(&self) -> Result<()> {
        // pip typically comes with Python, so install python
        self.install_python(&None).await
    }

    /// Install Rust using b00t cli
    async fn install_rust(&self) -> Result<()> {
        info!("🦀 Installing Rust via b00t cli");
        let output = tokio::process::Command::new("b00t-cli")
            .args(&["cli", "install", "rust"])
            .output()
            .await
            .context("Failed to run b00t-cli install rust")?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(anyhow::anyhow!("Rust installation failed: {}", stderr));
        }

        Ok(())
    }

    /// Install system package using b00t cli
    async fn install_system_package(&self, package: &str) -> Result<()> {
        info!("📦 Installing system package '{}' via b00t cli", package);
        let output = tokio::process::Command::new("b00t-cli")
            .args(&["cli", "install", package])
            .output()
            .await
            .context(format!("Failed to run b00t-cli install {}", package))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(anyhow::anyhow!("Package '{}' installation failed: {}", package, stderr));
        }

        Ok(())
    }
}

impl Default for McpRegistry {
    fn default() -> Self {
        let storage_path = dirs::home_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join(".b00t")
            .join("mcp_registry.json");

        Self::new(storage_path.clone()).unwrap_or_else(|_| Self {
            servers: HashMap::new(),
            storage_path,
            enable_official_sync: true,
        })
    }
}

/// Helper to create registration from b00t datum
pub fn create_registration_from_datum(
    id: String,
    name: String,
    command: String,
    args: Vec<String>,
) -> McpServerRegistration {
    McpServerRegistration {
        id: id.clone(),
        name: name.clone(),
        description: format!("b00t MCP server: {}", name),
        version: "0.1.0".to_string(),
        homepage: Some("https://github.com/elasticdotventures/dotfiles".to_string()),
        documentation: None,
        license: Some("Apache-2.0".to_string()),
        tags: vec!["b00t".to_string(), "local".to_string()],
        config: McpServerConfig {
            command,
            args,
            env: None,
            cwd: None,
            transport: ServerTransport::Stdio,
        },
        metadata: RegistrationMetadata {
            registered_at: Utc::now(),
            updated_at: Utc::now(),
            source: RegistrationSource::Local,
            health_status: HealthStatus::Unknown,
            last_health_check: None,
            dependencies: Vec::new(),
            installation_status: InstallationStatus::NotInstalled,
        },
    }
}
