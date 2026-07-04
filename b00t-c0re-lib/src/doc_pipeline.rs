//! Document evidence pipeline — type system for document → chunk → evidence → requirements.
//!
//! Implements the full traceability chain:
//! ```text
//! DocumentSource → SemanticChunk[] → Evidence[] → Requirement[] → FOLFormula<T>[]
//!        ↓                ↓              ↓              ↓
//!   UFO:Endurant    UFO:Perdurant  UFO:Relator   UFO:Endurant+Role
//! ```
//!
//! ## UFO (Unified Foundational Ontology) concept-as-code
//!
//! Rust traits encode Guizzardi et al. UFO stereotypes:
//! - **Endurant** (Object): exists wholly at each moment — DocumentSource, Requirement
//! - **Perdurant** (Event): unfolds over time — SemanticChunk (the chunking process)
//! - **Relator**: mediates between entities — Evidence (links chunk to requirement)
//! - **Quality**: intrinsic property — confidence, priority as typed values
//! - **Role**: anti-rigid, externally dependent — Requirement as stakeholder role
//! - **Category**: rigid, essential — P̲r̲e̲d̲i̲c̲a̲t̲e̲<̲T̲>̲ as FOL category
//!
//! ## FOL (First Order Logic) stereotype
//!
//! Generic `Predicate<T>` trait enables FOL formulas over any evidence/requirement type:
//! ```text
//! ∀r∈Requirement: isFunctional(r) → hasRationale(r)
//! ∃e∈Evidence: supports(e, requirement)
//! ```
//!
//! ## Proxy-Pointer RAG
//!
//! Every `Evidence` carries a `ProvenancePointer` back to source document + chunk,
//! enabling retrievable provenance for any derived requirement.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fmt::Debug;

// ── Section A: Document Source ───────────────────────────────────────────

/// Format of a source document ingested into the pipeline.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum DocumentFormat {
    Markdown,
    LaTeX,
    Pdf,
    Html,
    PlainText,
}

/// Source document metadata — the root of the evidence chain.
///
/// UFO stereotype: **Endurant** — the document exists wholly at each moment.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct DocumentSource {
    /// Unique identifier (e.g., arxiv:2404.17842)
    pub source_id: String,
    /// Human-readable title
    pub title: String,
    /// Author list
    pub authors: Vec<String>,
    /// Full abstract or summary
    pub abstract_text: String,
    /// Canonical URL to the source
    pub url: Option<String>,
    /// Direct PDF/download URL
    pub pdf_url: Option<String>,
    /// When the document was fetched from source
    pub fetched_at: DateTime<Utc>,
    /// Content hash for deduplication (SHA-256)
    pub content_hash: Option<String>,
    /// Original format
    pub format: DocumentFormat,
    /// Additional metadata (e.g., arxiv categories, DOI)
    #[serde(default)]
    pub metadata: HashMap<String, String>,
}

// ── Section B: Semantic Chunk ────────────────────────────────────────────

/// Metadata about a chunk for retrieval filtering.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
pub struct ChunkMetadata {
    /// Token count in this chunk
    pub token_count: usize,
    /// Character count
    pub char_count: usize,
    /// Section/heading the chunk belongs to
    pub section_header: Option<String>,
    /// Page range in source PDF (if applicable)
    pub page_range: Option<(u32, u32)>,
}

/// A semantically-coherent chunk of a document with vector embedding.
///
/// UFO stereotype: **Perdurant** — chunking is an event that unfolds over time.
/// Each chunk is a temporal part of the ingestion process.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SemanticChunk {
    /// Unique chunk identifier
    pub chunk_id: String,
    /// Back-reference to parent document
    pub source_id: String,
    /// 0-based index within the document's chunk sequence
    pub chunk_index: usize,
    /// Chunk text content
    pub content: String,
    /// Topic tags for filtering
    #[serde(default)]
    pub topic_tags: Vec<String>,
    /// Semantic embedding vector (float precision varies by model)
    #[serde(default)]
    pub embedding: Vec<f32>,
    /// Model used to generate the embedding (e.g., "all-MiniLM-L6-v2")
    pub embedding_model: Option<String>,
    /// Chunk quality confidence [0.0, 1.0]
    pub confidence: f32,
    /// When the chunk was created
    pub created_at: DateTime<Utc>,
    /// Additional chunk metadata
    #[serde(default)]
    pub metadata: ChunkMetadata,
}

