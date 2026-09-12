//! SP5-07 — offline driver for the `store-oxigraph` graph substrate.
//!
//! Build:
//!   cargo run -p b00t-c0re-lib --no-default-features --features store-oxigraph \
//!     --example b00t-graph -- <cmd>
//!
//! Input: a JSONL file of `[subject, predicate, object]` arrays, produced by
//!        `b00t graph emit-triples`.

use anyhow::{ensure, Context, Result};
use b00t_c0re_lib::graph_kerml::{graph_to_kerml, GraphView};
use b00t_c0re_lib::graph_load::load_graph;
use b00t_c0re_lib::graph_shapes::{validate_graph, Severity};
use b00t_c0re_lib::irontology_bridge::{
    KnowledgeStoreBackend, OxigraphStore, StoreConfig, compiled_knowledge_backend_data_path,
};
use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(name = "b00t-graph")]
struct Cli {
    #[command(subcommand)]
    cmd: Cmd,
}

#[derive(Subcommand)]
enum Cmd {
    /// Load a triples JSONL into a fresh store; print the quad count.
    Load { triples: String },
    /// Run a raw SPARQL SELECT/ASK against the loaded triples.
    Query {
        triples: String,
        sparql: String,
        #[arg(long)]
        json: bool,
    },
    /// Run the SHACL-lite shape suite. Exit 1 on any Violation with --strict.
    Validate {
        triples: String,
        #[arg(long)]
        strict: bool,
    },
    /// Emit a KerML view of the graph.
    Kerml {
        triples: String,
        #[arg(long, value_parser = ["datum-deps", "r0le-grants", "service-budgets"])]
        view: String,
    },
    /// Dump the loaded graph as Turtle RDF (for SPARQL indexing/discovery —
    /// a separate artifact from `kerml`'s concrete-syntax text view, not a
    /// renamed version of it).
    Turtle { triples: String },
    /// Subscribe to the best-effort `b00t.graph.reindex` hint (published by
    /// `b00t graph publish`) and rebuild a persistent local store on each
    /// event. See SP5 continuation: docs/superpowers/specs/
    /// 2026-09-11-sp5-graph-artifact-publish-design.md. No signature
    /// verification (matches the same v1 decision made for kr0ki#13 — S3
    /// access itself is the trust boundary for now, not covered here).
    Watch {
        #[arg(long, env = "NATS_URL")]
        nats_url: String,
        #[arg(long, env = "GRAPH_ARTIFACT_S3_BUCKET")]
        bucket: String,
        #[arg(long, default_value = "ap-southeast-4")]
        region: String,
        #[arg(long, default_value = "spire-agent")]
        profile: String,
    },
}

fn read_triples(path: &str) -> Result<Vec<(String, String, String)>> {
    let body = std::fs::read_to_string(path).with_context(|| format!("read {path}"))?;
    let mut out = Vec::new();
    for (i, line) in body.lines().enumerate() {
        if line.trim().is_empty() {
            continue;
        }
        let v: Vec<String> = serde_json::from_str(line)
            .with_context(|| format!("{path}:{} not a JSON [s,p,o] array", i + 1))?;
        ensure!(v.len() == 3, "{path}:{} expected 3 elements", i + 1);
        out.push((v[0].clone(), v[1].clone(), v[2].clone()));
    }
    Ok(out)
}

