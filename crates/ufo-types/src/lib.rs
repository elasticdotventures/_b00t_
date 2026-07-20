//! # ufo-types — UFO-grounded domain types for the b00t ecosystem
//!
//! This crate provides a domain-generic ontological foundation — grounded in
//! Guizzardi's Unified Foundational Ontology — for any b00t-ecosystem project
//! that wants ontologically-grounded types with deterministic, audit-ready
//! constraint evaluation. It defines:
//!
//! - **UFO stereotypes** (`stereotype`): `UfoStereotype` enum grounding
//!   every domain type in Guizzardi's Unified Foundational Ontology.
//!   Domain-generic — nothing here is specific to any one consumer.
//! - **Satisfies<C> trait** (`satisfies`): The core constraint evaluation
//!   pattern — any domain type can implement `Satisfies<Constraint>` with
//!   deterministic, audit-ready results. Domain-generic.
//! - **Capability types** (`capability`): `Task`, `Attempt`, `ActionRecord`,
//!   `Episode`, `ReviewVerdict`, `Solution`, `TrainingCorpus`,
//!   `EnergyBudget`, etc. — generic agent-capability/OODA types.
//!   Domain-generic.
//! - **DARED proposal types** (`dare`): `Decision`, `Alternative`, `Risk`,
//!   `ExecutiveDecision`, `OodaStateMachine` — a generic OODA state-change
//!   proposal framework codified as Rust generics. Domain-generic.
//! - **ISO standard wrappers** (`iso`): `Lei` (ISO 17442 Legal Entity
//!   Identifier), `Iso4217` currency codes, and `Ifrs9Classification`
//!   financial-instrument types. These ARE domain-specific — they encode
//!   financial/legal-entity accounting standards and are only meaningful to
//!   consumers working in that space (e.g. Tax-Lawyer). Not intended as a
//!   generic building block for unrelated domains.
//!
//! Any b00t-ecosystem project needing UFO-grounded domain
//! types and the `Satisfies<T>` pattern (e.g. `stereotype`, `satisfies`,
//! `capability`, `dare`) can depend on it directly. Only `iso` carries
//! genuinely finance/tax-domain-specific types.
//!
//! ## Architecture
//!
//! ```text
//! ┌──────────┐     ┌───────────────┐     ┌────────────────┐
//! │ ufo-types │────→│  ledger-core  │────→│  MCP actions   │
//! │ (traits)  │     │ (domain impls)│     │ (thin wrappers)│
//! └──────────┘     └───────────────┘     └────────────────┘
//! ```
//!
//! (The above pipeline is Tax-Lawyer's own consumption path; other
//! consumers wire `ufo-types` into their own domain-impl layer instead of
//! `ledger-core`.)
//!
//! ## Integration with evidence layer (NS-9, NS-10)
//!
//! The `Stereotyped` trait (from `stereotype`) and `IsoAuditable` trait
//! (from `satisfies`) bridge domain types to `evidence.rs`:
//!
//! - **NS-9** `record_is_a(subject, ufo_stereotype)` — uses `Stereotyped::ufo_stereotype()`
//! - **NS-10** `record_audited_by(subject, iso_standard)` — uses `IsoAuditable::iso_standard_ids()`
//!
//! The `EvidenceBridge::evaluate()` method on `SatisfiesResult` produces
//! all labels needed for both calls in one step.
//!
//! ## References
//!
//! - Guizzardi, G. (2005). _Ontological Foundations for Structural
//!   Conceptual Models_. PhD Thesis, University of Twente.
//! - ISO 17442:2012 — Legal Entity Identifier (LEI) (`iso` module only)
//! - ISO 4217:2015 — Codes for the representation of currencies (`iso` module only)
//! - IFRS 9 — Financial Instruments (IASB, 2014) (`iso` module only)
//!
//! The following references are specific to the Tax-Lawyer consumer and its
//! use of the domain-generic types above — they are not properties of this
//! crate itself:
//! - ITAA 1997 Division 355 — AU R&D Tax Incentive
//! - IRC Sec 41 — US R&D Tax Credit
//! - IRS Rev. Proc. 2024-28 — Crypto cost basis safe harbor
//! - ATO QC 53725 — AU crypto CGT treatment

pub mod capability;
pub mod dare;
pub mod iso;
pub mod satisfies;
pub mod stereotype;

// Re-export key types for convenience
pub use capability::{
    ActionRecord, AgentCapability, Attempt, AttemptStatus, CapabilityDomain, CarmackSolution,
    EnergyBudget, Episode, History, ReviewVerdict, ReviewerType, Solution, StateObservation, Task,
    TaskStatus, TrainingCorpus,
};
pub use dare::{
    Alternative, DaredAcceptanceCriteria, DaredDocument, DaredProposal, DaredValidationError,
    Decision, ExecutiveDecision, OodaEvent, OodaGuards, OodaPhase, OodaStateMachine,
    OodaStateMachineError, OodaTransition, Risk, RiskSeverity,
};
pub use iso::{Ifrs9Classification, Iso4217, Iso4217Error, Lei, LeiError};
pub use satisfies::{
    Disposition, EvidenceBridge, IsoAuditable, NodeId, Satisfies, SatisfiesResult,
};
pub use stereotype::{Stereotyped, UfoStereotype};
