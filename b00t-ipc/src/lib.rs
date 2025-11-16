//! b00t-ipc: Inter-Process Communication for Multi-Agent Systems
//!
//! Provides message bus, protocols, and primitives for b00t agent crews.
//!
//! # Core Concepts
//! - **Agent**: Autonomous process with identity, skills, personality
//! - **Message**: Typed communication between agents
//! - **Crew**: Coordinated group with roles & voting
//! - **Protocol**: Handshake, voting, negotiation patterns
//!
//! # Example
//! ```rust,no_run
//! use b00t_ipc::{Agent, Message, MessageBus};
//!
//! #[tokio::main]
//! async fn main() {
//!     let agent = Agent::new("alpha", vec!["rust", "testing"]);
//!     let bus = MessageBus::new().await.unwrap();
//!
//!     // Join crew
//!     bus.handshake("alpha", "beta").await.unwrap();
//! }
//! ```

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::{RwLock, mpsc};
use uuid::Uuid;

/// Agent identity and configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Agent {
    /// Unique agent identifier
    pub id: String,
    /// Process ID or session ID
    pub pid: String,
    /// Git branch for this agent's worktree
    pub branch: Option<String>,
    /// Agent skills (e.g., "rust", "docker", "testing")
    pub skills: Vec<String>,
    /// Personality traits
    pub personality: String,
    /// Humor level (none, low, moderate, high)
    pub humor: String,
    /// Current role in crew (specialist, captain, observer)
    pub role: AgentRole,
    /// IPC socket path
    pub socket: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum AgentRole {
    Specialist,
    Captain,
    Observer,
}

impl Agent {
    pub fn new(id: impl Into<String>, skills: Vec<impl Into<String>>) -> Self {
        Self {
            id: id.into(),
            pid: Uuid::new_v4().to_string(),
            branch: None,
            skills: skills.into_iter().map(Into::into).collect(),
            personality: "balanced".to_string(),
            humor: "moderate".to_string(),
            role: AgentRole::Specialist,
            socket: None,
        }
    }

    pub fn with_personality(mut self, personality: impl Into<String>) -> Self {
        self.personality = personality.into();
        self
    }

    pub fn with_role(mut self, role: AgentRole) -> Self {
        self.role = role;
        self
    }
}

