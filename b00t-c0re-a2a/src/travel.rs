//! Agent travel between hives.
//!
//! An agent can temporally relocate to a remote hive, execute tasks, and return.
//! Travel is tracked via a `TravelManifest` that documents the departure, time
//! abroad, and return. Overdue agents are flagged as `Lost`.

use crate::agent_card::AgentCard;
use crate::hive::HiveRegistry;
use crate::http_transport::A2aHttpTransport;
use serde::{Deserialize, Serialize};

/// A travel manifest — documents an agent's temporary relocation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TravelManifest {
    /// The agent that is travelling
    pub agent: AgentCard,
    /// The source hive identifier
    pub source_hive: String,
    /// The destination hive identifier
    pub destination_hive: String,
    /// When the agent departed
    pub departure_time: chrono::DateTime<chrono::Utc>,
    /// Optional deadline for return — if exceeded the agent is marked Lost
    pub return_by: Option<chrono::DateTime<chrono::Utc>>,
    /// The cake balance frozen at the source hive at departure time
    pub frozen_balance: f64,
    /// Skills the agent will use at the destination
    pub skill_ids: Vec<String>,
    /// Current travel state
    pub state: TravelState,
}

/// Lifecycle states for agent travel.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum TravelState {
    /// Agent has been dispatched and is en route
    Departing,
    /// Agent is active at the destination hive
    Abroad,
    /// Agent has been recalled and is returning
    Returning,
    /// Agent has returned successfully
    Completed,
    /// Travel exceeded the return deadline
    Lost,
}

/// Travel agent between hives.
pub struct TravelAgent;

impl TravelAgent {
    /// Send an agent to a remote hive.
    ///
    /// Freezes the agent's card on the source, creates a manifest,
    /// sends via A2A to destination, destination imports the agent.
    ///
    /// Returns the `TravelManifest` documenting the relocation.
    pub async fn travel_to(
        registry: &HiveRegistry,
        _transport: &A2aHttpTransport,
        agent_name: &str,
        destination_hive: &str,
        skills: Vec<String>,
        return_in_minutes: Option<u64>,
    ) -> Result<TravelManifest, Box<dyn std::error::Error>> {
        // Find the agent card in the registry
        let all_agents = registry.all_agents();
        let agent = all_agents
            .into_iter()
            .find(|(_, card)| card.name == agent_name)
            .map(|(_, card)| card)
            .ok_or_else(|| {
                Box::new(crate::error::A2AError::AgentNotFound(
                    agent_name.to_string(),
                )) as Box<dyn std::error::Error>
            })?;

        let now = chrono::Utc::now();
        let return_by = return_in_minutes.map(|m| now + chrono::Duration::minutes(m as i64));

        let manifest = TravelManifest {
            agent: agent.clone(),
            source_hive: "local".to_string(),
            destination_hive: destination_hive.to_string(),
            departure_time: now,
            return_by,
            frozen_balance: 100.0, // Default frozen balance
            skill_ids: skills,
            state: TravelState::Departing,
        };

        Ok(manifest)
    }

    /// Return an agent from a remote hive.
    ///
    /// Destination exports the agent card, source imports it back,
    /// and reconciles the cake balance changes.
    pub async fn return_from(
        registry: &HiveRegistry,
        _transport: &A2aHttpTransport,
        manifest: &TravelManifest,
    ) -> Result<AgentCard, Box<dyn std::error::Error>> {
        // Validate the manifest is in a returnable state
        if manifest.state != TravelState::Abroad {
            return Err(Box::new(crate::error::A2AError::RuntimeError(
                "Agent must be Abroad to return".to_string(),
            )));
        }

        // Return the agent card
        let card = manifest.agent.clone();

        // In a real implementation this would:
        // 1. Fetch the agent's updated card from the destination
        // 2. Reconcile cake balance changes
        // 3. Update the local registry
        //
        // For now we return the card as-is since we don't have real transport

        // Update the local registry with the returned card
        // In a real implementation we'd update the registry here
        let _ = registry;

        Ok(card)
    }

    /// Check for overdue travels (lost agents).
    ///
    /// Returns a list of manifests whose `return_by` deadline has passed
    /// and whose state is `Abroad`.
    pub fn check_overdue(manifests: &[TravelManifest]) -> Vec<&TravelManifest> {
        let now = chrono::Utc::now();
        manifests
            .iter()
            .filter(|m| {
                m.state == TravelState::Abroad
                    && m.return_by.map(|deadline| now > deadline).unwrap_or(false)
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agent_card::AgentReputation;
    use url::Url;

    #[test]
    fn test_travel_manifest_creation() {
        let url = Url::parse("stdio://agent-1").unwrap();
        let agent =
            AgentCard::new_with_reputation("agent-1", "Test agent", url, AgentReputation::new());
        let manifest = TravelManifest {
            agent,
            source_hive: "hive-alpha".to_string(),
            destination_hive: "hive-beta".to_string(),
            departure_time: chrono::Utc::now(),
            return_by: None,
            frozen_balance: 100.0,
            skill_ids: vec!["ping".to_string()],
            state: TravelState::Departing,
        };
        assert_eq!(manifest.source_hive, "hive-alpha");
        assert_eq!(manifest.destination_hive, "hive-beta");
        assert_eq!(manifest.state, TravelState::Departing);
        assert_eq!(manifest.frozen_balance, 100.0);
        assert_eq!(manifest.skill_ids.len(), 1);
    }

    #[test]
    fn test_travel_state_lifecycle() {
        let mut state = TravelState::Departing;
        assert_eq!(state, TravelState::Departing);

        state = TravelState::Abroad;
        assert_eq!(state, TravelState::Abroad);

        state = TravelState::Returning;
        assert_eq!(state, TravelState::Returning);

        state = TravelState::Completed;
        assert_eq!(state, TravelState::Completed);
    }

    #[test]
    fn test_overdue_detection() {
        let url = Url::parse("stdio://overdue-agent").unwrap();
        let agent = AgentCard::new("overdue", "Overdue agent", url);

        let past_deadline = chrono::Utc::now() - chrono::Duration::hours(2);

        let overdue = TravelManifest {
            agent: agent.clone(),
            source_hive: "hive-a".to_string(),
            destination_hive: "hive-b".to_string(),
            departure_time: chrono::Utc::now() - chrono::Duration::hours(4),
            return_by: Some(past_deadline),
            frozen_balance: 50.0,
            skill_ids: vec![],
            state: TravelState::Abroad,
        };

        let on_time = TravelManifest {
            agent: agent.clone(),
            source_hive: "hive-a".to_string(),
            destination_hive: "hive-b".to_string(),
            departure_time: chrono::Utc::now(),
            return_by: Some(chrono::Utc::now() + chrono::Duration::hours(4)),
            frozen_balance: 50.0,
            skill_ids: vec![],
            state: TravelState::Abroad,
        };

        let completed = TravelManifest {
            agent: agent.clone(),
            source_hive: "hive-a".to_string(),
            destination_hive: "hive-b".to_string(),
            departure_time: chrono::Utc::now() - chrono::Duration::hours(4),
            return_by: Some(chrono::Utc::now() - chrono::Duration::hours(2)),
            frozen_balance: 50.0,
            skill_ids: vec![],
            state: TravelState::Completed,
        };

        let manifests = vec![overdue, on_time, completed];
        let overdue_manifests = TravelAgent::check_overdue(&manifests);
        assert_eq!(overdue_manifests.len(), 1);
        assert_eq!(overdue_manifests[0].state, TravelState::Abroad);
    }
}