// ── Section C: Evidence — UFO Relator ────────────────────────────────────

/// Classification of extracted evidence.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum EvidenceType {
    /// A factual claim (e.g., "GPT-4 achieved 95% accuracy")
    Claim,
    /// A quantitative measurement (e.g., "8 criteria were used")
    Statistic,
    /// A constraint or boundary condition (e.g., "must handle 10k users")
    Constraint,
    /// A definition of a term or concept
    Definition,
    /// A direct observation from the source
    Observation,
}

/// Pointer back to source — the proxy-reference for RAG retrieval.
///
/// This is the immutable link in the provenance chain. Every derived
/// requirement can be traced back through Evidence → ProvenancePointer → Source.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ProvenancePointer {
    /// Source document ID
    pub source_id: String,
    /// Chunk ID within the source
    pub chunk_id: String,
    /// Starting line in the chunk
    pub line_start: usize,
    /// Ending line in the chunk
    pub line_end: usize,
    /// Verbatim quote snippet from the source
    pub quote_snippet: String,
}

/// Extracted evidence — a claim, statistic, or observation from a source.
///
/// UFO stereotype: **Relator** — Evidence mediates between a SemanticChunk
/// (where it was found) and a Requirement (that it supports).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Evidence {
    /// Unique evidence identifier
    pub evidence_id: String,
    /// Back-reference to source chunk
    pub chunk_id: String,
    /// Back-reference to source document
    pub source_id: String,
    /// The extracted statement/claim/fact
    pub statement: String,
    /// Classification of evidence type
    pub evidence_type: EvidenceType,
    /// Confidence in extraction [0.0, 1.0]
    pub confidence: f32,
    /// Method used for extraction (e.g., "llm", "regex", "manual")
    pub extraction_method: String,
    /// Verbatim quote from source proving this evidence
    pub source_quote: String,
    /// Line range in the source document
    pub line_range: Option<(usize, usize)>,
    /// Proxy-pointer for RAG — traceable back to source
    pub provenance: ProvenancePointer,
    /// When the evidence was extracted
    pub extracted_at: DateTime<Utc>,
}

// ── Section D: Requirement — SysMLv2 ReqIF compatible ────────────────────

/// Requirement type classification per SysMLv2 / ReqIF.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum RequirementType {
    Functional,
    NonFunctional,
    Constraint,
    Interface,
    #[serde(rename = "design")]
    DesignConstraint,
    Security,
    Performance,
    Stakeholder,
}

/// SysMLv2 stereotype for requirements modeling.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum SysMLv2Stereotype {
    /// <<requirement>> — base stereotype
    Requirement,
    /// <<functionalRequirement>>
    FunctionalRequirement,
    /// <<interfaceRequirement>>
    InterfaceRequirement,
    /// <<performanceRequirement>>
    PerformanceRequirement,
    /// <<designConstraint>>
    DesignConstraint,
    /// <<securityRequirement>>
    SecurityRequirement,
    /// <<stakeholderRequirement>>
    StakeholderRequirement,
}

/// ReqIF-compatible metadata for tool interchange.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ReqIFMetadata {
    /// ReqIF unique identifier
    pub reqif_id: String,
    /// Object type in ReqIF model
    pub object_type: String,
    /// Last modification timestamp
    pub last_change: Option<DateTime<Utc>>,
    /// Authoring tool identifier
    pub tool_id: Option<String>,
}

