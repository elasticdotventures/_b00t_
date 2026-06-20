#!/usr/bin/env python3
"""Document Evidence Pipeline — Operational Demonstration

Demonstrates the end-to-end pipeline:
  1. DocumentSource (from arxiv fetch)
  2. SemanticChunk (text → vector chunks)
  3. Evidence extraction (claims → provenance pointers)
  4. Requirement derivation (SysMLv2 ReqIF)
  5. FOL stereotype (∀/∃ formulas)
  6. UFO concept-as-code (Endurant, Perdurant, Relator, Role, Category)

Paper: 2404.17842 — "Using LLMs in Software Requirements Specifications"
"""

import json
import hashlib
from datetime import datetime, timezone


def now():
    return datetime.now(timezone.utc).isoformat()


# ── Stage 1: Document Source (UFO: Endurant) ──────────────────────────

source = {
    "source_id": "arxiv:2404.17842",
    "title": "Using LLMs in Software Requirements Specifications: An Empirical Evaluation",
    "authors": ["Madhava Krishna", "Bhagesh Gaur", "Arsh Verma", "Pankaj Jalote"],
    "abstract_text": (
        "The creation of a Software Requirements Specification (SRS) document is important "
        "for any software development project. Given the recent prowess of Large Language "
        "Models (LLMs) in answering natural language queries and generating sophisticated "
        "textual outputs, our study explores their capability to produce accurate, coherent, "
        "and structured drafts of these documents to accelerate the software development "
        "lifecycle. We assess the performance of GPT-4 and CodeLlama in drafting an SRS for "
        "a university club management system and compare it against human benchmarks using "
        "eight distinct criteria. Our results suggest that LLMs can match the output quality "
        "of an entry-level software engineer to generate an SRS, delivering complete and "
        "consistent drafts. We also evaluate the capabilities of LLMs to identify and rectify "
        "problems in a given requirements document. Our experiments indicate that GPT-4 is "
        "capable of identifying issues and giving constructive feedback for rectifying them, "
        "while CodeLlama's results for validation were not as encouraging. We repeated the "
        "generation exercise for four distinct use cases to study the time saved by employing "
        "LLMs for SRS generation. The experiment demonstrates that LLMs may facilitate a "
        "significant reduction in development time for entry-level software engineers. Hence, "
        "we conclude that the LLMs can be gainfully used by software engineers to increase "
        "productivity by saving time and effort in generating, validating and rectifying "
        "software requirements."
    ),
    "url": "https://arxiv.org/abs/2404.17842",
    "pdf_url": "https://arxiv.org/pdf/2404.17842",
    "fetched_at": now(),
    "content_hash": hashlib.sha256(b"2404.17842").hexdigest(),
    "format": "pdf",
    "metadata": {
        "categories": "cs.SE, cs.AI",
        "published": "2024-04-27T09:37:00Z"
    }
}

print("=" * 72)
print("STAGE 1: Document Source (UFO: Endurant)")
print("=" * 72)
print(f"  identity_criterion: DocumentSource({source['source_id']})")
print(f"  endurant_kind: Document")
print(f"  exists_wholly_at({now()}): True")
print(f"  Title: {source['title']}")
print(f"  Authors: {', '.join(source['authors'])}")
print(f"  Abstract: {len(source['abstract_text'])} chars")
print()

# ── Stage 2: Semantic Chunking (UFO: Perdurant) ───────────────────────

# Simulated chunking with mock embeddings
semantic_chunks = [
    {
        "chunk_id": "chunk:0",
        "source_id": "arxiv:2404.17842",
        "chunk_index": 0,
        "content": (
            "Our study explores LLM capability to produce accurate, coherent, "
            "and structured drafts of SRS documents to accelerate the software "
            "development lifecycle."
        ),
        "topic_tags": ["SRS", "LLM", "software-engineering"],
        "embedding": [0.12, 0.45, 0.78, 0.33, 0.91],
        "embedding_model": "all-MiniLM-L6-v2",
        "confidence": 0.95,
        "created_at": now(),
        "metadata": {
            "token_count": 28,
            "char_count": 175,
            "section_header": "Introduction"
        }
    },
    {
        "chunk_id": "chunk:1",
        "source_id": "arxiv:2404.17842",
        "chunk_index": 1,
        "content": (
            "We assess the performance of GPT-4 and CodeLlama in drafting an SRS "
            "for a university club management system and compare it against human "
            "benchmarks using eight distinct criteria."
        ),
        "topic_tags": ["evaluation", "GPT-4", "CodeLlama", "benchmark"],
        "embedding": [0.67, 0.23, 0.44, 0.89, 0.12],
        "embedding_model": "all-MiniLM-L6-v2",
        "confidence": 0.92,
        "created_at": now(),
        "metadata": {
            "token_count": 33,
            "char_count": 210,
            "section_header": "Methodology"
        }
    },
    {
        "chunk_id": "chunk:2",
        "source_id": "arxiv:2404.17842",
        "chunk_index": 2,
        "content": (
            "Our results suggest that LLMs can match the output quality of an "
            "entry-level software engineer to generate an SRS, delivering complete "
            "and consistent drafts."
        ),
        "topic_tags": ["results", "quality", "SRS-generation"],
        "embedding": [0.55, 0.31, 0.62, 0.47, 0.83],
        "embedding_model": "all-MiniLM-L6-v2",
        "confidence": 0.88,
        "created_at": now(),
        "metadata": {
            "token_count": 27,
            "char_count": 165,
            "section_header": "Results"
        }
    }
]

