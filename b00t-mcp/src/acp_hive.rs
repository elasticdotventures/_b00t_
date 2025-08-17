//! ACP Hive Communication for MCP Agents
//! 
//! Enables MCP agents to participate in coordinated hive missions using the 
//! Agent Coordination Protocol (ACP) StepSync Protocol.

use anyhow::{Result, Context};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::time::{Duration, Instant};
use tokio::time::sleep;
use tracing::{info, warn, debug, error};
use uuid::Uuid;

use b00t_acp::{Agent, AgentConfig, ACPMessage, MessageType};

/// Hive mission configuration for ACP coordination
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HiveMission {
    /// Unique mission identifier
    pub mission_id: String,
    /// Mission namespace (e.g., account.username)
    pub namespace: String,
    /// Expected number of agents in the hive
    pub expected_agents: usize,
    /// Current step in the mission
    pub current_step: u64,
    /// Mission timeout in seconds
    pub timeout_seconds: u64,
    /// Mission description
    pub description: String,
}

/// Agent status in the hive mission
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentStatus {
    /// Agent identifier
    pub agent_id: String,
    /// Current step the agent is on
    pub step: u64,
    /// Agent's last known status
    pub status: String,
    /// Last activity timestamp
    pub last_seen: chrono::DateTime<chrono::Utc>,
    /// Agent role in the mission
    pub role: String,
}

/// Hive coordination status
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HiveStatus {
    /// Mission information
    pub mission: HiveMission,
    /// All known agents in the hive
    pub agents: HashMap<String, AgentStatus>,
    /// Current coordination step
    pub current_step: u64,
    /// Agents ready for next step
    pub agents_ready: Vec<String>,
    /// Step completion status
    pub step_complete: bool,
}

/// ACP Hive Client for MCP agent coordination
#[derive(Clone)]
pub struct AcpHiveClient {
    agent: Agent,
    mission: HiveMission,
    agent_status: AgentStatus,
    hive_status: HiveStatus,
}

impl AcpHiveClient {
    /// Create new hive client for an MCP agent
    pub async fn new(
        agent_id: String,
        role: String,
        mission: HiveMission,
        nats_url: String,
    ) -> Result<Self> {
        // Create agent configuration for hive communication
        let config = AgentConfig::new(
            agent_id.clone(),
            nats_url,
            mission.namespace.clone(),
        )
        .with_role(role.clone())
        .with_timeout(30000);

        // Initialize ACP agent
        let agent = Agent::new(config).await
            .context("Failed to initialize ACP agent")?;

        // Initialize agent status
        let agent_status = AgentStatus {
            agent_id: agent_id.clone(),
            step: mission.current_step,
            status: "initialized".to_string(),
            last_seen: chrono::Utc::now(),
            role: role.clone(),
        };

        // Initialize hive status
        let mut agents = HashMap::new();
        agents.insert(agent_id.clone(), agent_status.clone());

        let hive_status = HiveStatus {
            mission: mission.clone(),
            agents,
            current_step: mission.current_step,
            agents_ready: vec![],
            step_complete: false,
        };

        info!("🐝 Initialized ACP hive client for agent {} in mission {}", 
              agent_id, mission.mission_id);

        Ok(Self {
            agent,
            mission,
            agent_status,
            hive_status,
        })
    }

    /// Send status update to the hive
    pub async fn send_status(&mut self, description: &str, payload: Option<serde_json::Value>) -> Result<()> {
        let payload = payload.unwrap_or_else(|| {
            serde_json::json!({
                "description": description,
                "mission_id": self.mission.mission_id,
                "role": self.agent_status.role,
                "timestamp": chrono::Utc::now().to_rfc3339()
            })
        });

        // Create ACP status message
        let message = ACPMessage::status(
            self.agent_status.agent_id.clone(),
            self.agent_status.step,
            payload,
        );

        info!("🐝 Sending hive status: {} (step {})", description, self.agent_status.step);
        
        // Send via ACP agent (stub for now)
        self.agent.send_message(&message).await
            .context("Failed to send status to hive")?;

        // Update local status
        self.agent_status.status = description.to_string();
        self.agent_status.last_seen = chrono::Utc::now();

        Ok(())
    }

