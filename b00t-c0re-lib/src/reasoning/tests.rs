use super::*;

fn spo(s: &str, p: &str, o: &str) -> (String, String, String) {
    (s.into(), p.into(), o.into())
}

fn pair(a: &str, b: &str) -> (String, String) {
    (a.into(), b.into())
}

// ── Horn clause (crepe) tests ──────────────────────────────────────────────

#[test]
fn test_direct_reachability() {
    let triples = vec![spo("a", "b00t:relatedTo", "b")];
    let result = graph_rules::derive(triples);
    assert!(result.reachable.contains(&pair("a", "b")));
}

#[test]
fn test_transitive_reachability_three_hops() {
    let triples = vec![
        spo("a", "b00t:dependsOn", "b"),
        spo("b", "b00t:dependsOn", "c"),
        spo("c", "b00t:relatedTo", "d"),
    ];
    let result = graph_rules::derive(triples);
    assert!(result.reachable.contains(&pair("a", "c")));
    assert!(result.reachable.contains(&pair("a", "d")));
    assert!(!result.reachable.contains(&pair("d", "a")), "graph is directed");
}

#[test]
fn test_depends_on_transitive() {
    let triples = vec![
        spo("llamacpp", "b00t:dependsOn", "systemd"),
        spo("systemd", "b00t:dependsOn", "linux-kernel"),
    ];
    let result = graph_rules::derive(triples);
    let deps = result.dependency_chain("llamacpp");
    assert!(deps.contains(&"systemd".to_string()));
    assert!(deps.contains(&"linux-kernel".to_string()));
}

#[test]
fn test_informed_by_ooda() {
    let triples = vec![
        spo("ooda:goal:abc123", "b00t:informedBy", "llamacpp"),
        spo("ooda:goal:abc123", "b00t:informedBy", "systemd"),
        spo("ooda:goal:def456", "b00t:informedBy", "rust"),
    ];
    let result = graph_rules::derive(triples);
    let skills = result.skills_for_goal("ooda:goal:abc123");
    assert!(skills.contains(&"llamacpp".to_string()));
    assert!(skills.contains(&"systemd".to_string()));
    assert_eq!(skills.len(), 2);
    let other = result.skills_for_goal("ooda:goal:def456");
    assert_eq!(other, vec!["rust".to_string()]);
}

#[test]
fn test_implements_direct() {
    let triples = vec![
        spo("usize", "b00t:implements", "Clone"),
        spo("usize", "b00t:implements", "Copy"),
    ];
    let result = graph_rules::derive(triples);
    let impls = result.types_implementing("Clone");
    assert!(impls.contains(&"usize".to_string()));
    let impls_copy = result.types_implementing("Copy");
    assert!(impls_copy.contains(&"usize".to_string()));
}

#[test]
fn test_non_relationship_predicate_not_an_edge() {
    let triples = vec![spo("x", "b00t:goalText", "some goal text")];
    let result = graph_rules::derive(triples);
    // goalText is not a relationship predicate — no edge, no reachability
    assert!(!result.reachable.contains(&pair("x", "some goal text")));
}

#[test]
fn test_empty_triples() {
    let result = graph_rules::derive(vec![]);
    assert!(result.reachable.is_empty());
    assert!(result.depends_on.is_empty());
    assert!(result.informed_by.is_empty());
}

// ── Lattice analytics (ascent) tests ──────────────────────────────────────

#[test]
fn test_shortest_path_direct() {
    let triples = vec![spo("a", "b00t:relatedTo", "b")];
    let result = analytics::analyze(triples);
    assert_eq!(result.shortest_path("a", "b"), Some(1));
}

#[test]
fn test_shortest_path_two_hops() {
    let triples = vec![
        spo("a", "b00t:dependsOn", "b"),
        spo("b", "b00t:dependsOn", "c"),
    ];
    let result = analytics::analyze(triples);
    assert_eq!(result.shortest_path("a", "b"), Some(1));
    assert_eq!(result.shortest_path("a", "c"), Some(2));
}

#[test]
fn test_shortest_path_prefers_direct_over_indirect() {
    // Two routes: a→b (1 hop) and a→x→b (2 hops)
    let triples = vec![
        spo("a", "b00t:relatedTo", "b"),
        spo("a", "b00t:relatedTo", "x"),
        spo("x", "b00t:relatedTo", "b"),
    ];
    let result = analytics::analyze(triples);
    assert_eq!(result.shortest_path("a", "b"), Some(1));
}

#[test]
fn test_skill_frequency_ranking() {
    let triples = vec![
        spo("ooda:goal:aaa", "b00t:informedBy", "llamacpp"),
        spo("ooda:goal:bbb", "b00t:informedBy", "llamacpp"),
        spo("ooda:goal:ccc", "b00t:informedBy", "systemd"),
        spo("ooda:goal:ddd", "b00t:informedBy", "llamacpp"),
    ];
    let result = analytics::analyze(triples);
    let top = result.top_skills(2);
    assert_eq!(top[0], ("llamacpp".to_string(), 3));
    assert_eq!(top[1], ("systemd".to_string(), 1));
}

#[test]
fn test_no_path_for_irrelevant_predicate() {
    let triples = vec![spo("x", "b00t:goalText", "debug")];
    let result = analytics::analyze(triples);
    assert_eq!(result.path_count, 0);
    assert_eq!(result.shortest_path("x", "debug"), None);
}

// ── ReasoningEngine integration tests ─────────────────────────────────────