fn fresh_store() -> Result<OxigraphStore> {
    let dir = std::env::temp_dir().join(format!(
        "b00t-graph-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    let _ = std::fs::remove_dir_all(&dir);
    OxigraphStore::try_new(StoreConfig {
        endpoint: String::new(),
        namespace: "cli".into(),
        data_path: Some(dir),
    })
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    match cli.cmd {
        Cmd::Load { triples } => {
            let store = fresh_store()?;
            let n = load_graph(&store, &read_triples(&triples)?)?;
            println!(
                "loaded {n} triples ({} quads)",
                store.store().len().unwrap_or(0)
            );
        }
        Cmd::Query {
            triples,
            sparql,
            json,
        } => {
            let store = fresh_store()?;
            load_graph(&store, &read_triples(&triples)?)?;
            match store.store().query(&sparql)? {
                oxigraph::sparql::QueryResults::Boolean(b) => println!("{b}"),
                oxigraph::sparql::QueryResults::Solutions(sols) => {
                    for sol in sols {
                        let sol = sol?;
                        if json {
                            let map: std::collections::BTreeMap<String, String> = sol
                                .iter()
                                .map(|(v, t)| (v.as_str().to_string(), t.to_string()))
                                .collect();
                            println!("{}", serde_json::to_string(&map)?);
                        } else {
                            let row: Vec<String> = sol
                                .iter()
                                .map(|(v, t)| format!("{}={}", v.as_str(), t))
                                .collect();
                            println!("{}", row.join("\t"));
                        }
                    }
                }
                _ => {}
            }
        }
        Cmd::Validate { triples, strict } => {
            let store = fresh_store()?;
            load_graph(&store, &read_triples(&triples)?)?;
            let violations = validate_graph(&store)?;
            let mut hard = 0;
            for v in &violations {
                let tag = match v.severity {
                    Severity::Violation => {
                        hard += 1;
                        "VIOLATION"
                    }
                    Severity::Warning => "warning",
                };
                println!("[{tag}] {} {} — {}", v.shape, v.focus_node, v.message);
            }
            if violations.is_empty() {
                println!("all shapes pass");
            }
            if strict && hard > 0 {
                std::process::exit(1);
            }
        }
        Cmd::Kerml { triples, view } => {
            let store = fresh_store()?;
            load_graph(&store, &read_triples(&triples)?)?;
            let gv = match view.as_str() {
                "datum-deps" => GraphView::DatumDeps,
                "r0le-grants" => GraphView::R0leGrants,
                "service-budgets" => GraphView::ServiceBudgets,
                _ => unreachable!(),
            };
            print!("{}", graph_to_kerml(&store, gv)?);
        }
        Cmd::Turtle { triples } => {
            let store = fresh_store()?;
            load_graph(&store, &read_triples(&triples)?)?;
            let bytes = store.store().dump_graph_to_writer(
                oxigraph::model::GraphNameRef::DefaultGraph,
                oxigraph::io::RdfFormat::Turtle,
                Vec::new(),
            )?;
            std::io::Write::write_all(&mut std::io::stdout(), &bytes)?;
        }
        Cmd::Watch {
            nats_url,
            bucket,
            region,
            profile,
        } => {
            tokio::runtime::Runtime::new()
                .context("build tokio runtime")?
                .block_on(watch(&nats_url, &bucket, &region, &profile))?;
        }
    }
    Ok(())
}

/// Pulls `key` from `bucket` via the `aws` CLI + a named profile (mirrors
/// `b00t-cli/src/commands/finetune_job.rs`'s `push_to_s3` pattern for the
/// opposite direction — not imported directly, since b00t-c0re-lib cannot
/// depend on b00t-cli, which depends on it).
/// Pure argv builder for `aws s3 cp` (pull direction) — unit-testable
/// without the `aws` CLI installed. Mirrors
/// `b00t-cli/src/commands/finetune_job.rs`'s `aws_s3_cp_args` convention.
fn aws_s3_cp_pull_args(
    bucket: &str,
    key: &str,
    local_path: &std::path::Path,
    profile: &str,
    region: &str,
) -> Vec<String> {
    vec![
        "s3".to_string(),
        "cp".to_string(),
        format!("s3://{bucket}/{key}"),
        local_path.display().to_string(),
        "--profile".to_string(),
        profile.to_string(),
        "--region".to_string(),
        region.to_string(),
    ]
}

fn pull_from_s3(bucket: &str, key: &str, local_path: &std::path::Path, profile: &str, region: &str) -> Result<()> {
    let args = aws_s3_cp_pull_args(bucket, key, local_path, profile, region);
    let out = std::process::Command::new("aws")
        .args(&args)
        .output()
        .context(
            "aws CLI not found — install awscli and configure the 'spire-agent' profile (credential_process wrapper; see infrastructure repo issues #179/#181)",
        )?;
    if !out.status.success() {
        anyhow::bail!("aws s3 cp failed: {}", String::from_utf8_lossy(&out.stderr));
    }
    Ok(())
}

/// The S3 key a tagged artifact's Turtle file lives at. Pure, testable.
fn s3_key_for_tag(tag: &str) -> String {
    format!("graph/tags/{tag}/kerml-view.ttl")
}

/// Extracts the `tag` field from a reindex-event JSON payload. Pure,
/// testable without any I/O — the actual fetch/store side effects live in
/// `handle_reindex_event` below.
fn parse_reindex_tag(payload: &[u8]) -> Result<String> {
    let event: serde_json::Value =
        serde_json::from_slice(payload).context("parse reindex event JSON")?;
    Ok(event["tag"]
        .as_str()
        .context("event missing 'tag'")?
        .to_string())
}

/// One reindex cycle: fetch the tagged Turtle artifact, replace (not
/// accumulate into) the persistent local store's contents.
fn handle_reindex_event(payload: &[u8], bucket: &str, region: &str, profile: &str) -> Result<()> {
    let tag = parse_reindex_tag(payload)?;

    let tmp_dir = std::env::temp_dir().join(format!("graph-reindex-{tag}"));
    std::fs::create_dir_all(&tmp_dir).context("create scratch dir")?;
    let turtle_path = tmp_dir.join("kerml-view.ttl");
    pull_from_s3(
        bucket,
        &s3_key_for_tag(&tag),
        &turtle_path,
        profile,
        region,
    )?;
    let turtle_bytes = std::fs::read(&turtle_path).context("read fetched turtle file")?;

    let data_path = compiled_knowledge_backend_data_path("graph-reindex")
        .context("resolve persistent store path")?;
    let store = OxigraphStore::try_new(StoreConfig {
        endpoint: String::new(),
        namespace: "graph-reindex".into(),
        data_path: Some(data_path),
    })?;
    // Replace, not accumulate — a reindex is the new authoritative state,
    // not a merge with whatever was there before.
    store.store().clear().context("clear existing store contents")?;
    store
        .store()
        .load_from_slice(oxigraph::io::RdfFormat::Turtle, &turtle_bytes)
        .context("load turtle into persistent store")?;
    eprintln!(
        "reindexed tag={tag} ({} bytes loaded into persistent store)",
        turtle_bytes.len()
    );
    Ok(())
}

async fn watch(nats_url: &str, bucket: &str, region: &str, profile: &str) -> Result<()> {
    let client = async_nats::connect(nats_url)
        .await
        .with_context(|| format!("connect to NATS at {nats_url}"))?;
    let mut sub = client
        .subscribe("b00t.graph.reindex")
        .await
        .context("subscribe b00t.graph.reindex")?;
    eprintln!("watching b00t.graph.reindex on {nats_url} ...");
    while let Some(msg) = futures::StreamExt::next(&mut sub).await {
        if let Err(e) = handle_reindex_event(&msg.payload, bucket, region, profile) {
            eprintln!("reindex handling failed (non-fatal, will retry on next event): {e:#}");
        }
    }
    Ok(())
}

#[cfg(test)]
mod watch_tests {
    use super::*;

    #[test]
    fn s3_key_for_tag_matches_publish_side_layout() {
        // Must match graph.rs's own kerml_key format!() exactly — this is
        // the contract between `b00t graph publish` and `b00t-graph watch`.
        assert_eq!(s3_key_for_tag("v1.2.0"), "graph/tags/v1.2.0/kerml-view.ttl");
    }

    #[test]
    fn aws_s3_cp_pull_args_builds_expected_argv() {
        let args = aws_s3_cp_pull_args(
            "my-bucket",
            "graph/tags/v1.2.0/kerml-view.ttl",
            std::path::Path::new("/tmp/out.ttl"),
            "spire-agent",
            "ap-southeast-4",
        );
        assert_eq!(
            args,
            vec![
                "s3",
                "cp",
                "s3://my-bucket/graph/tags/v1.2.0/kerml-view.ttl",
                "/tmp/out.ttl",
                "--profile",
                "spire-agent",
                "--region",
                "ap-southeast-4",
            ]
        );
    }

    #[test]
    fn parse_reindex_tag_extracts_tag_field() {
        let payload = br#"{"tag":"v1.2.0","commit_sha":"deadbeef","content_hash":"abc","s3_prefix":"s3://x/y/"}"#;
        assert_eq!(parse_reindex_tag(payload).unwrap(), "v1.2.0");
    }

    #[test]
    fn parse_reindex_tag_errors_on_missing_tag() {
        let payload = br#"{"commit_sha":"deadbeef"}"#;
        assert!(parse_reindex_tag(payload).is_err());
    }

    #[test]
    fn parse_reindex_tag_errors_on_invalid_json() {
        let payload = b"not json";
        assert!(parse_reindex_tag(payload).is_err());
    }
}
