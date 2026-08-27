//! b00t MCP Registry Implementation
//!
//! Provides a local MCP registry that can:
//! - Register and discover MCP servers locally
//! - Proxy to the official MCP registry (modelcontextprotocol/registry)
//! - Auto-discover tools from registered servers
//! - Act as both an MCP server AND a registry

use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
use tracing::{debug, info, warn};

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
    /// Server URL, for http-stream/websocket transports (stdio servers leave this None)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
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
    /// Synced from the Vinkius Open Data Initiative (open, no-auth
    /// github.com/vinkius-labs/mcp-database markdown dataset — distinct from
    /// the paid discover-mcp/api.vinkius.com "unblessed" product)
    VinkiusMcpDatabase,
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

#[derive(Debug, Deserialize)]
struct RegistryPackage {
    #[serde(rename = "registryType")]
    registry_type: String,
    identifier: String,
}

fn infer_command(packages: &[RegistryPackage]) -> (String, Vec<String>) {
    for pkg in packages {
        match pkg.registry_type.as_str() {
            "npm" => return ("npx".to_string(), vec!["-y".to_string(), pkg.identifier.clone()]),
            "pypi" => return ("uvx".to_string(), vec![pkg.identifier.clone()]),
            _ => continue,
        }
    }
    ("echo".to_string(), vec!["no executable package found".to_string()])
}

/// Result of parsing one `mcps/*.md` file from the vinkius-mcp-database dataset.
#[derive(Debug, Clone, PartialEq, Eq)]
struct ParsedVinkiusEntry {
    name: String,
    description: String,
    category: Option<String>,
    tools: Vec<String>,
    requires_token: bool,
}

