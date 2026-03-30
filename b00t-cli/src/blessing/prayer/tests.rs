#[cfg(test)]
mod prayer_tests {
    use super::super::*;
    use crate::blessing::{BlessingGraph, BlessingNode, BlessingEdge};
    use std::collections::{BTreeMap, HashSet};

    fn sample_blessing_graph() -> BlessingGraph {
        BlessingGraph {
            nodes: vec![
                BlessingNode {
                    id: "blessing:observe-infrastructure".to_string(),
                    type_: "blessing".to_string(),
                    cost_tokens: 100,
                    role_access: vec!["observer".to_string()],
                    ..Default::default()
                },
                BlessingNode {
                    id: "blessing:terraform-apply".to_string(),
                    type_: "blessing".to_string(),
                    cost_tokens: 500,
                    role_access: vec!["executor".to_string()],
                    requires: vec!["blessing:observe-infrastructure".to_string()],
                    ..Default::default()
                },
                BlessingNode {
                    id: "blessing:prod-deploy".to_string(),
                    type_: "blessing".to_string(),
                    cost_tokens: 1000,
                    role_access: vec!["executor".to_string()],
                    ..Default::default()
                },
            ],
            edges: vec![],
        }
    }

    fn sample_policy() -> BlessingPolicy {
        let mut role_blessings: BTreeMap<String, HashSet<String>> = BTreeMap::new();

        let mut observer_blessings = HashSet::new();
        observer_blessings.insert("blessing:observe-infrastructure".to_string());
        role_blessings.insert("observer".to_string(), observer_blessings);

        let mut executor_blessings = HashSet::new();
        executor_blessings.insert("blessing:observe-infrastructure".to_string());
        executor_blessings.insert("blessing:terraform-apply".to_string());
        executor_blessings.insert("blessing:prod-deploy".to_string());
        role_blessings.insert("executor".to_string(), executor_blessings);

        let mut requires_voting = HashSet::new();
        requires_voting.insert("blessing:prod-deploy".to_string());

        BlessingPolicy {
            role_blessings,
            global_daily_budget: 10000,
            requires_voting,
        }
    }

    /// Test 1: Agent prayer approved - observer reads infrastructure
    #[test]
    fn test_prayer_approved_observer_observe() {
        let evaluator = BlessingEvaluator::new(sample_blessing_graph(), sample_policy());

        let request = BlessingRequest {
            blessing_id: "blessing:observe-infrastructure".to_string(),
            agent_role: "observer".to_string(),
            agent_blessings: vec![],
            available_budget: 1000,
            executive_override: false,
        };

        let result = evaluator.evaluate_prayer(&request);

        assert!(result.granted);
        assert!(result.blessing.is_some());
        assert_eq!(result.blessing.unwrap().id, "blessing:observe-infrastructure");
        assert!(result.denial_reason.is_none());
    }

    /// Test 2: Agent prayer denied - observer tries to terraform
    #[test]
    fn test_prayer_denied_role_not_allowed() {
        let evaluator = BlessingEvaluator::new(sample_blessing_graph(), sample_policy());

        let request = BlessingRequest {
            blessing_id: "blessing:terraform-apply".to_string(),
            agent_role: "observer".to_string(),
            agent_blessings: vec![],
            available_budget: 1000,
            executive_override: false,
        };

        let result = evaluator.evaluate_prayer(&request);

        assert!(!result.granted);
        assert!(result.blessing.is_none());
        assert!(result.denial_reason.is_some());
        assert!(result.suggestions.len() > 0);
    }

    /// Test 3: Agent prayer denied - missing budget
    #[test]
    fn test_prayer_denied_budget_insufficient() {
        let evaluator = BlessingEvaluator::new(sample_blessing_graph(), sample_policy());

        let request = BlessingRequest {
            blessing_id: "blessing:terraform-apply".to_string(),
            agent_role: "executor".to_string(),
            agent_blessings: vec!["blessing:observe-infrastructure".to_string()],
            available_budget: 100,  // Need 500
            executive_override: false,
        };

        let result = evaluator.evaluate_prayer(&request);

        assert!(!result.granted);
        assert!(result.denial_reason.is_some());
        assert!(result.denial_reason.unwrap().contains("Budget insufficient"));
    }

    /// Test 4: Agent prayer denied - missing prerequisite
    #[test]
    fn test_prayer_denied_missing_prerequisite() {
        let evaluator = BlessingEvaluator::new(sample_blessing_graph(), sample_policy());

        let request = BlessingRequest {
            blessing_id: "blessing:terraform-apply".to_string(),
            agent_role: "executor".to_string(),
            agent_blessings: vec![],  // Missing observe-infrastructure
            available_budget: 1000,
            executive_override: false,
        };

        let result = evaluator.evaluate_prayer(&request);

        assert!(!result.granted);
        assert!(result.denial_reason.is_some());
        assert!(result.denial_reason.unwrap().contains("prerequisite"));
    }

    /// Test 5: Agent prayer denied - requires voting
    #[test]
    fn test_prayer_denied_requires_voting() {
        let evaluator = BlessingEvaluator::new(sample_blessing_graph(), sample_policy());

        let request = BlessingRequest {
            blessing_id: "blessing:prod-deploy".to_string(),
            agent_role: "executor".to_string(),
            agent_blessings: vec![],
            available_budget: 10000,
            executive_override: false,
        };

        let result = evaluator.evaluate_prayer(&request);

        assert!(!result.granted);
        assert!(result.denial_reason.is_some());
        let denial = result.denial_reason.unwrap();
        assert!(denial.to_lowercase().contains("voting"));
    }

    /// Test 6: Executive override - bypass policy checks
    #[test]
    fn test_prayer_approved_executive_override() {
        let evaluator = BlessingEvaluator::new(sample_blessing_graph(), sample_policy());

        let request = BlessingRequest {
            blessing_id: "blessing:prod-deploy".to_string(),
            agent_role: "executor".to_string(),
            agent_blessings: vec![],
            available_budget: 10000,
            executive_override: true,  // Executive says yes
        };

        let result = evaluator.evaluate_prayer(&request);

        assert!(result.granted);
        assert!(result.blessing.is_some());
    }

    /// Test 7: Blessing not found
    #[test]
    fn test_prayer_denied_blessing_not_found() {
        let evaluator = BlessingEvaluator::new(sample_blessing_graph(), sample_policy());

        let request = BlessingRequest {
            blessing_id: "blessing:nonexistent".to_string(),
            agent_role: "executor".to_string(),
            agent_blessings: vec![],
            available_budget: 10000,
            executive_override: false,
        };

        let result = evaluator.evaluate_prayer(&request);

        assert!(!result.granted);
        assert!(result.denial_reason.is_some());
        assert!(result.denial_reason.unwrap().contains("does not exist"));
        assert!(result.suggestions.len() > 0);
    }

    /// Test 8: Policy check helper functions
    #[test]
    fn test_policy_check_result_helper() {
        let check = PolicyCheckResult::Approved;
        assert!(check.is_approved());
        assert!(check.denial_reason().is_none());

        let check2 = PolicyCheckResult::DeniedBlessingMissing("blessing:x".to_string());
        assert!(!check2.is_approved());
        assert!(check2.denial_reason().is_some());
    }
}