    /// Propose an action to the hive
    pub async fn propose_action(&mut self, action: &str, payload: Option<serde_json::Value>) -> Result<()> {
        let mut proposal_payload = payload.unwrap_or_else(|| serde_json::json!({}));
        proposal_payload["action"] = serde_json::json!(action);
        proposal_payload["mission_id"] = serde_json::json!(self.mission.mission_id);
        proposal_payload["proposer"] = serde_json::json!(self.agent_status.agent_id);
        proposal_payload["timestamp"] = serde_json::json!(chrono::Utc::now().to_rfc3339());

        let message = ACPMessage::propose(
            self.agent_status.agent_id.clone(),
            self.agent_status.step,
            proposal_payload,
        );

        info!("🐝 Proposing action to hive: {} (step {})", action, self.agent_status.step);

        self.agent.send_message(&message).await
            .context("Failed to propose action to hive")?;

        Ok(())
    }

    /// Wait for step synchronization - blocks until all agents are ready for next step
    pub async fn wait_for_step_sync(&mut self, target_step: u64, timeout_seconds: u64) -> Result<HiveStatus> {
        let start_time = Instant::now();
        let timeout_duration = Duration::from_secs(timeout_seconds);

        info!("🐝 Waiting for step sync: target step {} (timeout: {}s)", target_step, timeout_seconds);

        loop {
            // Check if we've timed out
            if start_time.elapsed() > timeout_duration {
                warn!("🐝 Step sync timeout reached for step {}", target_step);
                return Err(anyhow::anyhow!("Step synchronization timeout"));
            }

            // Listen for hive updates
            if let Ok(status) = self.check_hive_status().await {
                self.hive_status = status;

                // Check if step is complete
                if self.hive_status.step_complete && self.hive_status.current_step >= target_step {
                    info!("🐝 Step {} synchronized! All agents ready", target_step);
                    
                    // Advance our local step
                    self.agent_status.step = target_step + 1;
                    
                    return Ok(self.hive_status.clone());
                }

                // Report progress
                let ready_count = self.hive_status.agents_ready.len();
                let total_count = self.hive_status.agents.len();
                debug!("🐝 Step sync progress: {}/{} agents ready for step {}", 
                       ready_count, total_count, target_step);
            }

            // Wait before checking again
            sleep(Duration::from_millis(1000)).await;
        }
    }

    /// Signal readiness for next step
    pub async fn signal_step_ready(&mut self, step: u64) -> Result<()> {
        let message = ACPMessage::step_complete(
            self.agent_status.agent_id.clone(),
            step,
        );

        info!("🐝 Signaling ready for step {}", step);

        self.agent.send_message(&message).await
            .context("Failed to signal step readiness")?;

        self.agent_status.step = step;
        Ok(())
    }

    /// Get current hive status showing all participating agents
    pub async fn get_hive_status(&mut self) -> Result<HiveStatus> {
        self.check_hive_status().await
    }

    /// Check hive status by listening for recent messages
    async fn check_hive_status(&mut self) -> Result<HiveStatus> {
        // Listen for recent messages from all agents in the hive
        let subject_pattern = format!("{}.acp.{}.>", self.mission.namespace, self.agent_status.step);
        
        debug!("🐝 Checking hive status with pattern: {}", subject_pattern);

        // For now, simulate hive status (stub implementation)
        // In production, this would query NATS for recent messages
        let mut updated_status = self.hive_status.clone();

        // Update our own agent status in the hive
        updated_status.agents.insert(
            self.agent_status.agent_id.clone(),
            self.agent_status.clone(),
        );

        // Simulate other agents for demo purposes
        if self.mission.expected_agents > 1 {
            for i in 1..=self.mission.expected_agents {
                let other_agent_id = format!("agent_{}", i);
                if other_agent_id != self.agent_status.agent_id {
                    let other_status = AgentStatus {
                        agent_id: other_agent_id.clone(),
                        step: self.agent_status.step,
                        status: "active".to_string(),
                        last_seen: chrono::Utc::now(),
                        role: "worker".to_string(),
                    };
                    updated_status.agents.insert(other_agent_id.clone(), other_status);
                    updated_status.agents_ready.push(other_agent_id);
                }
            }
        }

        // Check if step is complete (all expected agents are ready)
        updated_status.step_complete = updated_status.agents_ready.len() >= self.mission.expected_agents;

        self.hive_status = updated_status.clone();
        Ok(updated_status)
    }

