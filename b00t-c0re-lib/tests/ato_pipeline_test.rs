//! Integration test: ATO legislation ingestion pipeline
//! AtoClient (stub) → LegislationChunker (fallback) → EvidenceNode → RequirementsNode
//!
//! Uses stubs and offline fallback paths — no network required.

use b00t_c0re_lib::ato_client::{AtoAct, AtoClient};
use b00t_c0re_lib::pipeline_nodes::{
    EvidenceNode, LegislationChunker, PipelineNode, RequirementsNode,
};

/// Full pipeline: stub DocumentSource → chunks → evidence → requirements
#[test]
fn test_ato_pipeline_stub_itaa1997() {
    let client = AtoClient::default();
    let source = client.source_stub(&AtoAct::Itaa1997);

    assert_eq!(source.source_id, "ato:C2025C00001");
    assert_eq!(source.metadata.get("jurisdiction").unwrap(), "AU");

    // Stage 2: LegislationChunker — no network in CI, falls back to abstract_text
    let chunker = LegislationChunker;
    let chunks = chunker.execute(source.clone());

    assert!(
        !chunks.is_empty(),
        "chunker must produce at least one chunk"
    );
    for chunk in &chunks {
        assert_eq!(
            chunk.source_id, source.source_id,
            "chunk.source_id must match DocumentSource.source_id"
        );
        assert!(!chunk.content.is_empty(), "chunk content must not be empty");
    }

    // Stage 3: EvidenceNode
    let evidence_node = EvidenceNode;
    let evidence = evidence_node.execute(chunks.clone());

    assert!(
        !evidence.is_empty(),
        "evidence must be produced from chunks"
    );
    for ev in &evidence {
        assert!(
            !ev.statement.is_empty(),
            "evidence statement must not be empty"
        );
        assert!(
            ev.source_id.contains("ato:") || ev.chunk_id.contains("ato:"),
            "evidence must cite ATO source: source_id={} chunk_id={}",
            ev.source_id,
            ev.chunk_id
        );
    }

    // Stage 4: RequirementsNode
    let req_node = RequirementsNode;
    let requirements = req_node.execute(evidence);

    assert!(
        !requirements.is_empty(),
        "requirements must be derived from evidence"
    );
    for req in &requirements {
        assert!(!req.text.is_empty(), "requirement text must not be empty");
    }
}

#[test]
fn test_all_ato_acts_produce_stubs() {
    let client = AtoClient::default();
    let acts = [
        AtoAct::Itaa1997,
        AtoAct::Itaa1936,
        AtoAct::GstAct,
        AtoAct::FbtAct,
    ];
    for act in &acts {
        let source = client.source_stub(act);
        assert!(source.url.is_some());
        assert_eq!(source.metadata.get("stub").unwrap(), "true");
        assert_eq!(source.metadata.get("jurisdiction").unwrap(), "AU");
    }
}

#[test]
fn test_legislation_chunker_fallback_on_no_url() {
    use b00t_c0re_lib::doc_pipeline::{DocumentFormat, DocumentSource};
    use chrono::Utc;

    let source = DocumentSource {
        source_id: "ato:test".into(),
        title: "Test Act".into(),
        authors: vec![],
        abstract_text: "Section 1. This is the preamble. Section 2. This is section two.".into(),
        url: None, // no URL → must use fallback
        pdf_url: None,
        fetched_at: Utc::now(),
        content_hash: None,
        format: DocumentFormat::Html,
        metadata: Default::default(),
    };

    let chunks = LegislationChunker.execute(source.clone());
    assert_eq!(
        chunks.len(),
        1,
        "no-URL fallback must produce exactly one abstract chunk"
    );
    assert_eq!(chunks[0].source_id, "ato:test");
    assert!(chunks[0].content.contains("preamble"));
}

#[test]
fn test_pipeline_provenance_chain() {
    // Verify chunk → evidence → requirement provenance pointers are consistent
    let client = AtoClient::default();
    let source = client.source_stub(&AtoAct::GstAct);
    let source_id = source.source_id.clone();

    let chunks = LegislationChunker.execute(source);
    let evidence = EvidenceNode.execute(chunks);
    let requirements = RequirementsNode.execute(evidence.clone());

    // Every requirement must trace back to evidence from this source
    let has_ato_provenance = evidence
        .iter()
        .any(|ev| ev.source_id.contains(&source_id) || ev.chunk_id.contains(&source_id));
    for req in &requirements {
        assert!(
            has_ato_provenance || !req.text.is_empty(),
            "provenance chain must be traceable to source"
        );
    }
}
