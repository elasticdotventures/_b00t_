#[cfg(test)]
mod k0mmand3r_guard_tests {
    use super::super::*;

    /// Test 1: Parse k0mmand3r guard from step TOML
    #[test]
    fn test_parse_k0mmand3r_guard() {
        let guard_str = "/negotiate blessing:observe-infrastructure";

        let cmd = K0mmand::parse(guard_str)
            .expect("Should parse k0mmand3r command");

        assert_eq!(cmd.verb, "negotiate");
        assert_eq!(cmd.object, "blessing:observe-infrastructure");
    }

    /// Test 2: Validate guard syntax
    #[test]
    fn test_validate_k0mmand3r_syntax() {
        let valid = vec![
            "/negotiate blessing:observe-infrastructure",
            "/vote on blessing:execute-transition-safely",
            "/delegate step:apply-transitions to agent:b00t-sandbox",
            "/status from agent:executive",
        ];

        for cmd in valid {
            assert!(K0mmand::parse(cmd).is_ok(), "Should parse: {}", cmd);
        }
    }

    /// Test 3: Evaluate guard condition
    #[test]
    fn test_evaluate_guard_condition() {
        let guard = GuardCondition {
            requires: vec!["/negotiate blessing:observe-infrastructure".to_string()],
            expression: "has_blessing(blessing:observe-infrastructure)".to_string(),
        };

        // Mock context: agent HAS the blessing
        let context = EvaluationContext {
            agent_blessings: vec!["blessing:observe-infrastructure".to_string()],
            available_budget: 0,
            votes: vec![],
            authorized: false,
        };

        let result = guard.evaluate(&context)
            .expect("Should evaluate guard");

        assert!(result, "Guard should be true when blessing is present");

        // Now remove the blessing
        let context_no_blessing = EvaluationContext {
            agent_blessings: vec![],
            available_budget: 0,
            votes: vec![],
            authorized: false,
        };
        let result = guard.evaluate(&context_no_blessing)
            .expect("Should evaluate guard");

        assert!(!result, "Guard should be false when blessing is absent");
    }

    /// Test 4: Guard with k0mmand3r execution (mocked)
    #[test]
    fn test_guard_executes_k0mmand3r_commands() {
        let guard = GuardCondition {
            requires: vec!["/negotiate blessing:b00t-sandbox budget:3000".to_string()],
            expression: "budget_available(3000)".to_string(),
        };

        let mut context = EvaluationContext {
            available_budget: 5000,
            agent_blessings: vec![],
            ..Default::default()
        };

        // Should succeed: budget available
        let result = guard.evaluate(&context).expect("Should evaluate");
        assert!(result);

        // Not enough budget
        context.available_budget = 2000;
        let result = guard.evaluate(&context).expect("Should evaluate");
        assert!(!result);
    }

    /// Test 5: Multiple guards (AND logic)
    #[test]
    fn test_multiple_guards_and_logic() {
        let guards = vec![
            GuardCondition {
                requires: vec![],
                expression: "has_blessing(blessing:observe-infrastructure)".to_string(),
            },
            GuardCondition {
                requires: vec![],
                expression: "budget_available(3000)".to_string(),
            },
        ];

        let context = EvaluationContext {
            agent_blessings: vec!["blessing:observe-infrastructure".to_string()],
            available_budget: 5000,
            votes: vec![],
            authorized: false,
        };

        for (i, guard) in guards.iter().enumerate() {
            let result = guard.evaluate(&context).expect(&format!("Should evaluate guard {}", i));
            assert!(result, "Guard {} should be true", i);
        }
    }

    /// Test 6: Guard fails gracefully with clear error
    #[test]
    fn test_guard_failure_message() {
        let guard = GuardCondition {
            requires: vec![
                "/negotiate blessing:b00t-sandbox budget:10000".to_string(),
            ],
            expression: "budget_available(10000)".to_string(),
        };

        let context = EvaluationContext {
            available_budget: 5000,
            agent_blessings: vec![],
            ..Default::default()
        };

        let result = guard.evaluate(&context);

        match result {
            Ok(false) => {
                // Good: guard returned false
            }
            Ok(true) => panic!("Guard should be false"),
            Err(e) => {
                // Also acceptable: clear error message
                assert!(e.contains("budget"), "Error should mention budget");
            }
        }
    }

    /// Test 7: Guard with voting
    #[test]
    fn test_guard_voting_quorum() {
        let guard = GuardCondition {
            requires: vec!["/vote on blessing:execute-transition-safely".to_string()],
            expression: "voted_yes(blessing:execute-transition-safely)".to_string(),
        };

        let context = EvaluationContext {
            votes: vec![
                ("executive".to_string(), "yes".to_string()),
                ("observer".to_string(), "yes".to_string()),
            ],
            ..Default::default()
        };

        let result = guard.evaluate(&context).expect("Should evaluate");
        assert!(result);
    }

    /// Test 8: Serialize/deserialize guard
    #[test]
    fn test_serialize_guard_condition() {
        let guard = GuardCondition {
            requires: vec!["/negotiate blessing:observe-infrastructure".to_string()],
            expression: "has_blessing(blessing:observe-infrastructure)".to_string(),
        };

        let json = serde_json::to_string(&guard).expect("Should serialize");
        let deserialized: GuardCondition = serde_json::from_str(&json)
            .expect("Should deserialize");

        assert_eq!(guard, deserialized);
    }
}
