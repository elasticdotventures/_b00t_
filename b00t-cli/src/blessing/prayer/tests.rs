#[cfg(test)]
mod prayer_tests {
    use super::super::*;
    use crate::blessing::{BlessingGraph, BlessingNode, LayerMetadata};
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
        assert_eq!(
            result.blessing.unwrap().id,
            "blessing:observe-infrastructure"
        );
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
            available_budget: 100, // Need 500
            executive_override: false,
        };

        let result = evaluator.evaluate_prayer(&request);

        assert!(!result.granted);
        assert!(result.denial_reason.is_some());
        assert!(
            result
                .denial_reason
                .unwrap()
                .contains("Budget insufficient")
        );
    }

    /// Test 4: Agent prayer denied - missing prerequisite
    #[test]
    fn test_prayer_denied_missing_prerequisite() {
        let evaluator = BlessingEvaluator::new(sample_blessing_graph(), sample_policy());

        let request = BlessingRequest {
            blessing_id: "blessing:terraform-apply".to_string(),
            agent_role: "executor".to_string(),
            agent_blessings: vec![], // Missing observe-infrastructure
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
            executive_override: true, // Executive says yes
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

    /// Test 9: CompositionPlan creation
    #[test]
    fn test_composition_plan_creation() {
        let layer1 = LayerMetadata {
            blessing_id: "blessing:terraform-apply".to_string(),
            artifact_path: std::path::PathBuf::from("/cache/layer1.gguf"),
            embedding_dim: 768,
            adapter_rank: 16,
            generated_at: chrono::Utc::now(),
            quality_score: 0.95,
        };

        let layer2 = LayerMetadata {
            blessing_id: "blessing:observe-infrastructure".to_string(),
            artifact_path: std::path::PathBuf::from("/cache/layer2.gguf"),
            embedding_dim: 768,
            adapter_rank: 8,
            generated_at: chrono::Utc::now(),
            quality_score: 0.90,
        };

        let plan = CompositionPlan {
            base_model_id: "meta-llama/Llama-2-7b".to_string(),
            layers: vec![layer1, layer2],
            total_adapter_params: 147456, // 768*16*12 + 768*8*12
        };

        assert_eq!(plan.base_model_id, "meta-llama/Llama-2-7b");
        assert_eq!(plan.layers.len(), 2);
        assert_eq!(plan.total_adapter_params, 147456);
    }

    /// Test 10: CompositionPlan token estimation
    #[test]
    fn test_composition_plan_token_estimation() {
        let layer = LayerMetadata {
            blessing_id: "blessing:terraform-apply".to_string(),
            artifact_path: std::path::PathBuf::from("/cache/layer1.gguf"),
            embedding_dim: 768,
            adapter_rank: 16,
            generated_at: chrono::Utc::now(),
            quality_score: 0.95,
        };

        let plan = CompositionPlan {
            base_model_id: "meta-llama/Llama-2-7b".to_string(),
            layers: vec![layer],
            total_adapter_params: 147456,
        };

        let tokens = plan.estimated_tokens_per_inference();
        // Base model overhead (2048) + adapter overhead (768 * 16 / 1000 ≈ 12)
        assert!(tokens > 2000);
        assert!(tokens < 3000);
    }

    /// Test 11: CompositionPlan memory budget check
    #[test]
    fn test_composition_plan_memory_budget() {
        let layer = LayerMetadata {
            blessing_id: "blessing:terraform-apply".to_string(),
            artifact_path: std::path::PathBuf::from("/cache/layer1.gguf"),
            embedding_dim: 768,
            adapter_rank: 16,
            generated_at: chrono::Utc::now(),
            quality_score: 0.95,
        };

        let plan = CompositionPlan {
            base_model_id: "meta-llama/Llama-2-7b".to_string(),
            layers: vec![layer],
            total_adapter_params: 147456,
        };

        // Should fit in generous budget
        assert!(plan.fits_in_budget(16000));

        // Should not fit in tiny budget
        assert!(!plan.fits_in_budget(100));
    }

    /// Test 12: BlessingPrayerResult with composition plan
    #[test]
    fn test_blessing_prayer_result_with_composition() {
        let layer = LayerMetadata {
            blessing_id: "blessing:terraform-apply".to_string(),
            artifact_path: std::path::PathBuf::from("/cache/layer1.gguf"),
            embedding_dim: 768,
            adapter_rank: 16,
            generated_at: chrono::Utc::now(),
            quality_score: 0.95,
        };

        let plan = CompositionPlan {
            base_model_id: "meta-llama/Llama-2-7b".to_string(),
            layers: vec![layer],
            total_adapter_params: 147456,
        };

        let result = BlessingPrayerResult {
            granted: true,
            blessing: Some(BlessingNode {
                id: "blessing:terraform-apply".to_string(),
                type_: "blessing".to_string(),
                cost_tokens: 500,
                ..Default::default()
            }),
            denial_reason: None,
            suggestions: vec![],
            composition_plan: Some(plan),
        };

        assert!(result.granted);
        assert!(result.composition_plan.is_some());
        let cp = result.composition_plan.unwrap();
        assert_eq!(cp.base_model_id, "meta-llama/Llama-2-7b");
        assert_eq!(cp.layers.len(), 1);
    }

    /// Test 13: Denial audit event hook stub (Phase 8)
    #[test]
    fn test_denial_audit_event_hook_stub() {
        // 🦨 Phase 8: Placeholder for denial_audit integration
        // Will be implemented in Phase 8 when orchestrator state machine is wired
        let denial_reason = "Role 'observer' cannot request terraform blessing";
        let agent_role = "observer";

        // Verify we can construct the audit data
        assert!(!denial_reason.is_empty());
        assert!(!agent_role.is_empty());

        // Real implementation will emit this to Kaizen loop
        // event: AuditEventEmitter::emit_denial_audit(blessing_id, reason, agent_role)
    }

    /// Test 14: CompositionValidation trait stub (Phase 8)
    #[test]
    fn test_composition_validation_trait_stub() {
        // 🦨 Phase 8: Placeholder for composition validation
        // CompositionValidation trait will validate & checkpoint
        // actual implementations deferred to Phase 8

        let layer = LayerMetadata {
            blessing_id: "blessing:terraform-apply".to_string(),
            artifact_path: std::path::PathBuf::from("/cache/layer1.gguf"),
            embedding_dim: 768,
            adapter_rank: 16,
            generated_at: chrono::Utc::now(),
            quality_score: 0.95,
        };

        let plan = CompositionPlan {
            base_model_id: "meta-llama/Llama-2-7b".to_string(),
            layers: vec![layer],
            total_adapter_params: 147456,
        };

        // Verify plan is constructable and has necessary fields for Phase 8
        assert!(!plan.base_model_id.is_empty());
        assert!(!plan.layers.is_empty());
        assert_eq!(plan.total_adapter_params, 147456);
    }
}
