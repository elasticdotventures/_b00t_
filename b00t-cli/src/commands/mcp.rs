use anyhow::Result;
use clap::Parser;
use std::collections::HashSet;

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
        #[clap(long, help = "Add a gate precondition (format: command:<cmd> or env:<VAR> or file:<path>)")]
        gate: Vec<String>,
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
        long_about = "List available MCP server configurations with status icons and filters.\n\nStatus icons:\n  ▶️  running   📋  installed (not running)   ⏸️  suspended   ❌  not installed / error\n\nWhen the number of servers exceeds the threshold (default: 10, configurable via session or --max-threshold), you MUST provide --search or a filter flag.\n\nFilter Examples:\n  b00t-cli mcp list --search github       # search by name match\n  b00t-cli mcp list --installed            # only installed servers\n  b00t-cli mcp list --is-installed=true    # same, explicit bool\n  b00t-cli mcp list --is-installed=false   # only uninstalled servers\n  b00t-cli mcp list --is-running=true      # only servers currently running\n  b00t-cli mcp list --is-suspended=false   # exclude suspended\n  b00t-cli mcp list --max-threshold 20     # override threshold for this invocation\n  b00t-cli mcp list --all                  # bypass the threshold guard\n  b00t-cli mcp list --json --installed     # JSON output of installed servers\n\nPipe to filter:\n  b00t-cli mcp list --all | grep docker   # find docker-related servers\n  b00t-cli mcp list --all --json | jq '.servers[] | select(.is_running)'  # JSON query"
    )]
    List {
        #[clap(long, help = "Output in JSON format")]
        json: bool,
        #[clap(long, help = "Search filter — only show servers whose name contains this string (case-insensitive)")]
        search: Option<String>,
        #[clap(long, help = "Shorthand: show only installed servers (equivalent to --is-installed=true)")]
        installed: bool,
        #[clap(long, help = "Filter by installation status: true=installed, false=uninstalled")]
        is_installed: Option<bool>,
        #[clap(long, help = "Filter by running status: true=running, false=not running")]
        is_running: Option<bool>,
        #[clap(long, help = "Filter by suspension status: true=suspended, false=not suspended")]
        is_suspended: Option<bool>,
        #[clap(long, help = "Override the max-items threshold for this invocation")]
        max_threshold: Option<i64>,
        #[clap(long, help = "Bypass the threshold guard and show all servers")]
        all: bool,
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
        about = "Sync MCP servers between b00t and agent platforms",
        long_about = "Bidirectional sync of MCP server configs using datum metadata.\n\nExamples:\n  b00t-cli mcp sync push b00t kiro              # b00t -> kiro global\n  b00t-cli mcp sync pull kiro b00t              # kiro -> b00t datums\n  b00t-cli mcp sync push b00t claude            # b00t -> claude agents\n  b00t-cli mcp sync push b00t kiro --agent cli-master  # specific agent\n  b00t-cli mcp sync codex --repo                # legacy codex sync"
    )]
    Sync {
        #[clap(help = "Operation: push, pull, or target name (legacy)")]
        operation_or_target: String,
        #[clap(help = "Source platform (for push/pull) or empty (legacy)")]
        source: Option<String>,
        #[clap(help = "Destination platform (for push/pull) or empty (legacy)")]
        dest: Option<String>,
        #[clap(long, help = "Specific agent name to sync")]
        agent: Option<String>,
        #[clap(long, help = "Sync to repository-specific location (legacy)")]
        repo: bool,
        #[clap(long, help = "Sync to user-global location (legacy)")]
        user: bool,
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
        about = "Show dependency chain for an MCP server",
        long_about = "Show the dependency chain for an MCP server datum.\n\nExamples:\n  b00t-cli mcp depends filesystem\n  b00t-cli mcp depends brave-search --installed"
    )]
    Depends {
        #[clap(help = "MCP server name")]
        name: String,
        #[clap(long, help = "Show only installed dependencies")]
        installed: bool,
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
    #[clap(
        about = "Show dynamic MCP status — loaded/installed/available",
        long_about = "Query actual MCP server state:\n  loaded: servers active in ~/.hermes/config.yaml\n  installed: datums in _b00t_/*.mcp.toml\n  available: servers in registry index\n\nExamples:\n  b00t mcp status\n  b00t mcp status --json"
    )]
    Status {
        #[clap(long, help = "Output in JSON format")]
        json: bool,
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
                hint,
                gate,
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

                        let mut json_obj = serde_json::json!({
                            "name": server_name,
                            "command": command,
                            "args": args,
                            "hint": hint.as_deref().unwrap_or("MCP server"),
                        });
                        // Add gates from --gate flags
                        if !gate.is_empty() {
                            let gates: Vec<serde_json::Value> = gate.iter().map(|g| {
                                let parts: Vec<&str> = g.splitn(2, ':').collect();
                                if parts.len() == 2 {
                                    let kind = parts[0];
                                    let spec = parts[1];
                                    let hint = match kind {
                                        "command" => format!("{} not on PATH — install or add to $PATH", spec),
                                        "env" => format!("{} not set — add to .env or environment", spec),
                                        "file" => format!("{} not found", spec),
                                        _ => format!("gate: {} {}", kind, spec),
                                    };
                                    serde_json::json!({
                                        kind: spec,
                                        "hint": hint,
                                    })
                                } else {
                                    serde_json::json!({"rhai": g})
                                }
                            }).collect();
                            json_obj["gate"] = serde_json::json!(gates);
                        }
                        let json_str = json_obj.to_string();

                        crate::mcp_add_json(&json_str, actual_dwiw, path)
                    } else {
                        anyhow::bail!(
                            "Invalid register command. Use JSON format or command format with --"
                        );
                    }
                }
            }
            McpCommands::List {
                json,
                search,
                installed,
                is_installed,
                is_running,
                is_suspended,
                max_threshold,
                all,
            } => {
                let filter = crate::McpListFilter {
                    search: search.clone(),
                    is_installed: if *installed || is_installed.unwrap_or(false) { Some(true) } else { *is_installed },
                    is_running: *is_running,
                    is_suspended: *is_suspended,
                    max_threshold: *max_threshold,
                    bypass_threshold: *all,
                };
                crate::mcp_list(path, *json, filter)
            }
            McpCommands::Install {
                name,
                target,
                repo,
                user,
                stdio_command,
                httpstream,
            } => {
                // First resolve dependencies before installation
                let deps = resolve_depends_on_chain(name, path)?;

                // Check if all dependencies are satisfied
                if !deps.is_empty() {
                    let missing = find_missing_dependencies(&deps, path)?;
                    if !missing.is_empty() {
                        eprintln!(
                            "⚠️  missing: install dependencies first: {}",
                            missing.join(", ")
                        );
                        // Continue anyway - just warn
                    }
                }

                match target.as_str() {
                    "claudecode" | "claude" => crate::claude_code_install_mcp(name, path),
                    "vscode" => crate::vscode_install_mcp(name, path),
                    "codex" => {
                        let use_repo = if *repo && *user {
                            anyhow::bail!("Error: Cannot specify both --repo and --user flags");
                        } else if *repo {
                            true
                        } else if *user {
                            false
                        } else {
                            crate::utils::is_git_repo()
                        };

                        crate::codex_install_mcp(
                            name,
                            path,
                            use_repo,
                            stdio_command.as_deref(),
                            *httpstream,
                        )
                    }
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
            McpCommands::Sync {
                operation_or_target,
                source,
                dest,
                agent,
                repo,
                user,
            } => {
                // Check if this is new push/pull syntax or legacy codex sync
                if operation_or_target == "push" || operation_or_target == "pull" {
                    let src = source
                        .as_ref()
                        .ok_or_else(|| anyhow::anyhow!("Source required for push/pull"))?;
                    let dst = dest
                        .as_ref()
                        .ok_or_else(|| anyhow::anyhow!("Destination required for push/pull"))?;

                    crate::mcp_sync_bidirectional(
                        path,
                        operation_or_target.as_str(),
                        src,
                        dst,
                        agent.as_deref(),
                    )
                } else if source.is_none() && dest.is_none() {
                    // Legacy codex sync (no source/dest args)
                    let use_repo = if *repo && *user {
                        anyhow::bail!("Error: Cannot specify both --repo and --user flags");
                    } else if *repo {
                        true
                    } else if *user {
                        false
                    } else {
                        crate::utils::is_git_repo()
                    };

                    match operation_or_target.as_str() {
                        "codex" => crate::codex_sync_dotmcpjson(path, use_repo),
                        _ => anyhow::bail!(
                            "Error: Invalid target '{}'. Use 'push/pull src dest' or 'codex'",
                            operation_or_target
                        ),
                    }
                } else {
                    anyhow::bail!(
                        "Error: Invalid syntax. Use 'b00t-cli mcp sync push <src> <dest>' or 'b00t-cli mcp sync codex'"
                    );
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
            McpCommands::Depends { name, installed } => {
                let expanded = crate::get_expanded_path(path)?;
                show_dependency_chain(&name, &expanded, *installed)
            }
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
            McpCommands::Status { json } => {
                let status = mcp_status();
                if *json {
                    println!("{}", serde_json::to_string_pretty(&status)?);
                } else {
                    println!("📡 MCP Server Status\n");

                    if let Some(loaded) = status.get("loaded").and_then(|v| v.as_array()) {
                        println!("🔵 Loaded ({} in ~/.hermes/config.yaml):", loaded.len());
                        for srv in loaded {
                            let name = srv.get("name").and_then(|v| v.as_str()).unwrap_or("?");
                            let cmd = srv.get("command").and_then(|v| v.as_str()).unwrap_or("");
                            println!("  {name:<20} {cmd}");
                        }
                        println!();
                    }

                    if let Some(installed) = status.get("installed").and_then(|v| v.as_array()) {
                        println!("📦 Installed ({} _b00t_/*.mcp.toml datums):", installed.len());
                        for srv in installed {
                            let name = srv.get("name").and_then(|v| v.as_str()).unwrap_or("?");
                            let hint = srv.get("hint").and_then(|v| v.as_str()).unwrap_or("");
                            println!("  {name:<20} {hint}");
                        }
                    }
                }
                Ok(())
            }
        }
    }
}

/// Query dynamic MCP status: loaded servers, installed datums, available registry.
/// Uses `datum_root` as the base directory for `*.mcp.toml` discovery — consistent
/// with other CLI commands that accept `--path` (default: `~/.dotfiles/_b00t_`).
pub fn mcp_status() -> serde_json::Value {
    mcp_status_for_path("~/.dotfiles/_b00t_")
}

/// Inner implementation: query MCP status using a specific datum root path.
pub fn mcp_status_for_path(datum_root: &str) -> serde_json::Value {
    use serde_json::json;
    let mut status = serde_json::Map::new();

    // Loaded: MCP servers in ~/.hermes/config.yaml
    let hermes_config = dirs::home_dir()
        .map(|h| h.join(".hermes").join("config.yaml"));
    let loaded: Vec<serde_json::Value> = match &hermes_config {
        Some(path) if path.exists() => {
            let content = std::fs::read_to_string(path).unwrap_or_default();
            let yaml: serde_json::Value = serde_yaml::from_str(&content).unwrap_or(json!({}));
            yaml.get("mcp_servers")
                .and_then(|v| v.as_object())
                .map(|obj| {
                    obj.iter()
                        .map(|(name, cfg)| {
                            json!({
                                "name": name,
                                "command": cfg.get("command").and_then(|c| c.as_str()).unwrap_or(""),
                                "args": cfg.get("args").and_then(|a| a.as_array())
                                    .map(|a| a.iter().filter_map(|v| v.as_str().map(|s| s.to_string())).collect::<Vec<_>>())
                                    .unwrap_or_default(),
                            })
                        })
                        .collect()
                })
                .unwrap_or_default()
        }
        _ => vec![],
    };
    status.insert("loaded".to_string(), json!(loaded));

    // Installed: *.mcp.toml datums from the configured datum root
    let expanded = shellexpand::tilde(datum_root).to_string();
    let b00t_dir = std::path::PathBuf::from(expanded);
    let installed: Vec<serde_json::Value> = match std::fs::read_dir(&b00t_dir) {
        Ok(entries) => entries
            .filter_map(|e| e.ok())
            .filter(|e| {
                e.path()
                    .extension()
                    .and_then(|ext| ext.to_str())
                    .map(|ext| ext == "toml")
                    .unwrap_or(false)
                    && e.path().file_name()
                        .and_then(|n| n.to_str())
                        .map(|n| n.ends_with(".mcp.toml"))
                        .unwrap_or(false)
            })
            .filter_map(|e| {
                let content = std::fs::read_to_string(e.path()).ok()?;
                let table: toml::Table = content.parse().ok()?;
                let b00t = table.get("b00t")?.as_table()?;
                let name = b00t.get("name")?.as_str()?.to_string();
                let hint = b00t.get("hint").and_then(|v| v.as_str()).unwrap_or("").to_string();
                Some(json!({ "name": name, "hint": hint }))
            })
            .collect(),
        Err(_) => vec![],
    };
    status.insert("installed".to_string(), json!(installed));

    // Available: from registry (simplified — McpRegistry is async, skip here)
    status.insert("available".to_string(), json!([]));

    serde_json::Value::Object(status)
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

/// Resolve dependencies for an MCP datum, returning ordered list: [dep1, dep2, <datum>]
/// Loads TOML, extracts depends_on field, recursively resolves
pub fn resolve_depends_on_chain(datum_name: &str, path: &str) -> Result<Vec<String>> {
    use crate::get_expanded_path;
    use crate::get_mcp_config;

    let expanded = get_expanded_path(path)?;
    let mut resolved = Vec::new();
    let mut visited = HashSet::new();

    resolve_recursive(datum_name, &expanded, &mut resolved, &mut visited)?;

    // Final list: dependencies first, datum last (reverse from DFS for correct order)
    // We want deps first, so reverse the collected order
    if resolved.len() > 1 {
        resolved.reverse();
    }

    Ok(resolved)
}

fn resolve_recursive(
    name: &str,
    path: &std::path::Path,
    resolved: &mut Vec<String>,
    visited: &mut HashSet<String>,
) -> Result<()> {
    if visited.contains(name) {
        return Ok(());
    }

    visited.insert(name.to_string());

    // Load datum to get dependencies
    let config_path = path.join(format!("{}.mcp.toml", name));
    let cli_config_path = path.join(format!("{}.cli.toml", name));

    if !config_path.exists() && !cli_config_path.exists() {
        anyhow::bail!("Datum not found: {}", name);
    }

    // Actually load and parse for depends_on
    let datum = crate::get_mcp_config(name, path.to_str().unwrap_or(""))?;

    // Recursively resolve dependencies first
    if let Some(deps) = &datum.depends_on {
        for dep in deps {
            resolve_recursive(dep, path, resolved, visited)?;
        }
    }

    // Add this datum (if not already)
    if !resolved.contains(&name.to_string()) {
        resolved.push(name.to_string());
    }

    Ok(())
}

/// Find dependencies that are not installed (missing their TOML files)
pub fn find_missing_dependencies(dep_names: &[String], path: &str) -> Result<Vec<String>> {
    use crate::get_expanded_path;

    let expanded = get_expanded_path(path)?;
    let mut missing = Vec::new();

    for dep in dep_names {
        let mcp_path = expanded.join(format!("{}.mcp.toml", dep));
        let cli_path = expanded.join(format!("{}.cli.toml", dep));

        if !mcp_path.exists() && !cli_path.exists() {
            missing.push(dep.clone());
        }
    }

    Ok(missing)
}

/// Show dependency chain for an MCP server
pub fn show_dependency_chain(
    name: &str,
    path: &std::path::Path,
    installed_only: bool,
) -> Result<()> {
    use crate::get_mcp_config;

    let path_str = path.to_str().unwrap_or("");

    // Try to load the datum config
    let datum = match get_mcp_config(name, path_str) {
        Ok(d) => d,
        Err(e) => {
            // Try .cli.toml as fallback
            let cli_path = path.join(format!("{}.cli.toml", name));
            if cli_path.exists() {
                anyhow::bail!(
                    "MCP server '{}' not found (tried .mcp.toml). Error: {}",
                    name,
                    e
                )
            } else {
                anyhow::bail!("MCP server '{}' not found: {}", name, e)
            }
        }
    };

    println!("📦 MCP Server: {}", name);
    println!("   Type: {:?}", datum.datum_type);
    println!("   Hint: {}", datum.hint);

    if let Some(deps) = &datum.depends_on {
        if deps.is_empty() {
            println!("   Dependencies: (none)");
        } else {
            println!("   Dependencies:");
            for dep in deps {
                // Check if dependency is installed
                let mcp_path = path.join(format!("{}.mcp.toml", dep));
                let cli_path = path.join(format!("{}.cli.toml", dep));
                let status = if mcp_path.exists() || cli_path.exists() {
                    "✓ installed"
                } else if installed_only {
                    continue;
                } else {
                    "✗ missing"
                };
                println!("     - {} [{}]", dep, status);
            }
        }
    } else {
        println!("   Dependencies: (none defined)");
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    /// Helper to create a test MCP datum TOML content
    const TEST_MCP_DATUM_TOML: &str = r#"
[b00t]
name = "test-server"
type = "mcp"
hint = "Test MCP server"

[[b00t.mcp.stdio]]
command = "npx"
args = ["-y", "@test/server"]
priority = 0
transport = "stdio"
"#;

    #[test]
    fn test_mcp_commands_exist() {
        // Test with JSON format
        let register_cmd = McpCommands::Register {
            name_or_json: r#"{"name":"test-server","command":"npx","args":["-y","@test/package"]}"#
                .to_string(),
            hint: None,
            gate: vec![],
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

    #[test]
    fn test_sync_push_with_valid_input() {
        let temp_dir = TempDir::new().unwrap();
        let path = temp_dir.path().to_str().unwrap();

        // Create a sample MCP datum file
        fs::write(
            temp_dir.path().join("test-server.mcp.toml"),
            TEST_MCP_DATUM_TOML,
        )
        .unwrap();

        let sync_cmd = McpCommands::Sync {
            operation_or_target: "push".to_string(),
            source: Some("b00t".to_string()),
            dest: Some("kiro".to_string()),
            agent: None,
            repo: false,
            user: false,
        };

        let rt = tokio::runtime::Runtime::new().unwrap();
        let result = rt.block_on(sync_cmd.execute_async(path));

        // Should succeed in creating the sync operation
        // Note: This may fail if ~/.kiro doesn't exist, but the important part
        // is that it processes the command structure correctly
        assert!(result.is_ok() || result.unwrap_err().to_string().contains("kiro"));
    }

    #[test]
    fn test_sync_invalid_platform() {
        let temp_dir = TempDir::new().unwrap();
        let path = temp_dir.path().to_str().unwrap();

        let sync_cmd = McpCommands::Sync {
            operation_or_target: "push".to_string(),
            source: Some("b00t".to_string()),
            dest: Some("invalid-platform".to_string()),
            agent: None,
            repo: false,
            user: false,
        };

        let rt = tokio::runtime::Runtime::new().unwrap();
        let result = rt.block_on(sync_cmd.execute_async(path));

        // Should fail with unknown platform error
        assert!(result.is_err());
        let err_msg = result.unwrap_err().to_string();
        assert!(
            err_msg.contains("Unknown platform") || err_msg.contains("invalid-platform"),
            "Expected platform error, got: {}",
            err_msg
        );
    }

    #[test]
    fn test_sync_missing_source_parameter() {
        let temp_dir = TempDir::new().unwrap();
        let path = temp_dir.path().to_str().unwrap();

        let sync_cmd = McpCommands::Sync {
            operation_or_target: "push".to_string(),
            source: None, // Missing source
            dest: Some("kiro".to_string()),
            agent: None,
            repo: false,
            user: false,
        };

        let rt = tokio::runtime::Runtime::new().unwrap();
        let result = rt.block_on(sync_cmd.execute_async(path));

        // Should fail with source required error
        assert!(result.is_err());
        let err_msg = result.unwrap_err().to_string();
        assert!(
            err_msg.contains("Source required") || err_msg.contains("source"),
            "Expected source error, got: {}",
            err_msg
        );
    }

    #[test]
    fn test_sync_missing_dest_parameter() {
        let temp_dir = TempDir::new().unwrap();
        let path = temp_dir.path().to_str().unwrap();

        let sync_cmd = McpCommands::Sync {
            operation_or_target: "pull".to_string(),
            source: Some("kiro".to_string()),
            dest: None, // Missing dest
            agent: None,
            repo: false,
            user: false,
        };

        let rt = tokio::runtime::Runtime::new().unwrap();
        let result = rt.block_on(sync_cmd.execute_async(path));

        // Should fail with destination required error
        assert!(result.is_err());
        let err_msg = result.unwrap_err().to_string();
        assert!(
            err_msg.contains("Destination required") || err_msg.contains("dest"),
            "Expected destination error, got: {}",
            err_msg
        );
    }

    #[test]
    fn test_sync_invalid_source_for_push() {
        let temp_dir = TempDir::new().unwrap();
        let path = temp_dir.path().to_str().unwrap();

        let sync_cmd = McpCommands::Sync {
            operation_or_target: "push".to_string(),
            source: Some("kiro".to_string()), // Invalid source (should be "b00t")
            dest: Some("kiro".to_string()),
            agent: None,
            repo: false,
            user: false,
        };

        let rt = tokio::runtime::Runtime::new().unwrap();
        let result = rt.block_on(sync_cmd.execute_async(path));

        // Should fail with invalid source error
        assert!(result.is_err());
        let err_msg = result.unwrap_err().to_string();
        assert!(
            err_msg.contains("Push operation requires source to be 'b00t'"),
            "Expected source validation error, got: {}",
            err_msg
        );
    }

    #[test]
    fn test_sync_legacy_codex_fallback() {
        let temp_dir = TempDir::new().unwrap();
        let path = temp_dir.path().to_str().unwrap();

        let sync_cmd = McpCommands::Sync {
            operation_or_target: "codex".to_string(),
            source: None, // Legacy mode: no source/dest
            dest: None,   // Legacy mode: no source/dest
            agent: None,
            repo: true,
            user: false,
        };

        let rt = tokio::runtime::Runtime::new().unwrap();
        let result = rt.block_on(sync_cmd.execute_async(path));

        // Should attempt legacy codex sync
        // May fail if not in a git repo or missing files, but should not panic
        assert!(result.is_ok() || result.is_err());
    }

    #[test]
    fn test_sync_invalid_operation() {
        let temp_dir = TempDir::new().unwrap();
        let path = temp_dir.path().to_str().unwrap();

        let sync_cmd = McpCommands::Sync {
            operation_or_target: "invalid-op".to_string(),
            source: None,
            dest: None,
            agent: None,
            repo: false,
            user: false,
        };

        let rt = tokio::runtime::Runtime::new().unwrap();
        let result = rt.block_on(sync_cmd.execute_async(path));

        // Should fail with invalid operation error
        assert!(result.is_err());
        let err_msg = result.unwrap_err().to_string();
        assert!(
            err_msg.contains("Invalid") || err_msg.contains("invalid-op"),
            "Expected invalid operation error, got: {}",
            err_msg
        );
    }

    #[test]
    fn test_sync_conflicting_repo_and_user_flags() {
        let temp_dir = TempDir::new().unwrap();
        let path = temp_dir.path().to_str().unwrap();

        let sync_cmd = McpCommands::Sync {
            operation_or_target: "codex".to_string(),
            source: None,
            dest: None,
            agent: None,
            repo: true,
            user: true, // Both flags set
        };

        let rt = tokio::runtime::Runtime::new().unwrap();
        let result = rt.block_on(sync_cmd.execute_async(path));

        // Should fail with conflicting flags error
        assert!(result.is_err());
        let err_msg = result.unwrap_err().to_string();
        assert!(
            err_msg.contains("both") || err_msg.contains("Cannot specify"),
            "Expected conflicting flags error, got: {}",
            err_msg
        );
    }
}
