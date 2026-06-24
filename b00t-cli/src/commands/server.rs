// 🤓 b00t server — OpenAI-compatible router + API key authority
//    Runs b00t-mcp in HTTP+LLM mode; manages keys via shared JSON file and OS keyring.
use clap::Subcommand;
use serde_json::Value;

#[derive(Debug, Subcommand)]
pub enum ServerCommands {
    #[clap(about = "Start the b00t-server (b00t-mcp in HTTP+LLM proxy mode)")]
    Start {
        #[clap(long, default_value = "5273", help = "Port to listen on")]
        port: u16,
        #[clap(long, default_value = "127.0.0.1", help = "Host to bind")]
        host: String,
        #[clap(long, help = "Upstream API base URL", env = "B00T_SERVER_UPSTREAM_URL")]
        upstream_url: Option<String>,
        #[clap(long, help = "Upstream API key", env = "B00T_SERVER_UPSTREAM_KEY")]
        upstream_key: Option<String>,
    },
    #[clap(about = "Manage API keys for b00t-server consumers")]
    Key {
        #[clap(subcommand)]
        action: KeyAction,
    },
}

#[derive(Debug, Subcommand)]
pub enum KeyAction {
    #[clap(about = "Create an API key for a consumer (sub-agent, MCP server, etc.)")]
    Create {
        #[clap(long, help = "Consumer identifier (e.g. rust-doc, pi, worker)")]
        consumer: String,
    },
    #[clap(about = "List all registered API keys")]
    List,
    #[clap(about = "Store cloud credentials in encrypted catalog (OS keyring + iterable)")]
    Set {
        #[clap(long, help = "Provider name (openai, cloudflare-r2, aws-s3, etc.)")]
        provider: String,
        #[clap(long, help = "Access key / key ID")]
        key: String,
        #[clap(long, help = "Read secret from stdin instead of prompt")]
        stdin: bool,
    },
    #[clap(about = "Remove cloud credentials")]
    Unset {
        #[clap(long, help = "Provider name")]
        provider: String,
    },
    #[clap(about = "List all stored credential providers (runtime iterable)")]
    ListCredentials,
    #[clap(about = "Check if credentials exist for a provider")]
    Check {
        #[clap(long, help = "Provider name")]
        provider: String,
    },
}

const KEYS_FILE: &str = "server-keys.json";

pub fn handle_server_command(cmd: &ServerCommands) -> anyhow::Result<()> {
    match cmd {
            ServerCommands::Start { port, host, upstream_url, upstream_key } => {
                let port_str = port.to_string();
                let mut cmd = std::process::Command::new("b00t-mcp");
                cmd.args(["--http", "--llm", "--port", &port_str, "--host", host]);
                if let Some(url) = upstream_url {
                    cmd.env("B00T_SERVER_UPSTREAM_URL", url);
                }
                if let Some(key) = upstream_key {
                    cmd.env("B00T_SERVER_UPSTREAM_KEY", key);
                }
                eprintln!("🚀 Starting b00t-server on http://{}:{}", host, port);
                eprintln!("   /v1/models  /v1/chat/completions  /v1/embeddings");
                let status = cmd.status()?;
                if !status.success() {
                    anyhow::bail!("b00t-mcp exited with {}", status);
                }
                Ok(())
            }
            ServerCommands::Key { action } => match action {
                KeyAction::Create { consumer } => {
                    let key = format!("b00t-sk-{}", uuid::Uuid::new_v4().simple());
                    let keys_path = dirs::home_dir()
                        .unwrap_or_else(|| std::path::PathBuf::from("."))
                        .join(".b00t")
                        .join(KEYS_FILE);
                    let mut data: Value = serde_json::from_str(
                        &std::fs::read_to_string(&keys_path).unwrap_or_default(),
                    )
                    .unwrap_or_else(|_| serde_json::from_str(r###"{"keys":{}}"###).unwrap());
                    let keys = data["keys"]
                        .as_object_mut()
                        .ok_or_else(|| anyhow::anyhow!("invalid keys file"))?;
                    keys.insert(key.clone(), serde_json::json!({
                        "consumer": consumer,
                        "created_at": chrono::Utc::now().to_rfc3339(),
                    }));
                    if let Some(parent) = keys_path.parent() {
                        std::fs::create_dir_all(parent)?;
                    }
                    std::fs::write(&keys_path, serde_json::to_string_pretty(&data)?)?;
                    eprintln!("✅ Key created for consumer '{}'", consumer);
                    eprintln!("   Key: {}", key);
                    println!("{}", key);
                    Ok(())
                }
                KeyAction::List => {
                    let keys_path = dirs::home_dir()
                        .unwrap_or_else(|| std::path::PathBuf::from("."))
                        .join(".b00t")
                        .join(KEYS_FILE);
                    let data: Value = serde_json::from_str(
                        &std::fs::read_to_string(&keys_path).unwrap_or_default(),
                    )
                    .unwrap_or_else(|_| serde_json::from_str(r###"{"keys":{}}"###).unwrap());
                    if let Some(keys) = data.get("keys").and_then(|k| k.as_object()) {
                        if keys.is_empty() {
                            println!("No API keys registered.");
                        } else {
                            println!("API Keys:");
                            for (k, v) in keys {
                                let consumer = v.get("consumer").and_then(|c| c.as_str()).unwrap_or("?");
                                let created = v.get("created_at").and_then(|c| c.as_str()).unwrap_or("?");
                                println!("  {:.12}...  {}  ({})", k, consumer, created);
                            }
                        }
                    } else {
                        println!("No API keys registered.");
                    }
                    Ok(())
                }
                KeyAction::Set { provider, key, stdin } => {
                    let secret = if *stdin {
                        let mut s = String::new();
                        std::io::stdin().read_line(&mut s)?;
                        s.trim().to_string()
                    } else {
                        rpassword::prompt_password(&format!("Enter secret for '{}': ", provider))?
                    };
                    if key.is_empty() || secret.is_empty() {
                        anyhow::bail!("key and secret cannot be empty");
                    }
                    b00t_c0re_lib::keyring_store::set_credential(provider, key, &secret)?;
                    Ok(())
                }
                KeyAction::Unset { provider } => {
                    b00t_c0re_lib::keyring_store::delete_credential(provider)?;
                    Ok(())
                }
                KeyAction::ListCredentials => {
                    let providers = b00t_c0re_lib::keyring_store::list_credentials()?;
                    if providers.is_empty() {
                        println!("No cloud credentials stored.");
                    } else {
                        println!("Stored credentials:");
                        for p in &providers {
                            println!("  🔐 {}", p);
                        }
                    }
                    Ok(())
                }
                KeyAction::Check { provider } => {
                    match b00t_c0re_lib::keyring_store::get_credential(provider)? {
                        Some((key_id, _)) => {
                            println!("✅ Credential exists: {} (key: {}...)", provider, &key_id[..key_id.len().min(12)]);
                        }
                        None => {
                            println!("❌ No credential for '{}'", provider);
                            println!("   Set: b00t server key set --provider {} --key <ACCESS_KEY>", provider);
                        }
                    }
                    Ok(())
                }
            },
    }
}
