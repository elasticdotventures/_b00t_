use anyhow::Result;
use clap::{Arg, Command};
use rmcp::{ServiceExt, transport::io::stdio};
use std::net::SocketAddr;
use std::path::Path;
use std::sync::Arc;

use axum::Router;
use rmcp::transport::streamable_http_server::{
    StreamableHttpServerConfig, StreamableHttpService, session::local::LocalSessionManager,
};
use tokio::net::TcpListener;
use tower_http::cors::CorsLayer;

use b00t_mcp::{
    B00tMcpServerRusty, GitHubAuthConfig, GitHubAuthState, MinimalOAuthConfig, MinimalOAuthState,
    github_auth_router, minimal_oauth_router, server_llm, server_skill,
};

/// Transport mode for the MCP server.
/// FOL-correct: explicit enumeration replaces implicit boolean triples.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[allow(dead_code)]
enum TransportMode {
    Stdio,
    Http,
    Llm, // HTTP + OpenAI-compatible /v1/ router
}

impl TransportMode {
    #[allow(dead_code)]
    fn from_matches(stdio: bool, http: bool, mode_str: Option<&String>, llm: bool) -> Self {
        if llm {
            return TransportMode::Llm;
        }
        let is_stdio = stdio || mode_str.map_or(false, |m| m == "stdio");
        let _is_http = http || mode_str.map_or(false, |m| m == "http");
        if is_stdio { TransportMode::Stdio } else { TransportMode::Http }
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    let matches = Command::new("b00t-mcp")
        .version(b00t_c0re_lib::version::VERSION)
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
            Arg::new("llm")
                .long("llm")
                .help("Also serve an OpenAI-compatible /v1/ router (b00t-server). Auto-enables --http.")
                .action(clap::ArgAction::SetTrue),
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
    let port = matches
        .get_one::<String>("port")
        .unwrap()
        .parse::<u16>()
        .expect("Invalid port number");

    let is_stdio_mode = matches.get_flag("stdio")
        || matches
            .get_one::<String>("mode")
            .map_or(false, |m| m == "stdio");
    let is_http_mode = matches.get_flag("http")
        || matches
            .get_one::<String>("mode")
            .map_or(false, |m| m == "http");
    let is_llm_mode = matches.get_flag("llm");

    // 🤓 Structured logging via tracing — enable with RUST_LOG=info or RUST_LOG=debug
    {
        use tracing_subscriber::EnvFilter;
        let filter = EnvFilter::try_from_default_env()
            .unwrap_or_else(|_| EnvFilter::new("warn"));
        let _ = tracing_subscriber::fmt()
            .with_env_filter(filter)
            .with_target(false)
            .try_init();
    }

    // 🤓 SkillExecutor — lazy MCP server lifecycle manager.
    //    Loads [b00t.mcp.lifecycle] from .mcp.toml datums, reaps idle servers.
    //    Child processes get kill_on_drop(true) — cleaned up on process exit.
    match server_skill::init_executor().await {
        Ok(n) if n > 0 => tracing::info!("SkillExecutor: {} skill(s) ready", n),
        Err(e) => tracing::warn!("SkillExecutor init failed: {} (continuing)", e),
        _ => {}
    }
    server_skill::start_reap_loop().await;

    // 🤓 Registry bridge spawn — sync official MCP registry and launch bridges
    //    for registered stdio-based servers. Bridges convert MCP notifications to NATS.
    if let Err(e) = spawn_registry_bridges_on_startup().await {
        eprintln!("⚠️  Registry bridge spawn failed: {} (continuing)", e);
    }

    if is_stdio_mode && !is_llm_mode {
        // Run as MCP server
        // eprintln!(
        //     "Starting b00t-mcp MCP server in directory: {} with config: {}",
        //     working_path.display(),
        //     config_path
        // );

        // No stderr output in stdio mode as it breaks the MCP protocol
        let server = B00tMcpServerRusty::new_flat(working_path, &config_path)?;
        let running_service = server.serve(stdio()).await?;

        // Keep the server running
        running_service.waiting().await?;
    } else if is_http_mode || is_llm_mode {
        // HTTP server mode
        let addr: SocketAddr = format!("{}:{}", host, port).parse()?;

        eprintln!("🌐 Starting HTTP MCP server on http://{}", addr);
        eprintln!(
            "🦀 Rusty MCP server with {} compile-time tools",
            B00tMcpServerRusty::new_flat(working_path, &config_path)?.tool_count()
        );

        // Create HTTP service with CORS support
        let http_config = StreamableHttpServerConfig::default();

        let working_dir_clone = working_dir.clone();
        let config_path_clone = config_path.clone();

        let service: StreamableHttpService<B00tMcpServerRusty, LocalSessionManager> =
            StreamableHttpService::new(
                move || {
                    B00tMcpServerRusty::new_flat(&working_dir_clone, &config_path_clone)
                        .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))
                },
                Default::default(),
                http_config,
            );

