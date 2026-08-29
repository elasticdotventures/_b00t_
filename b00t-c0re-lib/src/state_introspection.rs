//! Trait-backed state machine introspection.
//!
//! This module is deliberately independent from any one renderer. State
//! machines can expose their semantic shape through a trait or a macro, then
//! downstream code can serialize that shape as Mermaid, S5, SCXML, CLIF, or an
//! isometric scene graph.

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct StateTypeDescriptor {
    pub id: &'static str,
    pub rust_type: &'static str,
    pub classifier: &'static str,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct StateTransitionDescriptor {
    pub source: &'static str,
    pub event: &'static str,
    pub target: &'static str,
    pub guard: Option<&'static str>,
}

pub trait StateMachineIntrospection {
    fn machine_id() -> &'static str;
    fn initial_state() -> &'static str;
    fn final_states() -> &'static [&'static str];
    fn state_type_descriptors() -> Vec<StateTypeDescriptor>;
    fn transition_descriptors() -> Vec<StateTransitionDescriptor>;

    fn render_mermaid_state_diagram() -> String {
        let mut out = String::from("stateDiagram-v2\n");

        for state in Self::state_type_descriptors() {
            out.push_str(&format!("  state \"{}\" as {}\n", state.id, state.id));
        }

        out.push_str(&format!("  [*] --> {}\n", Self::initial_state()));
        for transition in Self::transition_descriptors() {
            let guard = transition
                .guard
                .map(|guard| format!(" [{guard}]"))
                .unwrap_or_default();
            out.push_str(&format!(
                "  {} --> {}: {}{}\n",
                transition.source, transition.target, transition.event, guard
            ));
        }
        for final_state in Self::final_states() {
            out.push_str(&format!("  {final_state} --> [*]\n"));
        }

        out
    }

    fn render_s5() -> String {
        let mut out = format!(
            "@machine id={} initial={} datamodel=rust\n",
            Self::machine_id(),
            Self::initial_state()
        );

        for state in Self::state_type_descriptors() {
            let transitions = Self::transition_descriptors()
                .into_iter()
                .filter(|transition| transition.source == state.id)
                .collect::<Vec<_>>();

            if Self::final_states().contains(&state.id) && transitions.is_empty() {
                out.push_str(&format!("@state {} final\n", state.id));
                continue;
            }

            if transitions.len() == 1 {
                let transition = &transitions[0];
                out.push_str(&format!(
                    "@state {} -{}{}-> {}\n",
                    state.id,
                    transition.event,
                    transition
                        .guard
                        .map(|guard| format!("[{guard}]"))
                        .unwrap_or_default(),
                    transition.target
                ));
                continue;
            }

            out.push_str(&format!("@state {}:\n", state.id));
            for transition in transitions {
                out.push_str(&format!(
                    "  -{}{}-> {}\n",
                    transition.event,
                    transition
                        .guard
                        .map(|guard| format!("[{guard}]"))
                        .unwrap_or_default(),
                    transition.target
                ));
            }
        }

        out
    }

    /// Render this state machine's shape as a W3C SCXML statechart
    /// document (via the `scxml` crate — a document model, not a runtime;
    /// this never replaces the implementor's own transition-enforcement
    /// logic, e.g. `can_transition`/`event_for_target` above, it only
    /// exports their shape). See `elasticdotventures/_b00t_#1177` (P5b).
    ///
    /// Note: `final_states()` here is a *softer* concept than SCXML's
    /// `Final` state kind — it marks a diagram-level "this is one of the
    /// ending states" annotation, not "this state has zero outgoing
    /// transitions." SCXML's `Final` kind is stricter (a real final state
    /// may have no outgoing transitions), so a `final_states()` entry that
    /// still has outgoing transitions (e.g. `OodaPhase::Complete`, which
    /// can still transition to `Failed`) is exported as `Atomic`, not
    /// `Final` — only entries with genuinely zero outgoing transitions
    /// (e.g. `OodaPhase::Failed`) become a real SCXML `Final` state.
    #[cfg(feature = "statechart")]
    fn render_scxml_statechart() -> scxml::model::Statechart {
        use scxml::model::{State, Transition};

        let finals = Self::final_states();
        let all_transitions = Self::transition_descriptors();

        let states: Vec<State> = Self::state_type_descriptors()
            .into_iter()
            .map(|descriptor| {
                let outgoing: Vec<_> = all_transitions
                    .iter()
                    .filter(|t| t.source == descriptor.id)
                    .collect();
                let mut state = if finals.contains(&descriptor.id) && outgoing.is_empty() {
                    State::final_state(descriptor.id)
                } else {
                    State::atomic(descriptor.id)
                };
                state.transitions = outgoing
                    .into_iter()
                    .map(|t| {
                        let mut transition = Transition::new(t.event, t.target);
                        if let Some(guard) = t.guard {
                            transition = transition.with_guard(guard);
                        }
                        transition
                    })
                    .collect();
                state
            })
            .collect();

        scxml::model::Statechart::new(Self::initial_state(), states).with_name(Self::machine_id())
    }

    /// Render this state machine's shape as an SCXML XML document string.
    #[cfg(feature = "statechart")]
    fn render_scxml() -> String {
        scxml::export::xml::to_xml(&Self::render_scxml_statechart())
    }
}

