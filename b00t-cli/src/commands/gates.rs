use anyhow::Result;
use clap::Parser;

#[derive(Parser)]
pub enum GatesCommands {
    #[clap(
        about = "List all [[b00t.gate]] declarations across MCP datums",
        long_about = "Scan all .mcp.toml files and show their gate preconditions, both explicit ([[b00t.gate]]) and auto-derived (requires, env).\n\nExamples:\n  b00t gates list\n  b00t gates list --search github\n  b00t gates list --by-kind env\n  b00t gates list --json"
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
            GatesCommands::List { search, by_kind, json } => {
                let all = crate::list_gates(path, search.as_deref())?;
                let json = *json;

                let filtered: Vec<_> = if let Some(kind) = by_kind {
                    all.into_iter().filter(|g| g.kind == *kind).collect()
                } else {
                    all
                };

                if json {
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
                            let status = check_gate_status(g);
                            println!("    {} {}  {}  {}  {}", status, origin_icon, g.kind, g.spec, crate::ansi::dim(&format!("({})", g.origin)));
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

/// Check if a gate's precondition is currently satisfied on this system.
fn check_gate_status(g: &crate::GateReport) -> &'static str {
    match g.kind.as_str() {
        "command" => {
            if crate::check_command_available(&g.spec) { "✅" } else { "⏳" }
        }
        "env" => {
            let val = std::env::var(&g.spec);
            if val.is_ok() && !val.unwrap_or_default().is_empty() {
                "✅"
            } else {
                // check .env
                let ws = std::env::var("WORKSPACE_ROOT").or_else(|_| std::env::var("HOME")).unwrap_or_default();
                let env_path = std::path::Path::new(&ws).join(".env");
                if env_path.exists() {
                    if let Ok(content) = std::fs::read_to_string(&env_path) {
                        let prefix = format!("{}=", g.spec);
                        for line in content.lines() {
                            if line.trim().starts_with(&prefix) {
                                let val = line.trim()[prefix.len()..].trim();
                                if !val.is_empty() && !val.starts_with('#') {
                                    return "✅";
                                }
                            }
                        }
                    }
                }
                "⏳"
            }
        }
        "file" => {
            let expanded = if g.spec.starts_with('~') {
                let home = std::env::var("HOME").unwrap_or_default();
                std::path::Path::new(&home).join(g.spec.strip_prefix("~/").unwrap_or(&g.spec))
            } else {
                std::path::Path::new(&g.spec).to_path_buf()
            };
            if expanded.exists() { "✅" } else { "⏳" }
        }
        "rhai" => "❓", // can't evaluate without context
        _ => "  ",
    }
}
