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
    #[clap(about = "Query per-consumer usage telemetry (Spotlight)")]
    Spotlight {
        #[clap(subcommand)]
        action: SpotlightAction,
    },
}

#[derive(Debug, Subcommand)]
pub enum SpotlightAction {
    #[clap(about = "Aggregate usage by consumer/model/endpoint")]
    Query {
        #[clap(long, help = "Filter to a single consumer (e.g. rust-doc)")]
        consumer: Option<String>,
        #[clap(long, help = "Only count events in the last N (e.g. 24h, 7d, 30m)")]
        since: Option<String>,
    },
}

#[derive(Debug, Subcommand)]
pub enum KeyAction {
    #[clap(about = "Create an API key for a consumer with ontology-class access")]
    Create {
        #[clap(long, help = "Consumer identifier (e.g. rust-doc, pi, worker)")]
        consumer: String,
        #[clap(
            long = "access",
            help = "Ontology class access: b00t:EmbeddingModel:execute (repeatable, default: all)"
        )]
        access: Vec<String>,
    },
    #[clap(about = "List all registered API keys")]
    List,
}

const KEYS_FILE: &str = "server-keys.json";

/// Locked, read-merge-write, atomic-rename insert into the shared keys file —
/// mirrors `b00t-mcp/src/server_llm.rs`'s `write_keys_file_locked` (#1128).
/// Re-reads the file's CURRENT on-disk state after acquiring the lock, so a
/// key a concurrently-running `b00t-mcp --http --llm` server just persisted
/// via `LlmState::save_keys_to_file` isn't lost to this process's own write.
fn create_key_locked(keys_path: &std::path::Path, key: &str, entry: Value) -> anyhow::Result<()> {
    use fs2::FileExt;

    if let Some(parent) = keys_path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let lock_file = std::fs::OpenOptions::new().create(true).write(true).open(keys_path)?;
    lock_file.lock_exclusive()?;

    let mut data: Value =
        serde_json::from_str(&std::fs::read_to_string(keys_path).unwrap_or_default())
            .unwrap_or_else(|_| serde_json::from_str(r###"{"keys":{}}"###).unwrap());
    data["keys"]
        .as_object_mut()
        .ok_or_else(|| anyhow::anyhow!("invalid keys file"))?
        .insert(key.to_string(), entry);

    let tmp_path = keys_path.with_extension("json.tmp");
    std::fs::write(&tmp_path, serde_json::to_string_pretty(&data)?)?;
    std::fs::rename(&tmp_path, keys_path)?;

    // Lock releases when lock_file drops at function end.
    Ok(())
}

pub fn handle_server_command(cmd: &ServerCommands) -> anyhow::Result<()> {
    match cmd {
        ServerCommands::Start {
            port,
            host,
            upstream_url,
            upstream_key,
        } => {
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
                let access_json: Vec<Value> = access
                    .iter()
                    .map(|a| {
                        let (class, action) = a.rsplit_once(':').unwrap_or((a, "execute"));
                        serde_json::json!({"class": class, "action": action})
                    })
                    .collect();
                let new_entry = serde_json::json!({
                    "consumer": consumer,
                    "created_at": chrono::Utc::now().to_rfc3339(),
                    "access": access_json,
                });
                // Locked read-merge-write + atomic rename — a `b00t-mcp
                // --http --llm` server process may be writing this same file
                // (LlmState::save_keys_to_file) around the same time; without
                // the lock, whichever of the two writes last would silently
                // drop the other's key. See #1128.
                create_key_locked(&keys_path, &key, new_entry)?;
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
                let data: Value =
                    serde_json::from_str(&std::fs::read_to_string(&keys_path).unwrap_or_default())
                        .unwrap_or_else(|_| serde_json::from_str(r###"{"keys":{}}"###).unwrap());
                if let Some(keys) = data.get("keys").and_then(|k| k.as_object()) {
                    if keys.is_empty() {
                        println!("No API keys registered.");
                    } else {
                        println!("API Keys:");
                        for (k, v) in keys {
                            let consumer =
                                v.get("consumer").and_then(|c| c.as_str()).unwrap_or("?");
                            let created =
                                v.get("created_at").and_then(|c| c.as_str()).unwrap_or("?");
                            println!("  {:.12}...  {}  ({})", k, consumer, created);
                        }
                    }
                } else {
                    println!("No API keys registered.");
                }
                Ok(())
            }
        },
        ServerCommands::Spotlight { action } => match action {
            SpotlightAction::Query { consumer, since } => {
                spotlight_query(consumer.as_deref(), since.as_deref())
            }
        },
    }
}

/// Parses a simple duration suffix (m/h/d — minutes/hours/days), e.g. "24h",
/// "7d", "30m". No external duration-parsing dependency needed for this scope.
fn parse_since(since: &str) -> anyhow::Result<chrono::Duration> {
    let since = since.trim();
    let (num_str, unit) = since.split_at(since.len().saturating_sub(1));
    let n: i64 = num_str
        .parse()
        .map_err(|_| anyhow::anyhow!("invalid --since '{}': expected e.g. 24h, 7d, 30m", since))?;
    match unit {
        "m" => Ok(chrono::Duration::minutes(n)),
        "h" => Ok(chrono::Duration::hours(n)),
        "d" => Ok(chrono::Duration::days(n)),
        _ => anyhow::bail!("invalid --since '{}': unit must be m, h, or d", since),
    }
}

fn spotlight_query(consumer_filter: Option<&str>, since: Option<&str>) -> anyhow::Result<()> {
    let log_path = dirs::home_dir()
        .unwrap_or_else(|| std::path::PathBuf::from("."))
        .join(".b00t")
        .join("spotlight.jsonl");
    let content = std::fs::read_to_string(&log_path).unwrap_or_default();

    let cutoff = since
        .map(parse_since)
        .transpose()?
        .map(|d| chrono::Utc::now() - d);

    #[derive(Default)]
    struct Agg {
        requests: u64,
        prompt_tokens: u64,
        completion_tokens: u64,
    }
    let mut by_consumer_model: std::collections::BTreeMap<(String, String), Agg> =
        std::collections::BTreeMap::new();

    for line in content.lines() {
        let Ok(event) = serde_json::from_str::<Value>(line) else { continue };
        let consumer = event.get("consumer").and_then(|c| c.as_str()).unwrap_or("unknown");
        if let Some(filter) = consumer_filter {
            if consumer != filter {
                continue;
            }
        }
        if let Some(cutoff) = cutoff {
            let ts_ok = event
                .get("ts")
                .and_then(|t| t.as_str())
                .and_then(|t| chrono::DateTime::parse_from_rfc3339(t).ok())
                .map(|t| t.with_timezone(&chrono::Utc) >= cutoff)
                .unwrap_or(false);
            if !ts_ok {
                continue;
            }
        }
        let model = event.get("model").and_then(|m| m.as_str()).unwrap_or("unknown");
        let entry = by_consumer_model
            .entry((consumer.to_string(), model.to_string()))
            .or_default();
        entry.requests += 1;
        entry.prompt_tokens += event.get("prompt_tokens").and_then(|v| v.as_u64()).unwrap_or(0);
        entry.completion_tokens += event.get("completion_tokens").and_then(|v| v.as_u64()).unwrap_or(0);
    }

    if by_consumer_model.is_empty() {
        println!("No spotlight events found (log: {})", log_path.display());
        return Ok(());
    }

    println!("{:<20} {:<20} {:>10} {:>15} {:>15}", "consumer", "model", "requests", "prompt_tok", "completion_tok");
    for ((consumer, model), agg) in &by_consumer_model {
        println!(
            "{:<20} {:<20} {:>10} {:>15} {:>15}",
            consumer, model, agg.requests, agg.prompt_tokens, agg.completion_tokens
        );
    }
    Ok(())
}
