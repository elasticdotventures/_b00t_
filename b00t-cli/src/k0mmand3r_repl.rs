//! k0mmand3r REPL: Interactive agent coordination interface
//!
//! Provides slash command processing for multi-agent crews:
//! - /handshake - Initiate connections
//! - /vote - Participate in decisions
//! - /crew - Manage team formation
//! - /delegate - Transfer authority
//! - /status - Query agent state

use anyhow::{Context, Result};
use b00t_ipc::{Agent, Message, MessageBus, VoteChoice};
use std::io::{self, Write};
use std::sync::Arc;
use uuid::Uuid;

pub struct Repl {
    agent: Agent,
    bus: Arc<MessageBus>,
}

impl Repl {
    pub async fn new(agent_id: String, skills: Vec<String>) -> Result<Self> {
        let agent = Agent::new(agent_id, skills);
        let bus = Arc::new(MessageBus::new().await?);
        bus.register(agent.clone()).await?;

        Ok(Self { agent, bus })
    }

    pub async fn run(&mut self) -> Result<()> {
        println!("🥾 b00t k0mmand3r REPL");
        println!("Agent: {} | Skills: {:?}", self.agent.id, self.agent.skills);
        println!("Type /help for commands, /quit to exit\n");

        loop {
            print!("{}> ", self.agent.id);
            io::stdout().flush()?;

            let mut input = String::new();
            io::stdin().read_line(&mut input)?;
            let input = input.trim();

            if input.is_empty() {
                continue;
            }

            match self.handle_command(input).await {
                Ok(should_continue) => {
                    if !should_continue {
                        break;
                    }
                }
                Err(e) => {
                    eprintln!("❌ Error: {}", e);
                }
            }
        }

        Ok(())
    }

    /// Returns Ok(true) to continue, Ok(false) to quit
    async fn handle_command(&mut self, input: &str) -> Result<bool> {
        if !input.starts_with('/') {
            // Non-command input, broadcast to crew
            self.bus
                .send(Message::Broadcast {
                    from: self.agent.id.clone(),
                    content: input.to_string(),
                })
                .await?;
            return Ok(true);
        }

        let parts: Vec<&str> = input.split_whitespace().collect();
        let cmd = parts[0];

        match cmd {
            "/help" => {
                self.print_help();
            }
            "/quit" | "/exit" => {
                println!("👋 Goodbye!");
                return Ok(false);
            }
            "/handshake" => {
                self.cmd_handshake(&parts[1..]).await?;
            }
            "/vote" => {
                self.cmd_vote(&parts[1..]).await?;
            }
            "/crew" => {
                self.cmd_crew(&parts[1..]).await?;
            }
            "/delegate" => {
                self.cmd_delegate(&parts[1..]).await?;
            }
            "/negotiate" => {
                self.cmd_negotiate(&parts[1..]).await?;
            }
            "/status" => {
                self.cmd_status().await?;
            }
            "/propose" => {
                self.cmd_propose(&parts[1..]).await?;
            }
            "/ahoy" => {
                self.cmd_ahoy(&parts[1..]).await?;
            }
            "/apply" => {
                self.cmd_apply(&parts[1..]).await?;
            }
            "/award" => {
                self.cmd_award(&parts[1..]).await?;
            }
            _ => {
                println!("❓ Unknown command: {}", cmd);
                println!("Type /help for available commands");
            }
        }

        Ok(true)
    }

    fn print_help(&self) {
        println!(
            r#"
🥾 k0mmand3r Commands:

  /handshake <agent> [proposal]  - Initiate connection with another agent
  /vote <proposal_id> <yes|no|abstain> [reason] - Cast vote on proposal
  /crew <form|join|leave> [members...] - Manage crew formation
  /delegate <agent> <budget> - Transfer crown 👑 and cake 🍰 budget
  /negotiate <resource> <amount> <reason> - Request resource allocation
  /status - Show current agent status
  /propose <description> - Create new proposal for voting
  /ahoy <role> <budget> <skills...> <description> - Announce role seeking applicants
  /apply <ahoy_id> <pitch...> - Apply for announced role
  /award <ahoy_id> <winner> - Award role to selected applicant (captain only)
  /help - Show this help
  /quit - Exit REPL

Examples:
  /handshake beta "Build multi-agent POC"
  /vote abc123 yes "Good architecture"
  /crew form beta gamma
  /delegate beta 100
  /negotiate cpu 4 "Need more cores for compilation"
  /ahoy "backend dev" 50 rust,docker "Build API microservice"
  /apply abc456 "5 years Rust experience, built 10+ APIs"
  /award abc456 beta
"#
        );
    }

