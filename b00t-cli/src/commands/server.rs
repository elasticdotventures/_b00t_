// 🤓 b00t server — OpenAI-compatible router + API key authority
//    Runs b00t-mcp in HTTP+LLM mode; manages API keys via shared JSON file.
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
    #[clap(about = "Create an API key for a consumer with ontology-class access")]
    Create {
        #[clap(long, help = "Consumer identifier (e.g. rust-doc, pi, worker)")]
        consumer: String,
        #[clap(long = "access", help = "Ontology class access: b00t:EmbeddingModel:execute (repeatable, default: all)")]
        access: Vec<String>,
    },
    #[clap(about = "List all registered API keys")]
    List,
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
                KeyAction::Create { consumer, access } => {
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
                    let access_json: Vec<Value> = access.iter().map(|a| {
                        let (class, action) = a.rsplit_once(':').unwrap_or((a, "execute"));
                        serde_json::json!({"class": class, "action": action})
                    }).collect();
                    keys.insert(key.clone(), serde_json::json!({
                        "consumer": consumer,
                        "created_at": chrono::Utc::now().to_rfc3339(),
                        "access": access_json,
                    }));
                    if let Some(parent) = keys_path.parent() {
                        std::fs::create_dir_all(parent)?;
                    }
                    std::fs::write(&keys_path, serde_json::to_string_pretty(&data)?)?;
                    eprintln!("✅ Key created for consumer '{}'", consumer);
                    if !access.is_empty() {
                        eprintln!("   Access: {}", access.join(", "));
                    } else {
                        eprintln!("   Access: ALL (no restrictions)");
                    }
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
            },
    }
}
