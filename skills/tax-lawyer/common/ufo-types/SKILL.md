---
name: tax-lawyer-ufo-types
description: |
  UFO (Unified Foundational Ontology) stereotypes and the Satisfies<C> trait
  pattern. Foundation for all Tax-Lawyer Platform domain objects. Every domain
  struct MUST declare its UfoCategory and implement Satisfies<ConstraintType>.
version: 1.0.0
tags: [ufo, ontology, satisfies, types, endurant, perdurant, moment, iso]
tier: frontier
complexity: 7
applies_to:
  - "UFO stereotype"
  - "Satisfies trait"
  - "ontology foundation"
  - "EndurantKind"
  - "PerdurantProcess"
  - "MomentRelator"
output_types: [.rs, .json]
depends_on: []
unlocks:
  - tax-lawyer-au-rd-tax-incentive
  - tax-lawyer-au-crypto
  - tax-lawyer-us-r-and-d-credit
  - tax-lawyer-us-crypto
metadata:
  ufo_stereotype: Abstract
  legislation: []
  iso_types: ["ISO 4217", "ISO 17442 (LEI)", "IFRS 9"]
---

## UFO — Unified Foundational Ontology

Reference: Guizzardi, G. (2005). _Ontological Foundations for Structural Conceptual Models_.
UFO is a formal top-level ontology for grounding domain models.

### Top-level Categories

| Category | Rust Enum Variant | Meaning | Tax-Lawyer Example |
|---|---|---|---|
| `Endurant` | `UfoCategory::Endurant` | Things that persist through time; wholly present at each moment | `AuRdActivity`, `QreActivity`, `CryptoWallet` |
| `Perdurant` | `UfoCategory::Perdurant` | Processes/events that unfold over time; only partially present at each moment | `AuRdExpenditure`, `CryptoTx` |
| `Moment` | `UfoCategory::Moment` | Qualities/relations that depend on their bearer for existence | `AuRdOffset`, `CryptoGain` |
| `Abstract` | `UfoCategory::Abstract` | Mathematical/logical objects; no spatiotemporal location | `AuRdEligibility`, `UsRdcFourPartTest` |

### Endurant Stereotypes

```rust
pub enum EndurantStereotype {
    Kind,       // natural kind — what things fundamentally ARE (e.g., AuRdActivity)
    SubKind,    // subtype of a Kind (e.g., CoreRdActivity vs SupportingActivity)
    Role,       // relational property — what something IS IN CONTEXT (e.g., Claimant)
    Phase,      // intrinsic property — contingent classification (e.g., Registered, Approved)
    Category,   // rigid, cross-sortal (e.g., TaxableEntity — applies to companies+individuals)
    Mixin,      // non-rigid, cross-sortal (e.g., Auditable)
    RoleMixin,  // relational, cross-sortal (e.g., TaxLiable)
}
```

### Perdurant Stereotypes

```rust
pub enum PerdurantStereotype {
    Process,    // homeomerous — every temporal part is same type (e.g., R&D program)
    State,      // maximal steady period within a process
    Event,      // atomic temporal occurrence (e.g., CryptoTx)
    Scenario,   // complex event with multiple participants
}
```

### Moment Stereotypes

```rust
pub enum MomentStereotype {
    Mode,       // intrinsic quality of one individual (e.g., CryptoGainAmount)
    Relator,    // quality that depends on multiple individuals (e.g., TaxObligation)
}
```

## The Satisfies<C> Trait Pattern

```rust
pub trait Satisfies<C> {
    type Evidence;
    type Error;

    fn satisfies(&self, constraint: &C) -> Result<Vec<Self::Evidence>, Self::Error>;
}
```

### Constraint implementation pattern:

```rust
// Domain struct with UFO annotation
pub struct AuRdActivity {
    pub ufo: UfoCategory,  // = Endurant
    pub ufo_kind: EndurantStereotype,  // = Kind
    pub lei: String,       // ISO 17442 LEI of the registrant
    pub activity_id: Uuid,
    pub activity_name: String,
    pub start_year: u32,
    pub end_year: u32,
    pub domain: RdDomain,
}

// Constraint struct (Abstract in UFO)
pub struct AuRdEligibility;

impl Satisfies<AuRdEligibility> for AuRdActivity {
    type Evidence = EvidenceNode;
    type Error = anyhow::Error;

    fn satisfies(&self, _c: &AuRdEligibility) -> Result<Vec<EvidenceNode>, anyhow::Error> {
        // ... check ITAA 1997 s 355-25 criteria
        // emit EvidenceNode per check
    }
}
```

## ISO Standard Types Used

| Standard | Identifier | Use in Platform |
|---|---|---|
| ISO 17442 | LEI (Legal Entity Identifier) | Identifies the claimant legal entity (20-char alphanumeric) |
| ISO 4217 | Currency codes | AUD, USD in tax calculations |
| ISO 8601 | Date/time | Fiscal year boundaries, transaction timestamps |
| IFRS 9 | Financial instruments | CryptoTx classification (asset vs liability) |

### LEI validation (Rust pattern)
```rust
// LEI: 18 alphanumeric + 2 check digits (ISO 17442)
fn validate_lei(lei: &str) -> bool {
    lei.len() == 20 && lei.chars().all(|c| c.is_alphanumeric())
}
```

## EvidenceNode Structure

```rust
pub struct EvidenceNode {
    pub evidence_type: EvidenceType,  // Claim | Statistic | Observation
    pub body: String,
    pub citation: String,             // e.g. "ITAA 1997 s 355-25(1)(a)"
    pub hash: String,                 // Blake3 hex of (body + citation)
}
```

Blake3 hashing ensures tamper-evidence: any change to body or citation
invalidates the audit trail entry.

# b00t:map v1
# summary: UFO stereotypes + Satisfies trait — ontological foundation for Tax-Lawyer Platform
# tags: ufo, ontology, satisfies, endurant, perdurant, moment, lei, iso
# tier: frontier
# complexity: 7