    async fn cmd_handshake(&self, args: &[&str]) -> Result<()> {
        if args.is_empty() {
            anyhow::bail!("Usage: /handshake <agent> [proposal]");
        }

        let to = args[0];
        let proposal = if args.len() > 1 {
            args[1..].join(" ")
        } else {
            "Join crew".to_string()
        };

        self.bus
            .send(Message::Handshake {
                from: self.agent.id.clone(),
                to: to.to_string(),
                proposal,
            })
            .await?;

        println!("🤝 Handshake sent to {}", to);
        Ok(())
    }

    async fn cmd_vote(&self, args: &[&str]) -> Result<()> {
        if args.len() < 2 {
            anyhow::bail!("Usage: /vote <proposal_id> <yes|no|abstain> [reason]");
        }

        let proposal_id = args[0];
        let vote = match args[1].to_lowercase().as_str() {
            "yes" | "y" => VoteChoice::Yes,
            "no" | "n" => VoteChoice::No,
            "abstain" | "a" => VoteChoice::Abstain,
            _ => anyhow::bail!("Vote must be yes, no, or abstain"),
        };

        let reason = if args.len() > 2 {
            Some(args[2..].join(" "))
        } else {
            None
        };

        self.bus
            .vote(proposal_id, self.agent.id.clone(), vote.clone())
            .await?;

        self.bus
            .send(Message::Vote {
                from: self.agent.id.clone(),
                proposal_id: proposal_id.to_string(),
                vote,
                reason,
            })
            .await?;

        println!("🗳️  Vote cast on proposal {}", proposal_id);

        // Check if proposal passed
        if self.bus.is_proposal_passed(proposal_id).await? {
            println!("✅ Proposal PASSED!");
        }

        Ok(())
    }

    async fn cmd_crew(&self, args: &[&str]) -> Result<()> {
        if args.is_empty() {
            anyhow::bail!("Usage: /crew <form|join|leave> [members...]");
        }

        let action = args[0];
        match action {
            "form" => {
                let members: Vec<String> = args[1..].iter().map(|s| s.to_string()).collect();

                self.bus
                    .send(Message::CrewForm {
                        initiator: self.agent.id.clone(),
                        members: members.clone(),
                        purpose: "Multi-agent collaboration".to_string(),
                    })
                    .await?;

                println!("👥 Crew formation initiated with: {:?}", members);
            }
            "join" | "leave" => {
                println!("⚠️  {} crew (not yet implemented)", action);
            }
            _ => {
                anyhow::bail!("Unknown crew action: {}", action);
            }
        }

        Ok(())
    }

    async fn cmd_delegate(&self, args: &[&str]) -> Result<()> {
        if args.len() < 2 {
            anyhow::bail!("Usage: /delegate <agent> <budget>");
        }

        let to = args[0];
        let budget: u64 = args[1].parse().context("Budget must be a number")?;

        self.bus
            .send(Message::Delegate {
                from: self.agent.id.clone(),
                to: to.to_string(),
                crown: "👑".to_string(),
                budget,
            })
            .await?;

        println!("👑 Delegated crown to {} with {}🍰 budget", to, budget);
        Ok(())
    }

    async fn cmd_negotiate(&self, args: &[&str]) -> Result<()> {
        if args.len() < 3 {
            anyhow::bail!("Usage: /negotiate <resource> <amount> <reason>");
        }

        let resource = args[0];
        let amount: u64 = args[1].parse().context("Amount must be a number")?;
        let reason = args[2..].join(" ");

        self.bus
            .send(Message::Negotiate {
                from: self.agent.id.clone(),
                resource: resource.to_string(),
                amount,
                reason,
            })
            .await?;

        println!("💬 Negotiation sent for {} {}", amount, resource);
        Ok(())
    }

