//! Agent coordination system using Redis pub/sub
//!
//! Provides comprehensive agent-to-agent communication including:
//! - Agent discovery and presence tracking
//! - Message routing and blocking reception
//! - Team captain delegation and voting systems
//! - Progress reporting and notifications

use crate::redis::{RedisComms, AgentMessage, AgentStatus};
use crate::B00tResult;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tokio::sync::{mpsc, oneshot};
use tokio::time::timeout;

/// Agent metadata for discovery and capabilities
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentMetadata {
    pub agent_id: String,
    pub agent_role: String, // e.g., "captain", "worker", "specialist"
    pub capabilities: Vec<String>, // Skills/domains this agent handles
    pub crew: Option<String>, // Crew membership
    pub status: AgentStatus,
    pub last_seen: u64, // Unix timestamp
    pub load: f32, // Current workload 0.0-1.0
    pub specializations: HashMap<String, f32>, // Domain -> proficiency score
}

/// Message types for agent coordination
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "msg_type", content = "data")]
pub enum CoordinationMessage {
    /// Agent presence announcement
    Presence {
        metadata: AgentMetadata,
    },

    /// Direct message between agents
    DirectMessage {
        from_agent: String,
        to_agent: String,
        subject: String,
        content: String,
        message_id: String,
        reply_to: Option<String>,
        requires_ack: bool,
    },

    /// Task delegation from captain to worker
    TaskDelegation {
        captain_id: String,
        worker_id: String,
        task_id: String,
        task_description: String,
        priority: TaskPriority,
        deadline: Option<u64>,
        required_capabilities: Vec<String>,
        blocking: bool, // Captain waits for completion
    },

    /// Task completion notification
    TaskCompletion {
        worker_id: String,
        captain_id: String,
        task_id: String,
        status: TaskCompletionStatus,
        result: Option<String>,
        artifacts: Vec<String>, // Paths to output files, etc.
    },

    /// Progress update during task execution
    ProgressUpdate {
        agent_id: String,
        task_id: String,
        progress_percent: f32,
        status_message: String,
        estimated_completion: Option<u64>,
    },

    /// Voting proposal from captain
    VotingProposal {
        captain_id: String,
        proposal_id: String,
        subject: String,
        description: String,
        options: Vec<VotingOption>,
        voting_type: VotingType,
        deadline: u64,
        eligible_voters: Vec<String>,
    },

    /// Vote submission
    Vote {
        voter_id: String,
        proposal_id: String,
        vote: VoteChoice,
        reasoning: Option<String>,
    },

    /// Notification about external events (files, PRs, etc.)
    EventNotification {
        event_type: String, // "file_created", "pr_opened", "build_failed", etc.
        source: String, // System/service that generated the event
        details: serde_json::Value,
        timestamp: u64,
        affected_agents: Option<Vec<String>>, // Target specific agents
    },

    /// Request for specific agent capabilities
    CapabilityRequest {
        requesting_agent: String,
        required_capabilities: Vec<String>,
        task_description: String,
        urgency: RequestUrgency,
    },

