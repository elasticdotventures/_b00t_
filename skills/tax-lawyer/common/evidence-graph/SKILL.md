---
name: tax-lawyer-evidence-graph
description: |
  Evidence graph construction for tax audit trails. Each Satisfies check emits
  EvidenceNode structs with Blake3 content hashes. Evidence chains are
  tamper-evident and link legislative citations to factual findings.
version: 1.0.0
tags: [evidence, blake3, audit, hash, chain, arc-kit-au, provenance]
tier: ch0nky
complexity: 6
applies_to:
  - "evidence graph"
  - "audit trail"
  - "Blake3"
  - "evidence chain"
  - "provenance"
  - "tamper-evident"
output_types: [.json, .rs]
depends_on: [tax-lawyer-ufo-types]
unlocks: []
metadata:
  ufo_stereotype: Perdurant
  legislation: []
  iso_types: []
---

## Evidence Graph Pattern

Every tax compliance check MUST produce an evidence graph — a directed acyclic
graph (DAG) of `EvidenceNode` structs linked by `EvidenceEdge` provenance arcs.

### EvidenceNode

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EvidenceNode {
    pub id: Uuid,                        // stable identifier
    pub evidence_type: EvidenceType,     // Claim | Statistic | Observation
    pub body: String,                    // human-readable finding
    pub citation: String,                // "ITAA 1997 s 355-25(1)(a)"
    pub hash: String,                    // Blake3(body || citation) as hex
    pub timestamp: chrono::DateTime<Utc>,
    pub verdict: Verdict,                // Pass | Fail | Inconclusive
}

pub enum EvidenceType {
    Claim,        // normative assertion derived from legislation
    Statistic,    // quantitative measurement (dollar amounts, dates, counts)
    Observation,  // factual record (existence of documents, entity state)
}

pub enum Verdict {
    Pass,
    Fail,
    Inconclusive,
}
```

### Blake3 Hashing

```rust
use blake3::Hasher;

pub fn hash_evidence(body: &str, citation: &str) -> String {
    let mut hasher = Hasher::new();
    hasher.update(body.as_bytes());
    hasher.update(b"||");
    hasher.update(citation.as_bytes());
    hasher.finalize().to_hex().to_string()
}
```

Blake3 is chosen over SHA-256:
- 3–5× faster on modern hardware
- SIMD-parallel tree hashing
- Same 256-bit security margin

### EvidenceEdge (provenance arc)

```rust
pub struct EvidenceEdge {
    pub from_id: Uuid,   // prerequisite node
    pub to_id: Uuid,     // dependent node
    pub edge_type: EvidgeEdgeType,
}

pub enum EvidgeEdgeType {
    Supports,     // node A supports the conclusion of node B
    Contradicts,  // node A contradicts node B
    Derives,      // node B is derived from node A (computation)
    Cites,        // node A cites node B as legislative authority
}
```

### Evidence Chain Construction Pattern

```rust
pub fn build_au_rd_evidence(activity: &AuRdActivity) -> Vec<EvidenceNode> {
    let mut nodes = vec![];

    // 1. Existence check (Observation)
    let obs = EvidenceNode {
        id: Uuid::new_v4(),
        evidence_type: EvidenceType::Observation,
        body: format!("AuRdActivity '{}' exists with LEI {}", activity.activity_name, activity.lei),
        citation: "ITAA 1997 s 355-25",
        hash: hash_evidence(&body, &citation),
        timestamp: Utc::now(),
        verdict: Verdict::Pass,
    };
    nodes.push(obs);

    // 2. Core activity test (Claim)
    let claim = EvidenceNode {
        evidence_type: EvidenceType::Claim,
        body: format!("Activity domain {:?} is eligible under experimental activities test", activity.domain),
        citation: "ITAA 1997 s 355-25(1)(a) — experimental activities in a field of science/technology",
        // ...
    };
    nodes.push(claim);

    nodes
}
```

## arc-kit-au Integration

`arc-kit-au` is b00t's internal evidence arc library (future). Until shipped:
- Store evidence nodes in-memory as `Vec<EvidenceNode>`
- Serialize to JSONL for audit log: `~/.b00t/tax-audit-<date>.jsonl`
- Each line = one EvidenceNode JSON

### Output JSONL format

```jsonl
{"id":"uuid","evidence_type":"Claim","body":"...","citation":"ITAA 1997 s 355-25","hash":"...","verdict":"Pass"}
{"id":"uuid","evidence_type":"Statistic","body":"Expenditure AUD 250,000 for FY2025","citation":"ITAA 1997 s 355-45","hash":"...","verdict":"Pass"}
```

## Audit Trail Requirements

For ATO R&D offset claims:
- Evidence must exist for **each** DoD criterion (355-25, 355-30, 355-35, 355-45)
- Expenditure evidence must reference source documents (invoices, timesheets)
- All hashes must be reproducible from the original body+citation strings

For IRS R&D credit (Form 6765):
- Evidence per QRE category (wages, supplies, contract research)
- 4-part test satisfaction must be documented per activity (not just aggregate)

# b00t:map v1
# summary: Blake3 evidence graph construction for ATO/IRS audit trails
# tags: evidence, blake3, audit, hash, provenance, arc-kit-au
# tier: ch0nky
# complexity: 6
