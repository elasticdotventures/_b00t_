//! Epoch 3: Retrospective cake issuance.
//! Cake is earned when a mission completes, not during execution.

use crate::types::*;

/// Result of a completed mission.
pub struct MissionResult {
    pub mission_id: String,
    pub agent_id: String,
    pub bounty: f64,          // Base cake bounty
    pub score: ScoreCard,     // Multi-dimensional scoring
    pub calories_burned: f64, // Total calories consumed
    pub completed_at: chrono::DateTime<chrono::Utc>,
}

/// Calculate cake payout for a mission result.
/// Cake = bounty * weighted_score - penalty
/// Penalty = max(0, (calories_burned - budget) * penalty_rate)
pub fn calculate_cake_payout(
    result: &MissionResult,
    budget_calories: f64,
    penalty_rate: f64,
) -> f64 {
    let base = result.score.cake_payout(result.bounty);
    let over_budget = (result.calories_burned - budget_calories).max(0.0);
    let penalty = over_budget * penalty_rate;
    (base - penalty).max(0.0) // Floor at 0 — no negative cake
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;

    fn make_result(bounty: f64, score: ScoreCard, calories_burned: f64) -> MissionResult {
        MissionResult {
            mission_id: "test-mission-1".to_string(),
            agent_id: "agent-1".to_string(),
            bounty,
            score,
            calories_burned,
            completed_at: Utc::now(),
        }
    }

    #[test]
    fn test_cake_payout_no_penalty() {
        let card = ScoreCard::new(1.0, 1.0, 1.0, 1.0, 1.0, 1.0);
        let result = make_result(100.0, card, 50.0);
        // weighted_score = 1.0, base = 100.0
        // budget = 100, burned = 50, no penalty
        let payout = calculate_cake_payout(&result, 100.0, 0.5);
        assert!((payout - 100.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_cake_payout_with_penalty() {
        let card = ScoreCard::new(1.0, 1.0, 1.0, 1.0, 1.0, 1.0);
        let result = make_result(100.0, card, 200.0);
        // base = 100.0, over_budget = 100.0, penalty = 100.0 * 0.5 = 50.0
        // payout = 100.0 - 50.0 = 50.0
        let payout = calculate_cake_payout(&result, 100.0, 0.5);
        assert!((payout - 50.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_cake_payout_floor_at_zero() {
        let card = ScoreCard::new(0.0, 0.0, 0.0, 0.0, 0.0, 0.0);
        let result = make_result(100.0, card, 200.0);
        // base = 0.0, over_budget = 100.0, penalty = 100.0 * 0.5 = 50.0
        // payout = 0.0 - 50.0 = -50.0, floored to 0.0
        let payout = calculate_cake_payout(&result, 100.0, 0.5);
        assert!((payout - 0.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_cake_payout_partial_score_with_penalty() {
        let card = ScoreCard::new(0.5, 0.5, 0.5, 0.5, 0.5, 0.5);
        let result = make_result(100.0, card, 150.0);
        // weighted_score = 0.5, base = 50.0
        // over_budget = 50.0, penalty = 50.0 * 0.5 = 25.0
        // payout = 50.0 - 25.0 = 25.0
        let payout = calculate_cake_payout(&result, 100.0, 0.5);
        assert!((payout - 25.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_cake_payout_zero_budget() {
        let card = ScoreCard::new(1.0, 1.0, 1.0, 1.0, 1.0, 1.0);
        let result = make_result(100.0, card, 50.0);
        // base = 100.0, over_budget = 50.0, penalty = 50.0 * 1.0 = 50.0
        // payout = 100.0 - 50.0 = 50.0
        let payout = calculate_cake_payout(&result, 0.0, 1.0);
        assert!((payout - 50.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_cake_payout_no_penalty_when_under_budget() {
        let card = ScoreCard::new(0.8, 0.9, 0.7, 1.0, 0.6, 0.5);
        let result = make_result(200.0, card, 30.0);
        // budget = 100, burned = 30, no penalty
        // weighted_score = (0.8*1.0 + 0.9*1.0 + 0.7*0.8 + 1.0*0.9 + 0.6*0.7 + 0.5*0.6) / 5.0
        // = (0.8 + 0.9 + 0.56 + 0.9 + 0.42 + 0.3) / 5.0 = 3.88 / 5.0 = 0.776
        // payout = 0.776 * 200.0 = 155.2
        let payout = calculate_cake_payout(&result, 100.0, 0.5);
        let expected_base = result.score.cake_payout(200.0);
        assert!((payout - expected_base).abs() < f64::EPSILON);
    }
}
