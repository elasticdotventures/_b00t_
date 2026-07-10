//! Compile BootDatum filesystem into SPO triples for Horn graph reasoning.
//!
//! Reads all datums from `_b00t_/` and emits structural relationship triples
//! using the `b00t:` predicate namespace (OWL2/BFO/SysMLv2 forward-compatible).
//!
//! # Subject URI convention
//!   `b00t:datum/<key>`  e.g. `b00t:datum/rust.cli`, `b00t:datum/docker.cli`
//!
//! # Predicate → datum field mapping
//!
//! | Predicate          | BootDatum field(s)                       | Semantic               |
//! |--------------------|------------------------------------------|------------------------|
//! | b00t:dependsOn     | depends_on                               | runtime prerequisite   |
//! | b00t:requires      | require                                  | hard constraint        |
//! | b00t:hasPart       | entangled_{cli,mcp,agents,docker,k8s,    | component composition  |
//! |                    |   ai_models,apis}                        |                        |
//! | b00t:hasKeyword    | keywords                                 | search affinity        |
//! | b00t:hasSkill      | skills                                   | capability tag         |
//! | b00t:hasType       | datum_type                               | classifier             |
//! | rdfs:label         | hint                                     | display name           |
//! | b00t:alias         | aliases                                  | alternate name         |
//! | b00t:unlocks       | unlocks, [b00t.skill].unlocks            | blessing tool globs    |
//! | b00t:dependsOn     | [b00t.skill].depends_on                  | skill prerequisite     |
//! | b00t:composes_with | [b00t.compose].composes_with             | capability composition |
//! | b00t:audits        | [b00t.compose].audits                    | verification pairing   |
//! | b00t:supersedes    | [b00t.compose].supersedes                | obsolescence           |
//! | b00t:measured      | [b00t.compose].measured                  | "metric=value" evidence|
//!
//! Fields left for future triples: `members`, `compliance`, `url`.

use anyhow::Result;
use crate::datum_utils::get_all_datums;

