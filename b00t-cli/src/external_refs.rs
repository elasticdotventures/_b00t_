//! External references — URI-based links from datums to external resources.
//!
//! Requirements (ReqIF), specifications, standards, and other external resources
//! are referenced by URI, not embedded in datums. This keeps b00t as a consumer
//! of external requirement systems, not a producer of ReqIF tooling.
//!
//! Relationship verbs are fixed and enumerable — they enable conflict detection
//! and digital-thread traversal without natural-language ambiguity.
//!
//! 🤓 Migration note: these types belong in ufo-types long-term. They live here
//! now because the local ufo-types checkout (v0.10.6) is stale vs the git
//! dependency (v0.14.1). When ufo-types is updated upstream, migrate this module
//! there and delete this file. See #1345.

use serde::{Deserialize, Serialize};

/// Fixed set of relationship verbs between a datum and an external resource.
///
/// Enumerable for conflict detection: `Satisfies` and `ConflictsWith` on the
/// same target is a conflict. `Verifies` implies `Satisfies` but adds test
/// evidence. `Constrains` is a hard boundary (must not violate).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RefRelationship {
    /// Datum satisfies the referenced requirement.
    Satisfies,
    /// Datum is constrained by the referenced requirement (must not violate).
    Constrains,
    /// Datum depends on the referenced resource being available/correct.
    DependsOn,
    /// Datum conflicts with the referenced requirement.
    ConflictsWith,
    /// Datum provides test/verification evidence for the referenced requirement.
    Verifies,
}

impl RefRelationship {
    /// All variants, for iteration and validation.
    pub const ALL: &[RefRelationship] = &[
        RefRelationship::Satisfies,
        RefRelationship::Constrains,
        RefRelationship::DependsOn,
        RefRelationship::ConflictsWith,
        RefRelationship::Verifies,
    ];

    /// Human-readable label.
    pub fn label(&self) -> &'static str {
        match self {
            RefRelationship::Satisfies => "satisfies",
            RefRelationship::Constrains => "constrains",
            RefRelationship::DependsOn => "depends_on",
            RefRelationship::ConflictsWith => "conflicts_with",
            RefRelationship::Verifies => "verifies",
        }
    }
}

impl std::fmt::Display for RefRelationship {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.label())
    }
}

/// A URI reference to an external resource (requirement, spec, standard).
///
/// Supports any URI scheme: `reqif://`, `https://`, `file://`, etc.
/// The optional `fragment` identifies a specific element within the resource
/// (e.g., `#REQ-001` in a ReqIF file).
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Uri {
    /// The full URI string (scheme + path, no fragment).
    pub value: String,
    /// Optional fragment identifier (the part after `#`).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fragment: Option<String>,
}

impl Uri {
    pub fn new(value: impl Into<String>) -> Self {
        Self {
            value: value.into(),
            fragment: None,
        }
    }

    #[allow(dead_code)]
    pub fn with_fragment(mut self, fragment: impl Into<String>) -> Self {
        self.fragment = Some(fragment.into());
        self
    }

    /// Parse a URI string into a Uri, splitting on `#`.
    pub fn parse(s: &str) -> Self {
        match s.split_once('#') {
            Some((base, frag)) => Self {
                value: base.to_string(),
                fragment: Some(frag.to_string()),
            },
            None => Self {
                value: s.to_string(),
                fragment: None,
            },
        }
    }
}

impl std::fmt::Display for Uri {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.value)?;
        if let Some(ref frag) = self.fragment {
            write!(f, "#{}", frag)?;
        }
        Ok(())
    }
}

/// A typed external reference from a datum to an external resource.
///
/// Combines a URI, a relationship verb, and an optional human-readable note.
/// Stored in datum TOML as:
/// ```toml
/// [b00t]
/// satisfies = ["reqif://docs/requirements/focus.reqif#REQ-001"]
/// constrains = ["https://standards.example.com/finops/v1#cost-allocation"]
/// ```
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ExternalRef {
    /// The relationship verb.
    pub relationship: RefRelationship,
    /// URI of the external resource.
    pub uri: Uri,
    /// Optional human-readable note (e.g., why this relationship exists).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub note: Option<String>,
}

impl ExternalRef {
    pub fn new(relationship: RefRelationship, uri: Uri) -> Self {
        Self {
            relationship,
            uri,
            note: None,
        }
    }

    #[allow(dead_code)]
    pub fn with_note(mut self, note: impl Into<String>) -> Self {
        self.note = Some(note.into());
        self
    }
}

impl std::fmt::Display for ExternalRef {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}: {}", self.relationship, self.uri)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn uri_parse_with_fragment() {
        let uri = Uri::parse("reqif://docs/focus.reqif#REQ-001");
        assert_eq!(uri.value, "reqif://docs/focus.reqif");
        assert_eq!(uri.fragment.as_deref(), Some("REQ-001"));
        assert_eq!(uri.to_string(), "reqif://docs/focus.reqif#REQ-001");
    }

    #[test]
    fn uri_parse_without_fragment() {
        let uri = Uri::parse("https://standards.example.com/finops/v1");
        assert_eq!(uri.value, "https://standards.example.com/finops/v1");
        assert!(uri.fragment.is_none());
    }

    #[test]
    fn external_ref_display() {
        let r = ExternalRef::new(
            RefRelationship::Satisfies,
            Uri::parse("reqif://docs/focus.reqif#REQ-001"),
        );
        assert_eq!(r.to_string(), "satisfies: reqif://docs/focus.reqif#REQ-001");
    }

    #[test]
    fn ref_relationship_all_variants() {
        assert_eq!(RefRelationship::ALL.len(), 5);
        for r in RefRelationship::ALL {
            assert!(!r.label().is_empty());
        }
    }

    #[test]
    fn external_ref_roundtrip_json() {
        let r = ExternalRef::new(
            RefRelationship::Constrains,
            Uri::parse("https://example.com/spec#4.2"),
        )
        .with_note("FOCUS cost allocation spec");
        let json = serde_json::to_string(&r).unwrap();
        let back: ExternalRef = serde_json::from_str(&json).unwrap();
        assert_eq!(back.relationship, RefRelationship::Constrains);
        assert_eq!(back.uri.fragment.as_deref(), Some("4.2"));
        assert_eq!(back.note.as_deref(), Some("FOCUS cost allocation spec"));
    }
}