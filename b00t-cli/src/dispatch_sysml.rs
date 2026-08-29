//! Represents `dispatch::default_dispatch_chain()` — the real, live router
//! `b00t <name>` resolution walks — as a version-controlled, round-trip-validated
//! SysML v2 model. First prototype of the b00t SysML v2 spine consolidation epic's
//! P1 milestone (`elasticdotventures/_b00t_#1177`): a Rust type (`dyn DispatchMode`)
//! becomes a compiled SysML v2 output, validated against the real `sysml-v2-parser`
//! grammar via `ufo_types::sysml::validate_sysml_v2` — the same round-trip discipline
//! `ledgrrr`'s `holon-viz::sysml_v2_roundtrip` tests use.
//!
//! Node/edge shape follows `systhread-core::iso_ir::extract_pipeline`'s "sequence"
//! pattern exactly (each mode a node, consecutive modes in priority order joined by
//! a `sequence` edge) — the dispatch chain genuinely is an ordered phase sequence,
//! not a general graph, so no richer shape is needed for this first prototype.
//!
//! `DispatchMode` isn't `Stereotyped` — each mode's classification is derived here,
//! directly from its own `name()`, rather than requiring every implementor to carry
//! a hand-written `impl Stereotyped` block just to be exportable. A blanket
//! `impl<T: DispatchMode> Stereotyped for T` doesn't work here even though it looks
//! tempting: `Stereotyped` would need to be a supertrait for `dispatch.rs`'s own
//! `mode.ufo_stereotype()` calls to typecheck, and a supertrait bound must already be
//! satisfied before a type can implement the subtrait — circular.

use ufo_types::iso_ir::{Edge, Node};
use ufo_types::stereotype::UfoStereotype;

use crate::dispatch::default_dispatch_chain;

/// Build the `Node`/`Edge` iso-IR for the dispatch chain's current, real order.
pub fn dispatch_chain_iso_ir() -> (Vec<Node>, Vec<Edge>) {
    let chain = default_dispatch_chain();
    let names: Vec<&'static str> = chain.iter().map(|mode| mode.name()).collect();

    let nodes: Vec<Node> = names
        .iter()
        .map(|name| Node {
            id: (*name).to_string(),
            label: (*name).to_string(),
            part_type: "DispatchMode".to_string(),
        })
        .collect();

    let edges: Vec<Edge> = names
        .windows(2)
        .map(|pair| Edge {
            id: format!("{}_next", pair[0]),
            from: pair[0].to_string(),
            to: pair[1].to_string(),
            edge_type: "sequence".to_string(),
            kind: None,
        })
        .collect();

    (nodes, edges)
}

/// Export the dispatch chain as SysML v2 text: one `part def` per mode, each
/// specializing (`:>`) the previous mode in priority order — the chain genuinely
/// is a linear priority sequence, so each step's real predecessor becomes its
/// SysML v2 base type, with a `UfoStereotype::Process` classification (derived
/// straight from the mode's own `name()`) and the sequence relationship both
/// recorded as comments (matching `critter-keeper::mesh_safety_gate`'s comment
/// convention).
///
/// `holon-viz::SysmlV2Emitter`'s edge-emission shape (`part def X : Y`, plain
/// `:` rather than `:>`) was the original model for this function, but that
/// path turned out to be dead code in `holon-viz`'s own test fixture — its
/// `CytoscapeGraph` never actually carries containment edges from
/// `Holon::child`'s parent link, so `:` was never exercised against the real
/// `sysml-v2-parser` grammar. This function's own round-trip test below is
/// what caught that: SysML v2's specialization operator is `:>`, not `:`, and
/// a `part def` name also cannot be redeclared once already emitted as a plain
/// definition — hence chaining `:>` directly off each mode's own declaration
/// instead of the original two-pass (all-nodes, then separately-named edges)
/// shape.
pub fn dispatch_chain_to_sysml_v2() -> String {
    let chain = default_dispatch_chain();

    let mut out = String::from("package B00tDispatchChain {\n");

    for (i, mode) in chain.iter().enumerate() {
        let stereotype = UfoStereotype::Process(mode.name().to_string());
        if i == 0 {
            out.push_str(&format!(
                "    part def {} {{\n        // {stereotype}\n    }}\n",
                mode.name()
            ));
        } else {
            let prev = chain[i - 1].name();
            out.push_str(&format!(
                "    part def {} :> {} {{\n        // {stereotype}\n        // sequence: {} -> {}\n    }}\n",
                mode.name(),
                prev,
                prev,
                mode.name()
            ));
        }
    }

    out.push_str("}\n");
    out
}

