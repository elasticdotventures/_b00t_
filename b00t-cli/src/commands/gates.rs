use anyhow::Result;
use clap::Parser;

#[derive(Parser)]
pub enum GatesCommands {
    #[clap(
        about = "Evaluate a proposed action through the Eisenhower/ZellijGate governance check",
        long_about = "Run an action string through ZellijGate::check (Eisenhower routing matrix).\n\nReturns Allow, Deny, or Hook(token) + audit log entry.\nSurfaces eisenhower_enabled datum flags as live evaluation.\n\nExamples:\n  b00t gates eval 'deploy to prod'\n  b00t gates eval 'run expensive finetune' --urgent --important\n  b00t gates eval 'cleanup tmp files' --json"
    )]
    Eval {
        #[clap(help = "Action string to evaluate through the governance gate")]
        action: String,
        #[clap(long, help = "Mark action as urgent (Eisenhower matrix input)")]
        urgent: bool,
        #[clap(long, help = "Mark action as important (Eisenhower matrix input)")]
        important: bool,
        #[clap(long, help = "Output as JSON")]
        json: bool,
    },
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
            GatesCommands::Eval { action, urgent, important, json: as_json } => {
                use b00t_c0re_gov::{ZellijGate, traits::{GovernanceGate, GateCheckContext}};
                use b00t_c0re_gov::types::GateResult;

                let gate = ZellijGate::new("b00t-gates-eval");
                let ctx = GateCheckContext {
                    agent_id: "b00t-cli".into(),
                    task: action.clone(),
                    action: action.clone(),
                    metadata: serde_json::json!({
                        "urgent": urgent,
                        "important": important,
                    }),
                };

                // ZellijGate::check is async; we're inside #[tokio::main] so
                // Runtime::new() would panic — use block_in_place instead.
                let result = tokio::task::block_in_place(|| {
                    tokio::runtime::Handle::current().block_on(gate.check(action, &ctx))
                });

                let (verdict, detail) = match &result {
                    GateResult::Allow => ("Allow", "action permitted".to_string()),
                    GateResult::Deny { reason, .. } => ("Deny", reason.clone()),
                    GateResult::Hook(token) => ("Hook", format!("hook_id={} type={:?}", token.id, token.hook_type)),
                };

                if *as_json {
                    println!("{}", serde_json::json!({
                        "action": action,
                        "urgent": urgent,
                        "important": important,
                        "verdict": verdict,
                        "detail": detail,
                    }));
                } else {
                    println!("{verdict}: {detail}");
                    println!("  action: {action} [urgent={urgent} important={important}]");
                }

                if matches!(result, GateResult::Deny { .. }) {
                    anyhow::bail!("Gate denied: {detail}");
                }
                Ok(())
            }
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