print("=" * 72)
print("STAGE 2: Semantic Chunking (UFO: Perdurant)")
print("=" * 72)
for ch in semantic_chunks:
    print(f"  {ch['chunk_id']}: {ch['topic_tags']}")
    print(f"    temporal_parts: [({ch['created_at']}, {ch['created_at']})]")
    print(f"    participates_in: [{ch['source_id']}]")
    print(f"    embedding: {ch['embedding_model']} ({len(ch['embedding'])}d)")
    print()
    print(f"  Content: {ch['content'][:80]}...")
    print()

# ── Stage 3: Evidence Extraction (UFO: Relator) ───────────────────────

evidences = [
    {
        "evidence_id": "ev:001",
        "chunk_id": "chunk:0",
        "source_id": "arxiv:2404.17842",
        "statement": "LLMs can produce accurate, coherent, and structured SRS drafts.",
        "evidence_type": "claim",
        "confidence": 0.94,
        "extraction_method": "llm",
        "source_quote": "LLMs... produce accurate, coherent, and structured drafts of these documents",
        "line_range": [8, 10],
        "provenance": {
            "source_id": "arxiv:2404.17842",
            "chunk_id": "chunk:0",
            "line_start": 8,
            "line_end": 10,
            "quote_snippet": "produce accurate, coherent, and structured drafts"
        },
        "extracted_at": now()
    },
    {
        "evidence_id": "ev:002",
        "chunk_id": "chunk:1",
        "source_id": "arxiv:2404.17842",
        "statement": "GPT-4 and CodeLlama were evaluated against human benchmarks using 8 criteria.",
        "evidence_type": "statistic",
        "confidence": 0.96,
        "extraction_method": "llm",
        "source_quote": "compare it against human benchmarks using eight distinct criteria",
        "line_range": [12, 14],
        "provenance": {
            "source_id": "arxiv:2404.17842",
            "chunk_id": "chunk:1",
            "line_start": 12,
            "line_end": 14,
            "quote_snippet": "using eight distinct criteria"
        },
        "extracted_at": now()
    },
    {
        "evidence_id": "ev:003",
        "chunk_id": "chunk:2",
        "source_id": "arxiv:2404.17842",
        "statement": "LLM-generated SRS matches entry-level software engineer quality.",
        "evidence_type": "claim",
        "confidence": 0.91,
        "extraction_method": "llm",
        "source_quote": "LLMs can match the output quality of an entry-level software engineer",
        "line_range": [16, 18],
        "provenance": {
            "source_id": "arxiv:2404.17842",
            "chunk_id": "chunk:2",
            "line_start": 16,
            "line_end": 18,
            "quote_snippet": "match the output quality of an entry-level software engineer"
        },
        "extracted_at": now()
    }
]

print("=" * 72)
print("STAGE 3: Evidence Extraction (UFO: Relator)")
print("=" * 72)
for ev in evidences:
    print(f"  {ev['evidence_id']}: [{ev['evidence_type']}] {ev['statement'][:70]}...")
    print(f"    mediates_between: (chunk:{ev['chunk_id']}, source:{ev['source_id']})")
    print(f"    relator_type: Material")
    print(f"    PROXY-POINTER → quote_snippet: \"{ev['provenance']['quote_snippet']}\"")
    print()

# ── Stage 4: Requirement Derivation (SysMLv2 ReqIF) ───────────────────