/// Derived requirement — traceable back through Evidence to Source.
///
/// UFO stereotype: **Endurant + Role** — a Requirement is an endurant object
/// that plays the role of constraining/describing a system. The Role is
/// anti-rigid: the same text could be a "suggestion" rather than a "requirement".
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Requirement {
    /// Unique requirement identifier
    pub req_id: String,
    /// Human-readable requirement text
    pub text: String,
    /// SysMLv2 / ReqIF type classification
    pub req_type: RequirementType,
    /// Priority (1 = highest, 5 = lowest)
    pub priority: u8,
    /// Rationale — why this requirement exists
    pub rationale: Option<String>,
    /// Evidence IDs that support this requirement (provenance chain)
    #[serde(default)]
    pub derived_from: Vec<String>,
    /// IDs of requirements this one satisfies/traces to
    #[serde(default)]
    pub satisfies: Vec<String>,
    /// Verification method or reference
    pub verified_by: Option<String>,
    /// Lifecycle status
    pub status: RequirementStatus,
    /// Source document this was derived from
    pub source_id: String,
    /// ReqIF interchange metadata
    #[serde(default)]
    pub reqif: Option<ReqIFMetadata>,
    /// SysMLv2 stereotype
    #[serde(default)]
    pub sysml_stereotype: Option<SysMLv2Stereotype>,
    /// When the requirement was created
    pub created_at: DateTime<Utc>,
}

/// Requirement lifecycle status.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum RequirementStatus {
    Proposed,
    Approved,
    Verified,
    Rejected,
    Implemented,
}

// ── Section E: FOL Stereotype — First Order Logic as Rust generics ────────

/// A predicate over a term — the atomic unit of FOL.
///
/// UFO stereotype: **Category** — predicates define rigid categories
/// (e.g., `isFunctional(x)` or `hasRationale(x)`).
///
/// # Examples
/// ```ignore
/// impl Predicate<Requirement> for IsFunctional {
///     fn apply(&self, req: &Requirement) -> bool {
///         matches!(req.req_type, RequirementType::Functional)
///     }
/// }
/// ```
pub trait Predicate<T>: Debug + Send + Sync {
    fn name(&self) -> &str;
    fn apply(&self, term: &T) -> bool;
    fn box_clone(&self) -> Box<dyn Predicate<T>>;
}

/// FOL quantifier.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Quantifier {
    /// ∀ — Universal: "for all"
    ForAll,
    /// ∃ — Existential: "there exists"
    Exists,
}

/// FOL logical connective.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Connective {
    /// ∧ — Conjunction
    And,
    /// ∨ — Disjunction
    Or,
    /// → — Implication
    Implies,
    /// ¬ — Negation
    Not,
}

/// A First Order Logic formula over a type T.
///
/// Generic over T so the same formula structure works for Evidence,
/// Requirement, or any domain type. The predicates are evaluated
/// against the terms to determine truth.
///
/// NOTE: This type uses trait objects (Box<dyn Predicate<T>>) and
/// therefore cannot derive Serialize/Deserialize/PartialEq. Use
/// `SerializableFOLFormula` for serialization and storage.
///
/// # Example formula
/// ```ignore
/// // ∀r ∈ Requirement: isFunctional(r) → hasRationale(r)
/// FOLFormula {
///     predicates: vec![Box::new(IsFunctional), Box::new(HasRationale)],
///     quantifier: Quantifier::ForAll,
///     connective: Connective::Implies,
///     terms: requirements,
/// }
/// ```
#[derive(Debug)]
pub struct FOLFormula<T> {
    /// Predicates applied to each term
    pub predicates: Vec<Box<dyn Predicate<T>>>,
    /// Quantifier scope (∀ or ∃)
    pub quantifier: Quantifier,
    /// How predicates are connected
    pub connective: Connective,
    /// Terms the formula ranges over
    pub terms: Vec<T>,
    /// Human-readable representation of the formula
    pub description: String,
}

/// Serialize-friendly FOL formula — uses predicate names instead of trait objects.
///
/// The trait-based `FOLFormula<T>` can't be easily serialized with standard serde
/// (trait objects need `erased_serde`). This struct uses string identifiers for
/// predicates, making it JSON/TOML-friendly for NoSQL storage and API exchange.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SerializableFOLFormula {
    /// Predicate names (e.g., "is_functional", "has_rationale")
    pub predicate_names: Vec<String>,
    /// Quantifier
    pub quantifier: Quantifier,
    /// Connective
    pub connective: Connective,
    /// Term identifiers (e.g., requirement IDs)
    pub term_ids: Vec<String>,
    /// Human-readable formula representation
    pub description: String,
}

/// Trait for types that can be stereotyped as FOL formulas.
pub trait FOLStereotype<T> {
    /// Convert to a FOL formula over T
    fn to_fol_formula(&self) -> FOLFormula<T>;
}

// ── Section F: UFO Concept-as-Code ───────────────────────────────────────

