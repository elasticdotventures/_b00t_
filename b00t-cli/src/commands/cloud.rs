use clap::Parser;
use anyhow::Result;
use b00t_c0re_lib::cloud::AbstractCloudProvider;

#[derive(Parser, Debug, Clone)]
pub enum CloudCommands {
    /// Serve as MCP server for a cloud provider
    #[clap(about = "Serve cloud provider as MCP stdio server")]
    Serve {
        /// Provider name: cloudflare
        #[arg(long, default_value = "cloudflare")]
        provider: String,
        /// Port for MCP server (0 = stdio)
        #[arg(long, default_value_t = 0)]
        port: u16,
    },
    /// List available cloud providers and their services
    #[clap(about = "List available cloud providers and services")]
    List {
        /// Provider filter (optional)
        #[arg(long)]
        provider: Option<String>,
        /// JSON output
        #[arg(long)]
        json: bool,
    },
}

pub fn handle_cloud_command(cmd: &CloudCommands) -> Result<()> {
    match cmd {
        CloudCommands::Serve { provider, port } => {
            match provider.as_str() {
                "cloudflare" => {
                    let cf = b00t_c0re_lib::cloud::CloudflareProvider::new();
                    let info = cf.account_info()?;
                    println!("🥾 cloud MCP: provider={} account={} plan={}", provider, info.account_id, info.plan);
                    if *port > 0 {
                        println!("  Serving MCP over HTTP on port {} ...", port);
                    } else {
                        println!("  Serving MCP over stdio ...");
                        // In future: emit JSON-RPC initialize response and loop
                        let init_response = serde_json::json!({
                            "jsonrpc": "2.0",
                            "result": {
                                "protocolVersion": "2024-11-05",
                                "capabilities": {
                                    "tools": {}
                                },
                                "serverInfo": {
                                    "name": "cloudflare-mcp",
                                    "version": "0.1.0"
                                }
                            },
                            "id": 0
                        });
                        println!("{}", serde_json::to_string_pretty(&init_response)?);
                    }
                }
                other => anyhow::bail!("Unknown provider: {}. Supported: cloudflare", other),
            }
            Ok(())
        }
        CloudCommands::List { provider, json } => {
            match provider.as_deref() {
                Some("cloudflare") | None => {
                    let cf = b00t_c0re_lib::cloud::CloudflareProvider::new();
                    if *json {
                        let services = cf.list_services()?;
                        println!("{}", serde_json::to_string_pretty(&services)?);
                    } else {
                        println!("🥾 Cloud Providers:");
                        println!("  cloudflare (active)");
                        println!("    Account: {}", cf.account_info()?.account_id);
                        let health = cf.env_health();
                        let detected = health.iter().filter(|v| v.detected).count();
                        let total = health.len();
                        println!("    Env vars: {}/{} detected", detected, total);
                        let services = cf.list_services()?;
                        for svc in &services {
                            let status_emoji = match svc.status.as_str() {
                                "active" => "✅",
                                "beta" => "🧪",
                                "preview" => "🔮",
                                _ => "❓",
                            };
                            println!("    {} {} ({})", status_emoji, svc.name, svc.kind);
                        }
                    }
                }
                Some(other) => println!("Unknown provider: {}", other),
            }
            Ok(())
        }
    }
}