#[test]
fn test_engine_combined_result() {
    let triples = vec![
        spo("a", "b00t:dependsOn", "b"),
        spo("b", "b00t:dependsOn", "c"),
        spo("ooda:goal:xyz", "b00t:informedBy", "a"),
    ];
    let result = ReasoningEngine::run(triples);
    assert!(result.horn.reachable.contains(&pair("a", "c")), "transitive reachability");
    assert_eq!(result.analytics.shortest_path("a", "c"), Some(2));
    let skills = result.horn.skills_for_goal("ooda:goal:xyz");
    assert_eq!(skills, vec!["a".to_string()]);
}

#[test]
fn test_engine_top_skills_convenience() {
    let triples = vec![
        spo("ooda:goal:1", "b00t:informedBy", "rust"),
        spo("ooda:goal:2", "b00t:informedBy", "rust"),
        spo("ooda:goal:3", "b00t:informedBy", "llamacpp"),
    ];
    let top = ReasoningEngine::top_skills(triples, 1);
    assert_eq!(top, vec![("rust".to_string(), 2)]);
}

#[test]
fn test_engine_reachable_from_convenience() {
    let triples = vec![
        spo("root", "b00t:dependsOn", "child"),
        spo("child", "b00t:dependsOn", "leaf"),
    ];
    let reachable = ReasoningEngine::reachable_from(triples, "root");
    assert!(reachable.contains(&"child".to_string()));
    assert!(reachable.contains(&"leaf".to_string()));
}

// ── Phase 2: trait_lower (syn → crepe triples) ────────────────────────────

#[test]
fn test_simple_impl_emits_implements_triple() {
    let src = "impl Clone for usize {}";
    let triples = trait_lower::parse_source_triples(src).unwrap();
    assert!(triples.iter().any(|(s, p, o)| s == "usize" && p == "b00t:implements" && o == "Clone"));
}

#[test]
fn test_generic_impl_emits_bound_triple() {
    let src = "impl<T: Clone> Clone for Vec<T> {}";
    let triples = trait_lower::parse_source_triples(src).unwrap();
    // Direct impl triple
    assert!(triples.iter().any(|(s, p, o)| s == "Vec<T>" && p == "b00t:implements" && o == "Clone"));
    // Bound prerequisite triple
    assert!(triples.iter().any(|(s, p, o)| {
        s == "Vec<T>" && p == "b00t:requires/Clone" && o == "T:Clone"
    }));
}

#[test]
fn test_where_clause_emits_bound_triple() {
    let src = "impl<T> Clone for Vec<T> where T: Clone {}";
    let triples = trait_lower::parse_source_triples(src).unwrap();
    assert!(triples.iter().any(|(s, p, o)| s == "Vec<T>" && p == "b00t:implements" && o == "Clone"));
    assert!(triples.iter().any(|(s, p, o)| {
        s == "Vec<T>" && p == "b00t:requires/Clone" && o == "T:Clone"
    }));
}

#[test]
fn test_multiple_trait_impls() {
    let src = r#"
        impl Clone for usize {}
        impl Clone for String {}
        impl<T: Clone> Clone for Vec<T> {}
        impl<T: Clone + PartialEq> PartialEq for Vec<T> {}
    "#;
    let triples = trait_lower::parse_source_triples(src).unwrap();
    let impls: Vec<_> = triples.iter().filter(|(_, p, _)| p == "b00t:implements").collect();
    assert_eq!(impls.len(), 4, "expected 4 impl triples, got {impls:?}");
}

#[test]
fn test_non_trait_impl_ignored() {
    // inherent impls (no trait) should produce no triples
    let src = "impl usize { fn foo() {} }";
    let triples = trait_lower::parse_source_triples(src).unwrap();
    assert!(triples.is_empty(), "inherent impls must not emit triples");
}

#[test]
fn test_multi_bound_generic() {
    let src = "impl<T: Clone + Send> MyTrait for Wrapper<T> {}";
    let triples = trait_lower::parse_source_triples(src).unwrap();
    // Should emit both bound triples
    assert!(triples.iter().any(|(_, p, o)| p == "b00t:requires/MyTrait" && o == "T:Clone"));
    assert!(triples.iter().any(|(_, p, o)| p == "b00t:requires/MyTrait" && o == "T:Send"));
}

#[test]
fn test_module_impls_recursed() {
    let src = r#"
        mod inner {
            impl Clone for u8 {}
        }
        impl Clone for u16 {}
    "#;
    let triples = trait_lower::parse_source_triples(src).unwrap();
    let subjects: Vec<_> = triples.iter()
        .filter(|(_, p, _)| p == "b00t:implements")
        .map(|(s, _, _)| s.as_str())
        .collect();
    assert!(subjects.contains(&"u8"),  "inner module impl not found");
    assert!(subjects.contains(&"u16"), "outer impl not found");
}

#[test]
fn test_proves_implements_direct() {
    let src = "impl Clone for usize {}";
    let triples = trait_lower::parse_source_triples(src).unwrap();
    assert!(trait_lower::proves_implements(&triples, "usize", "Clone"));
    assert!(!trait_lower::proves_implements(&triples, "String", "Clone"));
}

#[test]
fn test_horn_reasoning_over_trait_triples() {
    // Feed parsed triples directly into the Horn engine
    let src = r#"
        impl Clone for usize {}
        impl<T: Clone> Clone for Vec<T> {}
    "#;
    let triples = trait_lower::parse_source_triples(src).unwrap();
    let horn = graph_rules::derive(triples);
    // Direct facts present
    assert!(horn.implements.contains(&("usize".into(), "Clone".into())));
    assert!(horn.implements.contains(&("Vec<T>".into(), "Clone".into())));
}
