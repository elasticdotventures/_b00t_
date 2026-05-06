//! Crew management subcommand — Operator-Player-Captain hierarchy.
//! Subcommands: recruit, hire, roster

use clap::Parser;

#[derive(Parser)]
pub enum CrewCommand {
    /// Search available agents by skills
    Recruit {
        #[clap(required = true, help = "Required skills (comma-separated)")]
        skills: String,
        #[clap(long, default_value_t = 3, help = "Max candidates to return")]
        limit: usize,
    },
    /// Hire an agent to the crew
    Hire {
        #[clap(help = "Agent ID to hire")]
        agent_id: String,
        #[clap(long, help = "Role: mate or player (default: player)")]
        role: Option<String>,
    },
    /// Show current roster
    Roster,
}
