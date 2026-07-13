//! Trait-backed state machine introspection.
//!
//! This module is deliberately independent from any one renderer. State
//! machines can expose their semantic shape through a trait or a macro, then
//! downstream code can serialize that shape as Mermaid, S5, SCXML, CLIF, or an
//! isometric scene graph.

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StateTypeDescriptor {
    pub id: &'static str,
    pub rust_type: &'static str,
    pub classifier: &'static str,
}

#[derive(Debug, Clone, PartialEq, Eq)]
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
            if Self::final_states().contains(&state.id) {
                out.push_str(&format!("@state {} final\n", state.id));
                continue;
            }

            let transitions = Self::transition_descriptors()
                .into_iter()
                .filter(|transition| transition.source == state.id)
                .collect::<Vec<_>>();

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
