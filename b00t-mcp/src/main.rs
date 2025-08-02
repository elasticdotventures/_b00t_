use anyhow::Result;
use clap::{Arg, Command};
use rmcp::{ServiceExt, transport::io::stdio};
use std::path::Path;
use std::net::SocketAddr;

use rmcp::transport::streamable_http_server::{
    StreamableHttpService, StreamableHttpServerConfig,
    session::local::LocalSessionManager,
};
use tower_http::cors::CorsLayer;
use axum::Router;
use tokio::net::TcpListener;

use b00t_mcp::B00tMcpServerRusty;

#[tokio::main]
async fn main() -> Result<()> {
    let matches = Command::new("b00t-mcp")
        .version(env!("GIT_REPO_TAG_VERSION"))
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
        .arg(
            Arg::new("http")
                .long("http")
                .help("Run as MCP server using HTTP transport")
                .action(clap::ArgAction::SetTrue),
        )
        .arg(
            Arg::new("port")
                .short('p')
                .long("port")
                .value_name("PORT")
                .help("Port for HTTP server")
                .default_value("3000"),
        )
        .arg(
            Arg::new("host")
                .long("host")
                .value_name("HOST")
                .help("Host address for HTTP server")
                .default_value("127.0.0.1"),
        )
        .arg(
            Arg::new("mode")
                .help("Transport mode (stdio or http)")
                .value_parser(["stdio", "http"])
                .index(1),
        )
        .get_matches();

    let working_dir = matches.get_one::<String>("working-dir").unwrap().clone();
    let config_path = matches.get_one::<String>("config").unwrap().clone();
    let working_path = Path::new(&working_dir);
    
    let host = matches.get_one::<String>("host").unwrap().clone();
    let port = matches.get_one::<String>("port").unwrap().parse::<u16>()
        .expect("Invalid port number");

    let is_stdio_mode = matches.get_flag("stdio") || matches.get_one::<String>("mode").map_or(false, |m| m == "stdio");
    let is_http_mode = matches.get_flag("http") || matches.get_one::<String>("mode").map_or(false, |m| m == "http");
    
    if is_stdio_mode {
        // Run as MCP server
        // eprintln!(
        //     "Starting b00t-mcp MCP server in directory: {} with config: {}",
        //     working_path.display(),
        //     config_path
        // );

        // No stderr output in stdio mode as it breaks the MCP protocol
        let server = B00tMcpServerRusty::new(working_path, &config_path)?;
        let running_service = server.serve(stdio()).await?;

        // Keep the server running
        running_service.waiting().await?;
    } else if is_http_mode {
        // HTTP server mode
        let addr: SocketAddr = format!("{}:{}", host, port).parse()?;
        
        eprintln!("🌐 Starting HTTP MCP server on http://{}", addr);
        eprintln!("🦀 Rusty MCP server with {} compile-time tools", 
                 B00tMcpServerRusty::new(working_path, &config_path)?.tool_count());
        
        // Create HTTP service with CORS support  
        let http_config = StreamableHttpServerConfig::default();
        
        let working_dir_clone = working_dir.clone();
        let config_path_clone = config_path.clone();
        
        let service: StreamableHttpService<B00tMcpServerRusty, LocalSessionManager> = 
            StreamableHttpService::new(
                move || B00tMcpServerRusty::new(&working_dir_clone, &config_path_clone).map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e)),
                Default::default(),
                http_config,
            );
        
        // Create axum router with CORS
        let app = Router::new()
            .nest_service("/mcp", service)
            .layer(CorsLayer::permissive());

        // Start HTTP server
        let listener = TcpListener::bind(addr).await?;
        eprintln!("🚀 HTTP server listening on {}", addr);
        eprintln!("📍 MCP endpoint available at: http://{}/mcp", addr);
        
        axum::serve(listener, app).await?;
    } else {
        // Show usage information
        println!("b00t-mcp v{}", env!("CARGO_PKG_VERSION"));
        println!("MCP Server for b00t-cli command proxy with ACL filtering");
        println!();
        println!("Usage:");
        println!(
            "  {} stdio                             Run as MCP server with stdio transport",
            env!("CARGO_PKG_NAME")
        );
        println!(
            "  {} --stdio                           Run as MCP server with stdio transport",
            env!("CARGO_PKG_NAME")
        );
        println!(
            "  {} --directory <DIR> stdio           Run MCP server in specific directory",
            env!("CARGO_PKG_NAME")
        );
        println!(
            "  {} --directory <DIR> --stdio         Run MCP server in specific directory",
            env!("CARGO_PKG_NAME")
        );
        println!(
            "  {} --config <FILE> stdio             Run MCP server with custom ACL config",
            env!("CARGO_PKG_NAME")
        );
        println!(
            "  {} --http --port 3000                Run MCP server with HTTP transport",
            env!("CARGO_PKG_NAME")
        );
        println!(
            "  {} --http --host 0.0.0.0 --port 8080 Run HTTP server on all interfaces",
            env!("CARGO_PKG_NAME")
        );
        println!(
            "  {} --config <FILE> --stdio           Run MCP server with custom ACL config",
            env!("CARGO_PKG_NAME")
        );
        println!();
        println!("🦀 Rusty MCP Tools:");
        println!("  Tools are compile-time generated from b00t-cli CLAP structures");
        println!("  Type-safe execution with zero runtime parsing failures");
        println!("  Available via stdio (JSON-RPC) or HTTP (RESTful + SSE)");
        println!("  Examples: b00t_mcp_list, b00t_cli_detect, b00t_whoami");
        println!();
        println!("Example usage with MCP client:");
        println!("  Configure in .mcp.json or MCP client settings");
    }

    Ok(())
}