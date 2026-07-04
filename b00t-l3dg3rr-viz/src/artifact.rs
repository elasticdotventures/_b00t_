//! Artifact types — document-based datums with UFO classification and Dewey-style cataloging.
//!
//! # UFO stereotypes (Guizzardi 2005)
//! - **Endurant**: persistent entity (tool, agent, stack, job template)
//! - **Perdurant**: event/process (job execution, training run, OODA cycle)
//! - **Moment**: intrinsic property (quality, disposition, role)
//! - **Relator**: mediates between endurants (depends_on, entangled_mcp edges)
//! - **Quality**: measurable attribute (performance, cost, accuracy)

use serde::{Deserialize, Serialize};

/// UFO ontological stereotype — classifies what a datum IS at the foundational level.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum UfoStereotype {
    Endurant,
    Perdurant,
    Moment,
    Relator,
    Quality,
}

impl UfoStereotype {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Endurant => "endurant",
            Self::Perdurant => "perdurant",
            Self::Moment => "moment",
            Self::Relator => "relator",
            Self::Quality => "quality",
        }
    }
}

/// Dewey-style catalog number for hierarchical document classification.
///
/// Format: `CATEGORY.DOMAIN.SEQUENCE.TITLE` — each segment maps to a
/// progressively more specific namespace, similar to Dewey Decimal but
/// using semantic identifiers instead of numeric codes.
///
/// # Examples
/// ```text
/// PRD.ARCH.005.MBSE-VISUALIZATION  — Architecture PRD #5
/// PRD.DATA.003.SCHEMA-INDEX        — Data PRD #3
/// LEARN.RUST.001.KASUARI-SOLVER    — Learning datum
/// PATTERN.DESIGN.002.SEMANTIC-CLASS — Design pattern
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DeweyNumber {
    pub category: String,   // PRD, LEARN, PATTERN, SCHEMA, CASE-STUDY
    pub domain: String,     // ARCH, DATA, AI, SECURITY, DESIGN, OPS
    pub sequence: u32,      // 001-999
    pub title: String,      // human-readable slug
}

impl DeweyNumber {
    pub fn parse(raw: &str) -> Option<Self> {
        let parts: Vec<&str> = raw.split('.').collect();
        if parts.len() < 4 { return None; }
        Some(Self {
            category: parts[0].to_uppercase(),
            domain: parts[1].to_uppercase(),
            sequence: parts[2].parse().ok()?,
            title: parts[3..].join("."),
        })
    }

    pub fn to_string(&self) -> String {
        format!("{}.{}.{:03}.{}", self.category, self.domain, self.sequence, self.title)
    }
}

impl std::fmt::Display for DeweyNumber {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.to_string())
    }
}

/// Abstract document — any datum that represents a document-type artifact.
///
/// Every document has:
/// - A catalog number for hierarchical discovery
/// - A UFO stereotype for ontological classification
/// - A queryable summary/abstract
/// - Required and optional content stanzas
pub trait AbstractDocument {
    fn catalog_number(&self) -> &DeweyNumber;
    fn title(&self) -> &str;
    fn summary(&self) -> &str;
    fn ufo_stereotype(&self) -> UfoStereotype;
    fn required_stanzas() -> &'static [&'static str];
    fn optional_stanzas() -> &'static [&'static str];
}

/// Document status — lifecycle state of a document artifact.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DocumentStatus {
    Draft,
    Review,
    Approved,
    Superseded,
    Archived,
}

impl DocumentStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Draft => "draft",
            Self::Review => "review",
            Self::Approved => "approved",
            Self::Superseded => "superseded",
            Self::Archived => "archived",
        }
    }
}

