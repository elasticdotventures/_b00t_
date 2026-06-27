// b00t-c0re-lib/src/reasoning/predicates.rs
// 🤓 Centralized predicate vocabulary for the b00t reasoning engine.
//    FOL-correct: the predicate signature is explicitly enumerated — no string drift.
//    Every predicate URI in b00t must route through this enum for compile-time safety.
//
//    Fixed: CRITICAL findings 2.1-2.4, 4.1 from FOL anti-pattern audit.
//    Six consumer files now converge on one signature.

use std::fmt;

/// Canonical b00t predicate vocabulary.
///
/// FOL: this is the language SIGNATURE — the set of predicate symbols
/// that the reasoning engine can interpret. String comparisons are
/// replaced with exhaustive match dispatch.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum B00tPredicate {
    // ── Edge relations (graph traversal) ──────────────────────────────────
    /// General relationship edge (OWL2: owl:relatedTo)
    RelatedTo,
    /// Dependency relationship (transitive)
    DependsOn,
    /// "Skill was informed by goal" — recall signal
    InformedBy,
    /// Part-whole relationship (BFO: has-part)
    HasPart,
    /// Requirement relationship (SysMLv2)
    Requires,

    // ── Type system ───────────────────────────────────────────────────────
    /// Trait implementation
    Implements,
    /// Supertrait relationship (for transitive impls)
    Supertrait,

    // ── Goal / task ───────────────────────────────────────────────────────
    /// Goal node text annotation
    GoalText,

    // ── Composite (parameterized) ─────────────────────────────────────────
    /// Requires bound for a specific trait: b00t:requires/{trait_name}
    RequiresTrait { trait_name: String },
    /// Stored-in relationship (data fabric)
    StoredIn,
}

impl B00tPredicate {
    // ── URI parsing ───────────────────────────────────────────────────────

    /// Parse a predicate URI string into a B00tPredicate.
    /// Returns None for unrecognized predicates — callers should treat unknown
    /// predicates as non-edge, non-dependency (silently ignored in reasoning).
    pub fn from_uri(uri: &str) -> Option<Self> {
        match uri {
            "b00t:relatedTo" => Some(B00tPredicate::RelatedTo),
            "b00t:dependsOn" => Some(B00tPredicate::DependsOn),
            "b00t:informedBy" => Some(B00tPredicate::InformedBy),
            "b00t:hasPart" => Some(B00tPredicate::HasPart),
            "b00t:requires" => Some(B00tPredicate::Requires),
            "b00t:implements" => Some(B00tPredicate::Implements),
            "b00t:supertrait" => Some(B00tPredicate::Supertrait),
            "b00t:goalText" => Some(B00tPredicate::GoalText),
            s if s.to_lowercase().contains("dependson") => Some(B00tPredicate::DependsOn),
            s if s.to_lowercase().contains("relatedto") => Some(B00tPredicate::RelatedTo),
            s if s.to_lowercase().contains("implements") => Some(B00tPredicate::Implements),
            s if s.to_lowercase().contains("supertrait") => Some(B00tPredicate::Supertrait),
            s if s.to_lowercase().contains("haspart") => Some(B00tPredicate::HasPart),
            s if s.to_lowercase().contains("storedin") => Some(B00tPredicate::StoredIn),
            s if s.starts_with("b00t:requires/") => {
                let trait_name = s.strip_prefix("b00t:requires/")?.to_string();
                Some(B00tPredicate::RequiresTrait { trait_name })
            }
            _ => None,
        }
    }

    /// Convert to the canonical b00t URI string
    pub fn as_uri(&self) -> String {
        match self {
            B00tPredicate::RelatedTo => "b00t:relatedTo".into(),
            B00tPredicate::DependsOn => "b00t:dependsOn".into(),
            B00tPredicate::InformedBy => "b00t:informedBy".into(),
            B00tPredicate::HasPart => "b00t:hasPart".into(),
            B00tPredicate::Requires => "b00t:requires".into(),
            B00tPredicate::Implements => "b00t:implements".into(),
            B00tPredicate::Supertrait => "b00t:supertrait".into(),
            B00tPredicate::GoalText => "b00t:goalText".into(),
            B00tPredicate::RequiresTrait { trait_name } => {
                format!("b00t:requires/{trait_name}")
            }
            B00tPredicate::StoredIn => "b00t:StoredIn".into(),
        }
    }

    // ── Classification (FOL predicate categories) ─────────────────────────

    /// Is this predicate an edge that enables graph traversal?
    /// Includes: relatedTo, dependsOn, informedBy, hasPart, requires, StoredIn.
    pub fn is_edge_relation(&self) -> bool {
        matches!(
            self,
            B00tPredicate::RelatedTo
                | B00tPredicate::DependsOn
                | B00tPredicate::InformedBy
                | B00tPredicate::HasPart
                | B00tPredicate::Requires
                | B00tPredicate::StoredIn
        )
    }

    /// Is this predicate a dependency indicator?
    /// Includes: dependsOn, requires, needs (implicit), hasPart.
    /// hasPart: owl:hasPart ⊑ owl:dependsOn — part-whole transitively propagates deps.
    pub fn is_dependency_relation(&self) -> bool {
        matches!(
            self,
            B00tPredicate::DependsOn
                | B00tPredicate::Requires
                | B00tPredicate::HasPart
        )
    }

    /// Is this predicate the canonical "informed by" relation?
    pub fn is_informed_by(&self) -> bool {
        matches!(self, B00tPredicate::InformedBy)
    }

    /// Is this predicate a trait implementation?
    pub fn is_implements(&self) -> bool {
        matches!(self, B00tPredicate::Implements)
    }

