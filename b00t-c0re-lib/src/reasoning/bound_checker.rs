//! Bound-aware Horn proving for instantiated generics.
//!
//! Bridges the gap between Phase 2 (syn extracts bound triples) and Phase 3
//! (chalk FOHH). Handles the common case: single-level generic substitution.
//!
//! Given:
//!   ("Vec<T>", "b00t:implements",   "Clone")
//!   ("Vec<T>", "b00t:requires/Clone", "T:Clone")   ← from trait_lower
//!   ("usize",  "b00t:implements",   "Clone")        ← ground fact
//!
//! Proves: `Vec<usize>` implements `Clone` by:
//!   1. Matching `Vec<T>` template against query `Vec<usize>` → T = usize
//!   2. Checking all `b00t:requires/Clone` bounds: T:Clone → usize:Clone ✓
//!   3. Returning true

use std::collections::HashMap;

/// Attempt to unify a template type (e.g. `Vec<T>`) with a concrete type
/// (e.g. `Vec<usize>`). Returns param bindings if successful.
///
/// Handles one level of generic: `Outer<T>` vs `Outer<Concrete>`.
/// Does NOT handle higher-kinded, lifetime, or multi-param generics.
fn unify_single(template: &str, concrete: &str) -> Option<HashMap<String, String>> {
    if template == concrete {
        return Some(HashMap::new()); // identical — no params needed
    }

    // Try to match Outer<T> against Outer<Concrete>
    let (t_outer, t_inner) = split_outer_inner(template)?;
    let (c_outer, c_inner) = split_outer_inner(concrete)?;
    if t_outer != c_outer { return None; }

    // Check if inner is a single type parameter (identifier, no < >)
    if t_inner.contains('<') || t_inner.contains(',') {
        return None; // too complex for single-level unification
    }

    let mut bindings = HashMap::new();
    bindings.insert(t_inner.to_string(), c_inner.to_string());
    Some(bindings)
}

/// Split `Outer<Inner>` → ("Outer", "Inner"). Returns None if not that shape.
fn split_outer_inner(s: &str) -> Option<(&str, &str)> {
    let lt = s.find('<')?;
    if !s.ends_with('>') { return None; }
    Some((&s[..lt], &s[lt + 1..s.len() - 1]))
}

/// Apply bindings to a bound string like `"T:Clone"` → `"usize:Clone"`.
fn apply_bindings(bound: &str, bindings: &HashMap<String, String>) -> String {
    // bound format: "TypeParam:TraitName"
    let colon = match bound.find(':') {
        Some(i) => i,
        None => return bound.to_string(),
    };
    let param = &bound[..colon];
    let trait_name = &bound[colon + 1..];
    let resolved = bindings.get(param).map(|s| s.as_str()).unwrap_or(param);
    format!("{resolved}:{trait_name}")
}

/// Check whether `subject` implements `trait_name` given the triple set.
///
/// Algorithm:
///   1. Direct: check for `(subject, "b00t:implements", trait_name)`.
///   2. Generic: for each template type T such that `(T, "b00t:implements", trait_name)`,
///      try to unify T with subject. If unification succeeds, check all required
///      bounds are satisfied (recursively, depth-limited).
pub fn proves_implements(
    triples: &[(String, String, String)],
    subject: &str,
    trait_name: &str,
    depth: u8,
) -> bool {
    if depth == 0 { return false; }

    // 1. Direct ground fact
    if triples.iter().any(|(s, p, o)| s == subject && p == "b00t:implements" && o == trait_name) {
        return true;
    }

    // 2. Generic instantiation
    let req_pred = format!("b00t:requires/{trait_name}");
    for (template, pred, tmpl_trait) in triples {
        if pred != "b00t:implements" || tmpl_trait != trait_name { continue; }
        if template == subject { continue; } // already checked above
        let Some(bindings) = unify_single(template, subject) else { continue };

        // Collect bound requirements for this template
        let bounds: Vec<_> = triples
            .iter()
            .filter(|(s, p, _)| s == template && p == &req_pred)
            .map(|(_, _, o)| o.as_str())
            .collect();

        // All bounds must be satisfiable after substitution
        let all_satisfied = bounds.iter().all(|bound| {
            let resolved = apply_bindings(bound, &bindings);
            let colon = match resolved.find(':') {
                Some(i) => i,
                None => return false,
            };
            let bound_subj = &resolved[..colon];
            let bound_trait = &resolved[colon + 1..];
            proves_implements(triples, bound_subj, bound_trait, depth - 1)
        });

        if all_satisfied { return true; }
    }

    false
}

#[cfg(test)]
mod tests {
    use super::*;

    fn t(s: &str, p: &str, o: &str) -> (String, String, String) {
        (s.into(), p.into(), o.into())
    }

    #[test]
    fn test_direct_ground_fact() {
        let triples = vec![t("usize", "b00t:implements", "Clone")];
        assert!(proves_implements(&triples, "usize", "Clone", 3));
        assert!(!proves_implements(&triples, "String", "Clone", 3));
    }

    #[test]
    fn test_instantiated_generic_one_level() {
        // Vec<T>: Clone :- T: Clone  +  usize: Clone  ⊢  Vec<usize>: Clone
        let triples = vec![
            t("usize",  "b00t:implements",     "Clone"),
            t("Vec<T>", "b00t:implements",     "Clone"),
            t("Vec<T>", "b00t:requires/Clone", "T:Clone"),
        ];
        assert!(proves_implements(&triples, "Vec<usize>", "Clone", 3));
        assert!(!proves_implements(&triples, "Vec<MyType>", "Clone", 3));
    }

    #[test]
    fn test_nested_instantiation() {
        // Vec<Vec<usize>>: Clone via Vec<usize>: Clone via usize: Clone
        let triples = vec![
            t("usize",  "b00t:implements",     "Clone"),
            t("Vec<T>", "b00t:implements",     "Clone"),
            t("Vec<T>", "b00t:requires/Clone", "T:Clone"),
        ];
        assert!(proves_implements(&triples, "Vec<Vec<usize>>", "Clone", 4));
    }

    #[test]
    fn test_bound_not_satisfied_fails() {
        let triples = vec![
            t("Vec<T>", "b00t:implements",     "Clone"),
            t("Vec<T>", "b00t:requires/Clone", "T:Clone"),
            // deliberately NO usize: Clone ground fact
        ];
        assert!(!proves_implements(&triples, "Vec<usize>", "Clone", 3));
    }

    #[test]
    fn test_depth_limit_prevents_infinite_recursion() {
        // Self-referential bound (shouldn't happen but must not loop)
        let triples = vec![
            t("A<T>", "b00t:implements",     "Trait"),
            t("A<T>", "b00t:requires/Trait", "T:Trait"),
        ];
        assert!(!proves_implements(&triples, "A<A<usize>>", "Trait", 3));
    }

    #[test]
    fn test_unify_identical() {
        let b = unify_single("usize", "usize").unwrap();
        assert!(b.is_empty());
    }

    #[test]
    fn test_unify_outer_inner() {
        let b = unify_single("Vec<T>", "Vec<usize>").unwrap();
        assert_eq!(b.get("T").map(|s| s.as_str()), Some("usize"));
    }

    #[test]
    fn test_unify_mismatch_outer() {
        assert!(unify_single("Box<T>", "Vec<usize>").is_none());
    }
}
