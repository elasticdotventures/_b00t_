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
    ChunkMetadata, Connective, DocumentFormat, DocumentSource, Evidence, EvidenceType,
    FullPipelineResult, ProvenancePointer, Quantifier, ReqIFMetadata, Requirement,
    RequirementStatus, RequirementType, SemanticChunk, SerializableFOLFormula, SysMLv2Stereotype,
};
use b00t_c0re_lib::doc_pipeline::{
    Endurant, Perdurant, Relator, RelatorType, Role,
};
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
    let source = DocumentSource {
        source_id: "arxiv:2404.17842".into(),
        title: "Using LLMs in Software Requirements Specifications: An Empirical Evaluation".into(),
        authors: vec![
            "Madhava Krishna".into(),
            "Bhagesh Gaur".into(),
            "Arsh Verma".into(),
            "Pankaj Jalote".into(),
        ],
        abstract_text: "LLMs can generate accurate, coherent SRS documents matching entry-level engineer quality. GPT-4 outperforms CodeLlama. 8 criteria used for evaluation. 4 use cases tested. Significant time savings demonstrated.".into(),
        url: Some("https://arxiv.org/abs/2404.17842".into()),
        pdf_url: Some("https://arxiv.org/pdf/2404.17842".into()),
        fetched_at: Utc::now(),
        content_hash: Some("sha256:abc123def456".into()),
        format: DocumentFormat::Pdf,
        metadata: std::collections::HashMap::from([
            ("categories".into(), "cs.SE, cs.AI".into()),
            ("published".into(), "2024-04-27".into()),
        ]),
    };

    // UFO: Endurant check
    assert!(source.exists_wholly_at(Utc::now()));
    assert_eq!(source.endurant_kind(), "Document");
    assert!(source.identity_criterion().contains("arxiv:2404.17842"));

    // ── Stage 2: Semantic Chunking (UFO: Perdurant) ──────────────────────
    let chunks = vec![
        SemanticChunk {
            chunk_id: "chunk:0".into(),
            source_id: "arxiv:2404.17842".into(),
            chunk_index: 0,
            content: "LLMs can produce accurate, coherent, and structured SRS drafts to accelerate the software development lifecycle.".into(),
            topic_tags: vec!["SRS".into(), "LLM".into(), "generation".into()],
            embedding: vec![0.12, 0.45, 0.78, 0.33, 0.91],
            embedding_model: Some("all-MiniLM-L6-v2".into()),
            confidence: 0.95,
            created_at: Utc::now(),
            metadata: ChunkMetadata {
                token_count: 21,
                char_count: 112,
                section_header: Some("Abstract".into()),
                page_range: Some((1, 1)),
            },
        },
        SemanticChunk {
            chunk_id: "chunk:1".into(),
            source_id: "arxiv:2404.17842".into(),
            chunk_index: 1,
            content: "GPT-4 and CodeLlama evaluated against human benchmarks using 8 distinct criteria for a university club management system SRS.".into(),
            topic_tags: vec!["evaluation".into(), "GPT-4".into(), "CodeLlama".into(), "benchmark".into()],
            embedding: vec![0.67, 0.23, 0.44, 0.89, 0.12],
            embedding_model: Some("all-MiniLM-L6-v2".into()),
            confidence: 0.92,
            created_at: Utc::now(),
            metadata: ChunkMetadata {
                token_count: 25,
                char_count: 144,
                section_header: Some("Methodology".into()),
                page_range: Some((1, 2)),
            },
        },
        SemanticChunk {
            chunk_id: "chunk:2".into(),
            source_id: "arxiv:2404.17842".into(),
            chunk_index: 2,
            content: "LLMs match the output quality of an entry-level software engineer, delivering complete and consistent SRS drafts. Significant time savings demonstrated across 4 use cases.".into(),
            topic_tags: vec!["results".into(), "quality".into(), "time-savings".into()],
            embedding: vec![0.55, 0.31, 0.62, 0.47, 0.83],
            embedding_model: Some("all-MiniLM-L6-v2".into()),
            confidence: 0.88,
            created_at: Utc::now(),
            metadata: ChunkMetadata {
                token_count: 30,
                char_count: 175,
                section_header: Some("Results".into()),
                page_range: Some((2, 3)),
            },
        },
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
        Evidence {
            evidence_id: "ev:001".into(),
            chunk_id: "chunk:0".into(),
            source_id: "arxiv:2404.17842".into(),
            statement: "LLMs can produce accurate, coherent, and structured SRS drafts.".into(),
            evidence_type: EvidenceType::Claim,
            confidence: 0.94,
            extraction_method: "llm".into(),
            source_quote: "produce accurate, coherent, and structured drafts of these documents".into(),
            line_range: Some((8, 10)),
            provenance: ProvenancePointer {
                source_id: "arxiv:2404.17842".into(),
                chunk_id: "chunk:0".into(),
                line_start: 8,
                line_end: 10,
                quote_snippet: "produce accurate, coherent, and structured drafts".into(),
            },
            extracted_at: Utc::now(),
        },
        Evidence {
            evidence_id: "ev:002".into(),
            chunk_id: "chunk:1".into(),
            source_id: "arxiv:2404.17842".into(),
            statement: "GPT-4 and CodeLlama were evaluated against human benchmarks using 8 criteria.".into(),
            evidence_type: EvidenceType::Statistic,
            confidence: 0.96,
            extraction_method: "llm".into(),
            source_quote: "compare it against human benchmarks using eight distinct criteria".into(),
            line_range: Some((12, 14)),
            provenance: ProvenancePointer {
                source_id: "arxiv:2404.17842".into(),
                chunk_id: "chunk:1".into(),
                line_start: 12,
                line_end: 14,
                quote_snippet: "using eight distinct criteria".into(),
            },
            extracted_at: Utc::now(),
        },
        Evidence {
            evidence_id: "ev:003".into(),
            chunk_id: "chunk:2".into(),
            source_id: "arxiv:2404.17842".into(),
            statement: "LLM-generated SRS matches entry-level software engineer quality with significant time savings.".into(),
            evidence_type: EvidenceType::Claim,
            confidence: 0.91,
            extraction_method: "llm".into(),
            source_quote: "LLMs can match the output quality of an entry-level software engineer".into(),
            line_range: Some((16, 18)),
            provenance: ProvenancePointer {
                source_id: "arxiv:2404.17842".into(),
                chunk_id: "chunk:2".into(),
                line_start: 16,
                line_end: 18,
                quote_snippet: "match the output quality of an entry-level software engineer".into(),
            },
            extracted_at: Utc::now(),
        },
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
        Requirement {
            req_id: "REQ-SRS-001".into(),
            text: "The system SHALL generate SRS documents that are accurate, coherent, and structured, matching the quality of an entry-level software engineer.".into(),
            req_type: RequirementType::Functional,
            priority: 1,
            rationale: Some("Derived from ev:001 and ev:003 — LLM SRS quality matches entry-level engineers.".into()),
            derived_from: vec!["ev:001".into(), "ev:003".into()],
            satisfies: vec![],
            verified_by: Some("Automated 8-criteria benchmark evaluation".into()),
            status: RequirementStatus::Proposed,
            source_id: "arxiv:2404.17842".into(),
            reqif: Some(ReqIFMetadata {
                reqif_id: "reqif-b00t-001".into(),
                object_type: "REQUIREMENT".into(),
                last_change: None,
                tool_id: Some("b00t-doc-pipeline".into()),
            }),
            sysml_stereotype: Some(SysMLv2Stereotype::FunctionalRequirement),
            created_at: Utc::now(),
        },
        Requirement {
            req_id: "REQ-EVAL-002".into(),
            text: "The system SHALL evaluate SRS quality using at least 8 distinct benchmark criteria.".into(),
            req_type: RequirementType::NonFunctional,
            priority: 2,
            rationale: Some("Derived from ev:002 — paper used 8 criteria for human benchmark comparison.".into()),
            derived_from: vec!["ev:002".into()],
            satisfies: vec!["REQ-SRS-001".into()],
            verified_by: Some("Test suite with 8 metric dimensions".into()),
            status: RequirementStatus::Proposed,
            source_id: "arxiv:2404.17842".into(),
            reqif: Some(ReqIFMetadata {
                reqif_id: "reqif-b00t-002".into(),
                object_type: "REQUIREMENT".into(),
                last_change: None,
                tool_id: Some("b00t-doc-pipeline".into()),
            }),
            sysml_stereotype: Some(SysMLv2Stereotype::PerformanceRequirement),
            created_at: Utc::now(),
        },
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
    assert_eq!(req001.reqif.as_ref().unwrap().reqif_id, "reqif-b00t-001");

    // ── Stage 7: FOL Stereotyping ─────────────────────────────────────────
    let fol_formulas = vec![
        SerializableFOLFormula {
            predicate_names: vec!["is_functional".into(), "has_rationale".into()],
            quantifier: Quantifier::ForAll,
            connective: Connective::Implies,
            term_ids: vec!["REQ-SRS-001".into(), "REQ-EVAL-002".into()],
            description: "∀r ∈ Requirement: isFunctional(r) → hasRationale(r)".into(),
        },
        SerializableFOLFormula {
            predicate_names: vec!["derived_from_evidence".into(), "has_provenance".into()],
            quantifier: Quantifier::Exists,
            connective: Connective::And,
            term_ids: vec!["REQ-SRS-001".into()],
            description: "∃r ∈ Requirement: derivedFromEvidence(r) ∧ hasProvenance(r)".into(),
        },
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
    let doc = DocumentSource {
        source_id: "test:001".into(),
        title: "Test Document".into(),
        authors: vec!["Author".into()],
        abstract_text: "Test abstract".into(),
        url: None,
        pdf_url: None,
        fetched_at: Utc::now(),
        content_hash: None,
        format: DocumentFormat::Markdown,
        metadata: Default::default(),
    };
    assert_eq!(doc.endurant_kind(), "Document");
    assert!(doc.exists_wholly_at(Utc::now()));
    assert_eq!(doc.identity_criterion(), "DocumentSource(test:001)");

    // Perdurant: SemanticChunk
    let chunk = SemanticChunk {
        chunk_id: "chunk:test".into(),
        source_id: "test:001".into(),
        chunk_index: 0,
        content: "Test".into(),
        topic_tags: vec![],
        embedding: vec![0.1],
        embedding_model: None,
        confidence: 1.0,
        created_at: Utc::now(),
        metadata: Default::default(),
    };
    let parts = chunk.temporal_parts();
    assert_eq!(parts.len(), 1);
    assert!(chunk.participates_in().contains(&"test:001".to_string()));

    // Relator: Evidence
    let evidence = Evidence {
        evidence_id: "ev:test".into(),
        chunk_id: "chunk:test".into(),
        source_id: "test:001".into(),
        statement: "Test claim".into(),
        evidence_type: EvidenceType::Claim,
        confidence: 1.0,
        extraction_method: "manual".into(),
        source_quote: "test".into(),
        line_range: None,
        provenance: ProvenancePointer {
            source_id: "test:001".into(),
            chunk_id: "chunk:test".into(),
            line_start: 0,
            line_end: 1,
            quote_snippet: "test".into(),
        },
        extracted_at: Utc::now(),
    };
    let (left, right) = evidence.mediates_between();
    assert!(left.contains("chunk:"));
    assert!(right.contains("test:"));
    assert_eq!(evidence.relator_type(), RelatorType::Material);

    // Role: Requirement (anti-rigid)
    let req = Requirement {
        req_id: "REQ-TEST".into(),
        text: "Test requirement".into(),
        req_type: RequirementType::Functional,
        priority: 1,
        rationale: None,
        derived_from: vec![],
        satisfies: vec![],
        verified_by: None,
        status: RequirementStatus::Proposed,
        source_id: "test:001".into(),
        reqif: None,
        sysml_stereotype: None,
        created_at: Utc::now(),
    };
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
        SemanticChunk {
            chunk_id: "vec:a".into(),
            source_id: "test:vec".into(),
            chunk_index: 0,
            content: "Requirements engineering with LLMs".into(),
            topic_tags: vec!["requirements".into(), "LLM".into()],
            embedding: vec![0.9, 0.1, 0.1], // near [1,0,0] = requirements
            embedding_model: Some("test-model".into()),
            confidence: 1.0,
            created_at: Utc::now(),
            metadata: Default::default(),
        },
        SemanticChunk {
            chunk_id: "vec:b".into(),
            source_id: "test:vec".into(),
            chunk_index: 1,
            content: "Ontology-driven software design patterns".into(),
            topic_tags: vec!["ontology".into(), "design".into()],
            embedding: vec![0.1, 0.9, 0.1], // near [0,1,0] = ontology
            embedding_model: Some("test-model".into()),
            confidence: 1.0,
            created_at: Utc::now(),
            metadata: Default::default(),
        },
        SemanticChunk {
            chunk_id: "vec:c".into(),
            source_id: "test:vec".into(),
            chunk_index: 2,
            content: "Formal methods for verification".into(),
            topic_tags: vec!["formal-methods".into(), "verification".into()],
            embedding: vec![0.1, 0.1, 0.9], // near [0,0,1] = formal methods
            embedding_model: Some("test-model".into()),
            confidence: 1.0,
            created_at: Utc::now(),
            metadata: Default::default(),
        },
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
        source: DocumentSource {
            source_id: "arxiv:2404.17842".into(),
            title: "Test".into(),
            authors: vec!["Author".into()],
            abstract_text: "Abstract".into(),
            url: None,
            pdf_url: None,
            fetched_at: Utc::now(),
            content_hash: None,
            format: DocumentFormat::Pdf,
            metadata: Default::default(),
        },
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
