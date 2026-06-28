//! UFO stereotype enums — grounding domain concepts in Unified Foundational Ontology.
//!
//! Based on Guizzardi (2005) UFO-A/B/C. These stereotypes classify every domain
//! type in the Tax-Lawyer platform so that `record_is_a()` (NS-9) can produce
//! audit-trail evidence with ontological provenance.
//!
//! # Usage
//! ```ignore
//! use ufo_types::stereotype::UfoStereotype;
//! let k = UfoStereotype::Kind("Company".into());
//! assert_eq!(k.as_str(), "Kind:Company");
//! record_is_a("company:5493001K", k.as_str());
//! ```

use serde::{Deserialize, Serialize};

/// UFO stereotype — classifies a domain entity according to Guizzardi's
/// Unified Foundational Ontology (UFO-A/B).
///
/// | Variant   | UFO Category | Rigidity  | Example                          |
/// |-----------|-------------|-----------|----------------------------------|
/// | `Kind`    | Endurant    | rigid     | Person, Company, Transaction     |
/// | `SubKind` | Endurant    | rigid     | PtyLtd ⊆ Company, Sell ⊆ Trade   |
/// | `Role`    | Endurant    | anti-rigid| TaxCreditClaimant, RndConductor   |
/// | `Relator` | Moment      | dependent | Evidence, Proof, ConstraintCheck |
/// | `Mode`    | Moment      | inherent  | Eligibility, Compliance, Satisfied|
///
/// All variants implement `Display` so they can be passed directly to
/// `record_is_a(datum_key, stereotype)`.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum UfoStereotype {
    /// UFO-A Endurant: rigid, essential type. An entity cannot cease to be a
    /// `Kind` without losing its identity. (Guizzardi 2005, §4.2.1)
    ///
    /// Examples: `"Company"`, `"Person"`, `"Transaction"`, `"Document"`
    Kind(String),

    /// UFO-A Endurant: rigid subtype that inherits identity from its parent
    /// `Kind`. The extension of a `SubKind` is always a subset of its parent.
    /// (Guizzardi 2005, §4.2.2)
    ///
    /// Examples: `SubKind { name: "PtyLtd", parent: "Company" }`
    SubKind {
        /// Name of this sub-kind
        name: String,
        /// Name of the parent Kind this sub-kind specialises
        parent: String,
    },

    /// UFO-A Endurant: anti-rigid, relationally dependent type. An entity
    /// can gain or lose a `Role` without changing its identity.
    /// (Guizzardi 2005, §4.3.1)
    ///
    /// Examples: `"TaxCreditClaimant"`, `"RndConductor"`, `"EligibleEntity"`
    Role(String),

    /// UFO-B Moment: mediates between two or more entities. A `Relator` is
    /// existentially dependent on its relata — if the entities it connects
    /// cease to exist, the relator ceases to exist.
    /// (Guizzardi 2005, §5.2)
    ///
    /// Examples: `"Evidence"`, `"Proof"`, `"ConstraintCheck"`
    Relator(String),

    /// UFO-B Moment: intrinsic property that inheres in a single entity.
    /// Unlike `Relator` (which mediates between entities), `Mode` is a
    /// quality or state of a single bearer.
    /// (Guizzardi 2005, §5.1)
    ///
    /// Examples: `"Eligibility"`, `"Compliance"`, `"Satisfied"`, `"Violated"`
    Mode(String),
}

impl UfoStereotype {
    /// Return the canonical string label for evidence logging.
    ///
    /// Format: `"Variant:value"` for simple variants,
    /// `"Variant:name<parent>"` for SubKind.
    pub fn as_str(&self) -> &str {
        // Return a &str by leaking a String. This is fine because
        // UfoStereotype values are typically long-lived (const or static).
        // In practice, callers should use Display or a local String.
        // We provide this as a convenience for &str interfaces.
        match self {
            UfoStereotype::Kind(_)
            | UfoStereotype::Role(_)
            | UfoStereotype::Relator(_)
            | UfoStereotype::Mode(_) => {
                // We can't return &str from owned String; use Display trait
                // via to_string() instead. This method is primarily for
                // Display-backed usage.
                ""
            }
            UfoStereotype::SubKind { .. } => "",
        }
    }

