# Requirements: ATO Legislation Ingestion Pipeline
# Feature #1 from ideation (Score 95/100)
# Grounded in: STRATEGY.md (Australian Tax Capability)

## Overview

Ingest Australian tax legislation (ITAA 1936, ITAA 1997, GST Act) and ATO rulings into b00t's structured knowledge base using the existing document evidence pipeline.

## Functional Requirements

### REQ-LEG-001: ATO Document Fetch
**Priority**: P1 | **Type**: Functional | **Stereotype**: functionalRequirement
The system SHALL fetch ATO legislation documents from the Australian Government Federal Register of Legislation API (https://www.legislation.gov.au) in PDF and HTML formats.
- Derived from: STRATEGY.md "Track 1: Legislation ingestion"
- Acceptance: Successfully fetch ITAA 1997 (current compilation) within 30s

### REQ-LEG-002: Semantic Chunking
**Priority**: P1 | **Type**: Functional | **Stereotype**: functionalRequirement
The system SHALL chunk legislation documents into semantic sections (Parts, Divisions, Sections) using section header detection, preserving the hierarchical structure.
- Derived from: b00t doc_pipeline SemanticChunk type
- Acceptance: ITAA 1997 produces ≥500 chunks with correct section headers

### REQ-LEG-003: Evidence Extraction
**Priority**: P1 | **Type**: Functional | **Stereotype**: functionalRequirement
The system SHALL extract evidence items (claims, definitions, constraints) from each chunk with provenance pointers back to the specific section and subsection of the legislation.
- Derived from: b00t doc_pipeline Evidence + ProvenancePointer types
- Acceptance: Each evidence item references exact section (e.g., "ITAA97 s8-1")

### REQ-LEG-004: ATO Ruling Integration
**Priority**: P2 | **Type**: Functional | **Stereotype**: functionalRequirement
The system SHALL ingest ATO Public Rulings (TR, TD, MT series) and link them to the legislation sections they interpret.
- Derived from: STRATEGY.md "interpret ATO rulings"
- Acceptance: TR 2024 series rulings link to relevant ITAA sections

### REQ-LEG-005: Update Detection
**Priority**: P2 | **Type**: Functional | **Stereotype**: functionalRequirement
The system SHALL detect when legislation has been updated (new compilation published) and trigger re-ingestion.
- Derived from: STRATEGY.md "adapt to legislative changes"
- Acceptance: Detects new ITAA compilation within 24h of publication

## Non-Functional Requirements

### REQ-LEG-NF01: Accuracy
**Priority**: P1 | **Type**: NonFunctional | **Stereotype**: performanceRequirement
Evidence extraction accuracy SHALL exceed 95% when verified against manual legal review of 100 randomly selected sections.

### REQ-LEG-NF02: Provenance Completeness
**Priority**: P1 | **Type**: NonFunctional | **Stereotype**: performanceRequirement
100% of extracted evidence items SHALL have complete provenance pointers (source_id, section_number, line_range).

### REQ-LEG-NF03: Performance
**Priority**: P2 | **Type**: NonFunctional | **Stereotype**: performanceRequirement
Full pipeline (fetch → chunk → evidence) for ITAA 1997 SHALL complete within 5 minutes on standard hardware.

## Constraints

- Must use existing b00t pipeline infrastructure (doc_pipeline.rs, pipeline_nodes.rs)
- Must store evidence in NoSQL-compatible JSON format (FullPipelineResult serialization)
- Must comply with Australian copyright law (legislation is Crown copyright, permitted use)
- ATO API rate limits must be respected (1 req/3s as per arxiv-like pattern)

## Dependencies

- b00t doc_pipeline types (DocumentSource, SemanticChunk, Evidence, Requirement) — ✅ available
- b00t pipeline_nodes (FetchNode, ChunkNode, EvidenceNode) — ✅ available
- ATO legislation API access — needs investigation
- OPENAI_API_KEY for embedding generation — ✅ configured