/// UFO: Endurant — exists wholly at each moment in time.
///
/// An Endurant (Object) has all its parts present at every moment
/// it exists. Contrast with Perdurant, which has temporal parts.
pub trait Endurant {
    /// Identity criterion — what makes two instances the same endurant?
    fn identity_criterion(&self) -> String;
    /// Does this endurant exist wholly at the given time?
    fn exists_wholly_at(&self, time: DateTime<Utc>) -> bool;
    /// Kind/type of this endurant
    fn endurant_kind(&self) -> &str;
}

/// UFO: Perdurant — unfolds over time, has temporal parts.
///
/// A Perdurant (Event/Process) is composed of temporal parts —
/// each phase of a chunking pipeline is a temporal part.
pub trait Perdurant {
    /// Temporal parts of this event as (start, end) pairs
    fn temporal_parts(&self) -> Vec<(DateTime<Utc>, DateTime<Utc>)>;
    /// Entities participating in this event
    fn participates_in(&self) -> Vec<String>;
}

/// UFO: Relator — mediates between two or more entities.
///
/// A Relator is existentially dependent on its relata.
/// Evidence is a relator: it mediates between a SemanticChunk
/// (what was found) and a Requirement (what it supports).
pub trait Relator {
    /// The two (or more) entities being mediated
    fn mediates_between(&self) -> (String, String);
    /// Type of relator relationship
    fn relator_type(&self) -> RelatorType;
}

/// Classification of relator types.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum RelatorType {
    /// Material relation (e.g., "is-evidence-for")
    Material,
    /// Formal relation (e.g., "derived-from")
    Formal,
    /// Comparative relation
    Comparative,
    /// Causal relation
    Causal,
}

/// UFO: Quality — intrinsic property that inheres in an entity.
///
/// Qualities are existentially dependent on their bearers.
/// E.g., the confidence of a chunk, the priority of a requirement.
pub trait Quality {
    /// The quality space this quality belongs to
    fn quality_space(&self) -> String;
    /// The specific value region within that space
    fn value_region(&self) -> serde_json::Value;
}

/// UFO: Role — anti-rigid, relationally dependent type.
///
/// A Role is a type that an entity can play or cease to play
/// without changing its identity. "Requirement" is a role played
/// by a statement that constrains a system.
pub trait Role {
    /// The entity playing this role (identity of the bearer)
    fn played_by(&self) -> String;
    /// Roles are anti-rigid — an entity can gain/lose them
    fn is_anti_rigid(&self) -> bool;
}

/// UFO: Category — rigid, essential type.
///
/// A Category is a type that an entity cannot cease to be
/// without losing its identity. Predicates in FOL are categories.
pub trait Category {
    /// Categories are rigid — essential to identity
    fn is_rigid(&self) -> bool;
    /// Does this category subsume (include) the other?
    fn subsumes(&self, other: &Self) -> bool;
}

// ── UFO Trait Implementations ────────────────────────────────────────────

impl Endurant for DocumentSource {
    fn identity_criterion(&self) -> String {
        format!("DocumentSource({})", self.source_id)
    }
    fn exists_wholly_at(&self, time: DateTime<Utc>) -> bool {
        self.fetched_at <= time
    }
    fn endurant_kind(&self) -> &str {
        "Document"
    }
}

impl Endurant for Requirement {
    fn identity_criterion(&self) -> String {
        format!("Requirement({})", self.req_id)
    }
    fn exists_wholly_at(&self, time: DateTime<Utc>) -> bool {
        self.created_at <= time
    }
    fn endurant_kind(&self) -> &str {
        "Requirement"
    }
}

impl Role for Requirement {
    fn played_by(&self) -> String {
        self.source_id.clone()
    }
    fn is_anti_rigid(&self) -> bool {
        // A statement can cease to be a requirement (e.g., if deprecated)
        true
    }
}

impl Perdurant for SemanticChunk {
    fn temporal_parts(&self) -> Vec<(DateTime<Utc>, DateTime<Utc>)> {
        // A chunk is a single temporal part (instantaneous creation)
        vec![(self.created_at, self.created_at)]
    }
    fn participates_in(&self) -> Vec<String> {
        vec![self.source_id.clone()]
    }
}

