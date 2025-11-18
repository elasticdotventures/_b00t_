//! mcp-user - Standalone utility for executing MCP tools via stdio transport
//!
//! Provides a simplified interface for running MCP tools without registering them in datums.
//! Useful for quick testing, scripting, and agents that can't run MCP directly.
//!
//! # Examples
//!
//! ```bash
//! # Discover tools from an MCP server
//! mcp-user -c npx -a '-y,@modelcontextprotocol/server-filesystem' --discover
//!
//! # Execute a tool
//! mcp-user -c npx -a '-y,@modelcontextprotocol/server-filesystem' \
//!   read_file '{"path":"/tmp/test.txt"}'
//!
//! # Use with uvx (Python-based MCP servers)
//! mcp-user -c uvx -a 'mcp-server-playwright' screenshot '{"url":"https://example.com"}'
//! ```

use anyhow::{Context, Result};
use clap::Parser;

#[derive(Parser)]
#[clap(
    name = "mcp-user",
    about = "Execute MCP tools via stdio transport",
    long_about = "Standalone utility for executing MCP (Model Context Protocol) tools via stdio.\n\nProvides a simplified interface for running MCP tools without datum registration.\nUseful for testing, scripting, and agents that can't run MCP directly."
)]
struct Args {
    #[clap(
        short,
        long,
        help = "Server command (e.g., npx, uvx, docker, node, python)"
    )]
    command: String,

    #[clap(
        short,
        long,
        help = "Server arguments (comma-separated, e.g., '-y,@mcp/server-filesystem')"
    )]
    args: Option<String>,

    #[clap(help = "Tool name to execute (omit with --discover)")]
    tool: Option<String>,

    #[clap(help = "Tool parameters as JSON string (omit with --discover)")]
    params: Option<String>,

    #[clap(short, long, help = "Discover and list available tools only")]
    discover: bool,

    #[clap(short = 'f', long, help = "Output format: json, text (default: text)")]
    format: Option<String>,

    #[clap(long, help = "Working directory for server process")]
    cwd: Option<String>,

    #[clap(short, long, help = "Verbose output for debugging")]
    verbose: bool,
}

#[tokio::main]
async fn main() -> Result<()> {
    let args = Args::parse();

    // Initialize logging if verbose
    if args.verbose {
        env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();
    }

    // Parse server arguments
    let parsed_args = args
        .args
        .as_ref()
        .map(|a| a.split(',').map(|s| s.trim().to_string()).collect())
        .unwrap_or_default();

    // Create server config
    use b00t_c0re_lib::mcp_proxy::{GenericMcpProxy, McpServerConfig, McpToolRequest};

    let server_config = McpServerConfig {
        command: args.command.clone(),
        args: parsed_args,
        cwd: args.cwd.clone(),
        env: None,
        timeout_ms: Some(30000),
    };

    // Create MCP proxy
    let mut proxy = GenericMcpProxy::new();

    // Discover tools from server
    if args.verbose {
        eprintln!("🔌 Connecting to MCP server: {} ...", args.command);
    }

    let discovered_tools = proxy
        .discover_tools_from_server(server_config.clone())
        .await
        .context("Failed to discover tools from MCP server")?;

    if args.verbose {
        eprintln!("✅ Discovered {} tools", discovered_tools.len());
    }

    // Discovery mode: list tools and exit
    if args.discover {
        println!("📋 Available tools from '{}':\n", args.command);
        for tool_name in discovered_tools {
            // Get tool info for better display
            if let Some(info) = proxy.get_tool(&tool_name) {
                println!("  • {} - {}", tool_name, info.description);

                // Show input schema if available and verbose
                if args.verbose {
                    println!(
                        "    Schema: {}",
                        serde_json::to_string_pretty(&info.input_schema).unwrap_or_default()
                    );
                }
            } else {
                println!("  • {}", tool_name);
            }
        }
        return Ok(());
    }

    // Execute mode: validate tool name and params
    let tool_name = args
        .tool
        .ok_or_else(|| anyhow::anyhow!("Tool name required (or use --discover to list tools)"))?;

    let params_str = args
        .params
        .as_ref()
        .ok_or_else(|| anyhow::anyhow!("Tool parameters required (JSON string)"))?;

    // Parse params JSON
    let params_value: serde_json::Value =
        serde_json::from_str(params_str).context("Invalid JSON parameters")?;

    // Execute tool
    if args.verbose {
        eprintln!("🚀 Executing tool '{}'...", tool_name);
    }

    let request = McpToolRequest {
        tool: tool_name.clone(),
        params: params_value,
        request_id: Some(uuid::Uuid::new_v4().to_string()),
    };

    let response = proxy
        .execute_tool(request)
        .await
        .context("Failed to execute tool")?;

    // Format output
    match args.format.as_deref() {
        Some("json") => {
            println!("{}", serde_json::to_string_pretty(&response)?);
        }
        _ => {
            // Text mode (default)
            if response.success {
                if !args.verbose {
                    // In non-verbose mode, just output the data
                    if let Some(data) = response.data {
                        println!("{}", serde_json::to_string_pretty(&data)?);
                    }
                } else {
                    // Verbose mode: show full details
                    println!("✅ Success");
                    if let Some(data) = response.data {
                        println!("\n📊 Result:");
                        println!("{}", serde_json::to_string_pretty(&data)?);
                    }
                    println!("\n⏱️  Duration: {}ms", response.metadata.duration_ms);
                }
            } else {
                eprintln!(
                    "❌ Error: {}",
                    response
                        .error
                        .unwrap_or_else(|| "Unknown error".to_string())
                );
                std::process::exit(1);
            }
        }
    }

    Ok(())
}