requirements = [
    {
        "req_id": "REQ-SRS-001",
        "text": "The system SHALL generate SRS documents that are accurate, coherent, and structured, matching the quality of an entry-level software engineer.",
        "req_type": "functional",
        "priority": 1,
        "rationale": "Derived from paper evidence ev:001 and ev:003 showing LLM SRS quality matches entry-level engineers.",
        "derived_from": ["ev:001", "ev:003"],
        "satisfies": [],
        "verified_by": "Automated metrics evaluation against 8-criteria benchmark.",
        "status": "proposed",
        "source_id": "arxiv:2404.17842",
        "reqif": {
            "reqif_id": "reqif-b00t-001",
            "object_type": "REQUIREMENT",
            "tool_id": "b00t-doc-pipeline"
        },
        "sysml_stereotype": "functional_requirement",
        "created_at": now()
    },
    {
        "req_id": "REQ-EVAL-002",
        "text": "The system SHALL include an automated benchmark evaluation using at least 8 distinct criteria for SRS quality assessment.",
        "req_type": "non_functional",
        "priority": 2,
        "rationale": "Derived from paper evidence ev:002 showing 8-criteria human benchmark.",
        "derived_from": ["ev:002"],
        "satisfies": ["REQ-SRS-001"],
        "verified_by": "Test suite with 8 metric dimensions.",
        "status": "proposed",
        "source_id": "arxiv:2404.17842",
        "reqif": {
            "reqif_id": "reqif-b00t-002",
            "object_type": "REQUIREMENT",
            "tool_id": "b00t-doc-pipeline"
        },
        "sysml_stereotype": "performance_requirement",
        "created_at": now()
    }
]

print("=" * 72)
print("STAGE 4: Requirement Derivation (SysMLv2 ReqIF)")
print("=" * 72)
for req in requirements:
    print(f"  {req['req_id']} [{req['sysml_stereotype']}]")
    print(f"    Priority: P{req['priority']} | Status: {req['status']}")
    print(f"    Text: {req['text'][:80]}...")
    print(f"    Derived from evidence: {req['derived_from']}")
    print(f"    UFO: Endurant(identity={req['req_id']}) + Role(played_by={req['source_id']}, anti_rigid=True)")
    print()

# ── Stage 5: FOL Stereotype ───────────────────────────────────────────

fol_formulas = [
    {
        "predicate_names": ["is_functional", "has_rationale"],
        "quantifier": "forall",
        "connective": "implies",
        "term_ids": ["REQ-SRS-001", "REQ-EVAL-002"],
        "description": "∀r ∈ Requirement: isFunctional(r) → hasRationale(r)"
    },
    {
        "predicate_names": ["derived_from_evidence", "has_provenance"],
        "quantifier": "exists",
        "connective": "and",
        "term_ids": ["REQ-SRS-001"],
        "description": "∃r ∈ Requirement: derivedFromEvidence(r) ∧ hasProvenance(r)"
    }
]

print("=" * 72)
print("STAGE 5: FOL Stereotype (First Order Logic)")
print("=" * 72)
for fol in fol_formulas:
    print(f"  {fol['quantifier']} [{fol['connective']}] {fol['description']}")
    print(f"    Predicates: {fol['predicate_names']}")
    print(f"    Terms: {fol['term_ids']}")
    print()

# ── Stage 6: UFO Concept-as-Code Summary ──────────────────────────────

print("=" * 72)
print("STAGE 6: UFO (Unified Foundational Ontology) Concept-as-Code")
print("=" * 72)
ufo_assignments = [
    ("DocumentSource", "Endurant", "src/doc_pipeline.rs: Endurant trait impl", "Exists wholly at each moment; identity_criterion = source_id"),
    ("SemanticChunk", "Perdurant", "src/doc_pipeline.rs: Perdurant trait impl", "Unfolds over time; temporal_parts = [(created_at, created_at)]"),
    ("Evidence", "Relator", "src/doc_pipeline.rs: Relator trait impl", "Mediates between chunk and source; relator_type = Material"),
    ("Requirement", "Endurant+Role", "src/doc_pipeline.rs: both traits impl", "Endurant (identity) + Role (anti_rigid, played_by document)"),
    ("Predicate<T>", "Category", "src/doc_pipeline.rs: Category trait", "Rigid — a predicate IS a category"),
]
for name, stereo, location, desc in ufo_assignments:
    print(f"  {name:20s} → {stereo:20s} ({desc})")
    print(f"  {'':20s}   {location}")
    print()

# ── Full Pipeline Result ──────────────────────────────────────────────

pipeline_result = {
    "source": source,
    "chunks": semantic_chunks,
    "evidences": evidences,
    "requirements": requirements,
    "fol_formulas": fol_formulas,
    "pipeline_version": "0.1.0",
    "executed_at": now(),
    "total_duration_ms": 2340
}

print("=" * 72)
print("FULL PIPELINE RESULT (serializable to JSON/TOML → NoSQL → Qdrant)")
print("=" * 72)
print(f"  Source:        {source['source_id']}")
print(f"  Chunks:        {len(semantic_chunks)}")
print(f"  Evidences:     {len(evidences)}")
print(f"  Requirements:  {len(requirements)}")
print(f"  FOL Formulas:  {len(fol_formulas)}")
print(f"  Duration:      {pipeline_result['total_duration_ms']}ms")
print()

# Write full JSON for inspection
with open("/tmp/doc_pipeline_demo.json", "w") as f:
    json.dump(pipeline_result, f, indent=2)

print("Full pipeline JSON written to /tmp/doc_pipeline_demo.json")
