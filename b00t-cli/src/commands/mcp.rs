use anyhow::Result;
use clap::Parser;

#[derive(Parser)]
pub enum McpCommands {
    #[clap(
        about = "Register or remove MCP server configuration",
        long_about = "Register or remove MCP server configuration from JSON or command.\n\nJSON Examples:\n  b00t-cli mcp register '{\"name\":\"filesystem\",\"command\":\"npx\",\"args\":[\"-y\",\"@modelcontextprotocol/server-filesystem\"]}'\n  echo '{...}' | b00t-cli mcp register -\n\nCommand Examples:\n  b00t-cli mcp register brave-search -- npx -y @modelcontextprotocol/server-brave-search\n  b00t-cli mcp register filesystem --hint \"File system access\" -- npx -y @modelcontextprotocol/server-filesystem\n\nRemoval Examples:\n  b00t-cli mcp register --remove filesystem\n  b00t-cli mcp register --remove brave-search\n\nInstallation Examples:\n  b00t-cli mcp install brave-search claudecode\n  b00t-cli app vscode mcp install filesystem"
    )]
    Register {
        #[clap(help = "MCP server name (for command mode) or JSON configuration (for JSON mode)")]
        name_or_json: String,
        #[clap(long, help = "Description/hint for the MCP server")]
        hint: Option<String>,
        #[clap(long, help = "Remove the MCP server configuration")]
        remove: bool,
        #[clap(
            long,
            help = "Do What I Want - auto-cleanup and format JSON (default: enabled)"
        )]
        dwiw: bool,
        #[clap(
            long,
            help = "Disable auto-cleanup and format JSON",
            conflicts_with = "dwiw"
        )]
        no_dwiw: bool,
        #[clap(
            last = true,
            help = "Command and arguments (after --) for command mode"
        )]
        command_args: Vec<String>,
    },
    #[clap(
        about = "List available MCP server configurations",
        long_about = "List available MCP server configurations.\n\nExamples:\n  b00t-cli mcp list\n  b00t-cli mcp list --json"
    )]
    List {
        #[clap(long, help = "Output in JSON format")]
        json: bool,
    },
    #[clap(
        about = "Install MCP server to a target (claudecode, vscode, geminicli, dotmcpjson, roocode, codex, stdout)",
        long_about = "Install MCP server to a target application.\n\nExamples:\n  b00t-cli mcp install gh claudecode\n  b00t-cli mcp install filesystem geminicli --repo\n  b00t-cli mcp install browser-use dotmcpjson --stdio-command uvx\n  b00t-cli mcp install aws-knowledge dotmcpjson --httpstream\n  b00t-cli mcp install filesystem roocode\n  b00t-cli mcp install filesystem codex\n  b00t-cli mcp install filesystem stdout\n  b00t-cli app vscode mcp install filesystem"
    )]
    Install {
        #[clap(help = "MCP server name")]
        name: String,
        #[clap(
            help = "Installation target: claudecode, vscode, geminicli, dotmcpjson, roocode, codex, stdout"
        )]
        target: String,
        #[clap(long, help = "Install to repository-specific location (for geminicli)")]
        repo: bool,
        #[clap(long, help = "Install to user-global location (for geminicli)")]
        user: bool,
        #[clap(
            long,
            help = "Select stdio method by command (for multi-source MCP configs)"
        )]
        stdio_command: Option<String>,
        #[clap(long, help = "Use httpstream method (for multi-source MCP configs)")]
        httpstream: bool,
    },
    #[clap(
        about = "Output MCP servers in various formats",
        long_about = "Output MCP servers in various formats for configuration files.\n\nExamples:\n  b00t-cli mcp output filesystem,brave-search\n  b00t-cli mcp output --json filesystem\n  b00t-cli mcp output --mcpServers filesystem,brave-search"
    )]
    Output {
        #[clap(long = "json", help = "Output raw JSON format without wrapper", action = clap::ArgAction::SetTrue)]
        json: bool,
        #[clap(long = "mcpServers", help = "Output in mcpServers format (default)", action = clap::ArgAction::SetTrue)]
        mcp_servers: bool,
        #[clap(help = "Comma-separated list of MCP server names to output")]
        servers: String,
    },
    #[clap(
        about = "MCP Registry operations (list, search, install dependencies)",
        long_about = "Interact with b00t MCP registry for server management and dependency installation.\n\nExamples:\n  b00t-cli mcp registry list\n  b00t-cli mcp registry search --tag docker\n  b00t-cli mcp registry get io.b00t/server-name\n  b00t-cli mcp registry install-deps io.b00t/server-name\n  b00t-cli mcp registry sync-official\n  b00t-cli mcp registry sync-datums --path ~/.dotfiles/_b00t_"
    )]
    Registry {
        #[clap(subcommand)]
        action: RegistryAction,
    },
    #[clap(
        about = "Execute MCP tool via stdio transport",
        long_about = "Execute an MCP tool from a registered server via stdio transport.\n\nExamples (datum-based):\n  b00t-cli mcp execute filesystem read_file '{\"path\":\"/tmp/test.txt\"}'\n  b00t-cli mcp execute brave-search search '{\"query\":\"rust programming\"}'\n\nExamples (direct command):\n  b00t-cli mcp execute --command npx --args '-y,@modelcontextprotocol/server-filesystem' read_file '{\"path\":\"/file.txt\"}'\n  b00t-cli mcp execute -c uvx -a 'mcp-server-playwright' screenshot '{\"url\":\"https://example.com\"}'\n\nDiscovery:\n  b00t-cli mcp execute filesystem --discover\n  b00t-cli mcp execute --command npx --args '-y,@mcp/server-filesystem' --discover"
    )]
    Execute {
        #[clap(
            help = "MCP server name (from datum registry) or tool name (with --command). Optional in discovery mode with --command."
        )]
        server_or_tool: Option<String>,
        #[clap(help = "Tool name to execute (omit in discovery mode)")]
        tool: Option<String>,
        #[clap(help = "Tool parameters as JSON string (omit in discovery mode)")]
        params: Option<String>,
        #[clap(
            short,
            long,
            help = "Server command (alternative to server name, e.g., npx, uvx, docker)"
        )]
        command: Option<String>,
        #[clap(
            short,
            long,
            help = "Server arguments (comma-separated, e.g., '-y,@mcp/server')"
        )]
        args: Option<String>,
        #[clap(long, help = "Working directory for server process")]
        cwd: Option<String>,
        #[clap(short, long, help = "Discover and list available tools only")]
        discover: bool,
        #[clap(short = 'f', long, help = "Output format: json, text (default: text)")]
        format: Option<String>,
    },
}