    /// Create a new hive mission
    pub fn create_mission(
        mission_id: String,
        namespace: String,
        expected_agents: usize,
        description: String,
    ) -> HiveMission {
        HiveMission {
            mission_id,
            namespace,
            expected_agents,
            current_step: 1,
            timeout_seconds: 300, // 5 minutes default
            description,
        }
    }

    /// Join an existing hive mission
    pub async fn join_mission(
        agent_id: String,
        role: String,
        mission_id: String,
        namespace: String,
        nats_url: String,
    ) -> Result<Self> {
        // Create a mission config for joining
        let mission = HiveMission {
            mission_id,
            namespace,
            expected_agents: 3, // Default, will be updated from hive status
            current_step: 1,
            timeout_seconds: 300,
            description: format!("Joined mission"),
        };

        let mut client = Self::new(agent_id, role, mission, nats_url).await?;
        
        // Announce joining the mission
        client.send_status("joined_mission", None).await?;
        
        // Update mission details from hive
        client.check_hive_status().await?;

        info!("🐝 Agent {} joined mission {}", client.agent_status.agent_id, client.mission.mission_id);

        Ok(client)
    }

    /// Get agent ID
    pub fn agent_id(&self) -> &str {
        &self.agent_status.agent_id
    }

    /// Get mission information
    pub fn mission(&self) -> &HiveMission {
        &self.mission
    }

    /// Get current step
    pub fn current_step(&self) -> u64 {
        self.agent_status.step
    }
}

/// Helper functions for MCP tool integration
impl AcpHiveClient {
    /// Convert hive status to JSON for MCP responses
    pub fn hive_status_to_json(&self) -> Result<String> {
        serde_json::to_string_pretty(&self.hive_status)
            .context("Failed to serialize hive status")
    }

    /// Get agent summary for MCP display
    pub fn get_agent_summary(&self) -> String {
        format!(
            "Agent: {} | Mission: {} | Step: {} | Role: {} | Status: {}",
            self.agent_status.agent_id,
            self.mission.mission_id,
            self.agent_status.step,
            self.agent_status.role,
            self.agent_status.status
        )
    }

    /// Get hive participation summary
    pub fn get_hive_summary(&self) -> String {
        let active_agents = self.hive_status.agents.len();
        let ready_agents = self.hive_status.agents_ready.len();
        let expected_agents = self.mission.expected_agents;

        format!(
            "Mission: {} | Active: {}/{} agents | Ready: {} | Step: {} | Complete: {}",
            self.mission.mission_id,
            active_agents,
            expected_agents,
            ready_agents,
            self.hive_status.current_step,
            self.hive_status.step_complete
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mission_creation() {
        let mission = AcpHiveClient::create_mission(
            "test_mission".to_string(),
            "account.test".to_string(),
            3,
            "Test mission".to_string(),
        );

        assert_eq!(mission.mission_id, "test_mission");
        assert_eq!(mission.expected_agents, 3);
        assert_eq!(mission.current_step, 1);
    }

    #[test]
    fn test_agent_status_creation() {
        let status = AgentStatus {
            agent_id: "test_agent".to_string(),
            step: 1,
            status: "active".to_string(),
            last_seen: chrono::Utc::now(),
            role: "worker".to_string(),
        };

        assert_eq!(status.agent_id, "test_agent");
        assert_eq!(status.step, 1);
        assert_eq!(status.role, "worker");
    }

    // Note: Integration tests would require actual NATS server
    // For now, focusing on unit tests of data structures
}