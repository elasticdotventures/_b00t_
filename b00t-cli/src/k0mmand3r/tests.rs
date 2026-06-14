#[cfg(test)]
mod k0mmand3r_guard_tests {
    use super::super::*;

    /// Test 1: Parse k0mmand3r guard from step TOML
    #[test]
    fn test_parse_k0mmand3r_guard() {
        let guard_str = "/negotiate blessing:observe-infrastructure";

        let cmd = K0mmand::parse(guard_str).expect("Should parse k0mmand3r command");

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

        let result = guard.evaluate(&context).expect("Should evaluate guard");

        assert!(result, "Guard should be true when blessing is present");

        // Now remove the blessing
        let context_no_blessing = EvaluationContext {
            agent_blessings: vec![],
            available_budget: 0,
            votes: vec![],
            authorized: false,
        };
        let result = guard
            .evaluate(&context_no_blessing)
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
            let result = guard
                .evaluate(&context)
                .expect(&format!("Should evaluate guard {}", i));
            assert!(result, "Guard {} should be true", i);
        }
    }

    /// Test 6: Guard fails gracefully with clear error
    #[test]
    fn test_guard_failure_message() {
        let guard = GuardCondition {
            requires: vec!["/negotiate blessing:b00t-sandbox budget:10000".to_string()],
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
        let deserialized: GuardCondition = serde_json::from_str(&json).expect("Should deserialize");

        assert_eq!(guard, deserialized);
    }
}

#[cfg(test)]
mod k0mmand3r_typed_cmd_tests {
    use super::super::*;

    #[test]
    fn test_parse_negotiate_positional() {
        let cmd = K0mmand3rCmd::parse("/negotiate blessing observe-infra").unwrap();
        match cmd {
            K0mmand3rCmd::Negotiate {
                resource,
                id,
                modifiers,
            } => {
                assert_eq!(resource, "blessing");
                assert_eq!(id, "observe-infra");
                assert!(modifiers.is_empty());
            }
            _ => panic!("Expected Negotiate"),
        }
    }

    #[test]
    fn test_parse_negotiate_modifier() {
        let cmd = K0mmand3rCmd::parse("/negotiate blessing:observe-infra").unwrap();
        match cmd {
            K0mmand3rCmd::Negotiate { resource, id, .. } => {
                assert_eq!(resource, "blessing");
                assert_eq!(id, "observe-infra");
            }
            _ => panic!("Expected Negotiate"),
        }
    }

    #[test]
    fn test_parse_vote() {
        let cmd = K0mmand3rCmd::parse("/vote proposal-123 yes because reasons").unwrap();
        match cmd {
            K0mmand3rCmd::Vote {
                proposal,
                choice,
                reason,
            } => {
                assert_eq!(proposal, "proposal-123");
                assert_eq!(choice, b00t_ipc::VoteChoice::Yes);
                assert_eq!(reason, Some("because reasons".to_string()));
            }
            _ => panic!("Expected Vote"),
        }
    }

    #[test]
    fn test_parse_vote_abstain() {
        let cmd = K0mmand3rCmd::parse("/vote on proposal-456 choice abstain").unwrap();
        match cmd {
            K0mmand3rCmd::Vote {
                proposal, choice, ..
            } => {
                assert_eq!(proposal, "proposal-456");
                assert_eq!(choice, b00t_ipc::VoteChoice::Abstain);
            }
            _ => panic!("Expected Vote"),
        }
    }

    #[test]
    fn test_parse_delegate() {
        let cmd = K0mmand3rCmd::parse("/delegate agent:b00t-sandbox budget:5000").unwrap();
        match cmd {
            K0mmand3rCmd::Delegate { agent, budget } => {
                assert_eq!(agent, "b00t-sandbox");
                assert_eq!(budget, 5000);
            }
            _ => panic!("Expected Delegate"),
        }
    }

    #[test]
    fn test_parse_handshake_positional() {
        let cmd = K0mmand3rCmd::parse("/handshake executive propose deal").unwrap();
        match cmd {
            K0mmand3rCmd::Handshake { agent, proposal } => {
                assert_eq!(agent, "executive");
                assert_eq!(proposal, Some("propose deal".to_string()));
            }
            _ => panic!("Expected Handshake"),
        }
    }

    #[test]
    fn test_parse_handshake_modifier() {
        let cmd = K0mmand3rCmd::parse("/handshake to agent:observer").unwrap();
        match cmd {
            K0mmand3rCmd::Handshake { agent, proposal } => {
                assert_eq!(agent, "observer");
                assert_eq!(proposal, None);
            }
            _ => panic!("Expected Handshake"),
        }
    }