impl Relator for Evidence {
    fn mediates_between(&self) -> (String, String) {
        (
            format!("chunk:{}", self.chunk_id),
            format!("source:{}", self.source_id),
        )
    }
    fn relator_type(&self) -> RelatorType {
        RelatorType::Material
    }
}

// ── Section G: Pipeline Stage Result ─────────────────────────────────────

/// Stages in the document evidence pipeline.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum PipelineStage {
    Download,
    Chunk,
    Extract,
    Derive,
    Validate,
}

/// Result of a single pipeline stage.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct StageResult<T> {
    /// Which stage produced this result
    pub stage: PipelineStage,
    /// The stage output
    pub result: T,
    /// Duration in milliseconds
    pub duration_ms: u64,
    /// Non-fatal errors encountered
    #[serde(default)]
    pub errors: Vec<String>,
    /// Warnings encountered
    #[serde(default)]
    pub warnings: Vec<String>,
}

/// Complete result of the document evidence pipeline.
///
/// Represents the full traceability chain from source to requirements.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct FullPipelineResult {
    /// Original document source
    pub source: DocumentSource,
    /// Semantic chunks extracted from the document
    pub chunks: Vec<SemanticChunk>,
    /// Evidence items extracted from chunks
    pub evidences: Vec<Evidence>,
    /// Requirements derived from evidence
    pub requirements: Vec<Requirement>,
    /// FOL formulas over the requirements (serializable form)
    #[serde(default)]
    pub fol_formulas: Vec<SerializableFOLFormula>,
    /// Pipeline version for reproducibility
    pub pipeline_version: String,
    /// When the pipeline executed
    pub executed_at: DateTime<Utc>,
    /// Total execution time in milliseconds
    pub total_duration_ms: u64,
}

// ── Constructors — DRY factory methods ────────────────────────────────────

impl DocumentSource {
    /// Create from arxiv ID with minimal metadata.
    pub fn arxiv(id: &str, title: &str, authors: &[&str], abstract_text: &str) -> Self {
        Self {
            source_id: format!("arxiv:{id}"),
            title: title.into(),
            authors: authors.iter().map(|s| s.to_string()).collect(),
            abstract_text: abstract_text.into(),
            url: Some(format!("https://arxiv.org/abs/{id}")),
            pdf_url: Some(format!("https://arxiv.org/pdf/{id}")),
            fetched_at: Utc::now(),
            content_hash: None,
            format: DocumentFormat::Pdf,
            metadata: HashMap::new(),
        }
    }
}

impl ProvenancePointer {
    /// Create a provenance link back to source.
    pub fn new(source_id: &str, chunk_id: &str, line_start: usize, line_end: usize, quote: &str) -> Self {
        Self {
            source_id: source_id.into(),
            chunk_id: chunk_id.into(),
            line_start,
            line_end,
            quote_snippet: quote.into(),
        }
    }
}

impl Evidence {
    /// Extract evidence from a chunk with provenance link.
    pub fn from_chunk(
        id: &str, chunk_id: &str, source_id: &str,
        statement: &str, evidence_type: EvidenceType,
        confidence: f32, source_quote: &str,
        line_start: usize, line_end: usize,
    ) -> Self {
        Self {
            evidence_id: id.into(),
            chunk_id: chunk_id.into(),
            source_id: source_id.into(),
            statement: statement.into(),
            evidence_type,
            confidence,
            extraction_method: "llm".into(),
            source_quote: source_quote.into(),
            line_range: Some((line_start, line_end)),
            provenance: ProvenancePointer::new(source_id, chunk_id, line_start, line_end, source_quote),
            extracted_at: Utc::now(),
        }
    }
}

impl SemanticChunk {
    /// Create a chunk with topic tags and embedding.
    pub fn new(
        id: &str, source_id: &str, index: usize,
        content: &str, tags: &[&str], embedding: Vec<f32>,
        confidence: f32, section: Option<&str>,
    ) -> Self {
        Self {
            chunk_id: id.into(),
            source_id: source_id.into(),
            chunk_index: index,
            content: content.into(),
            topic_tags: tags.iter().map(|s| s.to_string()).collect(),
            embedding,
            embedding_model: Some("all-MiniLM-L6-v2".into()),
            confidence,
            created_at: Utc::now(),
            metadata: ChunkMetadata {
                token_count: content.split_whitespace().count(),
                char_count: content.len(),
                section_header: section.map(|s| s.into()),
                page_range: None,
            },
        }
    }
}