#[derive(Parser)]
pub enum RegistryAction {
    #[clap(about = "List all registered MCP servers")]
    List {
        #[clap(long, help = "Output in JSON format")]
        json: bool,
    },
    #[clap(about = "Search for MCP servers by keyword or tag")]
    Search {
        #[clap(long, help = "Search keyword in name/description")]
        keyword: Option<String>,
        #[clap(long, help = "Search by tag")]
        tag: Option<String>,
    },
    #[clap(about = "Get detailed information about a specific server")]
    Get {
        #[clap(help = "Server ID (e.g., io.b00t/server-name)")]
        server_id: String,
    },
    #[clap(about = "Install dependencies for an MCP server")]
    InstallDeps {
        #[clap(help = "Server ID to install dependencies for")]
        server_id: String,
    },
    #[clap(about = "Sync with official MCP registry")]
    SyncOfficial,
    #[clap(about = "Auto-discover MCP servers from system")]
    Discover,
    #[clap(about = "Export registry in MCP format")]
    Export {
        #[clap(long, short, help = "Output file (default: stdout)")]
        output: Option<String>,
    },
    #[clap(about = "Sync registry from datum TOML files")]
    SyncDatums {
        #[clap(
            long,
            help = "Path to datums directory",
            default_value = "~/.dotfiles/_b00t_"
        )]
        path: String,
    },
}

