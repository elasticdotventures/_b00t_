//! S5 token-reduced statechart authoring syntax.
//!
//! S5 is a compact authoring surface over [`StateMachineSpec`]. It is not an
//! execution engine: the canonical runtime model remains `StateMachineSpec`,
//! while SCXML/SCE, smcat, XState, CLIF, and local isometric renderers are
//! adapters around that IR.

use crate::flash_sheet::{SheetMetadata, SymbolicRule};
use crate::state_machine::{
    ClifPayload, LogicalAddress, StateMachineEvent, StateMachineSpec, StateNode, StateTransition,
};
use std::collections::{BTreeMap, BTreeSet};
use thiserror::Error;

const S5_FORMAT: &str = "s5";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct S5Document {
    pub machine: StateMachineSpec,
    pub initial: Option<LogicalAddress>,
}

impl S5Document {
    pub fn into_machine(self) -> StateMachineSpec {
        self.machine
    }
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum S5ParseError {
    #[error("missing @machine header")]
    MissingMachine,
    #[error("line {line}: invalid @machine header")]
    InvalidMachine { line: usize },
    #[error("line {line}: invalid @state declaration")]
    InvalidState { line: usize },
    #[error("line {line}: transition outside @state block")]
    TransitionOutsideState { line: usize },
    #[error("line {line}: action outside @state block")]
    ActionOutsideState { line: usize },
    #[error("line {line}: invalid transition")]
    InvalidTransition { line: usize },
    #[error("line {line}: invalid action")]
    InvalidAction { line: usize },
    #[error("unknown initial state: {0}")]
    UnknownInitial(String),
}

#[derive(Debug, Default)]
struct MachineHeader {
    id: String,
    initial: Option<String>,
    datamodel: Option<String>,
    metadata: SheetMetadata,
}

#[derive(Debug, Clone)]
struct StateBuilder {
    node: StateNode,
}

pub fn parse_s5(source: &str) -> Result<S5Document, S5ParseError> {
    let mut header = MachineHeader::default();
    let mut states: BTreeMap<String, StateBuilder> = BTreeMap::new();
    let mut transitions: Vec<StateTransition> = Vec::new();
    let mut current_state: Option<String> = None;

    for (idx, raw_line) in source.lines().enumerate() {
        let line_no = idx + 1;
        let line = raw_line.trim();

        if line.is_empty() || line.starts_with('#') {
            continue;
        }

        if line.starts_with("@machine") {
            header = parse_machine_header(line, line_no)?;
            continue;
        }

        if let Some(rest) = line.strip_prefix("@state ") {
            let (state_id, inline) = parse_state_declaration(rest, line_no)?;
            let root = machine_root(&header)?;
            let mut node = StateNode::new(state_address(&root, &state_id));

            if state_id.contains('.') {
                if let Some((parent, _)) = state_id.rsplit_once('.') {
                    node = node.with_superstate(state_address(&root, parent));
                }
            }

            let inline = inline.trim();
            let block_state = inline.is_empty();
            if inline == "final" {
                node = node.final_state();
            }

            states
                .entry(state_id.clone())
                .or_insert(StateBuilder { node });
            current_state = Some(state_id.clone());

            if !block_state && inline != "final" {
                let transition = parse_transition_for_state(&root, &state_id, inline, line_no)?;
                transitions.push(transition);
                current_state = None;
            }
            continue;
        }

        if let Some(initial) = line.strip_prefix("@initial ") {
            header.initial = Some(initial.trim().to_string());
            continue;
        }

        if let Some(rest) = line.strip_prefix("+entry:") {
            let state_id = current_state
                .as_deref()
                .ok_or(S5ParseError::ActionOutsideState { line: line_no })?;
            let action = rest.trim();
            if action.is_empty() {
                return Err(S5ParseError::InvalidAction { line: line_no });
            }
            states
                .get_mut(state_id)
                .expect("current state exists")
                .node
                .entry_actions
                .push(action.to_string());
            continue;
        }

        if let Some(rest) = line.strip_prefix("+exit:") {
            let state_id = current_state
                .as_deref()
                .ok_or(S5ParseError::ActionOutsideState { line: line_no })?;
            let action = rest.trim();
            if action.is_empty() {
                return Err(S5ParseError::InvalidAction { line: line_no });
            }
            states
                .get_mut(state_id)
                .expect("current state exists")
                .node
                .exit_actions
                .push(action.to_string());
            continue;
        }

        if line.starts_with('-') {
            let state_id = current_state
                .as_deref()
                .ok_or(S5ParseError::TransitionOutsideState { line: line_no })?;
            let root = machine_root(&header)?;
            transitions.push(parse_transition_for_state(&root, state_id, line, line_no)?);
            continue;
        }

        return Err(S5ParseError::InvalidState { line: line_no });
    }

    let root = machine_root(&header)?;
    let mut machine = StateMachineSpec::new(root.clone());
    machine.metadata.insert("format".into(), S5_FORMAT.into());
    if let Some(datamodel) = header.datamodel {
        machine.metadata.insert("datamodel".into(), datamodel);
    }
    machine.metadata.extend(header.metadata);

    let mut referenced_states = BTreeSet::new();
    for transition in &transitions {
        referenced_states.insert(transition.source.local_id.clone());
        referenced_states.insert(transition.target.local_id.clone());
    }

    for state_id in referenced_states {
        states
            .entry(state_id.clone())
            .or_insert_with(|| StateBuilder {
                node: StateNode::new(state_address(&root, state_id)),
            });
    }

    let initial = header
        .initial
        .map(|initial| {
            if states.contains_key(&initial) {
                Ok(state_address(&root, initial))
            } else {
                Err(S5ParseError::UnknownInitial(initial))
            }
        })
        .transpose()?;

    for (state_id, mut builder) in states {
        if initial
            .as_ref()
            .map(|address| address.local_id == state_id)
            .unwrap_or(false)
        {
            builder.node.is_initial = true;
        }
        machine.add_state(builder.node);
    }

    for transition in transitions {
        machine.add_transition(transition);
    }

    Ok(S5Document { machine, initial })
}

pub fn render_s5(document: &S5Document) -> String {
    let machine = &document.machine;
    let mut out = String::new();
    out.push_str(&format!("@machine id={}", machine.address.local_id));

    if let Some(initial) = &document.initial {
        out.push_str(&format!(" initial={}", initial.local_id));
    }

    if let Some(datamodel) = machine.metadata.get("datamodel") {
        out.push_str(&format!(" datamodel={datamodel}"));
    }
    out.push('\n');

    for (address, state) in &machine.states {
        let mut block = !state.entry_actions.is_empty() || !state.exit_actions.is_empty();
        let state_transitions = machine
            .transitions
            .iter()
            .filter(|transition| transition.source == *address)
            .collect::<Vec<_>>();

        block |= state_transitions.len() > 1;

        if state.is_final && state_transitions.is_empty() && !block {
            out.push_str(&format!("@state {} final\n", address.local_id));
            continue;
        }

        if !block && state_transitions.len() == 1 {
            out.push_str(&format!(
                "@state {} {}\n",
                address.local_id,
                render_transition_rhs(state_transitions[0])
            ));
            continue;
        }

        out.push_str(&format!("@state {}:\n", address.local_id));
        for action in &state.entry_actions {
            out.push_str(&format!("  +entry: {action}\n"));
        }
        for action in &state.exit_actions {
            out.push_str(&format!("  +exit: {action}\n"));
        }
        for transition in state_transitions {
            out.push_str(&format!("  {}\n", render_transition_rhs(transition)));
        }
    }

    out
}

fn parse_machine_header(line: &str, line_no: usize) -> Result<MachineHeader, S5ParseError> {
    let rest = line
        .strip_prefix("@machine")
        .ok_or(S5ParseError::InvalidMachine { line: line_no })?
        .trim()
        .trim_start_matches('(')
        .trim_end_matches(')');

    let mut header = MachineHeader::default();
    for item in rest.split_whitespace() {
        let (key, value) = item
            .split_once('=')
            .ok_or(S5ParseError::InvalidMachine { line: line_no })?;
        let value = value
            .trim_matches('"')
            .trim_matches('\'')
            .trim_end_matches(',');
        match key {
            "id" => header.id = value.to_string(),
            "initial" => header.initial = Some(value.to_string()),
            "datamodel" => header.datamodel = Some(value.to_string()),
            _ => {
                header.metadata.insert(key.to_string(), value.to_string());
            }
        }
    }

    if header.id.is_empty() {
        return Err(S5ParseError::InvalidMachine { line: line_no });
    }

    Ok(header)
}

fn parse_state_declaration(rest: &str, line_no: usize) -> Result<(String, String), S5ParseError> {
    let rest = rest.trim();
    if rest.is_empty() {
        return Err(S5ParseError::InvalidState { line: line_no });
    }

    if let Some(state_id) = rest.strip_suffix(':') {
        let state_id = state_id.trim();
        if state_id.is_empty() {
            return Err(S5ParseError::InvalidState { line: line_no });
        }
        return Ok((state_id.to_string(), String::new()));
    }

    let mut parts = rest.splitn(2, char::is_whitespace);
    let state_id = parts.next().unwrap_or_default().trim();
    if state_id.is_empty() {
        return Err(S5ParseError::InvalidState { line: line_no });
    }
    Ok((
        state_id.to_string(),
        parts.next().unwrap_or_default().trim().to_string(),
    ))
}

fn parse_transition_for_state(
    root: &LogicalAddress,
    source: &str,
    text: &str,
    line_no: usize,
) -> Result<StateTransition, S5ParseError> {
    let transition = text
        .trim()
        .strip_prefix('-')
        .ok_or(S5ParseError::InvalidTransition { line: line_no })?;
    let (left, target) = transition
        .split_once("->")
        .ok_or(S5ParseError::InvalidTransition { line: line_no })?;
    let (target, emits) = split_once_trim(target, "=>");
    if target.is_empty() {
        return Err(S5ParseError::InvalidTransition { line: line_no });
    }

    let (event_guard, action) = split_once_trim(left, "/");
    let (event, guard) = parse_event_guard(event_guard, line_no)?;
    let mut transition = StateTransition::new(
        state_address(root, source),
        event,
        state_address(root, target),
    );

    if let Some(guard) = guard {
        transition = transition.with_guard(SymbolicRule::new(
            format!("guard:{}:{}", source, target),
            guard,
        ));
    }

    if let Some(action) = action {
        transition.actions.push(action.to_string());
    }

    if let Some(emits) = emits {
        for emitted in emits
            .split(',')
            .map(str::trim)
            .filter(|value| !value.is_empty())
        {
            let (name, clif) = split_once_trim(emitted, ":clif=");
            let mut event = StateMachineEvent::new(name);
            if let Some(clif) = clif {
                event = event.with_clif(ClifPayload::new(clif));
            }
            transition = transition.emits(event);
        }
    }

    Ok(transition)
}

fn parse_event_guard(text: &str, line_no: usize) -> Result<(String, Option<String>), S5ParseError> {
    let text = text.trim();
    if text.is_empty() {
        return Err(S5ParseError::InvalidTransition { line: line_no });
    }

    if let Some((event, rest)) = text.split_once('[') {
        let guard = rest
            .strip_suffix(']')
            .ok_or(S5ParseError::InvalidTransition { line: line_no })?;
        let event = event.trim();
        if event.is_empty() || guard.trim().is_empty() {
            return Err(S5ParseError::InvalidTransition { line: line_no });
        }
        return Ok((event.to_string(), Some(guard.trim().to_string())));
    }

    Ok((text.to_string(), None))
}

fn render_transition_rhs(transition: &StateTransition) -> String {
    let mut out = format!("-{}", transition.event);
    if let Some(guard) = &transition.guard {
        out.push_str(&format!("[{}]", guard.expression));
    }
    if let Some(action) = transition.actions.first() {
        out.push_str(&format!("/{action}"));
    }
    out.push_str(&format!("-> {}", transition.target.local_id));
    if !transition.emitted_events.is_empty() {
        out.push_str(" => ");
        out.push_str(
            &transition
                .emitted_events
                .iter()
                .map(|event| match &event.clif {
                    Some(clif) => format!("{}:clif={}", event.name, clif.text),
                    None => event.name.clone(),
                })
                .collect::<Vec<_>>()
                .join(", "),
        );
    }
    out
}

fn split_once_trim<'a>(text: &'a str, delimiter: &str) -> (&'a str, Option<&'a str>) {
    match text.split_once(delimiter) {
        Some((left, right)) => (left.trim(), Some(right.trim())),
        None => (text.trim(), None),
    }
}

