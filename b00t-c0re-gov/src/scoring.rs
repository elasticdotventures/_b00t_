//! Calorie and Cake accounting — the economic engine.

/// Agent tier determines calorie burn rate per operation.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum AgentTier {
    GAI,       // 100x  — GPT-4, Claude Opus
    LLM,       // 10x   — GPT-4o-mini, Llama 3
    SLM,       // 1x    — Phi-4, Qwen 2.5
    Algorithmic, // 0.01x — Python script, grep, awk
}

impl AgentTier {
    pub fn calorie_multiplier(&self) -> f64 {
        match self {
            AgentTier::GAI => 100.0,
            AgentTier::LLM => 10.0,
            AgentTier::SLM => 1.0,
            AgentTier::Algorithmic => 0.01,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{ScoreCard, CalorieBalance};

    #[test]
    fn test_agent_tier_multipliers() {
        assert_eq!(AgentTier::GAI.calorie_multiplier(), 100.0);
        assert_eq!(AgentTier::LLM.calorie_multiplier(), 10.0);
        assert_eq!(AgentTier::SLM.calorie_multiplier(), 1.0);
        assert!(AgentTier::Algorithmic.calorie_multiplier() - 0.01 < f64::EPSILON);
    }

    #[test]
    fn test_weighted_score_calculation() {
        let card = ScoreCard::new(1.0, 1.0, 1.0, 1.0, 1.0, 1.0);
        // Default weights: roi=1.0, cost=1.0, time=0.8, accuracy=0.9, utility=0.7, risk=0.6
        // Sum = 5.0, weighted = (1.0*1.0 + 1.0*1.0 + 1.0*0.8 + 1.0*0.9 + 1.0*0.7 + 1.0*0.6) / 5.0 = 5.0/5.0 = 1.0
        assert!((card.weighted_score() - 1.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_weighted_score_custom_weights() {
        let card = ScoreCard::new(0.5, 0.5, 0.5, 0.5, 0.5, 0.5);
        let weights = [2.0, 2.0, 2.0, 2.0, 2.0, 2.0];
        // weighted = (0.5*2.0 + 0.5*2.0 + 0.5*2.0 + 0.5*2.0 + 0.5*2.0 + 0.5*2.0) / 12.0 = 6.0/12.0 = 0.5
        assert!((card.weighted_score_with(&weights) - 0.5).abs() < f64::EPSILON);
    }

    #[test]
    fn test_weighted_score_partial_credit() {
        let card = ScoreCard::new(0.0, 1.0, 0.8, 1.0, 0.5, 1.0);
        let score = card.weighted_score();
        assert!(score >= 0.0 && score <= 1.0);
    }

    #[test]
    fn test_cake_payout_no_penalty() {
        let card = ScoreCard::new(1.0, 1.0, 1.0, 1.0, 1.0, 1.0);
        // Score = 1.0, base_bounty = 100.0 => payout = 100.0
        assert!((card.cake_payout(100.0) - 100.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_cake_payout_partial_score() {
        let card = ScoreCard::new(0.5, 0.5, 0.5, 0.5, 0.5, 0.5);
        // weighted_score = (0.5*1.0 + 0.5*1.0 + 0.5*0.8 + 0.5*0.9 + 0.5*0.7 + 0.5*0.6) / 5.0
        // = (0.5 + 0.5 + 0.4 + 0.45 + 0.35 + 0.3) / 5.0 = 2.5 / 5.0 = 0.5
        // payout = 0.5 * 100.0 = 50.0
        assert!((card.cake_payout(100.0) - 50.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_calorie_balance_new() {
        let bal = CalorieBalance::new(1000.0);
        assert!((bal.current - 1000.0).abs() < f64::EPSILON);
        assert!((bal.total_burned - 0.0).abs() < f64::EPSILON);
        assert!((bal.total_earned - 1000.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_calorie_burn_sufficient() {
        let mut bal = CalorieBalance::new(100.0);
        let result = bal.burn(30.0);
        assert!(result);
        assert!((bal.current - 70.0).abs() < f64::EPSILON);
        assert!((bal.total_burned - 30.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_calorie_burn_exact() {
        let mut bal = CalorieBalance::new(50.0);
        let result = bal.burn(50.0);
        assert!(result);
        assert!((bal.current - 0.0).abs() < f64::EPSILON);
        assert!(bal.is_dead());
    }

    #[test]
    fn test_calorie_burn_insufficient() {
        let mut bal = CalorieBalance::new(10.0);
        let result = bal.burn(20.0);
        assert!(!result);
        assert!((bal.current - 10.0).abs() < f64::EPSILON);
        assert!(!bal.is_dead());
    }

    #[test]
    fn test_calorie_burn_death() {
        let mut bal = CalorieBalance::new(100.0);
        bal.burn(100.0);
        assert!(bal.is_dead());
        assert!((bal.current - 0.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_calorie_earn() {
        let mut bal = CalorieBalance::new(50.0);
        bal.earn(30.0);
        assert!((bal.current - 80.0).abs() < f64::EPSILON);
        assert!((bal.total_earned - 80.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_calorie_earn_revives() {
        let mut bal = CalorieBalance::new(0.0);
        assert!(bal.is_dead());
        bal.earn(10.0);
        assert!(!bal.is_dead());
    }
}