/// Compile all datums in `b00t_path` into SPO triples suitable for Horn reasoning.
///
/// Results feed `b00t_c0re_lib::reasoning::graph_rules::derive` and
/// `b00t_c0re_lib::reasoning::adjacency::find_adjacent`.
pub fn compile_datum_triples(b00t_path: &str) -> Result<Vec<(String, String, String)>> {
    let datums = get_all_datums(b00t_path)?;
    let mut triples: Vec<(String, String, String)> = Vec::with_capacity(datums.len() * 4);

    for (key, datum) in &datums {
        let subj = format!("b00t:datum/{key}");

        // Type classifier
        if let Some(ref dt) = datum.datum_type {
            let type_str = format!("{dt:?}").to_lowercase();
            triples.push((subj.clone(), "b00t:hasType".into(), format!("b00t:type/{type_str}")));
        }

        // Display label (rdfs:label — loads into OWL2 annotation store cleanly)
        if !datum.hint.is_empty() {
            triples.push((subj.clone(), "rdfs:label".into(), datum.hint.clone()));
        }

        // Runtime prerequisites (transitive closure via DependsOn Horn rule)
        emit_ref_list(&subj, "b00t:dependsOn", datum.depends_on.as_deref(), &mut triples);

        // Hard constraints (e.g. require = ["bash", "jq"])
        emit_ref_list(&subj, "b00t:requires", datum.require.as_deref(), &mut triples);

        // Component entanglement → hasPart (BFO bfo:has-part, SysMLv2 PartUsage)
        // 🤓 entangled_* = sibling components that together form a complete capability.
        //    A CLI tool + its MCP server + its AI model are parts of the same conceptual block.
        emit_ref_list(&subj, "b00t:hasPart", datum.entangled_cli.as_deref(),       &mut triples);
        emit_ref_list(&subj, "b00t:hasPart", datum.entangled_mcp.as_deref(),       &mut triples);
        emit_ref_list(&subj, "b00t:hasPart", datum.entangled_agents.as_deref(),    &mut triples);
        emit_ref_list(&subj, "b00t:hasPart", datum.entangled_docker.as_deref(),    &mut triples);
        emit_ref_list(&subj, "b00t:hasPart", datum.entangled_k8s.as_deref(),       &mut triples);
        emit_ref_list(&subj, "b00t:hasPart", datum.entangled_ai_models.as_deref(), &mut triples);
        emit_ref_list(&subj, "b00t:hasPart", datum.entangled_apis.as_deref(),      &mut triples);

        // Capability tags (literal annotations, not graph edges)
        emit_literal_list(&subj, "b00t:hasKeyword", datum.keywords.as_deref(), &mut triples);
        emit_literal_list(&subj, "b00t:hasSkill",   datum.skills.as_deref(),   &mut triples);

        // Alternate names (literals — lexical aliases, not datum refs)
        emit_literal_list(&subj, "b00t:alias", datum.aliases.as_deref(), &mut triples);

        // Blessing authorization: tool globs unlocked when this datum is learned
        emit_literal_list(&subj, "b00t:unlocks", datum.unlocks.as_deref(), &mut triples);

        // [b00t.compose] — composition knowledge (previously comment-prose only)
        if let Some(ref compose) = datum.compose {
            emit_ref_list(&subj, "b00t:composes_with", compose.composes_with.as_deref(), &mut triples);
            emit_ref_list(&subj, "b00t:audits",        compose.audits.as_deref(),        &mut triples);
            emit_ref_list(&subj, "b00t:supersedes",    compose.supersedes.as_deref(),    &mut triples);
            if let Some(ref measured) = compose.measured {
                for m in measured {
                    if !m.metric.is_empty() {
                        triples.push((subj.clone(), "b00t:measured".into(), format!("{}={}", m.metric, m.value)));
                    }
                }
            }
        }

        // [b00t.skill] table — generic JSON blob; lift depends_on/unlocks into the
        // graph iff not already emitted from the top-level [b00t] fields.
        // 🤓 skill depends_on reuses b00t:dependsOn (NOT b00t:depends_on) so the
        //    DependsOn Horn transitive-closure rule sees one predicate spelling.
        if let Some(ref skill) = datum.skill {
            emit_skill_table(&subj, skill, &mut triples);
        }
    }

    Ok(triples)
}

/// Emit reference triples (cross-datum object links).
/// Values containing `:` are kept verbatim (already URIs); plain keys are
/// resolved to `b00t:datum/<val>`.
fn emit_ref_list(
    subj: &str,
    pred: &str,
    vals: Option<&[String]>,
    out: &mut Vec<(String, String, String)>,
) {
    let Some(vals) = vals else { return };
    for v in vals {
        let v = v.trim();
        if v.is_empty() { continue; }
        let obj = if v.contains(':') {
            v.to_string()
        } else {
            format!("b00t:datum/{v}")
        };
        out.push((subj.to_string(), pred.to_string(), obj));
    }
}

/// Lift `depends_on` (refs) and `unlocks` (literals) out of the untyped
/// `[b00t.skill]` JSON table, skipping triples already emitted from the
/// top-level `[b00t]` fields.
fn emit_skill_table(
    subj: &str,
    skill: &serde_json::Value,
    out: &mut Vec<(String, String, String)>,
) {
    let str_items = |field: &str| -> Vec<String> {
        skill.get(field)
            .and_then(|v| v.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|v| v.as_str())
                    .map(|s| s.trim().to_string())
                    .filter(|s| !s.is_empty())
                    .collect()
            })
            .unwrap_or_default()
    };

    for dep in str_items("depends_on") {
        let obj = if dep.contains(':') { dep } else { format!("b00t:datum/{dep}") };
        let triple = (subj.to_string(), "b00t:dependsOn".to_string(), obj);
        if !out.contains(&triple) {
            out.push(triple);
        }
    }
    for unlock in str_items("unlocks") {
        let triple = (subj.to_string(), "b00t:unlocks".to_string(), unlock);
        if !out.contains(&triple) {
            out.push(triple);
        }
    }
}

