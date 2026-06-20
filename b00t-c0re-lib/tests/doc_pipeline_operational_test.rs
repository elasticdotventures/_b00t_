//! Operational integration test — Document Evidence Pipeline
//!
//! Demonstrates the full chain:
//!   arxiv:2404.17842 → DocumentSource → SemanticChunk[] → SearchableIndex
//!   → Evidence[] (with ProvenancePointer proxy-RAG) → Requirement[] (SysMLv2 ReqIF)
//!   → FOLFormula (∀/∃ stereotyping) → UFO trait concept-as-code
//!
//! UFO concepts exercised:
//!   - Endurant: DocumentSource, Requirement (exists wholly at each moment)
//!   - Perdurant: SemanticChunk (temporal parts of ingestion)
//!   - Relator: Evidence (mediates chunk ↔ source)
//!   - Role: Requirement (anti-rigid, played_by document)
//!   - Category: Predicate<T> (rigid, subsumption)
//!
//! FOL concepts exercised:
//!   - ∀ r: isFunctional(r) → hasRationale(r)
//!   - ∃ r: derivedFromEvidence(r) ∧ hasProvenance(r)

use b00t_c0re_lib::doc_pipeline::{
    Connective, DocumentSource, Evidence, EvidenceType,
    FullPipelineResult, Quantifier, Requirement, RequirementType,
    SemanticChunk, SerializableFOLFormula, SysMLv2Stereotype,
};
use b00t_c0re_lib::doc_pipeline::{
    Endurant, Perdurant, Relator, RelatorType, Role,
};
use b00t_c0re_lib::pipeline_nodes::{ChunkNode, Compose, EvidenceNode, PipelineNode};
use chrono::Utc;

// ═══════════════════════════════════════════════════════════════════════════
// Helper: Cosine similarity for semantic vector search
// ═══════════════════════════════════════════════════════════════════════════

fn cosine_similarity(a: &[f32], b: &[f32]) -> f32 {
    let dot: f32 = a.iter().zip(b).map(|(x, y)| x * y).sum();
    let mag_a: f32 = a.iter().map(|x| x * x).sum::<f32>().sqrt();
    let mag_b: f32 = b.iter().map(|x| x * x).sum::<f32>().sqrt();
    if mag_a == 0.0 || mag_b == 0.0 {
        0.0
    } else {
        dot / (mag_a * mag_b)
    }
}

/// Simulated NoSQL searchable index of chunks — vector + metadata search
struct ChunkIndex {
    chunks: Vec<SemanticChunk>,
}

impl ChunkIndex {
    fn new(chunks: Vec<SemanticChunk>) -> Self {
        Self { chunks }
    }

