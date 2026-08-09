// 🤓 b00t store — knowledge store CLI (put, get, list, query, sync)
use clap::Subcommand;
use std::collections::BTreeMap;
use std::path::PathBuf;

#[derive(Debug, Subcommand)]
pub enum StoreCommands {
    #[clap(about = "Store a file in the knowledge store with ontological metadata")]
    Put {
        #[clap(help = "File to store")]
        file: PathBuf,
        #[clap(
            long,
            help = "Ontology class (b00t:TrainingCorpus, b00t:FineTunedModel, etc.)"
        )]
        class: String,
        #[clap(long, help = "Consumer identifier (agent or MCP server name)")]
        consumer: String,
        #[clap(long, help = "Key=value tags (repeatable)", value_parser = parse_key_val)]
        tag: Vec<(String, String)>,
    },
    #[clap(about = "Retrieve a stored object by key")]
    Get {
        #[clap(help = "Object key")]
        key: String,
        #[clap(long, short, help = "Output file (default: stdout bytes)")]
        output: Option<PathBuf>,
    },
    #[clap(about = "List stored objects, optionally filtered by class or consumer")]
    List {
        #[clap(long, help = "Filter by ontology class")]
        class: Option<String>,
        #[clap(long, help = "Filter by consumer")]
        consumer: Option<String>,
    },
    #[clap(about = "Query stored objects by metadata tags")]
    Query {
        #[clap(long, help = "Key=value tag (repeatable)", value_parser = parse_key_val)]
        tag: Vec<(String, String)>,
    },
    #[clap(about = "Sync local store to cloud backend (S3/R2 via credential datums)")]
    Sync {
        #[clap(long, help = "Credential provider (cloudflare-r2, aws-s3)")]
        provider: String,
    },
    #[clap(about = "Initialise the knowledge store directory + backend")]
    Init,
    #[clap(about = "Show store status (backend, object count, disk usage)")]
    Status,
    #[clap(about = "Cross-engine consistency check: Store ↔ knowledge backend ↔ blobs")]
    Validate,
    #[clap(about = "Serve store.status over a NATS request-reply subject (#716 proof-of-concept)")]
    Serve {
        #[clap(long, help = "NATS server URL (defaults to NATS_URL env or nats://localhost:4222)")]
        nats_url: Option<String>,
    },
}

/// NATS subject exposing `store::status()` as a request-reply endpoint.
///
/// 🤓 #716 proof-of-concept: only `status` is exposed for this first slice.
/// `store.get` / `store.validate` are natural follow-ups on the same pattern
/// once this subject is proven out on the hive-lan NATS deployment.
pub const STORE_STATUS_SUBJECT: &str = "b00t.store.status";

/// Build the JSON reply payload for a `store.status` request.
/// Pure/sync so it is unit-testable without a live NATS broker.
pub fn store_status_reply_payload(count: usize, bytes: u64) -> Vec<u8> {
    serde_json::json!({
        "object_count": count,
        "total_bytes": bytes,
    })
    .to_string()
    .into_bytes()
}

#[cfg(test)]
mod nats_serve_tests {
    use super::*;

    #[test]
    fn store_status_subject_is_stable() {
        // 🤓 subject naming follows the existing b00t.<domain>.<verb> convention
        // used by b00t-lib-chat (b00t.tasks.*, b00t.notify.>).
        assert_eq!(STORE_STATUS_SUBJECT, "b00t.store.status");
    }

    #[test]
    fn store_status_reply_payload_round_trips() {
        let payload = store_status_reply_payload(42, 1024);
        let parsed: serde_json::Value = serde_json::from_slice(&payload)
            .expect("store_status_reply_payload must produce valid JSON");
        assert_eq!(parsed["object_count"], 42);
        assert_eq!(parsed["total_bytes"], 1024);
    }
}

fn parse_key_val(s: &str) -> Result<(String, String), String> {
    let (k, v) = s
        .split_once('=')
        .ok_or_else(|| format!("expected KEY=VALUE, got: {}", s))?;
    Ok((k.to_string(), v.to_string()))
}

/// 🤓 #707: read the active agentic role from `_B00T_ROLE`, set by `b00t learn
///    --agent/--role`. Store/pipeline/viz/install previously never consulted
///    it, so role context set during `learn` was silently lost downstream.
///    This is the first (store status) of several call sites; see #707 for
///    the remaining pipeline/viz/install scope, deliberately deferred here.
fn active_role() -> Option<String> {
    std::env::var("_B00T_ROLE")
        .ok()
        .filter(|r| !r.is_empty())
}

