//! b00t-agent: Multi-agent REPL binary
//!
//! Spawns autonomous agents that can coordinate via IPC,
//! vote on proposals, and self-organize into crews.
//!
//! Usage:
//!   b00t-agent --id alpha --skills rust,testing
//!   b00t-agent --id beta --skills docker,deploy --personality pragmatic

use anyhow::Result;
use clap::Parser;

#[path = "../k0mmand3r_repl.rs"]
mod k0mmand3r_repl;

use k0mmand3r_repl::Repl;

#[derive(Parser)]
#[clap(version, about = "b00t multi-agent REPL")]
struct Args {
    #[clap(long, help = "Agent identifier")]
    id: String,

    #[clap(
        long,
        help = "Comma-separated skills (e.g., rust,docker,testing)",
        value_delimiter = ','
    )]
    skills: Vec<String>,

    #[clap(
        long,
        help = "Agent personality (curious, pragmatic, balanced)",
        default_value = "balanced"
    )]
    personality: String,
}

#[tokio::main]
async fn main() -> Result<()> {
    let args = Args::parse();

    println!("🥾 Spawning agent: {}", args.id);
    println!("   Skills: {:?}", args.skills);
    println!("   Personality: {}\n", args.personality);

    let mut repl = Repl::new(args.id, args.skills).await?;
    repl.run().await?;

    Ok(())
}
