//! Addressed state-machine primitives for b00t runtime surfaces.
//!
//! This module owns the abstract model used by flash sheets and domain crates.
//! SCXML/SCE adapters can sit on top of these types, but the core contract is
//! intentionally Rust-native: logical graph addresses, events, guards, actions,
//! transform outcomes, CLIF payloads, and deterministic visual output.

use crate::flash_sheet::{SheetMetadata, SymbolicRule};
use std::collections::BTreeMap;

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct LogicalAddress {
    pub graph_path: Vec<String>,
    pub local_id: String,
}

impl LogicalAddress {
    pub fn new(
        graph_path: impl IntoIterator<Item = impl Into<String>>,
        local_id: impl Into<String>,
    ) -> Self {
        Self {
            graph_path: graph_path.into_iter().map(Into::into).collect(),
            local_id: local_id.into(),
        }
    }

    pub fn child(&self, edge: impl Into<String>, local_id: impl Into<String>) -> Self {
        let mut graph_path = self.graph_path.clone();
        graph_path.push(edge.into());
        Self {
            graph_path,
            local_id: local_id.into(),
        }
    }

    pub fn token(&self) -> String {
        if self.graph_path.is_empty() {
            self.local_id.clone()
        } else {
            format!("{}/{}", self.graph_path.join("/"), self.local_id)
        }
    }
}

