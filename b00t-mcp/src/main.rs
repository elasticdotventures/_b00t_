use anyhow::Result;
use clap::{Arg, Command};
use rmcp::{ServiceExt, transport::io::stdio};
use std::path::Path;

mod mcp_server;
mod acl;

use mcp_server::B00tMcpServer;

#[tokio::main]
async fn main() -> Result<()> {
    let matches = Command::new("b00t-mcp")
        .version(env!("CARGO_PKG_VERSION"))
        .author("b00t-mcp contributors")
        .about("MCP Server for b00t-cli command proxy with ACL filtering")
        .arg(
            Arg::new("working-dir")
                .short('d')
                .long("directory")
                .value_name("DIR")
                .help("Working directory for the MCP server")
                .default_value("."),
        )
        .arg(
            Arg::new("config")
                .short('c')
                .long("config")
                .value_name("FILE")
                .help("Path to ACL configuration file")
                .default_value("~/.dotfiles/b00t-mcp-acl.toml"),
        )
        .arg(
            Arg::new("stdio")
                .long("stdio")
                .help("Run as MCP server using stdio transport")
                .action(clap::ArgAction::SetTrue),
        )
        .get_matches();

    let working_dir = matches.get_one::<String>("working-dir").unwrap();
    let config_path = matches.get_one::<String>("config").unwrap();
    let working_path = Path::new(working_dir);

    if matches.get_flag("stdio") {
        // Run as MCP server
        eprintln!(
            "Starting b00t-mcp MCP server in directory: {} with config: {}",
            working_path.display(),
            config_path
        );

        let server = B00tMcpServer::new(working_path, config_path)?;

        // Start the MCP server with stdio transport
        let running_service = server.serve(stdio()).await?;

        // Keep the server running
        running_service.waiting().await?;
    } else {
        // Show usage information
        println!("b00t-mcp v{}", env!("CARGO_PKG_VERSION"));
        println!("MCP Server for b00t-cli command proxy with ACL filtering");
        println!();
        println!("Usage:");
        println!(
            "  {} --stdio                           Run as MCP server with stdio transport",
            env!("CARGO_PKG_NAME")
        );
        println!(
            "  {} --directory <DIR> --stdio         Run MCP server in specific directory",
            env!("CARGO_PKG_NAME")
        );
        println!(
            "  {} --config <FILE> --stdio           Run MCP server with custom ACL config",
            env!("CARGO_PKG_NAME")
        );
        println!();
        println!("MCP Tools Available:");
        println!("  b00t_detect      - Run b00t detect command");
        println!("  b00t_desires     - Run b00t desires command");
        println!("  b00t_install     - Run b00t install command");
        println!("  b00t_update      - Run b00t update command");
        println!("  b00t_up          - Run b00t up command");
        println!("  b00t_mcp         - Run b00t mcp commands");
        println!("  b00t_learn       - Run b00t learn command");
        println!("  b00t_whatismy    - Run b00t whatismy commands");
        println!();
        println!("Example usage with MCP client:");
        println!("  {} --stdio | your-mcp-client", env!("CARGO_PKG_NAME"));
    }

    Ok(())
}