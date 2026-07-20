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
}

fn parse_key_val(s: &str) -> Result<(String, String), String> {
    let (k, v) = s
        .split_once('=')
        .ok_or_else(|| format!("expected KEY=VALUE, got: {}", s))?;
    Ok((k.to_string(), v.to_string()))
}

pub fn handle_store_command(cmd: &StoreCommands) -> anyhow::Result<()> {
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
    }
    Ok(())
}