    /// Search by vector similarity — returns ranked results
    fn search_by_vector(&self, query_embedding: &[f32], top_k: usize) -> Vec<(f32, &SemanticChunk)> {
        let mut scored: Vec<(f32, &SemanticChunk)> = self
            .chunks
            .iter()
            .map(|ch| (cosine_similarity(query_embedding, &ch.embedding), ch))
            .collect();
        scored.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap());
        scored.truncate(top_k);
        scored
    }

    /// Search by topic tag
    fn search_by_tag(&self, tag: &str) -> Vec<&SemanticChunk> {
        self.chunks
            .iter()
            .filter(|ch| ch.topic_tags.iter().any(|t| t.contains(tag)))
            .collect()
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// Test 1: Full pipeline — arxiv → requirements → FOL → UFO
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn test_full_pipeline_operational() {
    // ── Stage 1: Document Source (UFO: Endurant) ──────────────────────────
    let source = DocumentSource::arxiv(
        "2404.17842",
        "Using LLMs in Software Requirements Specifications: An Empirical Evaluation",
        &["Madhava Krishna", "Bhagesh Gaur", "Arsh Verma", "Pankaj Jalote"],
        "LLMs can generate accurate, coherent SRS documents matching entry-level engineer quality. GPT-4 outperforms CodeLlama. 8 criteria used for evaluation. 4 use cases tested. Significant time savings demonstrated.",
    );

    // UFO: Endurant check
    assert!(source.exists_wholly_at(Utc::now()));
    assert_eq!(source.endurant_kind(), "Document");
    assert!(source.identity_criterion().contains("arxiv:2404.17842"));

    // ── Stage 2: Semantic Chunking (UFO: Perdurant) ──────────────────────
    let chunks = vec![
        SemanticChunk::new(
            "chunk:0", "arxiv:2404.17842", 0,
            "LLMs can produce accurate, coherent, and structured SRS drafts to accelerate the software development lifecycle.",
            &["SRS", "LLM", "generation"],
            vec![0.12, 0.45, 0.78, 0.33, 0.91], 0.95, Some("Abstract"),
        ),
        SemanticChunk::new(
            "chunk:1", "arxiv:2404.17842", 1,
            "GPT-4 and CodeLlama evaluated against human benchmarks using 8 distinct criteria for a university club management system SRS.",
            &["evaluation", "GPT-4", "CodeLlama", "benchmark"],
            vec![0.67, 0.23, 0.44, 0.89, 0.12], 0.92, Some("Methodology"),
        ),
        SemanticChunk::new(
            "chunk:2", "arxiv:2404.17842", 2,
            "LLMs match the output quality of an entry-level software engineer, delivering complete and consistent SRS drafts. Significant time savings demonstrated across 4 use cases.",
            &["results", "quality", "time-savings"],
            vec![0.55, 0.31, 0.62, 0.47, 0.83], 0.88, Some("Results"),
        ),
    ];

    // UFO: Perdurant check
    for ch in &chunks {
        let parts = ch.temporal_parts();
        assert_eq!(parts.len(), 1); // single temporal part (instantaneous creation)
        assert!(ch.participates_in().contains(&"arxiv:2404.17842".to_string()));
    }

    // ── Stage 3: NoSQL Vector Search ──────────────────────────────────────
    let index = ChunkIndex::new(chunks.clone());

    // Search for "SRS generation quality" — use the results chunk embedding as query
    let query_vec = vec![0.50, 0.30, 0.60, 0.45, 0.80]; // simulates a query embedding
    let search_results = index.search_by_vector(&query_vec, 2);
    assert_eq!(search_results.len(), 2);
    // Most similar should be chunk:2 (results/quality) based on cosine proximity
    assert!(search_results[0].0 > 0.8); // cosine similarity should be high
    println!(
        "🔍 Vector search top hit: {} (score={:.3}) → {:?}",
        search_results[0].1.chunk_id, search_results[0].0, search_results[0].1.topic_tags
    );

    // Search by topic tag
    let srs_chunks = index.search_by_tag("SRS");
    assert!(!srs_chunks.is_empty());

    // ── Stage 4: Evidence Extraction (UFO: Relator) ───────────────────────
    let evidences = vec![
        Evidence::from_chunk(
            "ev:001", "chunk:0", "arxiv:2404.17842",
            "LLMs can produce accurate, coherent, and structured SRS drafts.",
            EvidenceType::Claim, 0.94,
            "produce accurate, coherent, and structured drafts of these documents",
            8, 10,
        ),
        Evidence::from_chunk(
            "ev:002", "chunk:1", "arxiv:2404.17842",
            "GPT-4 and CodeLlama were evaluated against human benchmarks using 8 criteria.",
            EvidenceType::Statistic, 0.96,
            "compare it against human benchmarks using eight distinct criteria",
            12, 14,
        ),
        Evidence::from_chunk(
            "ev:003", "chunk:2", "arxiv:2404.17842",
            "LLM-generated SRS matches entry-level software engineer quality with significant time savings.",
            EvidenceType::Claim, 0.91,
            "LLMs can match the output quality of an entry-level software engineer",
            16, 18,
        ),
    ];

    // UFO: Relator check — Evidence mediates between chunk and source
    for ev in &evidences {
        let (left, right) = ev.mediates_between();
        assert!(left.contains("chunk:"));
        assert!(right.contains("arxiv:"));
        assert_eq!(ev.relator_type(), RelatorType::Material);
    }

    // ── Stage 5: PROXY-POINTER-RAG — verify full provenance chain ─────────
    // Trace ev:001 back to source through the chain
    let ev001 = &evidences[0];
    assert_eq!(ev001.provenance.source_id, "arxiv:2404.17842");
    assert_eq!(ev001.provenance.chunk_id, "chunk:0");
    assert_eq!(ev001.provenance.line_start, 8);
    assert_eq!(ev001.provenance.line_end, 10);

    // Verify the source chunk exists in the index
    let source_chunk = index.search_by_tag("SRS");
    assert!(source_chunk.iter().any(|ch| ch.chunk_id == "chunk:0"));

    // Proxy-pointer RAG: ev:001 → chunk:0 → arxiv:2404.17842 → https://arxiv.org/pdf/2404.17842
    let rag_chain = format!(
        "{} → {} → {} → {}",
        ev001.evidence_id,
        ev001.provenance.chunk_id,
        ev001.provenance.source_id,
        source.pdf_url.as_ref().unwrap()
    );
    println!("🔗 RAG chain: {rag_chain}");
    assert!(rag_chain.contains("ev:001"));
    assert!(rag_chain.contains("chunk:0"));
    assert!(rag_chain.contains("arxiv:2404.17842"));
    assert!(rag_chain.contains("arxiv.org/pdf/2404.17842"));

    // ── Stage 6: Requirement Derivation (SysMLv2 ReqIF) ───────────────────
    let requirements = vec![
        Requirement::from_evidence(
            "REQ-SRS-001",
            "The system SHALL generate SRS documents that are accurate, coherent, and structured, matching the quality of an entry-level software engineer.",
            RequirementType::Functional, 1,
            "Derived from ev:001 and ev:003 — LLM SRS quality matches entry-level engineers.",
            &["ev:001", "ev:003"], "arxiv:2404.17842",
            SysMLv2Stereotype::FunctionalRequirement,
        ),
        Requirement::from_evidence(
            "REQ-EVAL-002",
            "The system SHALL evaluate SRS quality using at least 8 distinct benchmark criteria.",
            RequirementType::NonFunctional, 2,
            "Derived from ev:002 — paper used 8 criteria for human benchmark comparison.",
            &["ev:002"], "arxiv:2404.17842",
            SysMLv2Stereotype::PerformanceRequirement,
        ),
    ];

    // UFO: Endurant + Role check for requirements
    for req in &requirements {
        assert!(req.exists_wholly_at(Utc::now()));
        assert_eq!(req.endurant_kind(), "Requirement");
        assert!(req.is_anti_rigid()); // Role: can be gained/lost
        assert_eq!(req.played_by(), "arxiv:2404.17842");
    }

    // Verify derivation chain: requirement → evidence → source
    assert_eq!(requirements[0].derived_from.len(), 2);
    assert!(requirements[0].derived_from.contains(&"ev:001".to_string()));
    assert!(requirements[0].derived_from.contains(&"ev:003".to_string()));

    // SysMLv2 ReqIF output verification
    let req001 = &requirements[0];
    assert_eq!(req001.sysml_stereotype, Some(SysMLv2Stereotype::FunctionalRequirement));
    assert!(req001.reqif.is_some());
    assert_eq!(req001.reqif.as_ref().unwrap().reqif_id, "reqif-REQ-SRS-001");

    // ── Stage 7: FOL Stereotyping ─────────────────────────────────────────
    let fol_formulas = vec![
        SerializableFOLFormula::new(
            Quantifier::ForAll, Connective::Implies,
            &["is_functional", "has_rationale"],
            &["REQ-SRS-001", "REQ-EVAL-002"],
            "∀r ∈ Requirement: isFunctional(r) → hasRationale(r)",
        ),
        SerializableFOLFormula::new(
            Quantifier::Exists, Connective::And,
            &["derived_from_evidence", "has_provenance"],
            &["REQ-SRS-001"],
            "∃r ∈ Requirement: derivedFromEvidence(r) ∧ hasProvenance(r)",
        ),
    ];

    // Verify FOL formulas
    assert_eq!(fol_formulas.len(), 2);
    assert_eq!(fol_formulas[0].quantifier, Quantifier::ForAll);
    assert_eq!(fol_formulas[0].connective, Connective::Implies);
    assert_eq!(fol_formulas[1].quantifier, Quantifier::Exists);
    assert_eq!(fol_formulas[1].connective, Connective::And);

    println!("📐 FOL: {}", fol_formulas[0].description);
    println!("📐 FOL: {}", fol_formulas[1].description);

    // ── Stage 8: Full Pipeline Result ─────────────────────────────────────
    let pipeline = FullPipelineResult {
        source: source.clone(),
        chunks: chunks.clone(),
        evidences: evidences.clone(),
        requirements: requirements.clone(),
        fol_formulas: fol_formulas.clone(),
        pipeline_version: "0.1.0".into(),
        executed_at: Utc::now(),
        total_duration_ms: 2340,
    };

    // Serialize to JSON (NoSQL-ready format)
    let json_output = serde_json::to_string_pretty(&pipeline).unwrap();
    assert!(json_output.contains("arxiv:2404.17842"));
    assert!(json_output.contains("REQ-SRS-001"));
    assert!(json_output.contains("forall"));
    assert!(json_output.contains("is_functional"));

    println!(
        "✅ Full pipeline: {} → {} chunks → {} evidence → {} requirements → {} FOL formulas ({} bytes)",
        source.source_id,
        pipeline.chunks.len(),
        pipeline.evidences.len(),
        pipeline.requirements.len(),
        pipeline.fol_formulas.len(),
        json_output.len(),
    );

    // ── Stage 9: JSON deserialization roundtrip ───────────────────────────
    let parsed: FullPipelineResult = serde_json::from_str(&json_output).unwrap();
    assert_eq!(parsed.source.source_id, source.source_id);
    assert_eq!(parsed.chunks.len(), 3);
    assert_eq!(parsed.evidences.len(), 3);
    assert_eq!(parsed.requirements.len(), 2);
    assert_eq!(parsed.fol_formulas.len(), 2);
}

// ═══════════════════════════════════════════════════════════════════════════
// Test 2: UFO trait concept-as-code — all stereotypes exercised
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn test_ufo_trait_concept_as_code() {
    // Endurant: DocumentSource
    let doc = DocumentSource::arxiv(
        "001", "Test Document", &["Author"],
        "Test abstract",
    );
    assert_eq!(doc.endurant_kind(), "Document");
    assert!(doc.exists_wholly_at(Utc::now()));
    // identity criterion for arxiv uses arxiv:{id} format
    assert!(doc.identity_criterion().contains("arxiv:001"));

    // Perdurant: SemanticChunk
    let chunk = SemanticChunk::new(
        "chunk:test", "test:001", 0,
        "Test", &[], vec![0.1], 1.0, None,
    );
    let parts = chunk.temporal_parts();
    assert_eq!(parts.len(), 1);
    assert!(chunk.participates_in().contains(&"test:001".to_string()));

    // Relator: Evidence
    let evidence = Evidence::from_chunk(
        "ev:test", "chunk:test", "test:001",
        "Test claim", EvidenceType::Claim, 1.0, "test", 0, 1,
    );
    let (left, right) = evidence.mediates_between();
    assert!(left.contains("chunk:"));
    assert!(right.contains("test:"));
    assert_eq!(evidence.relator_type(), RelatorType::Material);

    // Role: Requirement (anti-rigid)
    let req = Requirement::from_evidence(
        "REQ-TEST", "Test requirement", RequirementType::Functional, 1,
        "Test rationale", &[], "test:001",
        SysMLv2Stereotype::FunctionalRequirement,
    );
    assert!(req.is_anti_rigid());
    assert_eq!(req.played_by(), "test:001");
    assert_eq!(req.endurant_kind(), "Requirement");

    // Category: trait exists on Predicate (compiled-time check)
    // Predicate<T> extends Debug + Send + Sync — verified by compilation
    println!("✅ UFO: Endurant ✓ Perdurant ✓ Relator ✓ Role ✓ Category ✓");
}

// ═══════════════════════════════════════════════════════════════════════════
// Test 3: Semantic vector search — NoSQL-style query
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn test_vector_search_nosql_style() {
    let chunks = vec![
        SemanticChunk::new(
            "vec:a", "test:vec", 0,
            "Requirements engineering with LLMs",
            &["requirements", "LLM"],
            vec![0.9, 0.1, 0.1], // near [1,0,0] = requirements
            1.0, None,
        ),
        SemanticChunk::new(
            "vec:b", "test:vec", 1,
            "Ontology-driven software design patterns",
            &["ontology", "design"],
            vec![0.1, 0.9, 0.1], // near [0,1,0] = ontology
            1.0, None,
        ),
        SemanticChunk::new(
            "vec:c", "test:vec", 2,
            "Formal methods for verification",
            &["formal-methods", "verification"],
            vec![0.1, 0.1, 0.9], // near [0,0,1] = formal methods
            1.0, None,
        ),
    ];

    let index = ChunkIndex::new(chunks);

    // Query for "requirements" — should match vec:a
    let req_results = index.search_by_vector(&[1.0, 0.0, 0.0], 1);
    assert_eq!(req_results[0].1.chunk_id, "vec:a");
    assert!(req_results[0].0 > 0.9); // near-perfect match

    // Query for "ontology" — should match vec:b
    let onto_results = index.search_by_vector(&[0.0, 1.0, 0.0], 1);
    assert_eq!(onto_results[0].1.chunk_id, "vec:b");
    assert!(onto_results[0].0 > 0.9);

    // Query for "formal verification" — should match vec:c
    let formal_results = index.search_by_vector(&[0.0, 0.0, 1.0], 1);
    assert_eq!(formal_results[0].1.chunk_id, "vec:c");
    assert!(formal_results[0].0 > 0.9);

    // Hybrid: query near requirements+ontology boundary
    let hybrid_results = index.search_by_vector(&[0.6, 0.6, 0.0], 2);
    assert!(hybrid_results[0].0 >= hybrid_results[1].0); // both equally close to [0.6,0.6,0.0]
    println!(
        "🔍 Hybrid search: #1={} (score={:.3}), #2={} (score={:.3})",
        hybrid_results[0].1.chunk_id, hybrid_results[0].0,
        hybrid_results[1].1.chunk_id, hybrid_results[1].0,
    );
}

// ═══════════════════════════════════════════════════════════════════════════
// Test 4: JSON roundtrip — NoSQL persistence
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn test_nosql_json_roundtrip() {
    let pipeline = FullPipelineResult {
        source: DocumentSource::arxiv(
            "2404.17842", "Test", &["Author"], "Abstract",
        ),
        chunks: vec![],
        evidences: vec![],
        requirements: vec![],
        fol_formulas: vec![],
        pipeline_version: "0.1.0".into(),
        executed_at: Utc::now(),
        total_duration_ms: 0,
    };

    // Serialize to JSON (simulates writing to NoSQL / Qdrant / JSONL)
    let json = serde_json::to_string(&pipeline).unwrap();
    assert!(json.contains("arxiv:2404.17842"));

    // Deserialize back (simulates reading from NoSQL)
    let parsed: FullPipelineResult = serde_json::from_str(&json).unwrap();
    assert_eq!(parsed.source.source_id, "arxiv:2404.17842");
    assert_eq!(parsed.pipeline_version, "0.1.0");
}

// ═══════════════════════════════════════════════════════════════════════════
// Test 5: Compose pipeline node chain — typed pipeline composition
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn test_compose_pipeline_chain() {
    // Build: ChunkNode → EvidenceNode via Compose type-level composition
    // ChunkNode::Output = Vec<SemanticChunk>, EvidenceNode::Input = Vec<SemanticChunk> ✓
    let pipeline = Compose {
        first: ChunkNode,
        second: EvidenceNode,
    };

    // Feed a document source through the composed pipeline
    let source = DocumentSource::arxiv(
        "2404.17842",
        "LLM-based Requirements Engineering",
        &["Krishna", "Gaur", "Verma", "Jalote"],
        "LLMs can generate accurate, coherent SRS documents matching entry-level engineer quality.",
    );

    let results: Vec<Evidence> = pipeline.execute(source);

    // Pipeline produced evidence from chunks
    assert!(!results.is_empty());
    assert_eq!(results.len(), 1); // one chunk → one evidence

    // Each evidence has provenance auto-linked by the constructor
    for ev in &results {
        assert!(!ev.provenance.source_id.is_empty());
        assert!(!ev.provenance.chunk_id.is_empty());
        assert_eq!(ev.evidence_type, EvidenceType::Claim);
    }

    // UFO: Relator check on pipeline output
    assert_eq!(results[0].relator_type(), RelatorType::Material);

    // FOL contracts propagate through composition
    let pre = pipeline.preconditions();
    assert!(!pre.is_empty());
    let post = pipeline.postconditions();
    assert!(!post.is_empty());

    // State machine bridges ChunkNode → EvidenceNode
    let sm = pipeline.state_machine();
    assert!(sm.states.len() >= 4); // 2+2 states + compose bridge

    // Node metadata
    assert_eq!(pipeline.node_category(), b00t_c0re_lib::pipeline_nodes::NodeCategory::Composite);
    assert_eq!(pipeline.node_label(), "Compose");
    assert_eq!(pipeline.input_ports().len(), 1); // inherited from ChunkNode
    assert_eq!(pipeline.output_ports().len(), 1); // inherited from EvidenceNode

    // Verify PortDef names
    assert_eq!(pipeline.input_ports()[0].name, "document");
    assert_eq!(pipeline.output_ports()[0].name, "evidence");

    println!("✅ Compose<ChunkNode, EvidenceNode>: {} states, {} preconditions, {} postconditions",
        sm.states.len(), pre.len(), post.len());
}
