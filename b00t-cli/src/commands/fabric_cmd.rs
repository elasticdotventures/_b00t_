//! `b00t data fabric` — CLI surface for the b00t data fabric.
//!
//! Backends (priority order):
//!   1. Irontology/Neumann  — semantic triple store (always compiled; default)
//!   2. DataFabricPipeline  — grafeo+zvec fanout (feature="data-fabric", optional FFI)
//!
//! # OODA loop usage
//! ```bash
//! # Record skill outcome after a learn cycle
//! b00t-cli data fabric upsert \
//!   --subject "ooda:goal:debug-qwen36" \
//!   --predicate "b00t:informedBy" \
//!   --object "llamacpp" \
//!   --namespace autolearn
//!
//! # Recall which skills addressed similar goals (frequency rank)
//! b00t-cli data fabric query \
//!   --predicate "b00t:informedBy" \
//!   --namespace autolearn \
//!   --format json | jq -r '.[].object' | sort | uniq -c | sort -rn | head -3
//! ```

use anyhow::{anyhow, Result};
use b00t_c0re_lib::irontology_bridge::{
    compiled_knowledge_backend, compiled_knowledge_backend_data_path, EdgeRecord, EdgeKind,
    FactRecord, IrontologyBridgeClient,
};
use clap::Parser;
use serde_json::json;

#[derive(Parser, Clone)]
pub enum FabricCommands {
    /// Query triples from the knowledge graph
    #[clap(about = "Query subject/predicate/object triples")]
    Query {
        #[clap(long)] subject: Option<String>,
        #[clap(long)] predicate: Option<String>,
        #[clap(long)] object: Option<String>,
        #[clap(long, default_value = "default")] namespace: String,
        #[clap(long, default_value_t = 50)] limit: usize,
        /// json | tsv | table
        #[clap(long, default_value = "json")] format: String,
    },

    /// Upsert a triple (subject → predicate → object)
    #[clap(about = "Store a triple in the knowledge graph")]
    Upsert {
        #[clap(long)] subject: String,
        #[clap(long)] predicate: String,
        /// Value: JSON scalar or plain string
        #[clap(long)] object: String,
        #[clap(long, default_value = "default")] namespace: String,
    },

    /// Add a directed edge between two subjects
    #[clap(about = "Add a typed edge between two nodes")]
    Edge {
        #[clap(long)] from: String,
        #[clap(long)] to: String,
        /// Edge kind: RelatedTo | DependsOn | SupersededBy | Other
        #[clap(long, default_value = "RelatedTo")] kind: String,
        #[clap(long, default_value_t = 1.0)] weight: f32,
        #[clap(long, default_value = "default")] namespace: String,
    },

    /// Show active backend, data path, and record count
    #[clap(about = "Show data fabric status for a namespace")]
    Status {
        #[clap(long, default_value = "default")] namespace: String,
    },

    /// Ingest a _b00t_ datum TOML file as triples
    #[clap(about = "Parse a datum .toml and upsert all key=value pairs as triples")]
    IngestDatum {
        path: std::path::PathBuf,
        #[clap(long)] namespace: Option<String>,
    },
}

pub fn handle_fabric_command(args: &FabricCommands) -> Result<()> {
    // block_in_place: moves this thread off the tokio worker pool temporarily,
    // allowing block_on from inside a #[tokio::main] context without panic.
    tokio::task::block_in_place(|| {
        tokio::runtime::Handle::current().block_on(fabric_async(args))
    })
}

