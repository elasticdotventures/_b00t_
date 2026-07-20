//! `b00t cake` — cake token balance, history, and ticket management.
//!
//! Cake tokens are awarded probabilistically via the hive lottery when a
//! `<|👍🏻|>` vote is confirmed by a pre-commit critical reviewer.
//!
//! Usage:
//!   b00t cake balance [agent]
//!   b00t cake history [--limit=N]
//!   b00t cake search <query>
//!   b00t cake ticket create --agent=<agent> --thumbs=up|down [--task=<id>] [--estimate=<mins>] [--actual=<mins>]
//!   b00t cake ticket resolve <ticket-id> --verdict=APPROVE|REJECT [--useful-work=<score>]

use anyhow::{Context, Result};
use clap::{Args, Subcommand};

use crate::cake_ledger::{CakeLedger, CakeTicketRequest, VoteTokenKind};

// ---------------------------------------------------------------------------
// Clap structures
// ---------------------------------------------------------------------------

#[derive(Debug, Args)]
pub struct CakeArgs {
    #[command(subcommand)]
    pub cmd: CakeCommands,
}

#[derive(Debug, Subcommand)]
pub enum CakeCommands {
    /// Show cake balance for an agent (default: operator)
    Balance {
        #[arg(default_value = "operator")]
        agent: String,
    },
    /// Show ticket history
    History {
        #[arg(default_value = "operator")]
        agent: String,
        #[arg(long, default_value_t = 20)]
        limit: usize,
        /// Only show winning tickets
        #[arg(long)]
        won: bool,
    },
    /// Search ticket history
    Search {
        query: String,
        #[arg(long, default_value_t = 20)]
        limit: usize,
    },
    /// Ticket management
    Ticket {
        #[command(subcommand)]
        cmd: TicketCommands,
    },
}

#[derive(Debug, Subcommand)]
pub enum TicketCommands {
    /// Create a pending cake ticket from a vote token
    Create {
        #[arg(long, default_value = "operator")]
        agent: String,
        /// Vote: up (<|👍🏻|>) or down (<|👎🏻|>)
        #[arg(long)]
        thumbs: String,
        #[arg(long)]
        task: Option<String>,
        #[arg(long)]
        git_ref: Option<String>,
        #[arg(long)]
        estimate: Option<i64>,
        #[arg(long)]
        actual: Option<i64>,
        #[arg(long)]
        justification: Option<String>,
    },
    /// Resolve a pending ticket — runs lottery on APPROVE
    Resolve {
        ticket_id: String,
        #[arg(long, default_value = "APPROVE")]
        verdict: String,
        #[arg(long)]
        reviewer_output: Option<String>,
        /// Useful-work score 0.5–2.0 (default 1.0)
        #[arg(long, default_value_t = 1.0)]
        useful_work: f64,
    },
}

// ---------------------------------------------------------------------------
// Handler
// ---------------------------------------------------------------------------

pub fn handle_cake_command(args: &CakeArgs) -> Result<()> {
    let ledger = CakeLedger::open().context("open cake ledger")?;

    match &args.cmd {
        CakeCommands::Balance { agent } => {
            let bal = ledger.balance(agent)?;
            println!("🍰 {agent}: <|🍰:{bal}|>");
        }

        CakeCommands::History { agent, limit, won } => {
            let tickets = ledger.history(agent, *limit)?;
            let filtered: Vec<_> = tickets
                .iter()
                .filter(|t| !won || t.amount.unwrap_or(0) > 0)
                .collect();

            if filtered.is_empty() {
                println!("No cake history for '{agent}'.");
                return Ok(());
            }
            println!("🍰 Cake history for '{agent}' ({})", filtered.len());
            println!(
                "{:<24} {:<8} {:<10} {:<8} {}",
                "ID", "thumbs", "verdict", "amount", "created"
            );
            println!("{}", "-".repeat(72));
            for t in filtered {
                let verdict = t.reviewer_verdict.as_deref().unwrap_or("pending");
                let amount = t
                    .amount
                    .map(|a| a.to_string())
                    .unwrap_or_else(|| "?".into());
                let short_id = &t.id[..t.id.len().min(22)];
                println!(
                    "{short_id:<24} {:<8} {verdict:<10} {amount:<8} {}",
                    t.thumbs, t.created_at
                );
            }
        }

        CakeCommands::Search { query, limit } => {
            let tickets = ledger.search(query, *limit)?;
            if tickets.is_empty() {
                println!("No tickets match '{query}'.");
                return Ok(());
            }
            println!("🔍 {} ticket(s) matching '{query}'", tickets.len());
            for t in &tickets {
                let verdict = t.reviewer_verdict.as_deref().unwrap_or("pending");
                let amount = t.amount.unwrap_or(0);
                println!(
                    "  {} | {} | {} | {} | <|🍰:{amount}|>",
                    t.id, t.agent, t.thumbs, verdict
                );
                if let Some(ref j) = t.justification {
                    println!("    justification: {j}");
                }
            }
        }

        CakeCommands::Ticket { cmd } => match cmd {
            TicketCommands::Create {
                agent,
                thumbs,
                task,
                git_ref,
                estimate,
                actual,
                justification,
            } => {
                let kind = match thumbs.as_str() {
                    "up" | "👍" | "👍🏻" | "<|👍🏻|>" => VoteTokenKind::ThumbsUp,
                    "down" | "👎" | "👎🏻" | "<|👎🏻|>" => VoteTokenKind::ThumbsDown,
                    other => anyhow::bail!("Unknown thumbs value '{other}'. Use: up | down"),
                };
                let req = CakeTicketRequest {
                    agent: agent.clone(),
                    thumbs: kind,
                    task_id: task.clone(),
                    git_ref: git_ref.clone(),
                    estimate_mins: *estimate,
                    actual_mins: *actual,
                    justification: justification.clone(),
                };
                let id = ledger.create_ticket(req)?;
                println!("🎟️  Ticket created: {id}");
                println!(
                    "   Resolve after pre-commit review: b00t cake ticket resolve {id} --verdict=APPROVE"
                );
            }

            TicketCommands::Resolve {
                ticket_id,
                verdict,
                reviewer_output,
                useful_work,
            } => {
                let output = reviewer_output.as_deref().unwrap_or("");
                let outcome = ledger.resolve_ticket(ticket_id, verdict, output, *useful_work)?;
                if outcome.won {
                    println!(
                        "🎉 Lottery WON! <|🍰:{}|> awarded (p={:.2}, roll={:.3})",
                        outcome.amount, outcome.p_cake, outcome.luck_roll
                    );
                    let bal = ledger.balance("operator")?;
                    println!("   New balance: <|🍰:{bal}|>");
                } else if verdict == "APPROVE" {
                    println!(
                        "😢 Lottery lost. (p={:.2}, roll={:.3}) — better luck next time.",
                        outcome.p_cake, outcome.luck_roll
                    );
                } else {
                    println!("❌ Ticket voided — verdict: {verdict}. No cake.");
                }
            }
        },
    }
    Ok(())
}
