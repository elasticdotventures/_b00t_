//! Core ACP protocol types and step synchronization logic

use serde::{Deserialize, Serialize};
use chrono::{DateTime, Utc};
use std::collections::HashMap;
use uuid::Uuid;

/// ACP Message Types as defined in the protocol specification
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "UPPERCASE")]
pub enum MessageType {
    /// Convey current state or logs of an agent
    Status,
    /// Suggest an action, plan, or mutation
    Propose, 
    /// Mark the completion of a step
    Step,
}

/// Core ACP message structure
/// 
/// All ACP messages MUST include: step, agent_id, type, payload
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ACPMessage {
    /// Step number (monotonically increasing)
    pub step: u64,
    /// Agent unique identifier
    pub agent_id: String,
    /// Message type
    #[serde(rename = "type")]
    pub message_type: MessageType,
    /// Arbitrary JSON payload
    pub payload: serde_json::Value,
    /// Message timestamp (ISO 8601)
    pub timestamp: DateTime<Utc>,
    /// Optional message ID for tracking
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message_id: Option<Uuid>,
    /// Optional correlation ID for request/response patterns
    #[serde(skip_serializing_if = "Option::is_none")]
    pub correlation_id: Option<Uuid>,
}

impl ACPMessage {
    /// Create a new STATUS message
    pub fn status(agent_id: String, step: u64, payload: serde_json::Value) -> Self {
        Self {
            step,
            agent_id,
            message_type: MessageType::Status,
            payload,
            timestamp: Utc::now(),
            message_id: Some(Uuid::new_v4()),
            correlation_id: None,
        }
    }

    /// Create a new PROPOSE message
    pub fn propose(agent_id: String, step: u64, payload: serde_json::Value) -> Self {
        Self {
            step,
            agent_id,
            message_type: MessageType::Propose,
            payload,
            timestamp: Utc::now(),
            message_id: Some(Uuid::new_v4()),
            correlation_id: None,
        }
    }

    /// Create a new STEP message
    pub fn step_complete(agent_id: String, step: u64) -> Self {
        Self {
            step,
            agent_id,
            message_type: MessageType::Step,
            payload: serde_json::json!({"step": step}),
            timestamp: Utc::now(),
            message_id: Some(Uuid::new_v4()),
            correlation_id: None,
        }
    }

    /// Get NATS subject for this message
    /// Format: acp.{step}.{agent_id}.{type}
    pub fn subject(&self) -> String {
        format!("acp.{}.{}.{}", 
            self.step, 
            self.agent_id, 
            format!("{:?}", self.message_type).to_lowercase()
        )
    }

    /// Get wildcard subject for listening to all messages in a step
    /// Format: acp.{step}.>
    pub fn step_wildcard(step: u64) -> String {
        format!("acp.{}.>", step)
    }

    /// Get wildcard subject for listening to all messages from an agent
    /// Format: acp.>.{agent_id}.>
    pub fn agent_wildcard(agent_id: &str) -> String {
        format!("acp.>.{}.>", agent_id)
    }
}

/// Step barrier synchronization manager
/// 
/// Ensures all agents complete a step before advancing to the next
#[derive(Debug)]
pub struct StepBarrier {
    current_step: u64,
    known_agents: Vec<String>,
    step_completions: HashMap<u64, Vec<String>>,
    timeout_ms: u64,
}

impl StepBarrier {
    /// Create new step barrier
    pub fn new(known_agents: Vec<String>, timeout_ms: u64) -> Self {
        Self {
            current_step: 0,
            known_agents,
            step_completions: HashMap::new(),
            timeout_ms,
        }
    }

    /// Get current step number
    pub fn current_step(&self) -> u64 {
        self.current_step
    }

    /// Record that an agent completed a step
    pub fn record_step_completion(&mut self, step: u64, agent_id: String) {
        let completions = self.step_completions.entry(step).or_insert_with(Vec::new);
        if !completions.contains(&agent_id) {
            completions.push(agent_id);
        }
    }

    /// Check if step is complete (all known agents have sent STEP message)
    pub fn is_step_complete(&self, step: u64) -> bool {
        if let Some(completions) = self.step_completions.get(&step) {
            completions.len() >= self.known_agents.len() && 
            self.known_agents.iter().all(|agent| completions.contains(agent))
        } else {
            false
        }
    }

    /// Advance to next step if current step is complete
    pub fn try_advance_step(&mut self) -> bool {
        if self.is_step_complete(self.current_step) {
            self.current_step += 1;
            true
        } else {
            false
        }
    }

    /// Force advance to next step (timeout scenario)
    pub fn force_advance_step(&mut self) {
        self.current_step += 1;
    }

    /// Get agents that haven't completed the current step
    pub fn pending_agents(&self, step: u64) -> Vec<String> {
        let empty_vec = Vec::new();
        let completed = self.step_completions.get(&step).unwrap_or(&empty_vec);
        self.known_agents.iter()
            .filter(|agent| !completed.contains(agent))
            .cloned()
            .collect()
    }

    /// Add new agent to known agents list
    pub fn add_agent(&mut self, agent_id: String) {
        if !self.known_agents.contains(&agent_id) {
            self.known_agents.push(agent_id);
        }
    }

    /// Remove agent from known agents list
    pub fn remove_agent(&mut self, agent_id: &str) {
        self.known_agents.retain(|id| id != agent_id);
        // Remove from all step completions
        for completions in self.step_completions.values_mut() {
            completions.retain(|id| id != agent_id);
        }
    }

    /// Get list of known agents
    pub fn known_agents(&self) -> &[String] {
        &self.known_agents
    }

    /// Clear old step completion data (cleanup)
    pub fn cleanup_old_steps(&mut self, keep_last_n: usize) {
        if self.current_step > keep_last_n as u64 {
            let cutoff = self.current_step - keep_last_n as u64;
            self.step_completions.retain(|&step, _| step >= cutoff);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_acp_message_creation() {
        let msg = ACPMessage::status(
            "test-agent".to_string(),
            1,
            serde_json::json!({"status": "ready"})
        );
        
        assert_eq!(msg.step, 1);
        assert_eq!(msg.agent_id, "test-agent");
        assert_eq!(msg.message_type, MessageType::Status);
        assert!(msg.message_id.is_some());
    }

    #[test]
    fn test_message_subject() {
        let msg = ACPMessage::propose(
            "claude.124435".to_string(),
            5,
            serde_json::json!({"action": "deploy"})
        );
        
        assert_eq!(msg.subject(), "acp.5.claude.124435.propose");
    }

    #[test]
    fn test_step_barrier() {
        let mut barrier = StepBarrier::new(
            vec!["agent1".to_string(), "agent2".to_string()],
            30000
        );
        
        assert_eq!(barrier.current_step(), 0);
        assert!(!barrier.is_step_complete(0));
        
        barrier.record_step_completion(0, "agent1".to_string());
        assert!(!barrier.is_step_complete(0));
        
        barrier.record_step_completion(0, "agent2".to_string());
        assert!(barrier.is_step_complete(0));
        
        assert!(barrier.try_advance_step());
        assert_eq!(barrier.current_step(), 1);
    }

    #[test]
    fn test_pending_agents() {
        let mut barrier = StepBarrier::new(
            vec!["a1".to_string(), "a2".to_string(), "a3".to_string()],
            30000
        );
        
        barrier.record_step_completion(0, "a1".to_string());
        barrier.record_step_completion(0, "a3".to_string());
        
        let pending = barrier.pending_agents(0);
        assert_eq!(pending, vec!["a2"]);
    }
}