impl McpCommands {
    pub async fn execute_async(&self, path: &str) -> Result<()> {
        match self {
            McpCommands::Register {
                name_or_json,
                hint: _,
                remove,
                dwiw,
                no_dwiw,
                command_args,
            } => {
                if *remove {
                    // Remove mode: delete the MCP server configuration
                    crate::mcp_remove(name_or_json, path)
                } else {
                    let actual_dwiw = !no_dwiw && *dwiw;

                    // Check if it's JSON mode (starts with { or -)
                    if name_or_json.starts_with('{') || name_or_json == "-" {
                        // JSON mode
                        crate::mcp_add_json(name_or_json, actual_dwiw, path)
                    } else if !command_args.is_empty() {
                        // Command mode: b00t-cli mcp register server-name -- npx -y @package
                        let server_name = name_or_json;
                        let command = &command_args[0];
                        let args = if command_args.len() > 1 {
                            command_args[1..].to_vec()
                        } else {
                            vec![]
                        };

                        let json_str = serde_json::json!({
                            "name": server_name,
                            "command": command,
                            "args": args
                        })
                        .to_string();

                        crate::mcp_add_json(&json_str, actual_dwiw, path)
                    } else {
                        anyhow::bail!(
                            "Invalid register command. Use JSON format or command format with --"
                        );
                    }
                }
            }
            McpCommands::List { json } => crate::mcp_list(path, *json),
            McpCommands::Install {
                name,
                target,
                repo,
                user,
                stdio_command,
                httpstream,
            } => {
                match target.as_str() {
                    "claudecode" | "claude" => crate::claude_code_install_mcp(name, path),
                    "vscode" => crate::vscode_install_mcp(name, path),
                    "codex" => crate::codex_install_mcp(name, path),
                    "geminicli" => {
                        // Determine installation location: default to repo if in git repo, otherwise user
                        let use_repo = if *repo && *user {
                            anyhow::bail!("Error: Cannot specify both --repo and --user flags");
                        } else if *repo {
                            true
                        } else if *user {
                            false
                        } else {
                            // Default behavior: repo if in git repo, otherwise user
                            crate::utils::is_git_repo()
                        };
                        crate::gemini_install_mcp(name, path, use_repo)
                    }
                    "dotmcpjson" => crate::dotmcpjson_install_mcp(
                        name,
                        path,
                        stdio_command.as_deref(),
                        *httpstream,
                    ),
                    "roocode" => {
                        // Design with internal arrays so we can extend merge/symlink targets over time.
                        // Primary write target is .roo/mcp.json. Merge from .mcp.json if present.
                        // Then non-destructively symlink .roo/mcp.json to .mcp.json (skip if .mcp.json exists and is not a symlink).
                        // For now, use the same logic as dotmcpjson but write to .roo/mcp.json
                        crate::dotmcpjson_install_mcp(
                            name,
                            path,
                            stdio_command.as_deref(),
                            *httpstream,
                        )
                    }
                    "stdout" => {
                        // Output just the JSON for the specified server
                        crate::mcp_output(path, false, name)
                    }
                    _ => {
                        anyhow::bail!(
                            "Error: Invalid target '{}'. Valid targets are: claudecode, vscode, geminicli, dotmcpjson, roocode, codex, stdout",
                            target
                        );
                    }
                }
            }
            McpCommands::Output {
                json,
                mcp_servers,
                servers,
            } => {
                let use_mcp_servers_wrapper = !json && (*mcp_servers || !servers.contains(','));
                crate::mcp_output(path, use_mcp_servers_wrapper, servers)
            }
            McpCommands::Registry { action } => action.execute_async().await,
            McpCommands::Execute {
                server_or_tool,
                tool,
                params,
                command,
                args,
                cwd,
                discover,
                format,
            } => {
                use b00t_c0re_lib::mcp_proxy::{GenericMcpProxy, McpServerConfig, McpToolRequest};
                use serde_json::Value as JsonValue;

                // Determine operating mode: datum-based or direct command
                let (server_config, tool_name) = if let Some(cmd) = command {
                    // Direct command mode: --command specified
                    let parsed_args = args
                        .as_ref()
                        .map(|a| a.split(',').map(|s| s.trim().to_string()).collect())
                        .unwrap_or_default();

                    let config = McpServerConfig {
                        command: cmd.clone(),
                        args: parsed_args,
                        cwd: cwd.clone(),
                        env: None,
                        timeout_ms: Some(30000),
                    };

                    // In direct mode, server_or_tool is the tool name (optional in discovery mode)
                    let tool_name = if *discover {
                        None
                    } else {
                        server_or_tool.clone()
                    };

                    (config, tool_name)
                } else {
                    // Datum-based mode: lookup server from registry
                    let server_name = server_or_tool.as_ref().ok_or_else(|| {
                        anyhow::anyhow!("Server name required (or use --command for direct mode)")
                    })?;

                    // Load MCP datum config
                    let datum = crate::get_mcp_config(server_name, path)?;

                    // Extract stdio method from datum
                    if let Some(mcp) = datum.mcp {
                        if let Some(stdio_methods) = mcp.stdio {
                            if let Some(first_method) = stdio_methods.first() {
                                // Parse stdio method (it's stored as a HashMap<String, Value>)
                                let cmd = first_method
                                    .get("command")
                                    .and_then(|v| v.as_str())
                                    .ok_or_else(|| {
                                        anyhow::anyhow!("Missing 'command' in stdio method")
                                    })?
                                    .to_string();

                                let parsed_args = first_method
                                    .get("args")
                                    .and_then(|v| v.as_array())
                                    .map(|arr| {
                                        arr.iter()
                                            .filter_map(|v| v.as_str().map(String::from))
                                            .collect()
                                    })
                                    .unwrap_or_default();

                                let server_config = McpServerConfig {
                                    command: cmd,
                                    args: parsed_args,
                                    cwd: cwd.clone(),
                                    env: None,
                                    timeout_ms: Some(30000),
                                };

                                (server_config, tool.clone())
                            } else {
                                anyhow::bail!(
                                    "No stdio methods defined for server '{}'",
                                    server_name
                                );
                            }
                        } else {
                            anyhow::bail!("No stdio configuration for server '{}'", server_name);
                        }
                    } else {
                        anyhow::bail!("'{}' is not an MCP server", server_name);
                    }
                };

                // Create MCP proxy
                let mut proxy = GenericMcpProxy::new();

                // Discover tools from server
                println!("🔌 Connecting to MCP server...");
                let discovered_tools = proxy
                    .discover_tools_from_server(server_config.clone())
                    .await?;

                println!("✅ Discovered {} tools", discovered_tools.len());

                // Discovery mode: list tools and exit
                if *discover {
                    println!("\n📋 Available tools:");
                    for tool_name in discovered_tools {
                        // Get tool info for better display
                        if let Some(info) = proxy.get_tool(&tool_name) {
                            println!("  • {} - {}", tool_name, info.description);
                        } else {
                            println!("  • {}", tool_name);
                        }
                    }
                    return Ok(());
                }

                // Execute mode: validate tool name and params
                let tool_name = tool_name.ok_or_else(|| {
                    anyhow::anyhow!("Tool name required (or use --discover to list tools)")
                })?;

                let params_str = params
                    .as_ref()
                    .ok_or_else(|| anyhow::anyhow!("Tool parameters required (JSON string)"))?;

                // Parse params JSON
                let params_value: JsonValue = serde_json::from_str(params_str)
                    .map_err(|e| anyhow::anyhow!("Invalid JSON parameters: {}", e))?;

                // Execute tool
                println!("🚀 Executing tool '{}'...", tool_name);
                let request = McpToolRequest {
                    tool: tool_name.clone(),
                    params: params_value,
                    request_id: Some(uuid::Uuid::new_v4().to_string()),
                };

                let response = proxy.execute_tool(request).await?;

                // Format output
                match format.as_deref() {
                    Some("json") => {
                        println!("{}", serde_json::to_string_pretty(&response)?);
                    }
                    _ => {
                        // Text mode (default)
                        if response.success {
                            println!("✅ Success");
                            if let Some(data) = response.data {
                                println!("\n📊 Result:");
                                println!("{}", serde_json::to_string_pretty(&data)?);
                            }
                        } else {
                            println!(
                                "❌ Error: {}",
                                response
                                    .error
                                    .unwrap_or_else(|| "Unknown error".to_string())
                            );
                        }
                        println!("\n⏱️  Duration: {}ms", response.metadata.duration_ms);
                    }
                }

                Ok(())
            }
        }
    }
}