/// PRD (Product Requirements Document) — formal requirements artifact.
///
/// Required stanzas: `[prd]`, `[prd.overview]`, `[prd.acceptance]`
/// Optional stanzas: `[prd.dependencies]`, `[prd.risks]`, `[prd.references]`
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PrdDatum {
    pub name: String,
    pub catalog: DeweyNumber,
    pub title: String,
    pub summary: String,
    pub ufo_stereotype: UfoStereotype,
    pub domain: String,
    pub status: DocumentStatus,
    pub related: Vec<String>,

    // Required stanzas
    pub overview: String,
    pub acceptance_criteria: String,

    // Optional stanzas
    pub dependencies: Option<String>,
    pub risks: Option<String>,
    pub references: Option<String>,
    pub author: Option<String>,
}

impl AbstractDocument for PrdDatum {
    fn catalog_number(&self) -> &DeweyNumber { &self.catalog }
    fn title(&self) -> &str { &self.title }
    fn summary(&self) -> &str { &self.summary }
    fn ufo_stereotype(&self) -> UfoStereotype { self.ufo_stereotype }

    fn required_stanzas() -> &'static [&'static str] {
        &["[prd.overview]", "[prd.acceptance]"]
    }

    fn optional_stanzas() -> &'static [&'static str] {
        &["[prd.dependencies]", "[prd.risks]", "[prd.references]", "[prd.author]"]
    }
}

/// TOML stanza validator — checks that a document has all required stanzas.
pub fn validate_stanzas(toml_content: &str, required: &[&str]) -> Result<(), Vec<String>> {
    let mut missing = Vec::new();
    for stanza in required {
        if !toml_content.contains(stanza) {
            missing.push(stanza.to_string());
        }
    }
    if missing.is_empty() { Ok(()) } else { Err(missing) }
}

/// Auto-classify an orphan datum by UFO stereotype + suggest parent/treatment.
///
/// Returns the stereotype, a suggested Dewey catalog number (for docs),
/// and a suggested parent endurant name (for tools).
pub fn classify_orphan(
    label: &str,
    existing_endurants: &[String],
) -> (UfoStereotype, Option<DeweyNumber>, Option<String>) {
    let lower = label.to_lowercase();

    // Perdurants — document/reference material
    if lower.contains("prd-") || lower.contains(".prd") {
        let cat = catalog_for("PRD", label);
        let parent = find_best_parent(label, existing_endurants);
        return (UfoStereotype::Perdurant, cat, parent);
    }
    if lower.contains("learn") || lower.contains(".learn") {
        let cat = catalog_for("LEARN", label);
        return (UfoStereotype::Perdurant, cat, None);
    }
    if lower.contains("case-study") || lower.contains("pattern") {
        let cat = catalog_for("PATTERN", label);
        return (UfoStereotype::Perdurant, cat, None);
    }
    if lower.contains("schema") || lower.contains("protocol") {
        let cat = catalog_for("SCHEMA", label);
        return (UfoStereotype::Perdurant, cat, None);
    }

    // Endurants — tools, agents, services
    if lower.contains(".cli") || lower.contains(".mcp") || lower.contains(".agent")
        || lower.contains(".ai") || lower.contains(".docker") || lower.contains(".k8s")
        || lower.contains(".hive") || lower.contains(".stack") || lower.contains(".job")
    {
        let parent = find_best_parent(label, existing_endurants);
        return (UfoStereotype::Endurant, None, parent);
    }

    // Moments — configurations, quality attributes
    if lower.contains("config") || lower.contains("gate") || lower.contains("guard")
        || lower.contains("skill.source") || lower.contains("budget")
    {
        return (UfoStereotype::Moment, None, None);
    }

    // Default — treat as perdurant reference
    let cat = catalog_for("REF", label);
    (UfoStereotype::Perdurant, cat, None)
}

fn catalog_for(category: &str, label: &str) -> Option<DeweyNumber> {
    let slug: String = label
        .chars()
        .map(|c| if c.is_alphanumeric() || c == '-' { c } else { '.' })
        .collect();
    let parts: Vec<&str> = slug.split('.').collect();
    let domain = parts.get(1).map(|s| s.to_uppercase()).unwrap_or_else(|| "GEN".into());
    let seq = (label.as_bytes().iter().map(|&b| b as u32).sum::<u32>() % 999) + 1;
    let title = parts.get(2).map(|s| s.to_uppercase()).unwrap_or_else(|| "UNTITLED".into());
    Some(DeweyNumber {
        category: category.to_string(),
        domain,
        sequence: seq,
        title,
    })
}