        // Load ACL config for development settings
        let acl_config = match b00t_mcp::acl::AclFilter::load_from_file(&config_path) {
            Ok(filter) => Some(filter.config().clone()),
            Err(_) => {
                eprintln!("⚠️  No ACL config found at {}, using defaults", config_path);
                None
            }
        };

        // Check for development mode bypass
        if let Some(ref config) = acl_config {
            if let Some(ref dev) = config.dev {
                if dev.bypass_oauth.unwrap_or(false) {
                    eprintln!("🚧 DEV MODE: OAuth bypass enabled in ACL config");
                    eprintln!(
                        "    Local user: {}",
                        dev.local_user.as_ref().unwrap_or(&"local-dev".to_string())
                    );
                }
            }
        }

        let auth_provider = server_llm::AuthProvider::from_env_or_default();

        // Log auth provider
        match auth_provider {
            server_llm::AuthProvider::Dev => eprintln!("🔓 auth: dev mode (no auth required)"),
            server_llm::AuthProvider::Basic => eprintln!("🔑 auth: basic (API keys from server-keys.json)"),
            server_llm::AuthProvider::Hydra => eprintln!("🔐 auth: hydra (OAuth 2.1 via Hydra introspection)"),
        }

        // Create GitHub auth state
        let github_config = GitHubAuthConfig::default();
        let github_state = GitHubAuthState::new(github_config);

        // Create minimal OAuth state with GitHub auth and ACL config
        let oauth_config = MinimalOAuthConfig::default();
        let oauth_state =
            MinimalOAuthState::new(oauth_config, github_state.clone()).with_acl_config(acl_config);

        // Create axum router with CORS, OAuth, and GitHub auth
        let mut app = Router::new()
            .nest_service("/mcp", service)
            .merge(minimal_oauth_router(oauth_state))
            .merge(github_auth_router(github_state));

        if is_llm_mode {
            let llm_state = Arc::new(server_llm::LlmState::new_with_auth(auth_provider));
            eprintln!("🤖 LLM proxy mode activated — upstream auto-discovered (env or local probe)");
            app = app.merge(server_llm::llm_router(llm_state.clone(), auth_provider));
        }

        let app = app.layer(CorsLayer::permissive());

        // Start HTTP server
        let listener = TcpListener::bind(addr).await?;
        eprintln!("🚀 HTTP server listening on {}", addr);
        eprintln!("📍 MCP endpoint available at: http://{}/mcp", addr);
        eprintln!("🔐 OAuth endpoints:");
        eprintln!(
            "    Discovery: http://{}/.well-known/oauth-authorization-server",
            addr
        );
        eprintln!("    Authorize: http://{}/oauth/authorize", addr);
        eprintln!("    Token: http://{}/oauth/token", addr);
        eprintln!("🐙 GitHub Auth endpoints:");
        eprintln!("    Login: http://{}/auth/github", addr);
        eprintln!("    Callback: http://{}/auth/github/callback", addr);

        axum::serve(listener, app).await?;
    } else {
        // Show usage information
        println!("b00t-mcp v{}", b00t_c0re_lib::version::VERSION);
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

/// Sync the official MCP registry and spawn bridges for stdio-based servers.
/// Bridges connect to MCP servers, read notifications, and publish to NATS.
async fn spawn_registry_bridges_on_startup() -> anyhow::Result<()> {
    use b00t_chat::{ChatClient, McpBridge, McpServerSpec};
    use b00t_c0re_lib::mcp_registry::ServerTransport;

    let mut registry = b00t_mcp::mcp_registry_tools::REGISTRY.lock().await;
    let sync_count = registry.sync_official_registry().await?;
    if sync_count > 0 {
        eprintln!("📡 Synced {} servers from official MCP registry", sync_count);
    }

    let client = ChatClient::nats(None, None, None)
        .map_err(|e| anyhow::anyhow!("Failed to create NATS client for bridge spawn: {}", e))?;
    let transport = client.transport().clone();
    let servers: Vec<_> = registry.list().into_iter().cloned().collect();

    drop(registry);

    let mut bridges = Vec::new();
    for server in &servers {
        if !matches!(server.config.transport, ServerTransport::Stdio) {
            continue;
        }
        let spec = McpServerSpec {
            id: server.id.clone(),
            label: server.name.clone(),
            command: server.config.command.clone(),
            args: server.config.args.clone(),
            env: server.config.env.clone(),
            cwd: server.config.cwd.clone(),
        };
        let mut bridge = McpBridge::new(spec);
        match bridge.start(&transport).await {
            Ok(()) => {
                eprintln!("🔗 Bridge started for {}", server.id);
                bridges.push(bridge);
            }
            Err(e) => {
                eprintln!("⚠️  Failed to start bridge for {}: {}", server.id, e);
            }
        }
    }

    eprintln!("🔗 Spawned {} MCP notification bridges", bridges.len());
    Ok(())
}