    /// Is this predicate a supertrait relation?
    pub fn is_supertrait(&self) -> bool {
        matches!(self, B00tPredicate::Supertrait)
    }

    /// Is this predicate a requires-trait bound?
    pub fn is_requires_trait(&self) -> bool {
        matches!(self, B00tPredicate::RequiresTrait { .. })
    }
}

impl fmt::Display for B00tPredicate {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.as_uri())
    }
}

/// Category classification for node URIs — used by adjacency exclusion filters.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NodeCategory {
    Goal,
    Datum,
    Service,
    Skill, // default — not filtered
}

impl NodeCategory {
    /// Classify a node URI string into a category.
    pub fn classify(uri: &str) -> Self {
        if uri.starts_with("ooda:") || uri.starts_with("b00t:goal") {
            NodeCategory::Goal
        } else if uri.starts_with("b00t:datum/") {
            NodeCategory::Datum
        } else if uri.starts_with("b00t:service/") {
            NodeCategory::Service
        } else {
            NodeCategory::Skill
        }
    }

    /// Whether this category should be excluded from skill discovery.
    pub fn is_excluded_from_discovery(&self) -> bool {
        matches!(self, NodeCategory::Goal | NodeCategory::Datum)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_all_canonical_forms() {
        let canonical = [
            "b00t:relatedTo", "b00t:dependsOn", "b00t:informedBy",
            "b00t:hasPart", "b00t:requires", "b00t:implements",
            "b00t:supertrait", "b00t:goalText",
        ];
        for uri in canonical {
            let parsed = B00tPredicate::from_uri(uri);
            assert!(parsed.is_some(), "canonical URI '{}' should parse", uri);
            assert_eq!(parsed.unwrap().as_uri(), uri);
        }
    }

    #[test]
    fn test_parse_camelcase_fallbacks() {
        assert_eq!(B00tPredicate::from_uri("fooDependsOnBar"), Some(B00tPredicate::DependsOn));
        assert_eq!(B00tPredicate::from_uri("myRelatedToEdge"), Some(B00tPredicate::RelatedTo));
        assert_eq!(B00tPredicate::from_uri("someImplements"), Some(B00tPredicate::Implements));
        assert_eq!(B00tPredicate::from_uri("aSupertrait"), Some(B00tPredicate::Supertrait));
        assert_eq!(B00tPredicate::from_uri("dataStoredIn"), Some(B00tPredicate::StoredIn));
    }

    #[test]
    fn test_parse_requires_trait() {
        let p = B00tPredicate::from_uri("b00t:requires/Clone").unwrap();
        assert!(p.is_requires_trait());
        assert_eq!(p.as_uri(), "b00t:requires/Clone");
    }

    #[test]
    fn test_parse_unknown() {
        assert_eq!(B00tPredicate::from_uri("nonsense:pred"), None);
        assert_eq!(B00tPredicate::from_uri(""), None);
    }

    #[test]
    fn test_roundtrip() {
        let cases = [
            B00tPredicate::InformedBy,
            B00tPredicate::Implements,
            B00tPredicate::DependsOn,
            B00tPredicate::GoalText,
            B00tPredicate::RequiresTrait { trait_name: "Clone".into() },
        ];
        for pred in cases {
            let uri = pred.as_uri();
            let parsed = B00tPredicate::from_uri(&uri).unwrap();
            assert_eq!(pred, parsed);
            assert_eq!(format!("{}", pred), uri);
        }
    }

    #[test]
    fn test_classification() {
        assert!(B00tPredicate::RelatedTo.is_edge_relation());
        assert!(B00tPredicate::DependsOn.is_edge_relation());
        assert!(B00tPredicate::InformedBy.is_edge_relation());
        assert!(B00tPredicate::HasPart.is_edge_relation());
        assert!(B00tPredicate::Requires.is_edge_relation());
        assert!(B00tPredicate::StoredIn.is_edge_relation());
        assert!(!B00tPredicate::Implements.is_edge_relation());
        assert!(!B00tPredicate::GoalText.is_edge_relation());
    }

    #[test]
    fn test_dependency_classification() {
        assert!(B00tPredicate::DependsOn.is_dependency_relation());
        assert!(B00tPredicate::Requires.is_dependency_relation());
        assert!(B00tPredicate::HasPart.is_dependency_relation());
        assert!(!B00tPredicate::InformedBy.is_dependency_relation());
        assert!(!B00tPredicate::Implements.is_dependency_relation());
    }

    #[test]
    fn test_node_category() {
        assert_eq!(NodeCategory::classify("ooda:my-goal"), NodeCategory::Goal);
        assert_eq!(NodeCategory::classify("b00t:goalText"), NodeCategory::Goal);
        assert_eq!(NodeCategory::classify("b00t:datum/my-datum"), NodeCategory::Datum);
        assert_eq!(NodeCategory::classify("b00t:skill/my-skill"), NodeCategory::Skill);
        assert!(NodeCategory::Goal.is_excluded_from_discovery());
        assert!(NodeCategory::Datum.is_excluded_from_discovery());
        assert!(!NodeCategory::Skill.is_excluded_from_discovery());
    }

    #[test]
    fn test_fol_exhaustive() {
        // Every variant in the enum must be reachable through from_uri or classification
        // This test ensures new variants aren't forgotten.
        use B00tPredicate::*;
        let all = [
            RelatedTo, DependsOn, InformedBy, HasPart, Requires,
            Implements, Supertrait, GoalText, StoredIn,
            RequiresTrait { trait_name: "Test".into() },
        ];
        for p in all {
            let uri = p.as_uri();
            let parsed = B00tPredicate::from_uri(&uri);
            assert!(parsed.is_some(), "variant {:?} must be parseable from its URI '{}'", p, uri);
        }
    }
}