fn find_best_parent(label: &str, candidates: &[String]) -> Option<String> {
    let lower = label.to_lowercase();
    let mut best: Option<(usize, &String)> = None;
    for c in candidates {
        let c_lower = c.to_lowercase();
        // Score by keyword overlap: how many words in the candidate appear in the label
        let score = c_lower.split(|ch: char| !ch.is_alphanumeric())
            .filter(|w| w.len() >= 2 && lower.contains(w))
            .count() * 10
            + c_lower.chars().zip(lower.chars()).take_while(|(a, b)| a == b).count();
        if score > 0 && score >= best.map(|(s, _)| s).unwrap_or(0) {
            best = Some((score, c));
        }
    }
    best.map(|(_, c)| c.clone())
}

/// Curate orphan datums: classify, catalog, and suggest patches.
pub fn curate_orphans(
    nodes: &[serde_json::Value],
    edges: &[serde_json::Value],
) -> Vec<serde_json::Value> {
    use std::collections::HashSet;
    let connected: HashSet<&str> = edges
        .iter()
        .flat_map(|e| {
            let f = e.get("from").and_then(|v| v.as_str()).unwrap_or("");
            let t = e.get("to").and_then(|v| v.as_str()).unwrap_or("");
            [f, t]
        })
        .collect();

    let endurant_names: Vec<String> = nodes
        .iter()
        .filter(|n| connected.contains(n.get("id").and_then(|v| v.as_str()).unwrap_or("")))
        .map(|n| n.get("id").and_then(|v| v.as_str()).unwrap_or("").to_string())
        .collect();

    nodes
        .iter()
        .filter(|n| {
            let id = n.get("id").and_then(|v| v.as_str()).unwrap_or("");
            !connected.contains(id)
        })
        .map(|n| {
            let id = n.get("id").and_then(|v| v.as_str()).unwrap_or("");
            let label = n.get("label").and_then(|v| v.as_str()).unwrap_or("");
            let role = n.get("role").and_then(|v| v.as_str()).unwrap_or("");
            let (stereotype, catalog, parent) = classify_orphan(label, &endurant_names);
            serde_json::json!({
                "id": id,
                "label": label.trim(),
                "role": role,
                "ufo_stereotype": stereotype.as_str(),
                "catalog": catalog.map(|c| c.to_string()),
                "suggested_parent": parent,
                "action": match stereotype {
                    UfoStereotype::Endurant if parent.is_some() => "add_edge_to_parent",
                    UfoStereotype::Endurant => "needs_manual_review",
                    UfoStereotype::Perdurant => "catalog_and_nest",
                    UfoStereotype::Moment => "attach_to_system",
                    UfoStereotype::Relator | UfoStereotype::Quality => "review",
                },
            })
        })
        .collect()
}

#[cfg(test)]
mod curation_tests {
    use super::*;

    #[test]
    fn classifies_prd_as_perdurant_with_catalog() {
        let parents = vec!["b00t-l3dg3rr-viz".into()];
        let (stereotype, catalog, parent) =
            classify_orphan("PRD-ARCH-005-MBSE-VISUALIZATION", &parents);
        assert_eq!(stereotype, UfoStereotype::Perdurant);
        assert!(catalog.is_some());
        assert_eq!(catalog.unwrap().category, "PRD");
    }

    #[test]
    fn classifies_cli_as_endurant_with_parent_suggestion() {
        let parents = vec!["b00t.cli".into(), "git.cli".into(), "gh.cli".into()];
        let (stereotype, _, parent) =
            classify_orphan("gh-issues.agent\nAgent", &parents);
        assert_eq!(stereotype, UfoStereotype::Endurant);
        // Should match gh.cli (prefix overlap)
        assert_eq!(parent, Some("gh.cli".into()));
    }

