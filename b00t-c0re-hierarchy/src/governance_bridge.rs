//! Bridge between the hierarchy crate and b00t-c0re-gov.
//! When a mission completes, this module scores the result and
//! calculates the cake payout via the governance epoch3 engine.

use crate::roles::{Agent, MissionTopic};
use b00t_c0re_gov::epoch3::{MissionResult, calculate_cake_payout};
use b00t_c0re_gov::types::ScoreCard;

/// Default penalty rate for calorie over-budget (10% per excess calorie).
const DEFAULT_PENALTY_RATE: f64 = 0.1;

/// Complete a mission with scoring, returning the cake payout amount.
///
/// # Arguments
/// * `mission` - The completed mission (its bounty feeds the base payout).
/// * `agent` - The agent who completed the mission.
/// * `calories_burned` - Total calories consumed during the mission.
/// * `score` - Multi-dimensional score card assessing performance.
/// * `budget_calories` - Allowed calorie budget; exceeding this incurs a penalty.
///
/// # Returns
/// The cake payout amount (>= 0.0), floored at zero.
pub fn complete_mission_with_scoring(
    mission: &MissionTopic,
    agent: &Agent,
    calories_burned: f64,
    score: ScoreCard,
    budget_calories: f64,
) -> f64 {
    let result = MissionResult {
        mission_id: mission.id.clone(),
        agent_id: agent.id.clone(),
        bounty: mission.bounty,
        score,
        calories_burned,
        completed_at: chrono::Utc::now(),
    };

    calculate_cake_payout(&result, budget_calories, DEFAULT_PENALTY_RATE)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::roles::{Agent, MissionTopic, Role, TopicStatus};

    fn make_agent(id: &str) -> Agent {
        Agent {
            id: id.to_string(),
            role: Role::Specialist,
            skills: vec!["rust".to_string(), "governance".to_string()],
            cake_balance: 50.0,
            is_alive: true,
            manager_id: None,
            is_player: false,
        }
    }

    fn make_mission(id: &str, bounty: f64) -> MissionTopic {
        MissionTopic {
            id: id.to_string(),
            bounty,
            description: "Test mission for governance bridge".to_string(),
            required_skills: vec!["rust".to_string()],
            status: TopicStatus::Completed,
        }
    }

    #[test]
    fn test_complete_mission_with_scoring_returns_positive_payout() {
        let mission = make_mission("bridge-test-1", 100.0);
        let agent = make_agent("agent-alpha");
        let score = ScoreCard::new(0.9, 0.8, 0.7, 1.0, 0.6, 0.5);

        let payout = complete_mission_with_scoring(
            &mission, &agent, 50.0, // calories burned (under budget)
            score, 100.0, // calorie budget
        );

        assert!(
            payout > 0.0,
            "Payout should be positive for good performance, got {}",
            payout
        );
        assert!(
            payout <= mission.bounty,
            "Payout {} should not exceed bounty {}",
            payout,
            mission.bounty
        );
    }

    #[test]
    fn test_complete_mission_with_scoring_perfect_score_no_penalty() {
        let mission = make_mission("bridge-test-2", 200.0);
        let agent = make_agent("agent-beta");
        // Perfect score in every dimension
        let score = ScoreCard::new(1.0, 1.0, 1.0, 1.0, 1.0, 1.0);

        let payout = complete_mission_with_scoring(
            &mission, &agent, 30.0, // well under budget
            score, 100.0, // generous budget
        );

        // Perfect score * bounty = 200.0, no penalty since under budget
        let expected = 200.0;
        assert!(
            (payout - expected).abs() < f64::EPSILON,
            "Expected payout {:.2}, got {:.2}",
            expected,
            payout
        );
    }

    #[test]
    fn test_complete_mission_with_scoring_penalty_reduces_payout() {
        let mission = make_mission("bridge-test-3", 100.0);
        let agent = make_agent("agent-gamma");
        let score = ScoreCard::new(1.0, 1.0, 1.0, 1.0, 1.0, 1.0);

        let payout = complete_mission_with_scoring(
            &mission, &agent, 200.0, // double the budget
            score, 100.0, // budget = 100
        );

        // Perfect score = 100.0 base
        // Over-budget = 100.0, penalty = 100.0 * 0.1 = 10.0
        // Payout = 100.0 - 10.0 = 90.0
        let expected = 90.0;
        assert!(
            (payout - expected).abs() < f64::EPSILON,
            "Expected payout {:.2}, got {:.2}",
            expected,
            payout
        );
    }

    #[test]
    fn test_complete_mission_with_scoring_agent_is_player_field() {
        let mission = make_mission("bridge-test-4", 75.0);
        let mut agent = make_agent("player-1");
        agent.is_player = true; // this agent represents a human

        let score = ScoreCard::new(0.6, 0.5, 0.4, 0.7, 0.5, 0.3);

        let payout = complete_mission_with_scoring(&mission, &agent, 40.0, score, 60.0);

        assert!(payout >= 0.0, "Payout must be >= 0, got {}", payout);
        // Verify the agent field is correctly accessible in the bridge
        assert!(agent.is_player, "Agent should be marked as player");
    }

    #[test]
    fn test_complete_mission_with_scoring_zero_score_yields_zero_payout() {
        let mission = make_mission("bridge-test-5", 100.0);
        let agent = make_agent("agent-delta");
        // Zero score in all dimensions
        let score = ScoreCard::new(0.0, 0.0, 0.0, 0.0, 0.0, 0.0);

        let payout = complete_mission_with_scoring(&mission, &agent, 10.0, score, 100.0);

        assert!(
            (payout - 0.0).abs() < f64::EPSILON,
            "Zero score should yield zero payout, got {}",
            payout
        );
    }
}
