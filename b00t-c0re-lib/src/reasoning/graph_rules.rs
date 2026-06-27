//! Horn clause derivation over SPO triples using crepe (Datalog proc-macro).
//!
//! Uses u32 interned IDs internally (crepe 0.2 derives Copy for all relation types;
//! String is non-Copy so we must intern first). Public API surfaces (String, String) pairs.

use crepe::crepe;
use std::collections::{HashMap, HashSet};
use super::predicates::B00tPredicate;

crepe! {
    @input struct RawSpo(u32, u32, u32);      // (subject_id, predicate_id, object_id)
    @input struct EdgePred(u32);              // predicate is a graph-traversal edge
    @input struct DepPred(u32);              // predicate indicates dependency
    @input struct InformedByPred(u32);       // "b00t:informedBy"
    @input struct ImplementsPred(u32);       // "b00t:implements"
    @input struct SupertraitPred(u32);       // "b00t:supertrait"

    struct GEdge(u32, u32);

    @output struct Reachable(u32, u32);
    @output struct DependsOn(u32, u32);
    @output struct InformedBy(u32, u32);
    @output struct Implements(u32, u32);

    GEdge(s, o) <- RawSpo(s, p, o), EdgePred(p);
    Reachable(x, y) <- GEdge(x, y);
    Reachable(x, z) <- GEdge(x, y), Reachable(y, z);

    DependsOn(s, o) <- RawSpo(s, p, o), DepPred(p);
    DependsOn(s, z) <- DependsOn(s, y), DependsOn(y, z);

    InformedBy(s, o) <- RawSpo(s, p, o), InformedByPred(p);

    Implements(s, o) <- RawSpo(s, p, o), ImplementsPred(p);
    // Supertrait transitivity: if A: B and B: C then A: C
    Implements(a, c) <- Implements(a, b), RawSpo(b, p, c), SupertraitPred(p);
}

fn classify_predicate(p: &str) -> Option<B00tPredicate> {
    B00tPredicate::from_uri(p)
}

fn is_edge_pred(p: &str) -> bool {
    classify_predicate(p).map(|pred| pred.is_edge_relation()).unwrap_or(false)
}

fn is_dep_pred(p: &str) -> bool {
    classify_predicate(p).map(|pred| pred.is_dependency_relation()).unwrap_or(false)
}

fn is_informed_by(p: &str) -> bool {
    classify_predicate(p).map(|pred| pred.is_informed_by()).unwrap_or(false)
}

fn is_implements(p: &str) -> bool {
    classify_predicate(p).map(|pred| pred.is_implements()).unwrap_or(false)
}

fn is_supertrait(p: &str) -> bool {
    classify_predicate(p).map(|pred| pred.is_supertrait()).unwrap_or(false)
}

struct Interner {
    to_id: HashMap<String, u32>,
    to_str: Vec<String>,
}

impl Interner {
    fn new() -> Self { Self { to_id: HashMap::new(), to_str: Vec::new() } }

    fn intern(&mut self, s: &str) -> u32 {
        if let Some(&id) = self.to_id.get(s) { return id; }
        let id = self.to_str.len() as u32;
        self.to_id.insert(s.to_string(), id);
        self.to_str.push(s.to_string());
        id
    }

    fn get(&self, id: u32) -> &str { &self.to_str[id as usize] }

    fn pair(&self, a: u32, b: u32) -> (String, String) {
        (self.get(a).to_string(), self.get(b).to_string())
    }
}

/// Results of running Horn clause derivation over a triple set.
/// All fields are `HashSet<(subject, object)>` pairs.
pub struct HornResults {
    pub reachable: HashSet<(String, String)>,
    pub depends_on: HashSet<(String, String)>,
    pub informed_by: HashSet<(String, String)>,
    pub implements: HashSet<(String, String)>,
}

impl HornResults {
    pub fn reachable_from(&self, subject: &str) -> Vec<String> {
        self.reachable.iter().filter(|(s, _)| s == subject).map(|(_, o)| o.clone()).collect()
    }

    pub fn skills_for_goal(&self, goal: &str) -> Vec<String> {
        self.informed_by.iter().filter(|(g, _)| g == goal).map(|(_, s)| s.clone()).collect()
    }

    pub fn dependency_chain(&self, subject: &str) -> Vec<String> {
        self.depends_on.iter().filter(|(s, _)| s == subject).map(|(_, o)| o.clone()).collect()
    }

    pub fn types_implementing(&self, trait_name: &str) -> Vec<String> {
        self.implements.iter().filter(|(_, t)| t == trait_name).map(|(ty, _)| ty.clone()).collect()
    }
}

/// Run Horn clause derivation over the given SPO triples.
pub fn derive(triples: Vec<(String, String, String)>) -> HornResults {
    let mut intern = Interner::new();
    let mut rt = Crepe::new();

    for (s, p, o) in &triples {
        let si = intern.intern(s);
        let pi = intern.intern(p);
        let oi = intern.intern(o);
        rt.extend([RawSpo(si, pi, oi)]);
        if is_edge_pred(p)    { rt.extend([EdgePred(pi)]); }
        if is_dep_pred(p)     { rt.extend([DepPred(pi)]); }
        if is_informed_by(p)  { rt.extend([InformedByPred(pi)]); }
        if is_implements(p)   { rt.extend([ImplementsPred(pi)]); }
        if is_supertrait(p)   { rt.extend([SupertraitPred(pi)]); }
    }

    let (reachable, depends_on, informed_by, implements) = rt.run();

    HornResults {
        reachable:   reachable.into_iter().map(|Reachable(a, b)|   intern.pair(a, b)).collect(),
        depends_on:  depends_on.into_iter().map(|DependsOn(a, b)|  intern.pair(a, b)).collect(),
        informed_by: informed_by.into_iter().map(|InformedBy(a, b)| intern.pair(a, b)).collect(),
        implements:  implements.into_iter().map(|Implements(a, b)|  intern.pair(a, b)).collect(),
    }
}
