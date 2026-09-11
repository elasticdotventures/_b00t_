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
use b00t_c0re_lib::irontology_bridge::{KnowledgeStoreBackend, OxigraphStore, StoreConfig};
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
    }
    Ok(())
}