/// Emit literal triples (annotation values, not cross-datum links).
fn emit_literal_list(
    subj: &str,
    pred: &str,
    vals: Option<&[String]>,
    out: &mut Vec<(String, String, String)>,
) {
    let Some(vals) = vals else { return };
    for v in vals {
        let v = v.trim();
        if !v.is_empty() {
            out.push((subj.to_string(), pred.to_string(), v.to_string()));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    fn write_datum(dir: &TempDir, name: &str, content: &str) {
        fs::write(dir.path().join(name), content).unwrap();
    }

    fn triples_for_toml(toml: &str) -> Vec<(String, String, String)> {
        let dir = TempDir::new().unwrap();
        write_datum(&dir, "test.cli.toml", toml);
        let path = dir.path().to_str().unwrap().to_string();
        compile_datum_triples(&path).unwrap()
    }

    #[test]
    fn test_depends_on_emits_b00t_depends_on() {
        let triples = triples_for_toml(r#"
[b00t]
name = "test"
type = "cli"
hint = "A test tool"
depends_on = ["python.cli", "uv.cli"]
        "#);
        let deps: Vec<_> = triples.iter()
            .filter(|(_, p, _)| p == "b00t:dependsOn")
            .collect();
        assert_eq!(deps.len(), 2);
        assert!(deps.iter().any(|(_, _, o)| o == "b00t:datum/python.cli"));
        assert!(deps.iter().any(|(_, _, o)| o == "b00t:datum/uv.cli"));
    }

    #[test]
    fn test_entangled_mcp_emits_has_part() {
        let triples = triples_for_toml(r#"
[b00t]
name = "rust tools"
type = "cli"
hint = "Rust toolchain"
entangled_mcp = ["rust-crate-docs-docker.mcp"]
        "#);
        let parts: Vec<_> = triples.iter()
            .filter(|(_, p, _)| p == "b00t:hasPart")
            .collect();
        assert_eq!(parts.len(), 1);
        assert_eq!(parts[0].2, "b00t:datum/rust-crate-docs-docker.mcp");
    }

    #[test]
    fn test_hint_emits_rdfs_label() {
        let triples = triples_for_toml(r#"
[b00t]
name = "git"
type = "cli"
hint = "Version control system"
        "#);
        let label = triples.iter().find(|(_, p, _)| p == "rdfs:label");
        assert!(label.is_some());
        assert_eq!(label.unwrap().2, "Version control system");
    }

    #[test]
    fn test_subject_uri_format() {
        let triples = triples_for_toml(r#"
[b00t]
name = "cargo"
type = "cli"
hint = "Rust package manager"
depends_on = ["rust.cli"]
        "#);
        let dep = triples.iter().find(|(_, p, _)| p == "b00t:dependsOn").unwrap();
        assert!(dep.0.starts_with("b00t:datum/"));
    }

    #[test]
    fn test_keywords_emit_has_keyword_literals() {
        let triples = triples_for_toml(r#"
[b00t]
name = "jq"
type = "cli"
hint = "JSON processor"
keywords = ["json", "filter", "stream"]
        "#);
        let kw: Vec<_> = triples.iter()
            .filter(|(_, p, _)| p == "b00t:hasKeyword")
            .map(|(_, _, o)| o.as_str())
            .collect();
        assert!(kw.contains(&"json"));
        assert!(kw.contains(&"filter"));
        assert!(kw.contains(&"stream"));
    }

    /// Fixture datums live in tests/fixtures/datum_triples/ (never embedded).
    fn fixture_triples() -> Vec<(String, String, String)> {
        let path = format!("{}/tests/fixtures/datum_triples", env!("CARGO_MANIFEST_DIR"));
        compile_datum_triples(&path).unwrap()
    }

    fn objects_of<'a>(
        triples: &'a [(String, String, String)],
        subj: &str,
        pred: &str,
    ) -> Vec<&'a str> {
        triples.iter()
            .filter(|(s, p, _)| s == subj && p == pred)
            .map(|(_, _, o)| o.as_str())
            .collect()
    }

    #[test]
    fn test_fixture_compose_emits_composes_with_audits_supersedes() {
        let triples = fixture_triples();
        let subj = "b00t:datum/composer.mcp";

        let cw = objects_of(&triples, subj, "b00t:composes_with");
        assert_eq!(cw.len(), 2);
        assert!(cw.contains(&"b00t:datum/grammar-verify"));
        assert!(cw.contains(&"b00t:datum/b00t-lsp"));

        assert_eq!(objects_of(&triples, subj, "b00t:audits"), vec!["b00t:datum/assimilate"]);
        assert_eq!(objects_of(&triples, subj, "b00t:supersedes"), vec!["b00t:datum/whole-file-read"]);
    }

    #[test]
    fn test_fixture_compose_emits_measured_metric_value_pairs() {
        let triples = fixture_triples();
        let measured = objects_of(&triples, "b00t:datum/composer.mcp", "b00t:measured");
        assert_eq!(measured.len(), 2);
        assert!(measured.contains(&"context_savings_record_lesson=94%"));
        assert!(measured.contains(&"context_savings_proxy_chat=83%"));
    }

    #[test]
    fn test_fixture_aliases_emit_alias_literals() {
        let triples = fixture_triples();
        let aliases = objects_of(&triples, "b00t:datum/aliased.skill", "b00t:alias");
        assert_eq!(aliases.len(), 2);
        assert!(aliases.contains(&"ali"));
        assert!(aliases.contains(&"alia"));
        // mcp fixture aliases light up too
        assert_eq!(objects_of(&triples, "b00t:datum/composer.mcp", "b00t:alias"), vec!["kompozer"]);
    }

    #[test]
    fn test_fixture_skill_table_emits_depends_on_and_unlocks() {
        let triples = fixture_triples();
        let subj = "b00t:datum/skilldeps.skill";

        // z3-verify appears top-level AND in [b00t.skill] — dedupe leaves one
        let deps = objects_of(&triples, subj, "b00t:dependsOn");
        assert_eq!(deps.len(), 2, "expected deduped depends_on, got {deps:?}");
        assert!(deps.contains(&"b00t:datum/z3-verify"));
        assert!(deps.contains(&"b00t:datum/gbnf-grammar"));

        let unlocks = objects_of(&triples, subj, "b00t:unlocks");
        assert_eq!(unlocks.len(), 2);
        assert!(unlocks.contains(&"skilldeps::author"));
        assert!(unlocks.contains(&"skilldeps::audit"));
    }

    #[test]
    fn test_top_level_unlocks_emits_unlocks_literals() {
        let triples = triples_for_toml(r#"
[b00t]
name = "rusty"
type = "skill"
hint = "Blessing datum"
unlocks = ["cargo.*", "rustfmt"]
        "#);
        let unlocks: Vec<_> = triples.iter()
            .filter(|(_, p, _)| p == "b00t:unlocks")
            .map(|(_, _, o)| o.as_str())
            .collect();
        assert_eq!(unlocks.len(), 2);
        assert!(unlocks.contains(&"cargo.*"));
        assert!(unlocks.contains(&"rustfmt"));
    }

    #[test]
    fn test_empty_datum_produces_minimal_triples() {
        let triples = triples_for_toml(r#"
[b00t]
name = "empty"
hint = ""
        "#);
        assert!(!triples.iter().any(|(_, p, _)| p == "b00t:dependsOn"));
        assert!(!triples.iter().any(|(_, p, _)| p == "rdfs:label"));
    }
}