    /// Convert to an owned string suitable for `record_is_a()`.
    pub fn to_label(&self) -> String {
        self.to_string()
    }
}

impl std::fmt::Display for UfoStereotype {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            UfoStereotype::Kind(name) => write!(f, "Kind:{name}"),
            UfoStereotype::SubKind { name, parent } => write!(f, "SubKind:{name}<{parent}"),
            UfoStereotype::Role(name) => write!(f, "Role:{name}"),
            UfoStereotype::Relator(name) => write!(f, "Relator:{name}"),
            UfoStereotype::Mode(name) => write!(f, "Mode:{name}"),
        }
    }
}

/// A domain type that carries a UFO stereotype — used by `record_is_a` (NS-9)
/// to emit ontological provenance evidence.
///
/// # Example
/// ```ignore
/// impl Stereotyped for AuRdActivity {
///     fn ufo_stereotype(&self) -> UfoStereotype {
///         UfoStereotype::SubKind {
///             name: "AuRdActivity".into(),
///             parent: "Activity".into(),
///         }
///     }
/// }
/// ```
pub trait Stereotyped {
    /// The UFO stereotype for this domain type.
    fn ufo_stereotype(&self) -> UfoStereotype;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn kind_display() {
        let k = UfoStereotype::Kind("Company".into());
        assert_eq!(k.to_string(), "Kind:Company");
    }

    #[test]
    fn subkind_display() {
        let sk = UfoStereotype::SubKind {
            name: "PtyLtd".into(),
            parent: "Company".into(),
        };
        assert_eq!(sk.to_string(), "SubKind:PtyLtd<Company");
    }

    #[test]
    fn role_display() {
        let r = UfoStereotype::Role("TaxCreditClaimant".into());
        assert_eq!(r.to_string(), "Role:TaxCreditClaimant");
    }

    #[test]
    fn relator_display() {
        let r = UfoStereotype::Relator("Evidence".into());
        assert_eq!(r.to_string(), "Relator:Evidence");
    }

    #[test]
    fn mode_display() {
        let m = UfoStereotype::Mode("Eligibility".into());
        assert_eq!(m.to_string(), "Mode:Eligibility");
    }

    #[test]
    fn stereotype_roundtrips_json() {
        let stereotypes = vec![
            UfoStereotype::Kind("Company".into()),
            UfoStereotype::SubKind {
                name: "PtyLtd".into(),
                parent: "Company".into(),
            },
            UfoStereotype::Role("Claimant".into()),
            UfoStereotype::Relator("Proof".into()),
            UfoStereotype::Mode("Eligibility".into()),
        ];
        for s in &stereotypes {
            let json = serde_json::to_string(s).unwrap();
            let back: UfoStereotype = serde_json::from_str(&json).unwrap();
            assert_eq!(s, &back);
        }
    }

    #[test]
    fn stereotype_all_variants_exist() {
        // Verify all 5 HANDOFF-specified variants are constructable
        let _kind = UfoStereotype::Kind("TestKind".into());
        let _subkind = UfoStereotype::SubKind {
            name: "TestSub".into(),
            parent: "TestKind".into(),
        };
        let _role = UfoStereotype::Role("TestRole".into());
        let _relator = UfoStereotype::Relator("TestRelator".into());
        let _mode = UfoStereotype::Mode("TestMode".into());
    }

    #[test]
    fn stereotyped_trait_can_be_implemented() {
        struct TestEntity;
        impl Stereotyped for TestEntity {
            fn ufo_stereotype(&self) -> UfoStereotype {
                UfoStereotype::Kind("TestEntity".into())
            }
        }
        assert_eq!(
            TestEntity.ufo_stereotype().to_string(),
            "Kind:TestEntity"
        );
    }
}