/// Message types for agent communication
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum Message {
    /// Handshake initiation
    Handshake {
        from: String,
        to: String,
        proposal: String,
    },
    /// Handshake response
    HandshakeReply {
        from: String,
        to: String,
        accept: bool,
        role: AgentRole,
    },
    /// Vote on proposal
    Vote {
        from: String,
        proposal_id: String,
        vote: VoteChoice,
        reason: Option<String>,
    },
    /// Crew formation
    CrewForm {
        initiator: String,
        members: Vec<String>,
        purpose: String,
    },
    /// Delegate crown (captain role)
    Delegate {
        from: String,
        to: String,
        crown: String, // 👑 emoji NFT
        budget: u64,   // 🍰 cake tokens
    },
    /// Status request/response
    Status {
        agent_id: String,
        skills: Vec<String>,
        role: AgentRole,
    },
    /// Negotiate resources
    Negotiate {
        from: String,
        resource: String,
        amount: u64,
        reason: String,
    },
    /// General broadcast to crew
    Broadcast { from: String, content: String },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum VoteChoice {
    Yes,
    No,
    Abstain,
}

/// Voting proposal state
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Proposal {
    pub id: String,
    pub description: String,
    pub initiator: String,
    pub votes: HashMap<String, VoteChoice>,
    pub quorum: usize,
    pub expires_at: Option<std::time::SystemTime>,
}

impl Proposal {
    pub fn new(description: impl Into<String>, initiator: impl Into<String>) -> Self {
        Self {
            id: Uuid::new_v4().to_string(),
            description: description.into(),
            initiator: initiator.into(),
            votes: HashMap::new(),
            quorum: 2, // Simple majority
            expires_at: None,
        }
    }

    pub fn cast_vote(&mut self, agent: String, vote: VoteChoice) {
        self.votes.insert(agent, vote);
    }

    pub fn is_passed(&self) -> bool {
        let yes_votes = self
            .votes
            .values()
            .filter(|v| matches!(v, VoteChoice::Yes))
            .count();
        yes_votes >= self.quorum
    }

    pub fn is_rejected(&self) -> bool {
        let no_votes = self
            .votes
            .values()
            .filter(|v| matches!(v, VoteChoice::No))
            .count();
        no_votes >= self.quorum
    }
}

/// Message bus for agent IPC
pub struct MessageBus {
    agents: Arc<RwLock<HashMap<String, Agent>>>,
    proposals: Arc<RwLock<HashMap<String, Proposal>>>,
    tx: mpsc::UnboundedSender<Message>,
    rx: Arc<RwLock<mpsc::UnboundedReceiver<Message>>>,
}

impl MessageBus {
    pub async fn new() -> Result<Self> {
        let (tx, rx) = mpsc::unbounded_channel();
        Ok(Self {
            agents: Arc::new(RwLock::new(HashMap::new())),
            proposals: Arc::new(RwLock::new(HashMap::new())),
            tx,
            rx: Arc::new(RwLock::new(rx)),
        })
    }

    /// Register an agent
    pub async fn register(&self, agent: Agent) -> Result<()> {
        let mut agents = self.agents.write().await;
        agents.insert(agent.id.clone(), agent);
        Ok(())
    }

    /// Send a message
    pub async fn send(&self, msg: Message) -> Result<()> {
        self.tx.send(msg).context("Failed to send message to bus")?;
        Ok(())
    }

    /// Receive next message
    pub async fn recv(&self) -> Option<Message> {
        let mut rx = self.rx.write().await;
        rx.recv().await
    }

    /// Handshake between two agents
    pub async fn handshake(&self, from: impl Into<String>, to: impl Into<String>) -> Result<()> {
        let msg = Message::Handshake {
            from: from.into(),
            to: to.into(),
            proposal: "Join crew".to_string(),
        };
        self.send(msg).await
    }

    /// Create a proposal for voting
    pub async fn create_proposal(
        &self,
        description: impl Into<String>,
        initiator: impl Into<String>,
    ) -> Result<String> {
        let proposal = Proposal::new(description, initiator);
        let id = proposal.id.clone();
        let mut proposals = self.proposals.write().await;
        proposals.insert(id.clone(), proposal);
        Ok(id)
    }

    /// Cast a vote on a proposal
    pub async fn vote(
        &self,
        proposal_id: &str,
        agent: impl Into<String>,
        vote: VoteChoice,
    ) -> Result<()> {
        let mut proposals = self.proposals.write().await;
        let proposal = proposals
            .get_mut(proposal_id)
            .context("Proposal not found")?;
        proposal.cast_vote(agent.into(), vote);
        Ok(())
    }

    /// Check if proposal passed
    pub async fn is_proposal_passed(&self, proposal_id: &str) -> Result<bool> {
        let proposals = self.proposals.read().await;
        let proposal = proposals.get(proposal_id).context("Proposal not found")?;
        Ok(proposal.is_passed())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_agent_creation() {
        let agent = Agent::new("alpha", vec!["rust", "testing"])
            .with_personality("curious")
            .with_role(AgentRole::Specialist);

        assert_eq!(agent.id, "alpha");
        assert_eq!(agent.skills, vec!["rust", "testing"]);
        assert_eq!(agent.personality, "curious");
        assert_eq!(agent.role, AgentRole::Specialist);
    }

    #[tokio::test]
    async fn test_message_bus() {
        let bus = MessageBus::new().await.unwrap();

        let alpha = Agent::new("alpha", vec!["rust"]);
        let beta = Agent::new("beta", vec!["docker"]);

        bus.register(alpha).await.unwrap();
        bus.register(beta).await.unwrap();

        bus.handshake("alpha", "beta").await.unwrap();

        let msg = bus.recv().await.unwrap();
        match msg {
            Message::Handshake { from, to, .. } => {
                assert_eq!(from, "alpha");
                assert_eq!(to, "beta");
            }
            _ => panic!("Expected Handshake message"),
        }
    }

    #[tokio::test]
    async fn test_voting() {
        let mut proposal = Proposal::new("Use Unix sockets", "alpha");

        proposal.cast_vote("alpha".to_string(), VoteChoice::Yes);
        assert!(!proposal.is_passed()); // Need quorum of 2

        proposal.cast_vote("beta".to_string(), VoteChoice::Yes);
        assert!(proposal.is_passed());
    }

    #[tokio::test]
    async fn test_proposal_lifecycle() {
        let bus = MessageBus::new().await.unwrap();

        let proposal_id = bus.create_proposal("Implement IPC", "alpha").await.unwrap();

        bus.vote(&proposal_id, "alpha", VoteChoice::Yes)
            .await
            .unwrap();
        assert!(!bus.is_proposal_passed(&proposal_id).await.unwrap());

        bus.vote(&proposal_id, "beta", VoteChoice::Yes)
            .await
            .unwrap();
        assert!(bus.is_proposal_passed(&proposal_id).await.unwrap());
    }
}