    /// Response to capability request
    CapabilityResponse {
        responding_agent: String,
        request_id: String,
        available: bool,
        estimated_availability: Option<u64>,
        proficiency_scores: HashMap<String, f32>,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum TaskPriority {
    Low,
    Normal,
    High,
    Critical,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum TaskCompletionStatus {
    Success,
    Failed(String), // Error message
    PartialSuccess(String), // Partial completion details
    Cancelled,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VotingOption {
    pub id: String,
    pub title: String,
    pub description: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum VotingType {
    SingleChoice,     // Pick one option
    RankedChoice,     // Rank options by preference
    Approval,         // Approve multiple options
    VetoCapable,      // Any agent can veto
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum VoteChoice {
    Single(String),                    // Option ID
    Ranked(Vec<String>),              // Ordered list of option IDs
    Approval(Vec<String>),            // List of approved option IDs
    Veto { option_id: String, alternative: Option<String> },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum RequestUrgency {
    Low,
    Normal,
    High,
    Emergency,
}

/// Agent coordinator handles all agent communication and coordination
pub struct AgentCoordinator {
    redis: RedisComms,
    agent_metadata: AgentMetadata,
    _message_handlers: HashMap<String, mpsc::UnboundedSender<CoordinationMessage>>,
    pending_tasks: HashMap<String, oneshot::Sender<TaskCompletion>>,
    pending_votes: HashMap<String, oneshot::Sender<HashMap<String, VoteChoice>>>,
}

impl AgentCoordinator {
    /// Create new agent coordinator
    pub fn new(redis: RedisComms, agent_metadata: AgentMetadata) -> Self {
        Self {
            redis,
            agent_metadata,
            _message_handlers: HashMap::new(),
            pending_tasks: HashMap::new(),
            pending_votes: HashMap::new(),
        }
    }

    /// Start agent coordination (announce presence, start listening)
    pub async fn start(&mut self) -> B00tResult<()> {
        // Announce presence
        self.announce_presence().await?;

        // Start listening for messages
        self.start_message_listener().await?;

        // Set up periodic presence updates
        self.start_presence_heartbeat().await?;

        Ok(())
    }

    /// Announce agent presence to the network
    pub async fn announce_presence(&self) -> B00tResult<()> {
        let message = CoordinationMessage::Presence {
            metadata: self.agent_metadata.clone(),
        };

        self.redis.publish("b00t:agents:presence", &AgentMessage::Session {
            session_id: self.agent_metadata.agent_id.clone(),
            event: crate::redis::SessionEvent::Created,
            data: HashMap::from([
                ("coordination_message".to_string(), serde_json::to_value(&message)?),
            ]),
        })?;

        Ok(())
    }

    /// Discover other agents in the network
    pub async fn discover_agents(&self) -> B00tResult<Vec<AgentMetadata>> {
        // Get all agents from Redis hash
        let agents_data = self.redis.hgetall("b00t:agents:registry")?;
        let mut agents = Vec::new();

        for (agent_id, metadata_json) in agents_data {
            if agent_id != self.agent_metadata.agent_id {
                if let Ok(metadata) = serde_json::from_str::<AgentMetadata>(&metadata_json) {
                    // Filter out stale agents (haven't been seen in 5 minutes)
                    let now = SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs();
                    if now - metadata.last_seen < 300 {
                        agents.push(metadata);
                    }
                }
            }
        }

        Ok(agents)
    }

    /// Send direct message to another agent
    pub async fn send_message(
        &self,
        to_agent: &str,
        subject: &str,
        content: &str,
        requires_ack: bool
    ) -> B00tResult<String> {
        let message_id = uuid::Uuid::new_v4().to_string();
        let message = CoordinationMessage::DirectMessage {
            from_agent: self.agent_metadata.agent_id.clone(),
            to_agent: to_agent.to_string(),
            subject: subject.to_string(),
            content: content.to_string(),
            message_id: message_id.clone(),
            reply_to: None,
            requires_ack,
        };

        self.send_coordination_message(&format!("b00t:agent:{}", to_agent), &message).await?;
        Ok(message_id)
    }

    /// Delegate task to worker agent (captain functionality)
    pub async fn delegate_task(
        &mut self,
        worker_id: &str,
        task_id: &str,
        task_description: &str,
        priority: TaskPriority,
        deadline: Option<Duration>,
        required_capabilities: Vec<String>,
        blocking: bool,
    ) -> B00tResult<Option<TaskCompletion>> {
        let deadline_timestamp = deadline.map(|d| {
            SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs() + d.as_secs()
        });

        let message = CoordinationMessage::TaskDelegation {
            captain_id: self.agent_metadata.agent_id.clone(),
            worker_id: worker_id.to_string(),
            task_id: task_id.to_string(),
            task_description: task_description.to_string(),
            priority,
            deadline: deadline_timestamp,
            required_capabilities,
            blocking,
        };

        // Set up completion listener if blocking
        let completion_receiver = if blocking {
            let (tx, rx) = oneshot::channel();
            self.pending_tasks.insert(task_id.to_string(), tx);
            Some(rx)
        } else {
            None
        };

        // Send delegation message
        self.send_coordination_message(&format!("b00t:agent:{}", worker_id), &message).await?;

        // If blocking, wait for completion
        if let Some(receiver) = completion_receiver {
            match timeout(Duration::from_secs(3600), receiver).await { // 1 hour timeout
                Ok(Ok(completion)) => Ok(Some(completion)),
                Ok(Err(_)) => anyhow::bail!("Task completion channel closed unexpectedly"),
                Err(_) => anyhow::bail!("Task delegation timed out after 1 hour"),
            }
        } else {
            Ok(None)
        }
    }

    /// Report task completion (worker functionality)
    pub async fn complete_task(
        &self,
        captain_id: &str,
        task_id: &str,
        status: TaskCompletionStatus,
        result: Option<String>,
        artifacts: Vec<String>,
    ) -> B00tResult<()> {
        let message = CoordinationMessage::TaskCompletion {
            worker_id: self.agent_metadata.agent_id.clone(),
            captain_id: captain_id.to_string(),
            task_id: task_id.to_string(),
            status,
            result,
            artifacts,
        };

        self.send_coordination_message(&format!("b00t:agent:{}", captain_id), &message).await?;
        Ok(())
    }

    /// Send progress update for a task
    pub async fn report_progress(
        &self,
        task_id: &str,
        progress_percent: f32,
        status_message: &str,
        estimated_completion: Option<Duration>,
    ) -> B00tResult<()> {
        let estimated_timestamp = estimated_completion.map(|d| {
            SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs() + d.as_secs()
        });

        let message = CoordinationMessage::ProgressUpdate {
            agent_id: self.agent_metadata.agent_id.clone(),
            task_id: task_id.to_string(),
            progress_percent,
            status_message: status_message.to_string(),
            estimated_completion: estimated_timestamp,
        };

        // Broadcast progress to all interested parties
        self.send_coordination_message("b00t:progress:updates", &message).await?;
        Ok(())
    }

    /// Create voting proposal (captain functionality)
    pub async fn create_voting_proposal(
        &mut self,
        subject: &str,
        description: &str,
        options: Vec<VotingOption>,
        voting_type: VotingType,
        deadline: Duration,
        eligible_voters: Vec<String>,
    ) -> B00tResult<HashMap<String, VoteChoice>> {
        let proposal_id = uuid::Uuid::new_v4().to_string();
        let deadline_timestamp = SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs() + deadline.as_secs();

        let message = CoordinationMessage::VotingProposal {
            captain_id: self.agent_metadata.agent_id.clone(),
            proposal_id: proposal_id.clone(),
            subject: subject.to_string(),
            description: description.to_string(),
            options,
            voting_type,
            deadline: deadline_timestamp,
            eligible_voters: eligible_voters.clone(),
        };

        // Set up vote collection
        let (tx, rx) = oneshot::channel();
        self.pending_votes.insert(proposal_id.clone(), tx);

        // Send proposal to eligible voters
        for voter in &eligible_voters {
            self.send_coordination_message(&format!("b00t:agent:{}", voter), &message).await?;
        }

        // Wait for voting to complete or timeout
        match timeout(deadline, rx).await {
            Ok(Ok(votes)) => Ok(votes),
            Ok(Err(_)) => anyhow::bail!("Voting channel closed unexpectedly"),
            Err(_) => anyhow::bail!("Voting deadline exceeded"),
        }
    }

    /// Submit vote for a proposal
    pub async fn submit_vote(
        &self,
        proposal_id: &str,
        vote: VoteChoice,
        reasoning: Option<String>,
    ) -> B00tResult<()> {
        let message = CoordinationMessage::Vote {
            voter_id: self.agent_metadata.agent_id.clone(),
            proposal_id: proposal_id.to_string(),
            vote,
            reasoning,
        };

        // Send vote back to captain
        self.send_coordination_message("b00t:votes:collection", &message).await?;
        Ok(())
    }

    /// Send event notification (file changes, PR updates, etc.)
    pub async fn notify_event(
        &self,
        event_type: &str,
        source: &str,
        details: serde_json::Value,
        affected_agents: Option<Vec<String>>,
    ) -> B00tResult<()> {
        let message = CoordinationMessage::EventNotification {
            event_type: event_type.to_string(),
            source: source.to_string(),
            details,
            timestamp: SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs(),
            affected_agents,
        };

        // Broadcast to all agents or specific targets
        self.send_coordination_message("b00t:events:notifications", &message).await?;
        Ok(())
    }

    /// Wait for specific message (blocking MCP command support)
    pub async fn wait_for_message(
        &self,
        timeout_duration: Duration,
        _filter: MessageFilter,
    ) -> B00tResult<CoordinationMessage> {
        let (tx, rx) = oneshot::channel();

        // TODO: Set up filtered message listener
        // This would require extending the message handling system

        match timeout(timeout_duration, rx).await {
            Ok(Ok(message)) => Ok(message),
            Ok(Err(_)) => anyhow::bail!("Message wait channel closed"),
            Err(_) => anyhow::bail!("Message wait timed out"),
        }
    }

    /// Request agents with specific capabilities
    pub async fn request_capability(
        &self,
        required_capabilities: Vec<String>,
        task_description: &str,
        urgency: RequestUrgency,
    ) -> B00tResult<Vec<(String, HashMap<String, f32>)>> {
        let message = CoordinationMessage::CapabilityRequest {
            requesting_agent: self.agent_metadata.agent_id.clone(),
            required_capabilities,
            task_description: task_description.to_string(),
            urgency,
        };

        // Broadcast capability request
        self.send_coordination_message("b00t:capabilities:requests", &message).await?;

        // TODO: Collect responses with timeout
        Ok(vec![])
    }

    // Private helper methods

    async fn send_coordination_message(&self, channel: &str, message: &CoordinationMessage) -> B00tResult<()> {
        let agent_message = AgentMessage::Session {
            session_id: uuid::Uuid::new_v4().to_string(),
            event: crate::redis::SessionEvent::Updated,
            data: HashMap::from([
                ("coordination_message".to_string(), serde_json::to_value(message)?),
            ]),
        };

        self.redis.publish(channel, &agent_message)?;
        Ok(())
    }

    async fn start_message_listener(&mut self) -> B00tResult<()> {
        // TODO: Implement Redis subscription listener
        // This would handle incoming coordination messages and route them to handlers
        Ok(())
    }

    async fn start_presence_heartbeat(&self) -> B00tResult<()> {
        // TODO: Implement periodic presence updates
        // Every 30 seconds, update agent metadata in Redis
        Ok(())
    }
}

/// Message filter for selective waiting
#[derive(Debug, Clone)]
pub struct MessageFilter {
    pub message_types: Option<Vec<String>>,
    pub from_agents: Option<Vec<String>>,
    pub task_ids: Option<Vec<String>>,
    pub subjects: Option<Vec<String>>,
}

/// Task completion result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskCompletion {
    pub task_id: String,
    pub status: TaskCompletionStatus,
    pub result: Option<String>,
    pub artifacts: Vec<String>,
    pub worker_id: String,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::redis::RedisConfig;

    #[test]
    fn test_agent_metadata_serialization() {
        let metadata = AgentMetadata {
            agent_id: "test-agent".to_string(),
            agent_role: "worker".to_string(),
            capabilities: vec!["rust".to_string(), "testing".to_string()],
            crew: Some("backend".to_string()),
            status: AgentStatus::Online,
            last_seen: 1234567890,
            load: 0.5,
            specializations: HashMap::from([
                ("rust".to_string(), 0.9),
                ("testing".to_string(), 0.8),
            ]),
        };

        let json = serde_json::to_string(&metadata).unwrap();
        let deserialized: AgentMetadata = serde_json::from_str(&json).unwrap();
        assert_eq!(metadata.agent_id, deserialized.agent_id);
    }

    #[test]
    fn test_voting_choice_serialization() {
        let choices = vec![
            VoteChoice::Single("option1".to_string()),
            VoteChoice::Ranked(vec!["opt1".to_string(), "opt2".to_string()]),
            VoteChoice::Veto {
                option_id: "bad_option".to_string(),
                alternative: Some("better_option".to_string())
            },
        ];

        for choice in choices {
            let json = serde_json::to_string(&choice).unwrap();
            let _: VoteChoice = serde_json::from_str(&json).unwrap();
        }
    }

    #[tokio::test]
    async fn test_agent_coordinator_creation() {
        let config = RedisConfig::default();
        let redis = RedisComms::new(config, "test-agent".to_string()).unwrap();

        let metadata = AgentMetadata {
            agent_id: "test-agent".to_string(),
            agent_role: "test".to_string(),
            capabilities: vec!["testing".to_string()],
            crew: None,
            status: AgentStatus::Online,
            last_seen: 1234567890,
            load: 0.0,
            specializations: HashMap::new(),
        };

        let coordinator = AgentCoordinator::new(redis, metadata);
        assert_eq!(coordinator.agent_metadata.agent_id, "test-agent");
    }
}