    #[test]
    fn test_parse_handshake_missing_agent() {
        let result = K0mmand3rCmd::parse("/handshake");
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("agent"));
    }

    #[test]
    fn test_parse_loop_spec_positional() {
        let cmd = K0mmand3rCmd::parse("/loop goal:deploy metric:uptime verify:healthcheck max:5")
            .unwrap();
        match cmd {
            K0mmand3rCmd::Loop { spec } => {
                assert_eq!(spec.goal, "deploy");
                assert_eq!(spec.metric, "uptime");
                assert_eq!(spec.verify, "healthcheck");
                assert_eq!(spec.max, Some(5));
            }
            _ => panic!("Expected Loop"),
        }
    }

    #[test]
    fn test_parse_loop_spec_modifiers() {
        let cmd =
            K0mmand3rCmd::parse("/loop with goal=deploy metric=uptime verify=healthcheck").unwrap();
        match cmd {
            K0mmand3rCmd::Loop { spec } => {
                assert_eq!(spec.goal, "deploy");
                assert_eq!(spec.metric, "uptime");
                assert_eq!(spec.verify, "healthcheck");
            }
            _ => panic!("Expected Loop"),
        }
    }

    #[test]
    fn test_parse_loop_spec_missing_goal() {
        let result = K0mmand3rCmd::parse("/loop metric:uptime");
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("goal"));
    }

    #[test]
    fn test_parse_crew_form() {
        let cmd = K0mmand3rCmd::parse("/crew form alice bob charlie").unwrap();
        match cmd {
            K0mmand3rCmd::Crew { action, members } => {
                assert_eq!(action, CrewAction::Form);
                assert_eq!(members, vec!["alice", "bob", "charlie"]);
            }
            _ => panic!("Expected Crew"),
        }
    }

    #[test]
    fn test_parse_unknown() {
        let cmd = K0mmand3rCmd::parse("/foobar something").unwrap();
        match cmd {
            K0mmand3rCmd::Unknown { raw } => {
                assert_eq!(raw, "/foobar something");
            }
            _ => panic!("Expected Unknown"),
        }
    }

    #[test]
    fn test_parse_empty_command() {
        let result = K0mmand3rCmd::parse("/");
        assert!(result.is_err());
    }

    #[test]
    fn test_parse_missing_slash() {
        let result = K0mmand3rCmd::parse("handshake agent:test");
        assert!(result.is_err());
    }

    #[test]
    fn test_loop_spec_from_tokens_positional() {
        let positional = vec!["goal:build|metric:compile-time|verify:tests-pass|max:10"];
        let modifiers = std::collections::BTreeMap::new();
        let spec = LoopSpec::from_tokens(&positional, &modifiers).unwrap();
        assert_eq!(spec.goal, "build");
        assert_eq!(spec.metric, "compile-time");
        assert_eq!(spec.verify, "tests-pass");
        assert_eq!(spec.max, Some(10));
        assert_eq!(spec.guard, None);
    }

    #[test]
    fn test_loop_spec_from_tokens_modifiers() {
        let positional: Vec<&str> = vec![];
        let mut modifiers = std::collections::BTreeMap::new();
        modifiers.insert("goal".to_string(), "deploy".to_string());
        modifiers.insert("metric".to_string(), "latency".to_string());
        modifiers.insert("guard".to_string(), "budget<100".to_string());
        let spec = LoopSpec::from_tokens(&positional, &modifiers).unwrap();
        assert_eq!(spec.goal, "deploy");
        assert_eq!(spec.metric, "latency");
        assert_eq!(spec.guard, Some("budget<100".to_string()));
    }

    #[test]
    fn test_loop_spec_from_tokens_missing_goal() {
        let positional: Vec<&str> = vec![];
        let modifiers = std::collections::BTreeMap::new();
        let result = LoopSpec::from_tokens(&positional, &modifiers);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("goal"));
    }

    #[test]
    fn test_handshake_validation() {
        let cmd = K0mmand3rCmd::parse("/handshake agent:executive").unwrap();
        assert_eq!(cmd.verb(), "handshake");
        assert_eq!(cmd.object(), "agent:executive");

        let cmd = K0mmand3rCmd::parse("/handshake observer do the thing").unwrap();
        match cmd {
            K0mmand3rCmd::Handshake { agent, proposal } => {
                assert_eq!(agent, "observer");
                assert_eq!(proposal, Some("do the thing".to_string()));
            }
            _ => panic!("Expected Handshake"),
        }
    }
}
