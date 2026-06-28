//! # ufo-types — UFO-grounded domain types for the Tax-Lawyer platform
//!
//! This crate provides the ontological foundation for the Tax-Lawyer
//! (#510 EPIC) by defining:
//!
//! - **UFO stereotypes** (`stereotype`): `UfoStereotype` enum grounding
//!   every domain type in Guizzardi's Unified Foundational Ontology.
//! - **Satisfies<C> trait** (`satisfies`): The core constraint evaluation
//!   pattern — every domain type implements `Satisfies<Constraint>` with
//!   deterministic, audit-ready results.
//! - **ISO standard wrappers** (`iso`): `Lei` (ISO 17442), `Iso4217`
//!   currency codes, and `Ifrs9Classification` financial instrument types.
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
//! - ISO 17442:2012 — Legal Entity Identifier (LEI)
//! - ISO 4217:2015 — Codes for the representation of currencies
//! - IFRS 9 — Financial Instruments (IASB, 2014)
//! - ITAA 1997 Division 355 — AU R&D Tax Incentive
//! - IRC Sec 41 — US R&D Tax Credit
//! - IRS Rev. Proc. 2024-28 — Crypto cost basis safe harbor
//! - ATO QC 53725 — AU crypto CGT treatment

pub mod iso;
pub mod satisfies;
pub mod stereotype;

// Re-export key types for convenience
pub use iso::{Ifrs9Classification, Iso4217, Iso4217Error, Lei, LeiError};
pub use satisfies::{
    Disposition, EvidenceBridge, IsoAuditable, NodeId, Satisfies, SatisfiesResult,
};
pub use stereotype::{Stereotyped, UfoStereotype};