impl RegistryAction {
    pub async fn execute_async(&self) -> Result<()> {
        use b00t_c0re_lib::mcp_registry::McpRegistry;

        let mut registry = McpRegistry::default();

        match self {
            RegistryAction::List { json } => {
                let servers = registry.list();

                if *json {
                    println!("{}", serde_json::to_string_pretty(&servers)?);
                } else {
                    println!("📋 Registered MCP Servers:\n");
                    for server in servers {
                        println!("  {} ({})", server.name, server.id);
                        println!(
                            "    Command: {} {}",
                            server.config.command,
                            server.config.args.join(" ")
                        );
                        println!("    Tags: {}", server.tags.join(", "));
                        println!("    Status: {:?}", server.metadata.health_status);
                        println!();
                    }
                }
                Ok(())
            }
            RegistryAction::Search { keyword, tag } => {
                let results = if let Some(tag_val) = tag {
                    registry.search_by_tag(tag_val)
                } else if let Some(kw) = keyword {
                    registry.search(kw)
                } else {
                    anyhow::bail!("Must provide --keyword or --tag");
                };

                println!("🔍 Search Results ({} matches):\n", results.len());
                for server in results {
                    println!("  {} - {}", server.id, server.description);
                    println!("    Tags: {}", server.tags.join(", "));
                    println!();
                }
                Ok(())
            }
            RegistryAction::Get { server_id } => {
                if let Some(server) = registry.get(server_id) {
                    println!("{}", serde_json::to_string_pretty(&server)?);
                    Ok(())
                } else {
                    anyhow::bail!("Server '{}' not found in registry", server_id)
                }
            }
            RegistryAction::InstallDeps { server_id } => {
                println!("📦 Installing dependencies for {}...", server_id);
                registry.install_dependencies(server_id).await?;
                println!("✅ Dependencies installed successfully");
                Ok(())
            }
            RegistryAction::SyncOfficial => {
                println!("🔄 Syncing with official MCP registry...");
                let count = registry.sync_official_registry().await?;
                println!("✅ Synced {} servers from official registry", count);
                Ok(())
            }
            RegistryAction::Discover => {
                println!("🔍 Auto-discovering MCP servers from system...");
                let count = registry.auto_discover().await?;
                println!("✅ Discovered {} MCP servers", count);
                Ok(())
            }
            RegistryAction::Export { output } => {
                let json = registry.export_to_mcp_format()?;

                if let Some(path) = output {
                    std::fs::write(path, &json)?;
                    println!("✅ Registry exported to {}", path);
                } else {
                    println!("{}", json);
                }
                Ok(())
            }
            RegistryAction::SyncDatums { path } => {
                println!("🔄 Syncing registry from datum files...");
                let count = registry.sync_from_datums(path)?;
                println!("✅ Synced {} MCP servers from datum files", count);
                Ok(())
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mcp_commands_exist() {
        // Test with JSON format
        let register_cmd = McpCommands::Register {
            name_or_json: r#"{"name":"test-server","command":"npx","args":["-y","@test/package"]}"#
                .to_string(),
            hint: None,
            remove: false,
            dwiw: false,
            no_dwiw: false,
            command_args: vec![],
        };

        // This should fail because we don't have a valid test directory, but the command should parse correctly
        // The important thing is that it doesn't panic and processes the JSON correctly
        let rt = tokio::runtime::Runtime::new().unwrap();
        let result = rt.block_on(register_cmd.execute_async("/tmp/nonexistent"));
        assert!(result.is_err()); // Expected to fail due to invalid path, but should not panic

        // Test install command enum creation
        let install_cmd = McpCommands::Install {
            name: "test-server".to_string(),
            target: "claudecode".to_string(),
            repo: false,
            user: false,
            stdio_command: None,
            httpstream: false,
        };

        // This should fail because the server doesn't exist, but should not panic
        let result = rt.block_on(install_cmd.execute_async("/tmp/nonexistent"));
        assert!(result.is_err()); // Expected to fail, but should not panic
    }
}