pub async fn handle_store_command(cmd: &StoreCommands) -> anyhow::Result<()> {
    match cmd {
        StoreCommands::Put {
            file,
            class,
            consumer,
            tag,
        } => {
            let tags: BTreeMap<String, String> = tag.iter().cloned().collect();
            let entry = b00t_c0re_lib::store::put(file, class, consumer, &tags)?;
            println!("{}", entry.key);
        }
        StoreCommands::Get { key, output } => {
            match b00t_c0re_lib::store::get(key, output.as_deref())? {
                Some(data) => {
                    if output.is_none() {
                        eprintln!("{} bytes returned (use -o to write to file)", data.len());
                    }
                }
                None => anyhow::bail!("object not found: {}", key),
            }
        }
        StoreCommands::List { class, consumer } => {
            let entries = b00t_c0re_lib::store::list(class.as_deref(), consumer.as_deref())?;
            if entries.is_empty() {
                println!("No stored objects.");
            } else {
                println!("Stored objects:");
                for e in &entries {
                    let tags: Vec<String> =
                        e.tags.iter().map(|(k, v)| format!("{}={}", k, v)).collect();
                    println!(
                        "  {}  {}  {}  {}  {}B  [{}]",
                        e.key,
                        e.ontology_class,
                        e.consumer,
                        &e.created_at[..10],
                        e.size_bytes,
                        tags.join(", "),
                    );
                }
            }
        }
        StoreCommands::Query { tag } => {
            let tags: BTreeMap<String, String> = tag.iter().cloned().collect();
            if tags.is_empty() {
                anyhow::bail!("at least one --tag KEY=VALUE required");
            }
            let entries = b00t_c0re_lib::store::query(&tags)?;
            if entries.is_empty() {
                println!("No matching objects.");
            } else {
                for e in &entries {
                    println!("  {}  sha256:{}", e.key, &e.checksum[..12]);
                }
            }
        }
        StoreCommands::Sync { provider } => {
            b00t_c0re_lib::store::sync(provider)?;
        }
        StoreCommands::Init => {
            b00t_c0re_lib::store::init()?;
            println!("✅ Knowledge store initialised");
        }
        StoreCommands::Status => {
            let (count, bytes) = b00t_c0re_lib::store::status();
            println!("Backend: {}", b00t_c0re_lib::compiled_knowledge_backend());
            println!("Objects: {}", count);
            println!("Bytes:   {}", bytes);
            println!(
                "Role:    {}",
                active_role().unwrap_or_else(|| "(none)".to_string())
            );
        }
        StoreCommands::Validate => {
            let report = b00t_c0re_lib::store::validate_consistency()?;
            println!("Backend:          {}", report.backend);
            println!("Manifest entries: {}", report.manifest_entries);
            println!("Related facts:    {}", report.related_facts);
            println!("Hash matches:     {}", report.hash_matches);
            println!("Hash mismatches:  {}", report.hash_mismatches);
            println!("Orphan facts:     {}", report.orphan_facts);
            if report.missing_facts.is_empty() {
                println!("Missing facts:    0");
            } else {
                println!("Missing facts:    {}", report.missing_facts.len());
                for d in &report.missing_facts {
                    println!("  ⚠️  {} → {}", d.manifest_key, d.detail);
                }
            }
            if report.healthy {
                println!("\n✅ Cross-engine consistency: HEALTHY");
            } else {
                println!("\n⚠️  Cross-engine consistency: DEGRADED");
            }
        }
        StoreCommands::Serve { nats_url } => {
            serve_nats(nats_url.clone()).await?;
        }
    }
    Ok(())
}

/// #716: expose `store.status` as a NATS request-reply subject so agents on
/// other hosts can query the store without SSH/shelling into this host.
///
/// Proof-of-concept scope: one subject (`b00t.store.status`). `store.get` and
/// `store.validate` follow the identical pattern once this is proven on the
/// hive-lan NATS deployment — see issue #716.
async fn serve_nats(nats_url: Option<String>) -> anyhow::Result<()> {
    use anyhow::Context;
    use futures::StreamExt;

    // Auth (B00T_HIVE_NATS_USER/PASSWORD) and URL (NATS_URL) env fallbacks are
    // applied inside b00t_chat::ChatTransport::from_config regardless of what
    // we pass here — matches the existing `b00t chat send --transport nats` path.
    let client = b00t_chat::ChatClient::nats(nats_url, None, None)
        .context("failed to configure NATS chat client")?;
    let nats = client
        .raw_nats_client()
        .await
        .context("failed to connect to NATS")?;

    let mut subscriber = nats
        .subscribe(STORE_STATUS_SUBJECT.to_string())
        .await
        .context("failed to subscribe to store.status subject")?;

    println!("🥾 Serving store.status on NATS subject: {STORE_STATUS_SUBJECT}");
    println!("   Ctrl-C to stop.");

    loop {
        tokio::select! {
            _ = tokio::signal::ctrl_c() => {
                println!("\n🥾 store serve --nats shutting down");
                break;
            }
            msg = subscriber.next() => {
                let Some(msg) = msg else {
                    println!("⚠️  NATS subscription closed");
                    break;
                };
                let Some(reply) = msg.reply else {
                    tracing::warn!("store.status request on {} had no reply subject; ignoring", msg.subject);
                    continue;
                };
                let (count, bytes) = b00t_c0re_lib::store::status();
                let payload = store_status_reply_payload(count, bytes);
                if let Err(e) = nats.publish(reply, payload.into()).await {
                    tracing::warn!("failed to publish store.status reply: {}", e);
                }
            }
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    // Serialize env-mutating tests — process env is global state (same
    // pattern as whoami.rs::tests::ENV_MUTEX).
    static ENV_MUTEX: Mutex<()> = Mutex::new(());

    #[test]
    fn test_active_role_none_when_unset() {
        let _guard = ENV_MUTEX.lock().unwrap();
        unsafe {
            std::env::remove_var("_B00T_ROLE");
        }
        assert_eq!(active_role(), None);
    }

    #[test]
    fn test_active_role_none_when_empty() {
        let _guard = ENV_MUTEX.lock().unwrap();
        unsafe {
            std::env::set_var("_B00T_ROLE", "");
        }
        assert_eq!(active_role(), None);
        unsafe {
            std::env::remove_var("_B00T_ROLE");
        }
    }

    #[test]
    fn test_active_role_propagates_from_env() {
        let _guard = ENV_MUTEX.lock().unwrap();
        unsafe {
            std::env::set_var("_B00T_ROLE", "executive");
        }
        assert_eq!(active_role(), Some("executive".to_string()));
        unsafe {
            std::env::remove_var("_B00T_ROLE");
        }
    }
}
