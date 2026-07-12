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
//!
//! Fields left for future triples: `members`, `compliance`, `aliases`, `url`.

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