impl Requirement {
    /// Derive a SysMLv2 requirement from evidence.
    pub fn from_evidence(
        id: &str, text: &str, req_type: RequirementType, priority: u8,
        rationale: &str, derived_from: &[&str], source_id: &str,
        stereotype: SysMLv2Stereotype,
    ) -> Self {
        Self {
            req_id: id.into(),
            text: text.into(),
            req_type,
            priority,
            rationale: Some(rationale.into()),
            derived_from: derived_from.iter().map(|s| s.to_string()).collect(),
            satisfies: vec![],
            verified_by: None,
            status: RequirementStatus::Proposed,
            source_id: source_id.into(),
            reqif: Some(ReqIFMetadata {
                reqif_id: format!("reqif-{id}"),
                object_type: "REQUIREMENT".into(),
                last_change: None,
                tool_id: Some("b00t-doc-pipeline".into()),
            }),
            sysml_stereotype: Some(stereotype),
            created_at: Utc::now(),
        }
    }
}

impl SerializableFOLFormula {
    /// Builder for FOL formulas — reduces boilerplate.
    pub fn new(
        quantifier: Quantifier, connective: Connective,
        predicates: &[&str], terms: &[&str], description: &str,
    ) -> Self {
        Self {
            predicate_names: predicates.iter().map(|s| s.to_string()).collect(),
            quantifier,
            connective,
            term_ids: terms.iter().map(|s| s.to_string()).collect(),
            description: description.into(),
        }
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;

    fn make_test_source() -> DocumentSource {
        DocumentSource {
            source_id: "arxiv:2404.17842".into(),
            title: "Test Paper".into(),
            authors: vec!["Author One".into()],
            abstract_text: "A test abstract about SRS generation.".into(),
            url: Some("https://arxiv.org/abs/2404.17842".into()),
            pdf_url: Some("https://arxiv.org/pdf/2404.17842".into()),
            fetched_at: Utc::now(),
            content_hash: Some("abc123".into()),
            format: DocumentFormat::Pdf,
            metadata: HashMap::new(),
        }
    }

    fn make_test_chunk() -> SemanticChunk {
        SemanticChunk {
            chunk_id: "chunk:0".into(),
            source_id: "arxiv:2404.17842".into(),
            chunk_index: 0,
            content: "LLMs can generate accurate SRS documents.".into(),
            topic_tags: vec!["SRS".into(), "LLM".into()],
            embedding: vec![0.1, 0.2, 0.3],
            embedding_model: Some("all-MiniLM-L6-v2".into()),
            confidence: 0.95,
            created_at: Utc::now(),
            metadata: ChunkMetadata {
                token_count: 10,
                char_count: 45,
                section_header: Some("Abstract".into()),
                page_range: None,
            },
        }
    }

    fn make_test_evidence() -> Evidence {
        Evidence {
            evidence_id: "ev:001".into(),
            chunk_id: "chunk:0".into(),
            source_id: "arxiv:2404.17842".into(),
            statement: "GPT-4 can generate SRS drafts matching entry-level engineer quality.".into(),
            evidence_type: EvidenceType::Claim,
            confidence: 0.92,
            extraction_method: "llm".into(),
            source_quote: "Our results suggest that LLMs can match the output quality of an entry-level software engineer".into(),
            line_range: Some((12, 14)),
            provenance: ProvenancePointer {
                source_id: "arxiv:2404.17842".into(),
                chunk_id: "chunk:0".into(),
                line_start: 12,
                line_end: 14,
                quote_snippet: "LLMs can match the output quality of an entry-level software engineer".into(),
            },
            extracted_at: Utc::now(),
        }
    }

    fn make_test_requirement() -> Requirement {
        Requirement {
            req_id: "REQ-001".into(),
            text: "The system SHALL generate SRS documents with quality matching entry-level software engineers.".into(),
            req_type: RequirementType::Functional,
            priority: 1,
            rationale: Some("Derived from LLM capability evidence in source paper".into()),
            derived_from: vec!["ev:001".into()],
            satisfies: vec![],
            verified_by: None,
            status: RequirementStatus::Proposed,
            source_id: "arxiv:2404.17842".into(),
            reqif: Some(ReqIFMetadata {
                reqif_id: "reqif-001".into(),
                object_type: "REQUIREMENT".into(),
                last_change: None,
                tool_id: Some("b00t-doc-pipeline".into()),
            }),
            sysml_stereotype: Some(SysMLv2Stereotype::FunctionalRequirement),
            created_at: Utc::now(),
        }
    }

    #[test]
    fn test_evidence_serialization_roundtrip() {
        let ev = make_test_evidence();
        let json = serde_json::to_string(&ev).unwrap();
        let parsed: Evidence = serde_json::from_str(&json).unwrap();
        assert_eq!(ev.evidence_id, parsed.evidence_id);
        assert_eq!(ev.provenance.source_id, parsed.provenance.source_id);
    }

    #[test]
    fn test_requirement_derived_from_evidence() {
        let req = make_test_requirement();
        assert_eq!(req.derived_from.len(), 1);
        assert_eq!(req.derived_from[0], "ev:001");
        assert_eq!(req.priority, 1);
    }

    #[test]
    fn test_provenance_pointer_roundtrip() {
        let pp = ProvenancePointer {
            source_id: "arxiv:2404.17842".into(),
            chunk_id: "chunk:0".into(),
            line_start: 12,
            line_end: 14,
            quote_snippet: "test quote".into(),
        };
        let json = serde_json::to_string(&pp).unwrap();
        let parsed: ProvenancePointer = serde_json::from_str(&json).unwrap();
        assert_eq!(pp.source_id, parsed.source_id);
        assert_eq!(pp.line_start, parsed.line_start);
    }

    #[test]
    fn test_document_source_endurant() {
        let doc = make_test_source();
        let id = doc.identity_criterion();
        assert!(id.contains("arxiv:2404.17842"));
        assert!(doc.exists_wholly_at(Utc::now()));
        assert_eq!(doc.endurant_kind(), "Document");
    }

    #[test]
    fn test_requirement_endurant_and_role() {
        let req = make_test_requirement();
        assert!(req.exists_wholly_at(Utc::now()));
        assert!(req.is_anti_rigid());
        assert_eq!(req.endurant_kind(), "Requirement");
    }

    #[test]
    fn test_chunk_perdurant() {
        let chunk = make_test_chunk();
        let parts = chunk.temporal_parts();
        assert_eq!(parts.len(), 1);
        assert!(chunk.participates_in().contains(&"arxiv:2404.17842".to_string()));
    }

    #[test]
    fn test_evidence_relator() {
        let ev = make_test_evidence();
        let (left, right) = ev.mediates_between();
        assert!(left.contains("chunk:"));
        assert!(right.contains("arxiv:"));
        assert_eq!(ev.relator_type(), RelatorType::Material);
    }

    #[test]
    fn test_pipeline_stage_serialization() {
        let stage = PipelineStage::Extract;
        let json = serde_json::to_string(&stage).unwrap();
        assert!(json.contains("extract"));
        let parsed: PipelineStage = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed, PipelineStage::Extract);
    }

    #[test]
    fn test_full_pipeline_result() {
        let result = FullPipelineResult {
            source: make_test_source(),
            chunks: vec![make_test_chunk()],
            evidences: vec![make_test_evidence()],
            requirements: vec![make_test_requirement()],
            fol_formulas: vec![SerializableFOLFormula {
                predicate_names: vec!["is_functional".into(), "has_rationale".into()],
                quantifier: Quantifier::ForAll,
                connective: Connective::Implies,
                term_ids: vec!["REQ-001".into()],
                description: "∀r: isFunctional(r) → hasRationale(r)".into(),
            }],
            pipeline_version: "0.1.0".into(),
            executed_at: Utc::now(),
            total_duration_ms: 1500,
        };
        let json = serde_json::to_string(&result).unwrap();
        let parsed: FullPipelineResult = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.source.source_id, "arxiv:2404.17842");
        assert_eq!(parsed.evidences.len(), 1);
        assert_eq!(parsed.requirements.len(), 1);
        assert_eq!(parsed.fol_formulas.len(), 1);
    }
}