#[macro_export]
macro_rules! impl_state_machine_introspection {
    (
        impl $target:ty {
            machine_id: $machine_id:expr,
            initial: $initial:expr,
            states: [$( $state_expr:expr => $state_id:expr ),+ $(,)?],
            finals: [$( $final_state:expr ),* $(,)?],
            can_transition: $can_transition:path,
            event_for_target: $event_for_target:path $(,)?
        }
    ) => {
        impl $crate::state_introspection::StateMachineIntrospection for $target {
            fn machine_id() -> &'static str {
                $machine_id
            }

            fn initial_state() -> &'static str {
                $initial
            }

            fn final_states() -> &'static [&'static str] {
                &[$( $final_state ),*]
            }

            fn state_type_descriptors() -> Vec<$crate::state_introspection::StateTypeDescriptor> {
                vec![
                    $(
                        $crate::state_introspection::StateTypeDescriptor {
                            id: $state_id,
                            rust_type: std::any::type_name::<$target>(),
                            classifier: "enum_variant",
                        }
                    ),+
                ]
            }

            fn transition_descriptors() -> Vec<$crate::state_introspection::StateTransitionDescriptor> {
                let states = vec![$( ($state_expr, $state_id) ),+];
                let mut transitions = Vec::new();

                for (source, source_id) in &states {
                    for (target, target_id) in &states {
                        if $can_transition(source, target) {
                            if let Some(event) = $event_for_target(target) {
                                transitions.push($crate::state_introspection::StateTransitionDescriptor {
                                    source: source_id,
                                    event,
                                    target: target_id,
                                    guard: None,
                                });
                            }
                        }
                    }
                }

                transitions
            }
        }
    };
}

#[cfg(all(test, feature = "statechart"))]
mod tests {
    use super::*;

    /// Minimal, hand-written (not macro-derived) implementor, isolated from
    /// any real domain's business rules, to test `render_scxml_statechart`'s
    /// Final-vs-Atomic disambiguation directly: `final_states()` is a soft,
    /// diagram-level annotation, not a promise of zero outgoing transitions.
    struct ToyMachine;

    impl StateMachineIntrospection for ToyMachine {
        fn machine_id() -> &'static str {
            "Toy"
        }
        fn initial_state() -> &'static str {
            "Start"
        }
        fn final_states() -> &'static [&'static str] {
            &["Start", "End", "Loop"]
        }
        fn state_type_descriptors() -> Vec<StateTypeDescriptor> {
            vec![
                StateTypeDescriptor {
                    id: "Start",
                    rust_type: "ToyMachine",
                    classifier: "enum_variant",
                },
                StateTypeDescriptor {
                    id: "End",
                    rust_type: "ToyMachine",
                    classifier: "enum_variant",
                },
                StateTypeDescriptor {
                    id: "Loop",
                    rust_type: "ToyMachine",
                    classifier: "enum_variant",
                },
            ]
        }
        fn transition_descriptors() -> Vec<StateTransitionDescriptor> {
            vec![
                StateTransitionDescriptor {
                    source: "Start",
                    event: "Go",
                    target: "End",
                    guard: None,
                },
                StateTransitionDescriptor {
                    source: "Start",
                    event: "Spin",
                    target: "Loop",
                    guard: None,
                },
                StateTransitionDescriptor {
                    source: "Loop",
                    event: "Retry",
                    target: "Loop",
                    guard: None,
                },
            ]
        }
    }

    #[test]
    fn render_scxml_reserves_final_for_states_with_zero_outgoing_transitions() {
        let chart = ToyMachine::render_scxml_statechart();

        // Listed in final_states() but has an outgoing transition -> Atomic.
        assert_eq!(
            chart.find_state("Start").unwrap().kind,
            scxml::model::StateKind::Atomic
        );
        // Listed in final_states() and has zero outgoing transitions -> real Final.
        assert_eq!(
            chart.find_state("End").unwrap().kind,
            scxml::model::StateKind::Final
        );
        // Listed in final_states() but has a self-loop — still an outgoing
        // transition as far as SCXML is concerned, so it stays Atomic too.
        assert_eq!(
            chart.find_state("Loop").unwrap().kind,
            scxml::model::StateKind::Atomic
        );

        scxml::validate(&chart).expect("toy statechart should be structurally valid");

        let xml = ToyMachine::render_scxml();
        let parsed = scxml::parse_xml(&xml).expect("exported XML must parse back");
        assert_eq!(parsed, chart);
    }
}
