#[cfg(test)]
mod moku_step_tests {
    use super::super::*;

    /// Test 1: Parse step.toml with state definitions
    #[test]
    fn test_parse_step_toml_with_states() {
        let toml_str = r#"
[b00t]
name = "execute-transition"
type = "step"

[b00t.step.states]
variants = ["Planning", "Validating", "Executing", "Verified", "Done"]
initial = "Planning"

[[b00t.step.state]]
name = "Planning"

[b00t.step.state.Planning.io]
input = { current_state = "json", desired_state = "json" }
output = { plan = "string" }
        "#;

        let step = MokuStep::from_toml(toml_str).expect("Should parse step TOML");

        assert_eq!(step.name, "execute-transition");
        assert_eq!(step.states.initial, "Planning");
        assert_eq!(step.states.variants.len(), 5);
    }

    /// Test 2: Get state definition by name
    #[test]
    fn test_get_state_definition() {
        let step = MokuStep {
            name: "test".to_string(),
            states: StatesConfig {
                variants: vec!["Planning".to_string(), "Done".to_string()],
                initial: "Planning".to_string(),
            },
            state_defs: vec![StateDefinition {
                name: "Planning".to_string(),
                instructions: None,
                io: Some(IOContract::default()),
                transition: None,
            }],
        };

        let state = step.get_state("Planning");
        assert!(state.is_some());
        assert_eq!(state.unwrap().name, "Planning");

        let missing = step.get_state("Missing");
        assert!(missing.is_none());
    }

    /// Test 3: Validate state machine has valid transitions
    #[test]
    fn test_validate_transitions() {
        let step = MokuStep {
            name: "test".to_string(),
            states: StatesConfig {
                variants: vec!["A".to_string(), "B".to_string(), "C".to_string()],
                initial: "A".to_string(),
            },
            state_defs: vec![
                StateDefinition {
                    name: "A".to_string(),
                    instructions: None,
                    io: None,
                    transition: Some(TransitionRule {
                        to: "B".to_string(),
                        requires: vec![],
                        guard: None,
                        output_contract: None,
                    }),
                },
                StateDefinition {
                    name: "B".to_string(),
                    instructions: None,
                    io: None,
                    // Invalid: transition to non-existent state
                    transition: Some(TransitionRule {
                        to: "Invalid".to_string(),
                        requires: vec![],
                        guard: None,
                        output_contract: None,
                    }),
                },
            ],
        };

        let result = step.validate();
        assert!(result.is_err(), "Should reject invalid transition");
    }

    /// Test 4: Extract guards from transitions
    #[test]
    fn test_extract_transition_guards() {
        let transition = TransitionRule {
            to: "Validating".to_string(),
            requires: vec![
                "/negotiate blessing:observe-infrastructure".to_string(),
                "/vote on blessing:execute-transition-safely".to_string(),
            ],
            guard: Some("has_blessing(blessing:observe-infrastructure)".to_string()),
            output_contract: None,
        };

        let guards = transition.extract_guards();
        assert_eq!(guards.len(), 1);
        assert_eq!(guards[0].requires.len(), 2);
        assert!(guards[0].requires.iter().any(|r| r.contains("negotiate")));
    }

    /// Test 5: Initial state is valid
    #[test]
    fn test_initial_state_is_valid() {
        let step = MokuStep {
            name: "test".to_string(),
            states: StatesConfig {
                variants: vec!["Planning".to_string(), "Done".to_string()],
                initial: "Planning".to_string(),
            },
            state_defs: vec![],
        };

        let result = step.validate_initial_state();
        assert!(result.is_ok());

        let step_invalid = MokuStep {
            name: "test".to_string(),
            states: StatesConfig {
                variants: vec!["Planning".to_string(), "Done".to_string()],
                initial: "Invalid".to_string(),
            },
            state_defs: vec![],
        };

        let result = step_invalid.validate_initial_state();
        assert!(result.is_err(), "Should reject invalid initial state");
    }

    /// Test 6: Serialize step back to TOML
    #[test]
    fn test_serialize_step_to_toml() {
        let step = MokuStep {
            name: "execute-transition".to_string(),
            states: StatesConfig {
                variants: vec!["Planning".to_string(), "Done".to_string()],
                initial: "Planning".to_string(),
            },
            state_defs: vec![StateDefinition {
                name: "Planning".to_string(),
                instructions: None,
                io: None,
                transition: None,
            }],
        };

        let toml_str = step.to_toml().expect("Should serialize");
        assert!(toml_str.contains("execute-transition"));
        assert!(toml_str.contains("Planning"));
    }

    /// Test 7: IO contract validation
    #[test]
    fn test_io_contract_validation() {
        let contract = IOContract {
            input: Some(
                vec![("state".to_string(), "json".to_string())]
                    .into_iter()
                    .collect(),
            ),
            output: Some(
                vec![("plan".to_string(), "json".to_string())]
                    .into_iter()
                    .collect(),
            ),
        };

        assert!(contract.input.is_some());
        assert!(contract.output.is_some());
    }

    /// Test 8: Mermaid diagram generation
    #[test]
    fn test_generate_mermaid_diagram() {
        let step = MokuStep {
            name: "execute-transition".to_string(),
            states: StatesConfig {
                variants: vec!["Planning".to_string(), "Done".to_string()],
                initial: "Planning".to_string(),
            },
            state_defs: vec![StateDefinition {
                name: "Planning".to_string(),
                instructions: None,
                io: None,
                transition: Some(TransitionRule {
                    to: "Done".to_string(),
                    requires: vec![],
                    guard: None,
                    output_contract: None,
                }),
            }],
        };

        let diagram = step.to_mermaid().expect("Should generate Mermaid diagram");
        assert!(diagram.contains("stateDiagram-v2"));
        assert!(diagram.contains("Planning"));
        assert!(diagram.contains("Done"));
        assert!(diagram.contains("-->"));
    }
}
