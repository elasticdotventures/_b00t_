use anyhow::Result;
use clap::Parser;

#[derive(Parser)]
pub enum GatesCommands {
    #[clap(
        about = "List all [[b00t.gate]] declarations across MCP datums",
        long_about = "Scan all .mcp.toml/.mcp.tomllm/.mcp.tomllmd files and show their gate preconditions, both explicit ([[b00t.gate]]) and auto-derived (requires, env).\n\nExamples:\n  b00t gates list\n  b00t gates list --search github\n  b00t gates list --by-kind env\n  b00t gates list --json"
    )]
    List {
        #[clap(long, help = "Filter by datum name or hint (case-insensitive)")]
        search: Option<String>,
        #[clap(long, help = "Filter by gate kind: command, env, file, rhai")]
        by_kind: Option<String>,
        #[clap(long, help = "Output in JSON format")]
        json: bool,
    },
}

impl GatesCommands {
    pub fn execute(&self, path: &str) -> Result<()> {
        match self {
            GatesCommands::List { search, by_kind, json: as_json } => {
                let all = crate::list_gates(path, search.as_deref())?;

                let filtered: Vec<_> = if let Some(kind) = by_kind {
                    all.into_iter().filter(|g| g.kind == *kind).collect()
                } else {
                    all
                };

                if *as_json {
                    println!("{}", serde_json::to_string_pretty(&filtered)?);
                } else if filtered.is_empty() {
                    println!("No gates found.");
                } else {
                    // Group by datum
                    use std::collections::BTreeMap;
                    let mut by_datum: BTreeMap<String, Vec<&crate::GateReport>> = BTreeMap::new();
                    for g in &filtered {
                        by_datum.entry(g.datum.clone()).or_default().push(g);
                    }

                    let total = filtered.len();
                    let datum_count = by_datum.len();
                    let kind_counts: BTreeMap<&str, usize> = {
                        let mut m: BTreeMap<&str, usize> = BTreeMap::new();
                        for g in &filtered {
                            *m.entry(&g.kind).or_insert(0) += 1;
                        }
                        m
                    };

                    println!("🔍 {} gates across {} datums", total, datum_count);
                    if let Some(k) = by_kind {
                        println!("   (filtered by kind: {})", k);
                    }
                    print!("   by kind:");
                    for (k, c) in &kind_counts {
                        print!("  {}={}", k, c);
                    }
                    println!();
                    println!();

                    for (datum, gates) in &by_datum {
                        println!("  {} — {} gates:", crate::ansi::bold(datum), gates.len());
                        for g in gates {
                            let origin_icon = match g.origin.as_str() {
                                "explicit" => "🔧",
                                "auto:requires" => "⚡",
                                "auto:env" => "📦",
                                _ => "  ",
                            };
                            let status_icon = match g.status {
                                "pass" => "✅",
                                "fail" => "⏳",
                                _ => "❓",
                            };
                            println!("    {} {}  {}  {}  {}", status_icon, origin_icon, g.kind, g.spec, crate::ansi::dim(&format!("({})", g.origin)));
                            if let Some(hint) = &g.hint {
                                println!("           {}", crate::ansi::yellow(&format!("↳ {hint}")));
                            }
                        }
                        println!();
                    }
                }

                Ok(())
            }
        }
    }
}