async fn fabric_async(args: &FabricCommands) -> Result<()> {
    match args {
        FabricCommands::Query { subject, predicate, object, namespace, limit, format } => {
            let client = IrontologyBridgeClient::new(namespace)?;
            let mut facts = client.query_triples(subject.clone(), predicate.clone()).await?;

            // Post-filter by object if requested
            if let Some(obj_filter) = object {
                facts.retain(|f| f.object.to_string().contains(obj_filter.as_str()));
            }
            facts.truncate(*limit);

            match format.as_str() {
                "tsv" => {
                    println!("subject\tpredicate\tobject");
                    for f in &facts {
                        println!("{}\t{}\t{}", f.subject, f.predicate, f.object);
                    }
                }
                "table" => {
                    println!("{:<40} {:<30} {}", "subject", "predicate", "object");
                    println!("{}", "─".repeat(100));
                    for f in &facts {
                        println!("{:<40} {:<30} {}", trunc(&f.subject, 38), trunc(&f.predicate, 28), trunc(&f.object.to_string(), 40));
                    }
                    println!("\n{} records", facts.len());
                }
                _ => {
                    let out: Vec<_> = facts.iter().map(|f| json!({
                        "subject": f.subject,
                        "predicate": f.predicate,
                        "object": f.object,
                    })).collect();
                    println!("{}", serde_json::to_string_pretty(&out)?);
                }
            }
            Ok(())
        }

        FabricCommands::Upsert { subject, predicate, object, namespace } => {
            let client = IrontologyBridgeClient::new(namespace)?;
            let obj_val: serde_json::Value = serde_json::from_str(object)
                .unwrap_or(serde_json::Value::String(object.clone()));
            client.upsert_facts(vec![FactRecord {
                subject: subject.clone(),
                predicate: predicate.clone(),
                object: obj_val,
            }]).await?;
            println!("✅ {subject} → {predicate} → {object}  [ns={namespace}]");
            Ok(())
        }

        FabricCommands::Edge { from, to, kind, weight, namespace } => {
            let client = IrontologyBridgeClient::new(namespace)?;
            let edge_kind = match kind.to_lowercase().as_str() {
                "classifiedas" | "classified_as" => EdgeKind::ClassifiedAs,
                "dependson" | "depends_on"        => EdgeKind::DependsOn,
                "storedin" | "stored_in"          => EdgeKind::StoredIn,
                _                                 => EdgeKind::Related,
            };
            client.upsert_edges(vec![EdgeRecord {
                from: from.clone(),
                to: to.clone(),
                kind: edge_kind,
                weight: *weight,
            }]).await?;
            println!("✅ edge: {from} -[{kind}:{weight:.2}]→ {to}  [ns={namespace}]");
            Ok(())
        }

        FabricCommands::Status { namespace } => {
            let backend = compiled_knowledge_backend();
            let data_path = compiled_knowledge_backend_data_path(namespace).unwrap_or_default();
            println!("🗄️  Data Fabric — namespace: {namespace}");
            println!("   backend : {backend}");
            println!("   path    : {}", data_path.display());
            println!("   exists  : {}", data_path.exists());
            if let Ok(client) = IrontologyBridgeClient::new(namespace) {
                match client.query_triples(None, None).await {
                    Ok(facts) => println!("   triples : {} (scan capped at store limit)", facts.len()),
                    Err(e)    => println!("   triples : error — {e:#}"),
                }
            }
            println!("   grafeo  : ⬜ (compile b00t-c0re-lib --features data-fabric to enable)");
            Ok(())
        }

        FabricCommands::IngestDatum { path, namespace } => {
            let content = std::fs::read_to_string(path)
                .map_err(|e| anyhow!("read {}: {e}", path.display()))?;

            let stem = path.file_stem().and_then(|s| s.to_str()).unwrap_or("datum");
            let ns = namespace.as_deref()
                .unwrap_or_else(|| stem.split('.').next().unwrap_or(stem));

            let client = IrontologyBridgeClient::new(ns)?;
            let subject = format!("b00t:datum/{ns}/{stem}");

            let toml_val: toml::Value = toml::from_str(&content)
                .map_err(|e| anyhow!("TOML parse error: {e}"))?;
            let facts = flatten_toml(&subject, &toml_val, ns);
            let n = facts.len();
            client.upsert_facts(facts).await?;

            println!("✅ ingested {n} triples from {} → ns={ns}", path.display());
            Ok(())
        }
    }
}

fn trunc(s: &str, max: usize) -> String {
    if s.len() <= max { s.to_string() }
    else { format!("{}…", &s[..max.saturating_sub(1)]) }
}

fn flatten_toml(subject: &str, val: &toml::Value, ns: &str) -> Vec<FactRecord> {
    let mut out = Vec::new();
    flatten_table(subject, val, ns, "", &mut out);
    out
}

fn flatten_table(subject: &str, val: &toml::Value, ns: &str, prefix: &str, out: &mut Vec<FactRecord>) {
    let toml::Value::Table(t) = val else { return };
    for (k, v) in t {
        let path = if prefix.is_empty() { k.clone() } else { format!("{prefix}/{k}") };
        match v {
            toml::Value::Table(_) => {
                // Recurse; sub-tables become path-prefixed predicates
                flatten_table(subject, v, ns, &path, out);
            }
            toml::Value::Array(arr) => {
                // Emit one triple per scalar array element
                for item in arr {
                    if matches!(item, toml::Value::Table(_)) { continue; }
                    let predicate = format!("b00t:{ns}/{path}");
                    out.push(FactRecord { subject: subject.to_string(), predicate, object: toml_to_json(item) });
                }
            }
            _ => {
                let predicate = format!("b00t:{ns}/{path}");
                out.push(FactRecord { subject: subject.to_string(), predicate, object: toml_to_json(v) });
            }
        }
    }
}

fn toml_to_json(v: &toml::Value) -> serde_json::Value {
    match v {
        toml::Value::String(s)   => json!(s),
        toml::Value::Integer(i)  => json!(i),
        toml::Value::Float(f)    => json!(f),
        toml::Value::Boolean(b)  => json!(b),
        toml::Value::Datetime(d) => json!(d.to_string()),
        toml::Value::Array(a)    => serde_json::Value::Array(a.iter().map(toml_to_json).collect()),
        toml::Value::Table(t)    => {
            let m: serde_json::Map<_,_> = t.iter().map(|(k,v)| (k.clone(), toml_to_json(v))).collect();
            serde_json::Value::Object(m)
        }
    }
}