crate::impl_type_introspection! {
    struct LogicalAddress {
        classifier: "state_machine.logical_address",
        fields: [
            graph_path: Vec<String> => "graph.path",
            local_id: String => "local.id",
        ],
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClifPayload {
    pub text: String,
    pub metadata: SheetMetadata,
}

impl ClifPayload {
    pub fn new(text: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            metadata: SheetMetadata::new(),
        }
    }
}

crate::impl_type_introspection! {
    struct ClifPayload {
        classifier: "logic.clif.payload",
        fields: [
            text: String => "clif.text",
            metadata: SheetMetadata => "metadata",
        ],
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StateMachineEvent {
    pub name: String,
    pub payload: SheetMetadata,
    pub clif: Option<ClifPayload>,
}

impl StateMachineEvent {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            payload: SheetMetadata::new(),
            clif: None,
        }
    }

    pub fn with_clif(mut self, clif: ClifPayload) -> Self {
        self.clif = Some(clif);
        self
    }
}

crate::impl_type_introspection! {
    struct StateMachineEvent {
        classifier: "state_machine.event",
        fields: [
            name: String => "event.name",
            payload: SheetMetadata => "event.payload",
            clif: Option<ClifPayload> => "logic.clif",
        ],
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StateNode {
    pub address: LogicalAddress,
    pub superstate: Option<LogicalAddress>,
    pub is_initial: bool,
    pub is_final: bool,
    pub entry_actions: Vec<String>,
    pub exit_actions: Vec<String>,
    pub state_local_metadata: SheetMetadata,
    pub metadata: SheetMetadata,
    pub symbolic_rules: Vec<SymbolicRule>,
}

impl StateNode {
    pub fn new(address: LogicalAddress) -> Self {
        Self {
            address,
            superstate: None,
            is_initial: false,
            is_final: false,
            entry_actions: Vec::new(),
            exit_actions: Vec::new(),
            state_local_metadata: SheetMetadata::new(),
            metadata: SheetMetadata::new(),
            symbolic_rules: Vec::new(),
        }
    }

    pub fn initial(mut self) -> Self {
        self.is_initial = true;
        self
    }

    pub fn final_state(mut self) -> Self {
        self.is_final = true;
        self
    }

    pub fn with_superstate(mut self, superstate: LogicalAddress) -> Self {
        self.superstate = Some(superstate);
        self
    }

    pub fn on_entry(mut self, action: impl Into<String>) -> Self {
        self.entry_actions.push(action.into());
        self
    }

    pub fn on_exit(mut self, action: impl Into<String>) -> Self {
        self.exit_actions.push(action.into());
        self
    }
}

crate::impl_type_introspection! {
    struct StateNode {
        classifier: "state_machine.state",
        fields: [
            address: LogicalAddress => "logical.address",
            superstate: Option<LogicalAddress> => "state.superstate",
            is_initial: bool => "state.initial",
            is_final: bool => "state.final",
            entry_actions: Vec<String> => "state.entry_actions",
            exit_actions: Vec<String> => "state.exit_actions",
            state_local_metadata: SheetMetadata => "state.local_metadata",
            metadata: SheetMetadata => "metadata",
            symbolic_rules: Vec<SymbolicRule> => "symbolic.rules",
        ],
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StateTransition {
    pub source: LogicalAddress,
    pub target: LogicalAddress,
    pub event: String,
    pub guard: Option<SymbolicRule>,
    pub actions: Vec<String>,
    pub emitted_events: Vec<StateMachineEvent>,
}

impl StateTransition {
    pub fn new(source: LogicalAddress, event: impl Into<String>, target: LogicalAddress) -> Self {
        Self {
            source,
            target,
            event: event.into(),
            guard: None,
            actions: Vec::new(),
            emitted_events: Vec::new(),
        }
    }

    pub fn with_guard(mut self, guard: SymbolicRule) -> Self {
        self.guard = Some(guard);
        self
    }

    pub fn emits(mut self, event: StateMachineEvent) -> Self {
        self.emitted_events.push(event);
        self
    }
}

crate::impl_type_introspection! {
    struct StateTransition {
        classifier: "state_machine.transition",
        fields: [
            source: LogicalAddress => "transition.source",
            target: LogicalAddress => "transition.target",
            event: String => "transition.event",
            guard: Option<SymbolicRule> => "transition.guard",
            actions: Vec<String> => "transition.actions",
            emitted_events: Vec<StateMachineEvent> => "transition.emitted_events",
        ],
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StateMachineSpec {
    pub address: LogicalAddress,
    pub states: BTreeMap<LogicalAddress, StateNode>,
    pub transitions: Vec<StateTransition>,
    pub metadata: SheetMetadata,
}

impl StateMachineSpec {
    pub fn new(address: LogicalAddress) -> Self {
        Self {
            address,
            states: BTreeMap::new(),
            transitions: Vec::new(),
            metadata: SheetMetadata::new(),
        }
    }

    pub fn add_state(&mut self, state: StateNode) {
        self.states.insert(state.address.clone(), state);
    }

    pub fn add_transition(&mut self, transition: StateTransition) {
        self.transitions.push(transition);
        self.transitions.sort_by(|left, right| {
            left.source
                .cmp(&right.source)
                .then(left.event.cmp(&right.event))
        });
    }

    pub fn apply_event(
        &self,
        current: &LogicalAddress,
        event: &StateMachineEvent,
    ) -> StateTransformOutcome {
        let transition = self
            .transitions
            .iter()
            .find(|candidate| candidate.source == *current && candidate.event == event.name);

        match transition {
            Some(transition) => StateTransformOutcome {
                machine: self.address.clone(),
                from: current.clone(),
                to: transition.target.clone(),
                consumed_event: event.clone(),
                emitted_events: transition.emitted_events.clone(),
                actions: transition.actions.clone(),
                guard: transition.guard.clone(),
                statig_outcome: StateDispatchOutcome::Transition,
                changed: true,
            },
            None => StateTransformOutcome {
                machine: self.address.clone(),
                from: current.clone(),
                to: current.clone(),
                consumed_event: event.clone(),
                emitted_events: Vec::new(),
                actions: Vec::new(),
                guard: None,
                statig_outcome: self
                    .states
                    .get(current)
                    .and_then(|state| state.superstate.as_ref())
                    .map(|_| StateDispatchOutcome::Super)
                    .unwrap_or(StateDispatchOutcome::Handled),
                changed: false,
            },
        }
    }

    pub fn render_svgbob(&self, current: &LogicalAddress) -> String {
        let mut out = String::new();
        out.push_str(&format!("StateMachine: {}\n\n", self.address.token()));

        for (address, state) in &self.states {
            let marker = if address == current { "*" } else { " " };
            let terminal = if state.is_final { " final" } else { "" };
            out.push_str(&format!("[{marker}] {}{terminal}\n", address.token()));
        }

        out.push('\n');
        for transition in &self.transitions {
            let guard = transition
                .guard
                .as_ref()
                .map(|rule| format!(" [{}]", rule.expression))
                .unwrap_or_default();
            out.push_str(&format!(
                "{} --{}{}--> {}\n",
                transition.source.token(),
                transition.event,
                guard,
                transition.target.token()
            ));
        }

        out
    }

    pub fn render_mermaid_state_diagram(&self) -> String {
        let mut out = String::from("stateDiagram-v2\n");

        for (address, state) in &self.states {
            out.push_str(&format!(
                "  state \"{}\" as {}\n",
                escape_mermaid_label(&address.local_id),
                mermaid_id(address)
            ));

            if state.is_initial {
                out.push_str(&format!("  [*] --> {}\n", mermaid_id(address)));
            }

            if state.is_final {
                out.push_str(&format!("  {} --> [*]\n", mermaid_id(address)));
            }

            for action in &state.entry_actions {
                out.push_str(&format!(
                    "  {}: entry / {}\n",
                    mermaid_id(address),
                    escape_mermaid_label(action)
                ));
            }

            for action in &state.exit_actions {
                out.push_str(&format!(
                    "  {}: exit / {}\n",
                    mermaid_id(address),
                    escape_mermaid_label(action)
                ));
            }
        }

        for transition in &self.transitions {
            let mut label = transition.event.clone();
            if let Some(guard) = &transition.guard {
                label.push_str(&format!(" [{}]", guard.expression));
            }
            if let Some(action) = transition.actions.first() {
                label.push_str(&format!(" / {action}"));
            }
            if !transition.emitted_events.is_empty() {
                label.push_str(" => ");
                label.push_str(
                    &transition
                        .emitted_events
                        .iter()
                        .map(|event| event.name.as_str())
                        .collect::<Vec<_>>()
                        .join(", "),
                );
            }

            out.push_str(&format!(
                "  {} --> {}: {}\n",
                mermaid_id(&transition.source),
                mermaid_id(&transition.target),
                escape_mermaid_label(&label)
            ));
        }

        out
    }

    pub fn to_graph_snapshot(&self, current: &LogicalAddress) -> StateMachineGraphSnapshot {
        let nodes = self
            .states
            .iter()
            .map(|(address, state)| StateMachineGraphNode {
                id: address.token(),
                label: state.address.local_id.clone(),
                address: address.clone(),
                superstate: state.superstate.clone(),
                visual_state: StateMachineVisualState::from_node(state, address == current),
                metadata: state.metadata.clone(),
            })
            .collect();

        let edges = self
            .transitions
            .iter()
            .map(|transition| StateMachineGraphEdge {
                from: transition.source.token(),
                to: transition.target.token(),
                event: transition.event.clone(),
                guard: transition
                    .guard
                    .as_ref()
                    .map(|rule| rule.expression.clone()),
                actions: transition.actions.clone(),
                emitted_events: transition
                    .emitted_events
                    .iter()
                    .map(|event| event.name.clone())
                    .collect(),
            })
            .collect();

        StateMachineGraphSnapshot {
            machine: self.address.clone(),
            current: current.clone(),
            nodes,
            edges,
        }
    }
}

crate::impl_type_introspection! {
    struct StateMachineSpec {
        classifier: "state_machine.spec",
        fields: [
            address: LogicalAddress => "logical.address",
            states: BTreeMap<LogicalAddress, StateNode> => "state_machine.states",
            transitions: Vec<StateTransition> => "state_machine.transitions",
            metadata: SheetMetadata => "metadata",
        ],
    }
}

fn mermaid_id(address: &LogicalAddress) -> String {
    let mut out = String::with_capacity(address.token().len() + 3);
    out.push_str("s__");
    for ch in address.token().chars() {
        if ch.is_ascii_alphanumeric() || ch == '_' {
            out.push(ch);
        } else {
            out.push('_');
        }
    }
    out
}

fn escape_mermaid_label(label: &str) -> String {
    label.replace('"', "'").replace('\n', " ")
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StateMachineVisualState {
    Idle,
    Active,
    Initial,
    Final,
    ActiveFinal,
}

crate::impl_type_introspection! {
    enum StateMachineVisualState {
        classifier: "state_machine.visual_state",
        variants: [
            Idle => "state.idle",
            Active => "state.active",
            Initial => "state.initial",
            Final => "state.final",
            ActiveFinal => "state.active_final",
        ],
    }
}

impl StateMachineVisualState {
    fn from_node(node: &StateNode, active: bool) -> Self {
        match (active, node.is_initial, node.is_final) {
            (true, _, true) => Self::ActiveFinal,
            (true, _, false) => Self::Active,
            (false, true, _) => Self::Initial,
            (false, false, true) => Self::Final,
            (false, false, false) => Self::Idle,
        }
    }

    pub fn role(self) -> &'static str {
        match self {
            Self::Idle => "state",
            Self::Active => "active_state",
            Self::Initial => "initial_state",
            Self::Final => "final_state",
            Self::ActiveFinal => "active_final_state",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StateMachineGraphNode {
    pub id: String,
    pub label: String,
    pub address: LogicalAddress,
    pub superstate: Option<LogicalAddress>,
    pub visual_state: StateMachineVisualState,
    pub metadata: SheetMetadata,
}

crate::impl_type_introspection! {
    struct StateMachineGraphNode {
        classifier: "state_machine.graph.node",
        fields: [
            id: String => "graph.node.id",
            label: String => "graph.node.label",
            address: LogicalAddress => "logical.address",
            superstate: Option<LogicalAddress> => "state.superstate",
            visual_state: StateMachineVisualState => "state.visual",
            metadata: SheetMetadata => "metadata",
        ],
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StateMachineGraphEdge {
    pub from: String,
    pub to: String,
    pub event: String,
    pub guard: Option<String>,
    pub actions: Vec<String>,
    pub emitted_events: Vec<String>,
}

crate::impl_type_introspection! {
    struct StateMachineGraphEdge {
        classifier: "state_machine.graph.edge",
        fields: [
            from: String => "graph.edge.from",
            to: String => "graph.edge.to",
            event: String => "transition.event",
            guard: Option<String> => "transition.guard",
            actions: Vec<String> => "transition.actions",
            emitted_events: Vec<String> => "transition.emitted_events",
        ],
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StateMachineGraphSnapshot {
    pub machine: LogicalAddress,
    pub current: LogicalAddress,
    pub nodes: Vec<StateMachineGraphNode>,
    pub edges: Vec<StateMachineGraphEdge>,
}

crate::impl_type_introspection! {
    struct StateMachineGraphSnapshot {
        classifier: "state_machine.graph.snapshot",
        fields: [
            machine: LogicalAddress => "state_machine.address",
            current: LogicalAddress => "state.current",
            nodes: Vec<StateMachineGraphNode> => "graph.nodes",
            edges: Vec<StateMachineGraphEdge> => "graph.edges",
        ],
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StateDispatchOutcome {
    Handled,
    Transition,
    Super,
}

crate::impl_type_introspection! {
    enum StateDispatchOutcome {
        classifier: "state_machine.dispatch_outcome",
        variants: [
            Handled => "dispatch.handled",
            Transition => "dispatch.transition",
            Super => "dispatch.super",
        ],
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StateTransformOutcome {
    pub machine: LogicalAddress,
    pub from: LogicalAddress,
    pub to: LogicalAddress,
    pub consumed_event: StateMachineEvent,
    pub emitted_events: Vec<StateMachineEvent>,
    pub actions: Vec<String>,
    pub guard: Option<SymbolicRule>,
    pub statig_outcome: StateDispatchOutcome,
    pub changed: bool,
}

crate::impl_type_introspection! {
    struct StateTransformOutcome {
        classifier: "state_machine.transform_outcome",
        fields: [
            machine: LogicalAddress => "state_machine.address",
            from: LogicalAddress => "state.from",
            to: LogicalAddress => "state.to",
            consumed_event: StateMachineEvent => "event.consumed",
            emitted_events: Vec<StateMachineEvent> => "event.emitted",
            actions: Vec<String> => "transition.actions",
            guard: Option<SymbolicRule> => "transition.guard",
            statig_outcome: StateDispatchOutcome => "statig.outcome",
            changed: bool => "state.changed",
        ],
    }
}

pub fn state_machine_type_descriptors() -> Vec<crate::type_introspection::TypeDescriptor> {
    use crate::type_introspection::TypeIntrospection;

    vec![
        <LogicalAddress as TypeIntrospection>::type_descriptor(),
        <ClifPayload as TypeIntrospection>::type_descriptor(),
        <StateMachineEvent as TypeIntrospection>::type_descriptor(),
        <StateNode as TypeIntrospection>::type_descriptor(),
        <StateTransition as TypeIntrospection>::type_descriptor(),
        <StateMachineSpec as TypeIntrospection>::type_descriptor(),
        <StateMachineVisualState as TypeIntrospection>::type_descriptor(),
        <StateMachineGraphNode as TypeIntrospection>::type_descriptor(),
        <StateMachineGraphEdge as TypeIntrospection>::type_descriptor(),
        <StateMachineGraphSnapshot as TypeIntrospection>::type_descriptor(),
        <StateDispatchOutcome as TypeIntrospection>::type_descriptor(),
        <StateTransformOutcome as TypeIntrospection>::type_descriptor(),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde::Deserialize;

    #[derive(Debug, Deserialize)]
    struct EventFixture {
        input: String,
        emitted: String,
    }

    #[test]
    fn state_machine_uses_logical_addresses_and_emits_events() {
        let fixture = include_str!("../tests/fixtures/state_machine_events.json");
        let fixture: EventFixture = serde_json::from_str(fixture).unwrap();
        let root = LogicalAddress::new(["flash", "sheet", "r1", "status"], "workflow");
        let pending = root.child("state", "pending");
        let ready = root.child("state", "ready");
        let mut machine = StateMachineSpec::new(root.clone());
        machine.add_state(StateNode::new(pending.clone()).initial());
        machine.add_state(StateNode::new(ready.clone()).final_state());
        machine.add_transition(
            StateTransition::new(pending.clone(), &fixture.input, ready.clone())
                .with_guard(SymbolicRule::new("cell-not-empty", "status != ''"))
                .emits(
                    StateMachineEvent::new(&fixture.emitted)
                        .with_clif(ClifPayload::new("(cell-change status ready)")),
                ),
        );

        let outcome = machine.apply_event(&pending, &StateMachineEvent::new(&fixture.input));

        assert!(outcome.changed);
        assert_eq!(outcome.to, ready);
        assert_eq!(outcome.statig_outcome, StateDispatchOutcome::Transition);
        assert_eq!(outcome.emitted_events[0].name, "sheet.ready");
        assert!(outcome.emitted_events[0].clif.is_some());
    }

    #[test]
    fn statig_superstate_shape_is_preserved() {
        let root = LogicalAddress::new(["graph"], "machine");
        let superstate = root.child("superstate", "editing");
        let leaf = root.child("state", "editing.value");
        let mut machine = StateMachineSpec::new(root);
        machine.add_state(
            StateNode::new(leaf.clone())
                .with_superstate(superstate.clone())
                .on_entry("enter_value")
                .on_exit("exit_value"),
        );

        let outcome = machine.apply_event(&leaf, &StateMachineEvent::new("unhandled"));

        assert_eq!(machine.states[&leaf].superstate, Some(superstate));
        assert_eq!(machine.states[&leaf].entry_actions, vec!["enter_value"]);
        assert_eq!(machine.states[&leaf].exit_actions, vec!["exit_value"]);
        assert_eq!(outcome.statig_outcome, StateDispatchOutcome::Super);
    }

    #[test]
    fn svgbob_render_is_deterministic_from_current_state() {
        let root = LogicalAddress::new(["graph"], "machine");
        let a = root.child("state", "a");
        let b = root.child("state", "b");
        let mut machine = StateMachineSpec::new(root);
        machine.add_state(StateNode::new(b.clone()).final_state());
        machine.add_state(StateNode::new(a.clone()).initial());
        machine.add_transition(StateTransition::new(a.clone(), "advance", b.clone()));

        let first = machine.render_svgbob(&a);
        let second = machine.render_svgbob(&a);

        assert_eq!(first, second);
        assert!(first.contains("[*] graph/state/a"));
        assert!(first.contains("graph/state/a --advance--> graph/state/b"));
    }

    #[test]
    fn graph_snapshot_is_renderer_neutral() {
        let root = LogicalAddress::new(["graph"], "machine");
        let a = root.child("state", "a");
        let b = root.child("state", "b");
        let mut machine = StateMachineSpec::new(root.clone());
        machine.add_state(StateNode::new(a.clone()).initial());
        machine.add_state(StateNode::new(b.clone()).final_state());
        machine.add_transition(
            StateTransition::new(a.clone(), "advance", b.clone())
                .with_guard(SymbolicRule::new("allowed", "can_advance"))
                .emits(StateMachineEvent::new("machine.advanced")),
        );

        let snapshot = machine.to_graph_snapshot(&b);

        assert_eq!(snapshot.machine, root);
        assert_eq!(snapshot.current, b);
        assert_eq!(snapshot.nodes.len(), 2);
        assert_eq!(snapshot.edges.len(), 1);
        assert_eq!(
            snapshot.nodes[0].visual_state,
            StateMachineVisualState::Initial
        );
        assert_eq!(
            snapshot.nodes[1].visual_state,
            StateMachineVisualState::ActiveFinal
        );
        assert_eq!(snapshot.nodes[1].visual_state.role(), "active_final_state");
        assert_eq!(snapshot.edges[0].event, "advance");
        assert_eq!(snapshot.edges[0].guard.as_deref(), Some("can_advance"));
        assert_eq!(snapshot.edges[0].emitted_events, vec!["machine.advanced"]);
    }

    #[test]
    fn mermaid_state_diagram_is_deterministic() {
        let root = LogicalAddress::new(["ooda"], "OodaPhase");
        let idle = root.child("state", "Idle");
        let observing = root.child("state", "Observing");
        let mut machine = StateMachineSpec::new(root);
        machine.add_state(StateNode::new(idle.clone()).initial());
        machine.add_state(StateNode::new(observing.clone()));
        machine.add_transition(
            StateTransition::new(idle, "GoToObserving", observing)
                .with_guard(SymbolicRule::new("guard", "observation_ready"))
                .emits(StateMachineEvent::new("ooda.observing")),
        );

        let first = machine.render_mermaid_state_diagram();
        let second = machine.render_mermaid_state_diagram();

        assert_eq!(first, second);
        assert!(first.starts_with("stateDiagram-v2\n"));
        assert!(first.contains("[*] --> s__ooda_state_Idle"));
        assert!(first.contains(
            "s__ooda_state_Idle --> s__ooda_state_Observing: GoToObserving [observation_ready] => ooda.observing"
        ));
    }
}