    #[test]
    fn classifies_config_as_moment() {
        let (stereotype, _, _) = classify_orphan("cloud-budget\nConfig", &[]);
        assert_eq!(stereotype, UfoStereotype::Moment);
    }

    #[test]
    fn curate_empty_graph_returns_empty() {
        let nodes: Vec<serde_json::Value> = vec![];
        let edges: Vec<serde_json::Value> = vec![];
        let result = curate_orphans(&nodes, &edges);
        assert!(result.is_empty());
    }

    #[test]
    fn curate_connected_node_not_in_orphans() {
        let nodes = vec![
            serde_json::json!({"id": "a", "label": "Tool A", "role": "task"}),
            serde_json::json!({"id": "b", "label": "Tool B", "role": "ingest"}),
        ];
        let edges = vec![
            serde_json::json!({"from": "a", "to": "b", "label": "depends_on"}),
        ];
        let result = curate_orphans(&nodes, &edges);
        // Both are connected → no orphans
        assert!(result.is_empty());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dewey_parse_and_display() {
        let d = DeweyNumber::parse("PRD.ARCH.005.MBSE-VISUALIZATION").unwrap();
        assert_eq!(d.category, "PRD");
        assert_eq!(d.domain, "ARCH");
        assert_eq!(d.sequence, 5);
        assert_eq!(d.title, "MBSE-VISUALIZATION");
        assert_eq!(d.to_string(), "PRD.ARCH.005.MBSE-VISUALIZATION");
    }

    #[test]
    fn dewey_parse_multi_segment_title() {
        let d = DeweyNumber::parse("LEARN.RUST.001.KASUARI.CONSTRAINT.SOLVER").unwrap();
        assert_eq!(d.category, "LEARN");
        assert_eq!(d.title, "KASUARI.CONSTRAINT.SOLVER");
    }

    #[test]
    fn dewey_rejects_short() {
        assert!(DeweyNumber::parse("PRD.ARCH").is_none());
    }

    #[test]
    fn prd_implements_abstract_document() {
        let prd = PrdDatum {
            name: "PRD-ARCH-005".into(),
            catalog: DeweyNumber::parse("PRD.ARCH.005.MBSE-VISUALIZATION").unwrap(),
            title: "MBSE Visualization".into(),
            summary: "Requirements for isometric model-based systems engineering visualization".into(),
            ufo_stereotype: UfoStereotype::Endurant,
            domain: "architecture".into(),
            status: DocumentStatus::Draft,
            related: vec!["PRD-ARCH-004".into()],
            overview: "Long-form overview...".into(),
            acceptance_criteria: "Must pass visual regression tests".into(),
            dependencies: None,
            risks: None,
            references: None,
            author: None,
        };
        assert_eq!(prd.title(), "MBSE Visualization");
        assert_eq!(prd.catalog_number().to_string(), "PRD.ARCH.005.MBSE-VISUALIZATION");
        assert_eq!(prd.ufo_stereotype(), UfoStereotype::Endurant);
        assert!(PrdDatum::required_stanzas().contains(&"[prd.overview]"));
        assert!(PrdDatum::optional_stanzas().contains(&"[prd.risks]"));
    }

    #[test]
    fn validate_missing_stanzas() {
        let toml = "[prd]\ntitle = \"Test\"\n[prd.overview]\ntext = \"...\"\n";
        let err = validate_stanzas(toml, &["[prd.overview]", "[prd.acceptance]"]).unwrap_err();
        assert!(err.contains(&"[prd.acceptance]".to_string()));
    }

    #[test]
    fn validate_all_present() {
        let toml = "[prd]\ntitle = \"Test\"\n[prd.overview]\ntext = \"...\"\n[prd.acceptance]\ntext = \"...\"\n";
        assert!(validate_stanzas(toml, &["[prd.overview]", "[prd.acceptance]"]).is_ok());
    }
}