/// Turn a kebab/snake-case filename slug into a human-ish title, used as a
/// fallback `name` when a file has no (or a malformed) H1 heading.
fn slug_to_title(slug: &str) -> String {
    slug.split(['-', '_'])
        .filter(|w| !w.is_empty())
        .map(|word| {
            let mut chars = word.chars();
            match chars.next() {
                Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
                None => String::new(),
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

/// Parse one vinkius-mcp-database markdown file body into its useful parts.
///
/// The dataset (6533 files as of this writing) is machine-generated from a
/// consistent template — H1 title, `## Overview` (with a `**Category:**
/// [slug](../categories/slug.md)` line + one-paragraph blurb), `## Description`
/// (longer blurb + `### ...` subsections we intentionally don't capture),
/// `## Available Tools (N)` (`- **tool_name**: tool description` bullets) —
/// but as an open community dataset it's not guaranteed uniform, so every
/// step here degrades gracefully (missing sections, missing bold-markers on
/// tool bullets, multi-line tool descriptions that spill onto an unmarked
/// continuation line, etc.) rather than erroring or panicking.
fn parse_vinkius_markdown(slug: &str, content: &str) -> ParsedVinkiusEntry {
    #[derive(PartialEq)]
    enum Section {
        None,
        Overview,
        Description,
        Tools,
        Other,
    }

    let mut name: Option<String> = None;
    let mut category: Option<String> = None;
    let mut tools: Vec<String> = Vec::new();
    let mut overview_lines: Vec<String> = Vec::new();
    let mut description_lines: Vec<String> = Vec::new();
    let mut overview_done = false;
    let mut description_done = false;
    let mut section = Section::None;

    for line in content.lines() {
        let trimmed = line.trim();

        // H1 title — take the first one encountered, wherever it appears
        // (some files lead with badge images before the heading).
        if name.is_none() {
            if let Some(rest) = trimmed.strip_prefix("# ") {
                let candidate = rest.trim();
                if !candidate.is_empty() {
                    name = Some(candidate.to_string());
                    continue;
                }
            }
        }

        if let Some(rest) = trimmed.strip_prefix("## ") {
            let heading = rest.trim().to_lowercase();
            section = if heading.starts_with("overview") {
                Section::Overview
            } else if heading.starts_with("description") {
                Section::Description
            } else if heading.starts_with("available tools") {
                Section::Tools
            } else {
                Section::Other
            };
            continue;
        }

        match section {
            Section::Overview => {
                if overview_done {
                    continue;
                }
                if let Some(cat) = trimmed.strip_prefix("**Category:**") {
                    if let Some(start) = cat.find('[') {
                        if let Some(end) = cat[start + 1..].find(']') {
                            let cat_slug = cat[start + 1..start + 1 + end].trim();
                            if !cat_slug.is_empty() {
                                category = Some(cat_slug.to_string());
                            }
                        }
                    }
                    continue;
                }
                // A subheading (### ...) ends the short blurb we care about.
                if trimmed.starts_with('#') {
                    overview_done = true;
                    continue;
                }
                if !trimmed.is_empty() {
                    overview_lines.push(trimmed.to_string());
                }
            }
            Section::Description => {
                if description_done {
                    continue;
                }
                // Stop at the first subsection (### What you can do / ### How
                // it works / ...) — we only want the lead paragraph, not the
                // full enumerated feature list, for a compact description.
                if trimmed.starts_with('#') {
                    description_done = true;
                    continue;
                }
                if !trimmed.is_empty() {
                    description_lines.push(trimmed.to_string());
                }
            }
            Section::Tools => {
                let Some(rest) = trimmed.strip_prefix("- ") else {
                    continue;
                };
                let Some(bold_start) = rest.find("**") else {
                    continue;
                };
                let after = &rest[bold_start + 2..];
                let Some(bold_end) = after.find("**") else {
                    continue;
                };
                let tool_name = after[..bold_end].trim();
                if !tool_name.is_empty() {
                    tools.push(tool_name.to_string());
                }
            }
            Section::None | Section::Other => {}
        }
    }

    let name = name.unwrap_or_else(|| slug_to_title(slug));

    let description = if !description_lines.is_empty() {
        description_lines.join(" ")
    } else if !overview_lines.is_empty() {
        overview_lines.join(" ")
    } else {
        format!("MCP server: {}", name)
    };

    // Heuristic: flag entries that document a hosted service requiring a
    // per-provider credential, so registry consumers can filter them out of
    // "zero-auth invokable" searches. Broader than the literal "Access Token"
    // phrase per the task brief's "or similar" — community write-ups phrase
    // this many different ways (Client ID/Secret, API key, Authorization Key).
    let lower = content.to_lowercase();
    let requires_token = ["access token", "api key", "api keys", "client id", "client secret", "authorization key", "auth token", "bearer token"]
        .iter()
        .any(|needle| lower.contains(needle));

    ParsedVinkiusEntry {
        name,
        description,
        category,
        tools,
        requires_token,
    }
}

#[cfg(test)]
mod registry_tests {
    use super::*;

    #[test]
    fn test_infer_command_npm() {
        let packages = vec![RegistryPackage {
            registry_type: "npm".to_string(),
            identifier: "@modelcontextprotocol/server-filesystem".to_string(),
        }];
        let (cmd, args) = infer_command(&packages);
        assert_eq!(cmd, "npx");
        assert_eq!(args, vec!["-y", "@modelcontextprotocol/server-filesystem"]);
    }

    #[test]
    fn test_infer_command_pypi() {
        let packages = vec![RegistryPackage {
            registry_type: "pypi".to_string(),
            identifier: "mcp-server-git".to_string(),
        }];
        let (cmd, args) = infer_command(&packages);
        assert_eq!(cmd, "uvx");
        assert_eq!(args, vec!["mcp-server-git"]);
    }

    #[test]
    fn test_infer_command_empty() {
        let packages: Vec<RegistryPackage> = vec![];
        let (cmd, args) = infer_command(&packages);
        assert_eq!(cmd, "echo");
        assert!(!args.is_empty());
    }

    #[test]
    fn test_infer_command_first_valid() {
        let packages = vec![
            RegistryPackage {
                registry_type: "unknown".to_string(),
                identifier: "skip-me".to_string(),
            },
            RegistryPackage {
                registry_type: "npm".to_string(),
                identifier: "valid-pkg".to_string(),
            },
        ];
        let (cmd, args) = infer_command(&packages);
        assert_eq!(cmd, "npx");
        assert_eq!(args, vec!["-y", "valid-pkg"]);
    }

    const DVC_MD: &str = r#"# DVC MCP Server

[![Deploy on Vinkius Edge](https://img.shields.io/badge/Deploy%20on-Vinkius%20Edge-blue?style=for-the-badge)](https://vinkius.com/ai-agent-connect/dvc)

## Overview

**Category:** [developer-tools](../categories/developer-tools.md)

Manage ML experiments via DVC — track projects and views, audit experiments history, and monitor model runs directly from any AI agent.

## Description
Connect your **DVC Studio** account to any AI agent and take full control of your machine learning experiments and data versioning workflows through natural conversation.

### What you can do

- **Project Orchestration** — Expose registered organization workspaces

### How it works

1. Subscribe to this server
2. Enter your DVC Studio Client Access Token (found in DVC Studio Profile Settings > Tokens)

## Available Tools (6)
- **get_project**: Get project
- **list_experiments**: List experiments
- **list_views**: List views
- **get_view**: Get view
- **list_projects**: List projects
- **get_user**: Get user profile


## 💬 Prompt Examples
Some examples here.
"#;

    #[test]
    fn test_parse_vinkius_markdown_dvc_shape() {
        let parsed = parse_vinkius_markdown("dvc", DVC_MD);
        assert_eq!(parsed.name, "DVC MCP Server");
        assert_eq!(parsed.category.as_deref(), Some("developer-tools"));
        assert!(parsed.description.contains("Connect your"));
        // Description must not bleed into the "### What you can do" bullets
        assert!(!parsed.description.contains("Project Orchestration"));
        assert_eq!(
            parsed.tools,
            vec![
                "get_project",
                "list_experiments",
                "list_views",
                "get_view",
                "list_projects",
                "get_user",
            ]
        );
        assert!(parsed.requires_token, "mentions 'Client Access Token'");
    }

    // Real edge case seen in the wild (conflux.md): a tool's description
    // spills onto a blank-line-separated continuation that is NOT prefixed
    // with "- **" — must not be mistaken for another tool bullet.
    #[test]
    fn test_parse_vinkius_markdown_multiline_tool_description() {
        let md = r#"# Conflux MCP Server

## Overview

**Category:** [developer-tools](../categories/developer-tools.md)

Query Conflux Network data.

## Description
Connect to the Conflux Network.

## Available Tools (2)
- **cfx_send_transaction**: Requires the node to manage the sender account.

Send an unsigned transaction to the Core Space network
- **cfx_get_status**: Get current state of the node
"#;
        let parsed = parse_vinkius_markdown("conflux", md);
        assert_eq!(
            parsed.tools,
            vec!["cfx_send_transaction", "cfx_get_status"]
        );
        assert!(!parsed.requires_token, "RPC URL config, not a token");
    }

    // Defensive parsing: a maximally malformed file (no H1, no known
    // sections, no tool bullets) must still yield a usable fallback
    // registration rather than panicking or erroring.
    #[test]
    fn test_parse_vinkius_markdown_malformed_file_falls_back() {
        let md = "just some\nunstructured text\nwith no headings at all";
        let parsed = parse_vinkius_markdown("some-weird-slug", md);
        assert_eq!(parsed.name, "Some Weird Slug");
        assert!(parsed.category.is_none());
        assert!(parsed.tools.is_empty());
        assert!(!parsed.description.is_empty());
        assert!(!parsed.requires_token);
    }

    #[test]
    fn test_parse_vinkius_markdown_no_bold_tool_bullets_skipped_gracefully() {
        let md = r#"# Some Server

## Available Tools (2)
- get_thing: no bold markers on this one
- **real_tool**: this one has bold markers
"#;
        let parsed = parse_vinkius_markdown("some-server", md);
        assert_eq!(parsed.tools, vec!["real_tool"]);
    }

    #[test]
    fn test_slug_to_title() {
        assert_eq!(slug_to_title("country-data-resolver"), "Country Data Resolver");
        assert_eq!(slug_to_title("dvc"), "Dvc");
        assert_eq!(slug_to_title(""), "");
    }

    /// Regression test for #1112: sync results must survive a *fresh*
    /// `McpRegistry` construction, not just be visible within the same
    /// in-process instance that ran the sync. Simulates what
    /// `sync_official_registry`/`sync_vinkius_mcp_database` do (insert an
    /// entry, then persist) without needing network access, then constructs
    /// a brand-new registry against the same `storage_path` — mirroring two
    /// separate CLI invocations (`sync-vinkius-database` then `search`).
    #[test]
    fn test_synced_entries_persist_and_reload_across_construction() {
        let storage_path = std::env::temp_dir()
            .join(format!("b00t-test-mcp-registry-persist-{}.json", std::process::id()));
        let _ = std::fs::remove_file(&storage_path);

        let mut registry = McpRegistry {
            servers: HashMap::new(),
            storage_path: storage_path.clone(),
            enable_official_sync: true,
        };
        registry.servers.insert(
            "test.persist/1112-fixture".to_string(),
            McpServerRegistration {
                id: "test.persist/1112-fixture".to_string(),
                name: "Persist Regression Fixture".to_string(),
                description: "regression fixture for issue #1112".to_string(),
                version: "0.0.0".to_string(),
                homepage: None,
                documentation: None,
                license: None,
                tags: vec!["test-1112".to_string()],
                config: McpServerConfig {
                    command: "echo".to_string(),
                    args: vec![],
                    env: None,
                    cwd: None,
                    transport: ServerTransport::Stdio,
                    url: None,
                },
                metadata: RegistrationMetadata {
                    registered_at: Utc::now(),
                    updated_at: Utc::now(),
                    source: RegistrationSource::VinkiusMcpDatabase,
                    health_status: HealthStatus::Unknown,
                    last_health_check: None,
                    dependencies: Vec::new(),
                    installation_status: InstallationStatus::NotInstalled,
                },
            },
        );
        registry.save().expect("save should succeed");

        // Before the #1112 fix, McpRegistry::new(..., false) never read
        // storage_path at all — this entry would be invisible here, matching
        // the reported "search returns 0 results after a separate sync
        // invocation" symptom.
        let reloaded =
            McpRegistry::new(storage_path.clone(), false).expect("fresh construction should succeed");
        let hits = reloaded.search("Persist Regression Fixture");
        assert!(
            hits.iter().any(|s| s.id == "test.persist/1112-fixture"),
            "synced entry must survive across a fresh McpRegistry construction"
        );

        let _ = std::fs::remove_file(&storage_path);
    }

    // Real end-to-end check: shallow-clone the actual vinkius-mcp-database
    // repo, sync it into a registry, and confirm the dvc.md entry (used as
    // the worked example in the task spec) round-trips through search().
    // Network + `git` binary required, so this is #[ignore]d by default;
    // run explicitly with `cargo test --ignored -- --nocapture`.
    #[tokio::test]
    #[ignore = "network: shallow-clones https://github.com/vinkius-labs/mcp-database"]
    async fn test_vinkius_mcp_database_sync_and_search_dvc() {
        let mut registry = McpRegistry {
            servers: HashMap::new(),
            storage_path: std::env::temp_dir().join("b00t-test-vinkius-registry.json"),
            enable_official_sync: true,
        };

        let synced = registry
            .sync_vinkius_mcp_database()
            .await
            .expect("vinkius-mcp-database sync should succeed");
        println!("synced {} servers from vinkius-mcp-database", synced);
        assert!(
            synced > 1000,
            "expected at least 1000 servers synced (repo has 6533 mcps/*.md files), got {}",
            synced
        );

        let dvc_hits = registry.search("dvc");
        assert!(!dvc_hits.is_empty(), "expected a 'dvc' search hit");
        let dvc = dvc_hits
            .iter()
            .find(|s| s.id.ends_with("/dvc"))
            .expect("expected the dvc.md-derived entry specifically");
        println!("dvc entry: {:#?}", dvc);
        assert_eq!(dvc.name, "DVC MCP Server");
        assert!(dvc.tags.contains(&"vinkius-mcp-database".to_string()));
        assert!(
            dvc.tags.contains(&"requires-token".to_string()),
            "dvc.md documents a 'Client Access Token' — must be tagged requires-token"
        );
        assert!(dvc.description.contains("list_experiments"));

        // Spot-check a few other entries parsed correctly (not empty/garbage).
        let others: Vec<_> = registry
            .list()
            .into_iter()
            .filter(|s| {
                s.tags.contains(&"vinkius-mcp-database".to_string()) && !s.id.ends_with("/dvc")
            })
            .take(3)
            .collect();
        assert_eq!(others.len(), 3);
        for s in &others {
            println!("spot-check: {} — {}", s.id, s.name);
            assert!(!s.name.trim().is_empty());
            assert!(!s.description.trim().is_empty());
            assert!(s.tags.contains(&"vinkius-mcp-database".to_string()));
        }
    }
}

impl McpRegistry {
    /// Create new MCP registry
    /// If from_file is true, will attempt to load from storage_path
    /// If from_file is false, will initialize empty and sync from datum files
    pub fn new(storage_path: PathBuf, from_file: bool) -> Result<Self> {
        let mut registry = Self {
            servers: HashMap::new(),
            storage_path,
            enable_official_sync: true,
        };

        if from_file {
            registry.load()?;
        } else {
            // #1112: seed with whatever a prior `sync-official`/
            // `sync-vinkius-database` invocation persisted (best-effort —
            // missing/corrupt cache is not fatal, just starts empty) before
            // layering datum files on top. Datum-backed servers still win on
            // id collision (sync_from_datums inserts after this), keeping
            // datum TOMLs authoritative for anything they define; this only
            // fills in the entries that have no datum at all — the synced
            // ones search couldn't find across invocations before.
            if let Err(e) = registry.load() {
                debug!("No prior synced registry cache to load: {}", e);
            }

            // Load from datum files instead of file. Datum TOMLs live one
            // level under the dotfiles root (~/.b00t/_b00t_/*.mcp.toml, per
            // the `sync-datums --path ~/.dotfiles/_b00t_` example in this
            // command's own --help) -- not directly in ~/.b00t/.
            if let Err(e) = registry.sync_from_datums("~/.b00t/_b00t_") {
                warn!("Failed to sync from datums: {}", e);
            }
        }

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

        self.servers = serde_json::from_str(&data).context("Failed to parse registry storage")?;

        info!("📂 Loaded {} servers from registry", self.servers.len());
        Ok(())
    }

    /// Persists `self.servers` as JSON to `storage_path`. #1112: this used to
    /// be a no-op ("registry is runtime-only") — that's still the right
    /// default for datum-backed and locally-registered entries (the datum
    /// TOML files remain their source of truth, re-derived fresh by
    /// `sync_from_datums` on every construction), but it left sync-derived
    /// entries (`sync_official_registry`, `sync_vinkius_mcp_database`) with
    /// nowhere durable to live: those servers have no corresponding on-disk
    /// datum, so `search` in a later CLI invocation always saw zero results
    /// from a prior `sync-*` invocation. Only the two sync methods call this
    /// now — register/unregister/health-status/etc. remain deliberately
    /// ephemeral, unchanged from before.
    fn save(&self) -> Result<()> {
        if let Some(parent) = self.storage_path.parent() {
            std::fs::create_dir_all(parent).context("Failed to create registry storage dir")?;
        }
        let data =
            serde_json::to_string_pretty(&self.servers).context("Failed to serialize registry")?;
        std::fs::write(&self.storage_path, data).context("Failed to write registry storage")?;
        debug!("💾 Saved {} servers to {}", self.servers.len(), self.storage_path.display());
        Ok(())
    }

    /// Register an MCP server
    pub fn register(&mut self, registration: McpServerRegistration) -> Result<()> {
        let server_id = registration.id.clone();

        info!("📝 Registering MCP server: {}", server_id);

        // Validate registration
        self.validate_registration(&registration)?;

        self.servers.insert(server_id.clone(), registration);
        // Don't save to file - registry is runtime-only, registration is ephemeral

        info!("✅ Successfully registered MCP server: {}", server_id);
        Ok(())
    }

    /// Unregister an MCP server
    pub fn unregister(&mut self, server_id: &str) -> Result<()> {
        if self.servers.remove(server_id).is_some() {
            // Don't save to file - registry is runtime-only, unregistration is ephemeral
            info!("🗑️  Unregistered MCP server: {}", server_id);
            Ok(())
        } else {
            Err(anyhow::anyhow!(
                "Server '{}' not found in registry",
                server_id
            ))
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
                s.name.to_lowercase().contains(&keyword_lower)
                    || s.description.to_lowercase().contains(&keyword_lower)
                    || s.tags
                        .iter()
                        .any(|t| t.to_lowercase().contains(&keyword_lower))
            })
            .collect()
    }

    /// Update server health status
    pub fn update_health(&mut self, server_id: &str, status: HealthStatus) -> Result<()> {
        if let Some(registration) = self.servers.get_mut(server_id) {
            registration.metadata.health_status = status;
            registration.metadata.last_health_check = Some(Utc::now());
            // Don't save to file - registry is runtime-only, health status is ephemeral
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

        serde_json::to_string_pretty(&export).context("Failed to export registry")
    }

    /// Import from MCP registry format
    pub fn import_from_mcp_format(&mut self, json: &str) -> Result<usize> {
        #[derive(Deserialize)]
        struct McpRegistryImport {
            servers: Vec<McpServerRegistration>,
        }

        let import: McpRegistryImport =
            serde_json::from_str(json).context("Failed to parse import data")?;

        let mut imported_count = 0;
        for mut server in import.servers {
            server.metadata.source = RegistrationSource::Imported;
            server.metadata.updated_at = Utc::now();
            self.servers.insert(server.id.clone(), server);
            imported_count += 1;
        }

        if imported_count > 0 {
            // Don't save to file - registry is runtime-only, import is ephemeral
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
            // #1112: persist so a later `search` invocation sees what this
            // sync fetched — non-fatal on write failure (still usable this
            // run; just won't survive to the next invocation).
            if let Err(e) = self.save() {
                warn!("Failed to persist synced registry: {}", e);
            }
        }

        info!("✅ Synced {} servers from official registry", synced_count);
        Ok(synced_count)
    }

    /// Fetch servers from official MCP registry
    async fn fetch_official_servers(&self) -> Result<Vec<McpServerRegistration>> {
        let url = std::env::var("MCP_REGISTRY_URL")
            .unwrap_or_else(|_| "https://registry.modelcontextprotocol.io".to_string());

        let list_url = format!("{}/v0.1/servers", url);
        info!("Fetching official MCP registry from {}", list_url);

        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(30))
            .user_agent("b00t-mcp-registry/0.1")
            .build()
            .context("Failed to build HTTP client")?;

        #[derive(Deserialize)]
        struct RegistryServer {
            server: RegistryServerDetail,
        }

        #[derive(Deserialize)]
        struct RegistryServerDetail {
            name: String,
            description: String,
            #[serde(default)]
            title: Option<String>,
            version: String,
            #[serde(default)]
            packages: Vec<RegistryPackage>,
        }

        #[derive(Deserialize)]
        struct ServerListResponse {
            servers: Vec<RegistryServer>,
            #[serde(default)]
            #[allow(dead_code)]
            metadata: Option<serde_json::Value>,
        }

        let response = client
            .get(&list_url)
            .query(&[("limit", "50")])
            .send()
            .await
            .context("Failed to fetch official registry")?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            warn!(
                "Official registry returned {}: {}",
                status,
                body.chars().take(200).collect::<String>()
            );
            return Ok(Vec::new());
        }

        let list: ServerListResponse = response
            .json()
            .await
            .context("Failed to parse registry response")?;

        let mut servers = Vec::new();
        for entry in list.servers {
            let detail = entry.server;
            let display_name = detail.title.unwrap_or_else(|| detail.name.clone());

            let (command, args) = infer_command(&detail.packages);

            let config = McpServerConfig {
                command,
                args,
                env: None,
                cwd: None,
                transport: ServerTransport::Stdio,
                url: None,
            };

            servers.push(McpServerRegistration {
                id: detail.name.clone(),
                name: display_name,
                description: detail.description,
                version: detail.version,
                homepage: None,
                documentation: None,
                license: None,
                tags: vec!["official-registry".to_string()],
                config,
                metadata: RegistrationMetadata {
                    registered_at: Utc::now(),
                    updated_at: Utc::now(),
                    source: RegistrationSource::OfficialRegistry,
                    health_status: HealthStatus::Unknown,
                    last_health_check: None,
                    dependencies: Vec::new(),
                    installation_status: InstallationStatus::NotInstalled,
                },
            });
        }

        info!("Fetched {} servers from official registry", servers.len());
        Ok(servers)
    }

    /// Sync with the Vinkius Open Data Initiative (open, no-auth
    /// github.com/vinkius-labs/mcp-database markdown dataset).
    ///
    /// NOTE: this is distinct from Vinkius's other, *paid* product — the
    /// `discover-mcp` npm package / api.vinkius.com live catalog, which
    /// 401s without a paid VINKIUS_CATALOG_TOKEN and is explicitly
    /// "unblessed" / out of scope here. Only the public mcp-database repo
    /// is indexed.
    pub async fn sync_vinkius_mcp_database(&mut self) -> Result<usize> {
        info!("🔄 Syncing with Vinkius Open Data Initiative (mcp-database)");

        let vinkius_servers = self.fetch_vinkius_mcp_database().await?;
        let mut synced_count = 0;

        for server in vinkius_servers {
            // Only add if not already registered locally
            if !self.servers.contains_key(&server.id) {
                info!("📥 Adding server from vinkius-mcp-database: {}", server.id);
                self.servers.insert(server.id.clone(), server);
                synced_count += 1;
            }
        }

        if synced_count > 0 {
            // #1112: persist so a later `search` invocation sees what this
            // sync fetched — non-fatal on write failure (still usable this
            // run; just won't survive to the next invocation).
            if let Err(e) = self.save() {
                warn!("Failed to persist synced registry: {}", e);
            }
        }

        info!(
            "✅ Synced {} servers from vinkius-mcp-database",
            synced_count
        );
        Ok(synced_count)
    }

    /// Fetch servers from the Vinkius Open Data Initiative
    /// (github.com/vinkius-labs/mcp-database).
    ///
    /// The dataset has thousands of one-file-per-server markdown docs under
    /// `mcps/*.md` (6533 as of writing) — fetching each individually via the
    /// GitHub contents/raw API would mean thousands of HTTP requests, so
    /// instead this does a single shallow (`--depth 1`) git clone to a temp
    /// dir, reads the files directly off disk, and removes the temp dir
    /// (via `TempDir`'s `Drop` impl) when done.
    async fn fetch_vinkius_mcp_database(&self) -> Result<Vec<McpServerRegistration>> {
        let repo_url = std::env::var("VINKIUS_MCP_DATABASE_URL")
            .unwrap_or_else(|_| "https://github.com/vinkius-labs/mcp-database".to_string());

        info!(
            "Shallow-cloning vinkius-mcp-database from {} for indexing",
            repo_url
        );

        let temp_dir = tempfile::Builder::new()
            .prefix("b00t-vinkius-mcp-database-")
            .tempdir()
            .context("Failed to create temp dir for vinkius-mcp-database clone")?;

        let clone_path = temp_dir.path();
        let clone_path_str = clone_path
            .to_str()
            .context("temp clone dir path is not valid UTF-8")?;

        let output = tokio::process::Command::new("git")
            .args([
                "clone",
                "--depth",
                "1",
                "--single-branch",
                "--quiet",
                &repo_url,
                clone_path_str,
            ])
            .output()
            .await
            .context("Failed to run `git clone` for vinkius-mcp-database")?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            warn!(
                "git clone of vinkius-mcp-database failed: {}",
                stderr.chars().take(500).collect::<String>()
            );
            return Ok(Vec::new());
        }

        let mcps_dir = clone_path.join("mcps");
        if !mcps_dir.is_dir() {
            warn!(
                "vinkius-mcp-database clone has no mcps/ directory at {}",
                mcps_dir.display()
            );
            return Ok(Vec::new());
        }

        let mut servers = Vec::new();
        let mut scanned = 0usize;
        let mut skipped = 0usize;

        let entries = std::fs::read_dir(&mcps_dir)
            .context("Failed to read mcps/ directory from vinkius-mcp-database clone")?;

        for entry in entries {
            let entry = match entry {
                Ok(e) => e,
                Err(e) => {
                    warn!("Skipping unreadable dir entry in mcps/: {}", e);
                    skipped += 1;
                    continue;
                }
            };
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) != Some("md") {
                continue;
            }

            let Some(slug) = path.file_stem().and_then(|s| s.to_str()) else {
                skipped += 1;
                continue;
            };
            let slug = slug.to_string();
            scanned += 1;

            let content = match std::fs::read_to_string(&path) {
                Ok(c) => c,
                Err(e) => {
                    warn!("Failed to read {}: {}", path.display(), e);
                    skipped += 1;
                    continue;
                }
            };
            if content.trim().is_empty() {
                skipped += 1;
                continue;
            }

            let parsed = parse_vinkius_markdown(&slug, &content);

            let id = format!("io.github.vinkius-labs.mcp-database/{}", slug);
            let homepage = format!(
                "https://github.com/vinkius-labs/mcp-database/blob/main/mcps/{}.md",
                slug
            );

            let mut tags = vec!["vinkius-mcp-database".to_string()];
            if let Some(cat) = &parsed.category {
                tags.push(format!("category:{}", cat));
            }
            if parsed.requires_token {
                tags.push("requires-token".to_string());
            }

            // No established precedent in this registry for a purely
            // metadata/discoverability entry with no locally-runnable
            // command (unlike official-registry/datum entries, these are
            // hosted-remote MCP servers documented for search, not proven
            // runnable via a local command). Mirror `infer_command`'s
            // existing "no executable package found" convention for the
            // no-package case, and additionally document the required
            // credential as an env var placeholder for token-gated entries.
            let (command, args, env) = if parsed.requires_token {
                (
                    "echo".to_string(),
                    vec![format!(
                        "{}: remote hosted MCP server (vinkius-mcp-database) — requires a provider access token, see documentation",
                        parsed.name
                    )],
                    Some(HashMap::from([(
                        "MCP_ACCESS_TOKEN".to_string(),
                        "<REQUIRED: obtain from provider — see documentation link>".to_string(),
                    )])),
                )
            } else {
                (
                    "echo".to_string(),
                    vec![format!(
                        "{}: metadata-only entry from vinkius-mcp-database — no runnable local command, see documentation",
                        parsed.name
                    )],
                    None,
                )
            };

            let description = if parsed.tools.is_empty() {
                parsed.description
            } else {
                format!("{} Tools: {}.", parsed.description, parsed.tools.join(", "))
            };

            servers.push(McpServerRegistration {
                id,
                name: parsed.name,
                description,
                version: "unknown".to_string(),
                homepage: Some(homepage.clone()),
                documentation: Some(homepage),
                license: None,
                tags,
                config: McpServerConfig {
                    command,
                    args,
                    env,
                    cwd: None,
                    transport: ServerTransport::Stdio,
                    url: None,
                },
                metadata: RegistrationMetadata {
                    registered_at: Utc::now(),
                    updated_at: Utc::now(),
                    source: RegistrationSource::VinkiusMcpDatabase,
                    health_status: HealthStatus::Unknown,
                    last_health_check: None,
                    dependencies: Vec::new(),
                    installation_status: InstallationStatus::NotInstalled,
                },
            });
        }

        info!(
            "Parsed {} servers from vinkius-mcp-database ({} md files scanned, {} skipped)",
            servers.len(),
            scanned,
            skipped
        );

        // temp_dir (and the clone within it) is removed here as it drops.
        drop(temp_dir);

        Ok(servers)
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
            // Don't save to file - registry is runtime-only, discovery is ephemeral
            info!("✅ Discovered {} MCP servers", discovered_count);
        }

        Ok(discovered_count)
    }

    /// Discover servers from a specific path
    async fn discover_from_path(&mut self, _path: &PathBuf) -> Result<usize> {
        // 🤓 Implementation would scan path for MCP server configurations
        Ok(0)
    }

    /// Sync registry from datum TOML files (registry-as-view)
    /// Scans _b00t_ directory and populates registry from .mcp.toml files
    pub fn sync_from_datums(&mut self, datums_path: &str) -> Result<usize> {
        use std::fs;

        info!("🔄 Syncing registry from datum files in {}", datums_path);

        let expanded_path = shellexpand::tilde(datums_path).to_string();
        let datums_dir = PathBuf::from(&expanded_path);

        if !datums_dir.exists() {
            warn!("Datums directory not found: {}", datums_path);
            return Ok(0);
        }

        let mut synced_count = 0;

        // Read all .mcp.toml files
        for entry in fs::read_dir(&datums_dir)? {
            let entry = entry?;
            let path = entry.path();

            if path.is_file() {
                if let Some(filename) = path.file_name().and_then(|s| s.to_str()) {
                    if filename.ends_with(".mcp.toml") {
                        match self.sync_datum_file(&path) {
                            Ok(true) => synced_count += 1,
                            Ok(false) => {} // Already synced
                            Err(e) => warn!("Failed to sync {}: {}", filename, e),
                        }
                    }
                }
            }
        }

        if synced_count > 0 {
            // Don't save to file - registry is runtime-only, datum sync is ephemeral
            info!("✅ Synced {} MCP servers from datums", synced_count);
        }

        Ok(synced_count)
    }

    /// Sync a single datum file to registry
    /// Returns Ok(true) if a new registration was added, Ok(false) if updated/unchanged
    fn sync_datum_file(&mut self, path: &PathBuf) -> Result<bool> {
        use serde::Deserialize;
        use std::fs;

        #[derive(Deserialize)]
        struct UnifiedConfig {
            b00t: BootDatumForRegistry,
        }

        #[derive(Deserialize)]
        struct BootDatumForRegistry {
            name: String,
            #[serde(default)]
            hint: String,
            command: Option<String>,
            args: Option<Vec<String>>,
            depends_on: Option<Vec<String>>,
            env: Option<HashMap<String, String>>,
            #[serde(default)]
            keywords: Option<Vec<String>>,
            #[serde(default)]
            #[allow(dead_code)]
            ansible: Option<serde_json::Value>,
            mcp: Option<serde_json::Value>,
        }

        let content = fs::read_to_string(path)?;
        let config: UnifiedConfig = toml::from_str(&content)
            .context(format!("Failed to parse TOML: {}", path.display()))?;

        let datum = config.b00t;

        // Extract command/args/transport/url. Prioritize mcp.stdio[0]; fall
        // back to mcp.httpstream[0] (streamable-HTTP / SSE-style servers,
        // e.g. flexo-mms-*-mcp) before falling back to a bare stdio guess.
        // Without the httpstream branch, any non-stdio datum silently
        // registered as command="npx" with no url -- a real bug found
        // 2026-08-22 while assimilating Open-MBEE's Flexo MCP servers.
        let stdio_entry = datum
            .mcp
            .as_ref()
            .and_then(|mcp_val| mcp_val.get("stdio"))
            .and_then(|v| v.as_array())
            .and_then(|arr| arr.first());

        let httpstream_entry = datum
            .mcp
            .as_ref()
            .and_then(|mcp_val| mcp_val.get("httpstream"))
            .and_then(|v| v.as_array())
            .and_then(|arr| arr.first());

        let (command, args, transport, url) = if let Some(method) = stdio_entry {
            let cmd = method
                .get("command")
                .and_then(|v| v.as_str())
                .unwrap_or("npx")
                .to_string();
            let method_args = method
                .get("args")
                .and_then(|v| v.as_array())
                .map(|arr| {
                    arr.iter()
                        .filter_map(|v| v.as_str().map(|s| s.to_string()))
                        .collect()
                })
                .unwrap_or_default();
            (cmd, method_args, ServerTransport::Stdio, None)
        } else if let Some(method) = httpstream_entry {
            let url = method
                .get("url")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string());
            (String::new(), Vec::new(), ServerTransport::HttpStream, url)
        } else {
            (
                datum.command.unwrap_or_else(|| "npx".to_string()),
                datum.args.unwrap_or_default(),
                ServerTransport::Stdio,
                None,
            )
        };

        // Convert depends_on to registry dependencies
        let dependencies = self.convert_datum_deps_to_registry_deps(&datum.depends_on);

        // Generate server ID from name
        let server_id = format!("local.b00t/{}", datum.name);

        // Check if already exists and is up to date
        let is_new = !self.servers.contains_key(&server_id);

        let registration = McpServerRegistration {
            id: server_id.clone(),
            name: datum.name.clone(),
            description: datum.hint.clone(),
            version: "0.1.0".to_string(),
            homepage: Some("https://github.com/elasticdotventures/dotfiles".to_string()),
            documentation: None,
            license: Some("Apache-2.0".to_string()),
            tags: datum
                .keywords
                .unwrap_or_else(|| vec!["b00t".to_string(), "local".to_string()]),
            config: McpServerConfig {
                command,
                args,
                env: datum.env,
                cwd: None,
                transport,
                url,
            },
            metadata: RegistrationMetadata {
                registered_at: self
                    .servers
                    .get(&server_id)
                    .map(|s| s.metadata.registered_at)
                    .unwrap_or_else(Utc::now),
                updated_at: Utc::now(),
                source: RegistrationSource::Local,
                health_status: HealthStatus::Unknown,
                last_health_check: None,
                dependencies,
                installation_status: InstallationStatus::NotInstalled,
            },
        };

        self.servers.insert(server_id, registration);
        Ok(is_new)
    }

    /// Convert datum depends_on references to registry dependencies
    fn convert_datum_deps_to_registry_deps(
        &self,
        depends_on: &Option<Vec<String>>,
    ) -> Vec<Dependency> {
        let Some(deps) = depends_on else {
            return Vec::new();
        };

        deps.iter()
            .filter_map(|dep| {
                // Parse datum ID format: "name.type" (e.g., "docker.cli", "python.cli")
                let parts: Vec<&str> = dep.split('.').collect();
                if parts.len() != 2 {
                    return None;
                }

                let (name, datum_type) = (parts[0], parts[1]);

                // Map datum type to dependency type
                let dep_type = match (name, datum_type) {
                    ("docker", "cli") | ("docker", "docker") => Some(DependencyType::Docker),
                    ("node", "cli") | ("node", _) => Some(DependencyType::Node),
                    ("npm", "cli") | ("npx", "cli") => Some(DependencyType::Npm),
                    ("python", "cli") | ("python3", "cli") => Some(DependencyType::Python),
                    ("pip", "cli") | ("pip3", "cli") => Some(DependencyType::Pip),
                    ("rust", "cli") | ("rustc", "cli") | ("cargo", "cli") => {
                        Some(DependencyType::Rust)
                    }
                    ("uvx", "cli") => Some(DependencyType::System("uvx".to_string())),
                    _ if datum_type == "cli" => Some(DependencyType::System(name.to_string())),
                    _ => None,
                };

                dep_type.map(|dt| Dependency {
                    dep_type: dt,
                    min_version: None,
                    installed: false, // Will be checked later
                    install_method: Some(format!("b00t-cli cli install {}", name)),
                })
            })
            .collect()
    }

    /// Install dependencies for an MCP server
    pub async fn install_dependencies(&mut self, server_id: &str) -> Result<()> {
        // Clone dependencies to avoid borrow conflicts
        let dependencies = {
            let registration = self
                .servers
                .get_mut(server_id)
                .ok_or_else(|| anyhow::anyhow!("Server '{}' not found", server_id))?;

            info!("📦 Installing dependencies for {}", server_id);
            registration.metadata.installation_status = InstallationStatus::Installing;
            registration.metadata.dependencies.clone()
        };
        // Don't save to file - registry is runtime-only, dependency status is ephemeral

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
                    // Don't save to file - registry is runtime-only, failure status is ephemeral
                    return Err(e);
                }
            }
        }

        // Update installation status
        let registration = self.servers.get_mut(server_id).unwrap();
        registration.metadata.dependencies = installed_deps;
        registration.metadata.installation_status = InstallationStatus::Installed;
        registration.metadata.updated_at = Utc::now();
        // Don't save to file - registry is runtime-only, installation status is ephemeral

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
            .args(["cli", "install", "docker"])
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
            .args(["cli", "install", "node"])
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
            .args(["cli", "install", "python"])
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
            .args(["cli", "install", "rust"])
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
            .args(["cli", "install", package])
            .output()
            .await
            .context(format!("Failed to run b00t-cli install {}", package))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(anyhow::anyhow!(
                "Package '{}' installation failed: {}",
                package,
                stderr
            ));
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

        // Initialize without loading from file, instead sync from datum files
        Self::new(storage_path.clone(), false).unwrap_or_else(|_| Self {
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
            url: None,
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