    async fn cmd_status(&self) -> Result<()> {
        println!("\n📊 Agent Status:");
        println!("  ID: {}", self.agent.id);
        println!("  PID: {}", self.agent.pid);
        println!("  Skills: {:?}", self.agent.skills);
        println!("  Personality: {}", self.agent.personality);
        println!("  Role: {:?}", self.agent.role);
        println!();

        self.bus
            .send(Message::Status {
                agent_id: self.agent.id.clone(),
                skills: self.agent.skills.clone(),
                role: self.agent.role.clone(),
            })
            .await?;

        Ok(())
    }

    async fn cmd_propose(&self, args: &[&str]) -> Result<()> {
        if args.is_empty() {
            anyhow::bail!("Usage: /propose <description>");
        }

        let description = args.join(" ");
        let proposal_id = self
            .bus
            .create_proposal(&description, &self.agent.id)
            .await?;

        println!("📝 Proposal created: {}", proposal_id);
        println!("   Description: {}", description);
        println!("   Use: /vote {} yes|no|abstain", proposal_id);

        Ok(())
    }

    async fn cmd_ahoy(&self, args: &[&str]) -> Result<()> {
        if args.len() < 4 {
            anyhow::bail!("Usage: /ahoy <role> <budget> <skills> <description...>");
        }

        let role = args[0].to_string();
        let budget: u64 = args[1].parse().context("Budget must be a number")?;
        let required_skills: Vec<String> =
            args[2].split(',').map(|s| s.trim().to_string()).collect();
        let description = args[3..].join(" ");
        let ahoy_id = Uuid::new_v4().to_string();

        self.bus
            .send(Message::Ahoy {
                from: self.agent.id.clone(),
                ahoy_id: ahoy_id.clone(),
                role: role.clone(),
                description: description.clone(),
                required_skills: required_skills.clone(),
                budget,
            })
            .await?;

        println!("📢 Ahoy! Seeking applicants:");
        println!("   ID: {}", ahoy_id);
        println!("   Role: {}", role);
        println!("   Budget: {}🍰", budget);
        println!("   Required skills: {:?}", required_skills);
        println!("   Description: {}", description);
        println!("\n   Agents can apply with: /apply {} <pitch>", ahoy_id);

        Ok(())
    }

    async fn cmd_apply(&self, args: &[&str]) -> Result<()> {
        if args.len() < 2 {
            anyhow::bail!("Usage: /apply <ahoy_id> <pitch...>");
        }

        let ahoy_id = args[0].to_string();
        let pitch = args[1..].join(" ");
        let relevant_skills = self.agent.skills.clone();

        self.bus
            .send(Message::Apply {
                from: self.agent.id.clone(),
                ahoy_id: ahoy_id.clone(),
                pitch: pitch.clone(),
                relevant_skills: relevant_skills.clone(),
            })
            .await?;

        println!("✋ Application submitted:");
        println!("   Ahoy ID: {}", ahoy_id);
        println!("   Skills: {:?}", relevant_skills);
        println!("   Pitch: {}", pitch);

        Ok(())
    }

    async fn cmd_award(&self, args: &[&str]) -> Result<()> {
        if args.len() < 2 {
            anyhow::bail!("Usage: /award <ahoy_id> <winner>");
        }

        let ahoy_id = args[0].to_string();
        let winner = args[1].to_string();
        // TODO: Retrieve budget from ahoy announcement
        let budget = 0;

        self.bus
            .send(Message::Award {
                from: self.agent.id.clone(),
                ahoy_id: ahoy_id.clone(),
                winner: winner.clone(),
                budget,
            })
            .await?;

        println!("🏆 Role awarded!");
        println!("   Ahoy ID: {}", ahoy_id);
        println!("   Winner: {}", winner);
        println!("   Reward: {}🍰", budget);

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_repl_creation() {
        let repl = Repl::new(
            "test-agent".to_string(),
            vec!["rust".to_string(), "testing".to_string()],
        )
        .await
        .unwrap();

        assert_eq!(repl.agent.id, "test-agent");
        assert_eq!(repl.agent.skills, vec!["rust", "testing"]);
    }
}