fn machine_root(header: &MachineHeader) -> Result<LogicalAddress, S5ParseError> {
    if header.id.is_empty() {
        return Err(S5ParseError::MissingMachine);
    }
    Ok(LogicalAddress::new(["statechart"], &header.id))
}

fn state_address(root: &LogicalAddress, state_id: impl Into<String>) -> LogicalAddress {
    root.child("state", state_id)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde::Deserialize;

    #[derive(Debug, Deserialize)]
    struct Fixture {
        source: String,
        machine_id: String,
        initial: String,
        states: Vec<String>,
        events: Vec<String>,
    }

    #[test]
    fn parses_s5_fixture_into_state_machine_spec() {
        let fixture = include_str!("../tests/fixtures/s5_lamp.json");
        let fixture: Fixture = serde_json::from_str(fixture).unwrap();

        let document = parse_s5(&fixture.source).unwrap();

        assert_eq!(document.machine.address.local_id, fixture.machine_id);
        assert_eq!(document.initial.unwrap().local_id, fixture.initial);
        for state in fixture.states {
            assert!(
                document
                    .machine
                    .states
                    .keys()
                    .any(|address| address.local_id == state)
            );
        }
        for event in fixture.events {
            assert!(
                document
                    .machine
                    .transitions
                    .iter()
                    .any(|transition| transition.event == event)
            );
        }
    }

    #[test]
    fn preserves_guards_actions_emitted_events_and_clif() {
        let document = parse_s5(
            r#"
            @machine id=workflow initial=idle datamodel=null
            @state idle -submit[valid(x)]/enqueue(x)-> processing => accepted:clif=(accepted x)
            @state processing final
            "#,
        )
        .unwrap();

        let transition = &document.machine.transitions[0];
        assert_eq!(transition.event, "submit");
        assert_eq!(
            transition
                .guard
                .as_ref()
                .map(|guard| guard.expression.as_str()),
            Some("valid(x)")
        );
        assert_eq!(transition.actions, vec!["enqueue(x)"]);
        assert_eq!(transition.emitted_events[0].name, "accepted");
        assert_eq!(
            transition.emitted_events[0]
                .clif
                .as_ref()
                .map(|clif| clif.text.as_str()),
            Some("(accepted x)")
        );
    }

    #[test]
    fn renders_parseable_s5() {
        let document = parse_s5(
            r#"
            @machine id=lamp initial=off
            @state off -switch-> on
            @state on:
              +entry: log_on()
              -switch-> off
            "#,
        )
        .unwrap();

        let rendered = render_s5(&document);
        let reparsed = parse_s5(&rendered).unwrap();

        assert_eq!(reparsed.machine.address, document.machine.address);
        assert_eq!(reparsed.machine.states.len(), document.machine.states.len());
        assert_eq!(
            reparsed.machine.transitions.len(),
            document.machine.transitions.len()
        );
    }
}