/// Export the dispatch chain as a Mermaid flowchart — the P2 milestone's first
/// prototype (`elasticdotventures/_b00t_#1177`), following `ledger-core`'s
/// `WorkflowToml::to_mermaid()` precedent (one declarative source, multiple
/// generated views) but with a shape that matches this router's actual
/// semantics rather than reusing `dispatch_chain_to_sysml_v2`'s linear
/// `:>`-chain shape verbatim.
///
/// `resolve_all_datum_dispatches` (`dispatch.rs`) is NOT a fail-fast sequential
/// chain — every mode's `try_resolve` runs independently against the same
/// candidate (fan-out), and cross-mode precedence is resolved afterward,
/// uniformly, by the stereotype-implication filter (see that function's own
/// doc comment). So this flowchart draws every mode as a parallel branch from
/// the same input, all converging on one collection step, followed by the
/// two real post-processing stages `dispatch.rs` actually performs: dropping
/// less-specific implied matches, then (only for the single-result caller,
/// `resolve_datum_dispatch`) preferring Runtime/CliPassthrough/Polyseme over
/// any other remaining match.
///
/// Follows `to_mermaid()`'s own test convention: assert on string content,
/// not a parsed Mermaid AST — there's no Mermaid grammar validator in this
/// workspace the way `sysml-v2-parser` exists for SysML v2.
pub fn dispatch_chain_to_mermaid() -> String {
    let chain = default_dispatch_chain();

    let mut out = String::from(
        "%%{ init: { 'theme': 'neutral' } }%%\nflowchart TD\n\
         %% Generated from dispatch::default_dispatch_chain()\n\
         \x20   Start([\"candidate + path\"])\n",
    );

    for mode in &chain {
        out.push_str(&format!(
            "    Start --> {name}[\"{name}::try_resolve\"]\n",
            name = mode.name()
        ));
        out.push_str(&format!("    {name} --> Collect\n", name = mode.name()));
    }

    out.push_str(
        "    Collect{{\"collect all Some(_) hits\"}}\n\
         \x20   Collect --> Filter[\"drop implied matches\\n(stereotype hierarchy)\"]\n\
         \x20   Filter --> Prefer[\"prefer Runtime > CliPassthrough > Polyseme\\n(resolve_datum_dispatch only)\"]\n\
         \x20   Prefer -->|match found| Resolved([\"DatumDispatch\"])\n\
         \x20   Prefer -->|no matches| Unresolved([\"None\"])\n",
    );

    out
}

