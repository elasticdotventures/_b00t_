//! FOL-driven adjacent skill discovery.
//!
//! Uses pre-computed Horn results + raw triple scan to find skills
//! adjacent to a goal in the knowledge graph.
//!
//! Scoring weights:
//!   +3  skill was `b00t:informedBy` from a past goal whose `b00t:goalText` overlaps goal_words
//!   +1  skill was `b00t:informedBy` from any past goal (general recall fallback)
//!   +2  skill is Horn-reachable from a matched goal node (FOL transitive closure)
//!   +1  skill is Horn-reachable from ANY past goal (broad adjacency)

use std::collections::{HashMap, HashSet};
use super::HornResults;

/// Find skills adjacent to a goal via FOL graph traversal.
///
/// `triples` — raw SPO triples from the namespace (needed for goalText lookup)
/// `horn`    — pre-computed Horn results (reachable, depends_on, informed_by)
/// `goal_words` — lowercased tokens from the current goal text
pub fn find_adjacent(
    triples: &[(String, String, String)],
    horn: &HornResults,
    goal_words: &[&str],
    top: usize,
) -> Vec<(String, u32)> {
    let mut scores: HashMap<String, u32> = HashMap::new();

    // Step 1: find past goal nodes whose goalText overlaps current goal
    let matched_goals: HashSet<&str> = triples
        .iter()
        .filter(|(_, p, o)| {
            p == "b00t:goalText"
                && goal_words.iter().any(|w| o.to_lowercase().contains(*w))
        })
        .map(|(s, _, _)| s.as_str())
        .collect();

    // Step 2: score informedBy objects by goal text match quality
    for (subj, pred, obj) in triples {
        if pred == "b00t:informedBy" {
            let w: u32 = if matched_goals.contains(subj.as_str()) { 3 } else { 1 };
            *scores.entry(obj.clone()).or_default() += w;
        }
    }

    // Step 3: FOL reachability — anything reachable from matched goal nodes gets bonus
    // 🤓 horn.reachable includes transitive closure via all edge predicates (including informedBy)
    for (from, to) in &horn.reachable {
        let w: u32 = if matched_goals.contains(from.as_str()) { 2 } else { 1 };
        *scores.entry(to.clone()).or_default() += w;
    }

    // Step 4: skills that depend_on a high-scoring seed (+2) also get boosted
    let seeds: HashSet<String> = scores
        .iter()
        .filter(|(_, v)| **v >= 3)
        .map(|(k, _)| k.clone())
        .collect();
    for (from, to) in &horn.depends_on {
        if seeds.contains(from.as_str()) && !scores.contains_key(to) {
            *scores.entry(to.clone()).or_default() += 2;
        }
    }

    // Exclude goal-node URIs and datum subjects — these are not loadable skills
    scores.retain(|k, _| {
        !k.starts_with("ooda:")
            && !k.starts_with("b00t:datum/")
            && !k.starts_with("b00t:goal")
    });

    let mut ranked: Vec<(String, u32)> = scores.into_iter().collect();
    ranked.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(&b.0)));
    ranked.truncate(top);
    ranked
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::reasoning::graph_rules;

    fn t(s: &str, p: &str, o: &str) -> (String, String, String) {
        (s.into(), p.into(), o.into())
    }

    #[test]
    fn test_matched_goal_scores_3() {
        let triples = vec![
            t("ooda:goal:abc", "b00t:goalText",   "debug llamacpp systemd"),
            t("ooda:goal:abc", "b00t:informedBy", "llamacpp"),
        ];
        let horn = graph_rules::derive(triples.clone());
        let adj = find_adjacent(&triples, &horn, &["llamacpp", "debug"], 5);
        let score = adj.iter().find(|(s, _)| s == "llamacpp").map(|(_, c)| *c);
        // direct match (+3) + reachable from goal (+2) = 5
        assert!(score.is_some_and(|s| s >= 3));
    }

    #[test]
    fn test_unmatched_goal_falls_back_score_1() {
        let triples = vec![
            t("ooda:goal:xyz", "b00t:goalText",   "unrelated topic"),
            t("ooda:goal:xyz", "b00t:informedBy", "rust"),
        ];
        let horn = graph_rules::derive(triples.clone());
        let adj = find_adjacent(&triples, &horn, &["completely", "different"], 5);
        // Falls back: +1 (general recall) + reachable bonus
        assert!(adj.iter().any(|(s, _)| s == "rust"));
    }

    #[test]
    fn test_excludes_ooda_and_datum_uris() {
        let triples = vec![
            t("ooda:goal:abc", "b00t:goalText",   "debug rust"),
            t("ooda:goal:abc", "b00t:informedBy", "rust"),
        ];
        let horn = graph_rules::derive(triples.clone());
        let adj = find_adjacent(&triples, &horn, &["rust"], 10);
        assert!(!adj.iter().any(|(s, _)| s.starts_with("ooda:")));
        assert!(!adj.iter().any(|(s, _)| s.starts_with("b00t:datum/")));
    }

    #[test]
    fn test_depends_on_propagation() {
        let triples = vec![
            t("ooda:goal:abc", "b00t:goalText",   "build rust project"),
            t("ooda:goal:abc", "b00t:informedBy", "rust"),
            t("rust",          "b00t:dependsOn",  "cargo"),
        ];
        let horn = graph_rules::derive(triples.clone());
        let adj = find_adjacent(&triples, &horn, &["rust", "build"], 10);
        // cargo is reachable from rust (which is reachable from goal) → should appear
        assert!(adj.iter().any(|(s, _)| s == "cargo"));
    }

    #[test]
    fn test_multiple_goals_aggregate_score() {
        let triples = vec![
            t("ooda:goal:a1", "b00t:goalText",   "learn rust async"),
            t("ooda:goal:a1", "b00t:informedBy", "rust"),
            t("ooda:goal:a2", "b00t:goalText",   "debug rust memory"),
            t("ooda:goal:a2", "b00t:informedBy", "rust"),
            t("ooda:goal:a2", "b00t:informedBy", "valgrind"),
        ];
        let horn = graph_rules::derive(triples.clone());
        let adj = find_adjacent(&triples, &horn, &["rust"], 10);
        let rust_score = adj.iter().find(|(s, _)| s == "rust").map(|(_, c)| *c).unwrap_or(0);
        let valg_score = adj.iter().find(|(s, _)| s == "valgrind").map(|(_, c)| *c).unwrap_or(0);
        // rust appears in both matching goals → higher score than valgrind
        assert!(rust_score > valg_score);
    }

    #[test]
    fn test_top_limits_results() {
        let triples = (0u32..20).flat_map(|i| {
            let g = format!("ooda:goal:g{i}");
            let s = format!("skill{i}");
            vec![
                (g.clone(), "b00t:goalText".into(),   format!("goal about {s}")),
                (g,         "b00t:informedBy".into(), s),
            ]
        }).collect::<Vec<_>>();
        let horn = graph_rules::derive(triples.clone());
        let adj = find_adjacent(&triples, &horn, &["goal", "about"], 5);
        assert_eq!(adj.len(), 5);
    }
}