/// Export `resolve_datum_dispatch`'s priority/collapse rule as an executable
/// Rhai script — P2's second prototype (`elasticdotventures/_b00t_#1177`).
///
/// Rhai stays strictly in its existing, proven role across this ecosystem
/// (diagram/state-machine/guard-expression codegen — `ledger-core::workflow.rs`,
/// `mdbook-rhai-mermaid`, `nem-poweragent-lab`'s `mission-engine`) — never MCP
/// transport/dispatch. The fan-out itself (running every mode's `try_resolve`,
/// real file I/O and TOML parsing) stays real Rust; this script only takes
/// the resulting hit list (mode names that returned `Some(_)`) and expresses
/// `resolve_datum_dispatch`'s preference rule — Runtime > CliPassthrough >
/// Polyseme, else the first remaining hit — the same hand-maintained
/// precedence `dispatch.rs`'s own `matches!` block encodes (not
/// auto-derived from the chain, since that precedence isn't derivable from
/// mode order either — this mirrors dispatch.rs by hand exactly as it does).
///
/// Follows `to_rhai()`'s own shape (`// GENERATED from ...` header) but its
/// test, unlike `to_mermaid()`'s string-containment convention, actually
/// executes the generated script inside a real `rhai::Engine` — proving the
/// priority rule, not just its presence as text.
///
/// An if/return chain, not a `switch`: Rhai's `switch` matches a value
/// against literal patterns (`switch x { 1 => ..., _ => ... }`) — it can't
/// take an arbitrary boolean expression as a case, so `switch true { expr
/// => ..., .. }` doesn't parse. An if/return chain expresses the same
/// first-match-wins precedence and is valid Rhai.
pub fn dispatch_chain_to_rhai() -> String {
    "// GENERATED from dispatch::default_dispatch_chain()\n\
     // Fan-out (running every mode's try_resolve) and its file I/O stay\n\
     // real Rust — this only expresses resolve_datum_dispatch's\n\
     // Runtime > CliPassthrough > Polyseme preference rule.\n\
     fn resolve_dispatch(hits) {\n\
     \x20   if hits.len() == 0 {\n\
     \x20       return ();\n\
     \x20   }\n\
     \x20   if hits.contains(\"RuntimeMode\") {\n\
     \x20       return \"RuntimeMode\";\n\
     \x20   }\n\
     \x20   if hits.contains(\"CliPassthroughMode\") {\n\
     \x20       return \"CliPassthroughMode\";\n\
     \x20   }\n\
     \x20   if hits.contains(\"PolysemeMode\") {\n\
     \x20       return \"PolysemeMode\";\n\
     \x20   }\n\
     \x20   hits[0]\n\
     }\n"
    .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn iso_ir_has_one_node_per_chain_entry_and_sequence_edges_between_consecutive_pairs() {
        let (nodes, edges) = dispatch_chain_iso_ir();
        let chain_len = default_dispatch_chain().len();

        assert_eq!(nodes.len(), chain_len);
        assert_eq!(edges.len(), chain_len - 1);
        assert!(edges.iter().all(|e| e.edge_type == "sequence"));

        assert_eq!(nodes[0].id, "RuntimeMode");
        assert_eq!(nodes[1].id, "CliPassthroughMode");
        assert_eq!(edges[0].from, "RuntimeMode");
        assert_eq!(edges[0].to, "CliPassthroughMode");
    }

    #[test]
    fn sysml_v2_export_contains_every_mode_and_every_sequence_edge() {
        let sysml = dispatch_chain_to_sysml_v2();
        for mode in &default_dispatch_chain() {
            assert!(
                sysml.contains(mode.name()),
                "missing mode {} in:\n{sysml}",
                mode.name()
            );
        }
        assert!(sysml.contains("sequence: RuntimeMode -> CliPassthroughMode"));
    }

    #[test]
    fn sysml_v2_export_round_trips_through_the_real_parser() {
        let sysml = dispatch_chain_to_sysml_v2();
        let result = ufo_types::sysml::validate_sysml_v2(&sysml);
        assert!(
            result.disposition.is_satisfied(),
            "dispatch_chain_to_sysml_v2() output failed to parse as SysML v2: {:?}\n---\n{sysml}",
            result.disposition
        );
    }

    #[test]
    fn mermaid_export_is_a_flowchart_with_every_mode_as_a_parallel_branch() {
        let mermaid = dispatch_chain_to_mermaid();
        assert!(mermaid.contains("flowchart TD"));
        for mode in &default_dispatch_chain() {
            assert!(
                mermaid.contains(mode.name()),
                "missing mode {} in:\n{mermaid}",
                mode.name()
            );
        }
    }

    #[test]
    fn mermaid_export_models_fanout_not_a_linear_chain() {
        // Every mode branches directly off Start (fan-out), not off the
        // previous mode (which would model a fail-fast sequential chain —
        // not what resolve_all_datum_dispatches actually does).
        let mermaid = dispatch_chain_to_mermaid();
        for mode in &default_dispatch_chain() {
            assert!(
                mermaid.contains(&format!("Start --> {}", mode.name())),
                "expected {} to branch directly off Start in:\n{mermaid}",
                mode.name()
            );
        }
        assert!(mermaid.contains("Collect"));
        assert!(mermaid.contains("prefer Runtime > CliPassthrough > Polyseme"));
    }

    #[test]
    fn rhai_script_prefers_runtime_over_other_hits() {
        let script = dispatch_chain_to_rhai();
        let engine = rhai::Engine::new();
        let ast = engine
            .compile(&script)
            .expect("generated rhai script failed to compile");

        let hits: rhai::Array = vec![
            "PolysemeMode".to_string().into(),
            "RuntimeMode".to_string().into(),
            "CliPassthroughMode".to_string().into(),
        ];
        let result: String = engine
            .call_fn(&mut rhai::Scope::new(), &ast, "resolve_dispatch", (hits,))
            .expect("resolve_dispatch call failed");
        assert_eq!(result, "RuntimeMode");
    }

    #[test]
    fn rhai_script_prefers_cli_passthrough_over_polyseme_when_runtime_absent() {
        let script = dispatch_chain_to_rhai();
        let engine = rhai::Engine::new();
        let ast = engine.compile(&script).unwrap();

        let hits: rhai::Array = vec![
            "PolysemeMode".to_string().into(),
            "CliPassthroughMode".to_string().into(),
        ];
        let result: String = engine
            .call_fn(&mut rhai::Scope::new(), &ast, "resolve_dispatch", (hits,))
            .unwrap();
        assert_eq!(result, "CliPassthroughMode");
    }

    #[test]
    fn rhai_script_falls_back_to_first_hit_for_an_unlisted_mode() {
        let script = dispatch_chain_to_rhai();
        let engine = rhai::Engine::new();
        let ast = engine.compile(&script).unwrap();

        let hits: rhai::Array = vec!["OodaMode".to_string().into()];
        let result: String = engine
            .call_fn(&mut rhai::Scope::new(), &ast, "resolve_dispatch", (hits,))
            .unwrap();
        assert_eq!(result, "OodaMode");
    }

    #[test]
    fn rhai_script_returns_unit_when_no_hits() {
        let script = dispatch_chain_to_rhai();
        let engine = rhai::Engine::new();
        let ast = engine.compile(&script).unwrap();

        let hits: rhai::Array = vec![];
        let result: () = engine
            .call_fn(&mut rhai::Scope::new(), &ast, "resolve_dispatch", (hits,))
            .unwrap();
        let _ = result;
    }
}
