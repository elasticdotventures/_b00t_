# Role Taxonomy Unification Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Merge `b00t-cli::agentic_role` (the ZST/`KnownRole` system, real runtime resolution via `_B00T_ROLE`) and `b00t-c0re-hierarchy::Role` (plain enum, `Team`/`Agent` roster storage, no runtime resolution) into one new crate, `b00t-c0re-role`, resolving the documented #905/#909 `Operator` naming collision and retiring `Bouncer`/`Mate`/`Player` as role variants.

**Architecture:** Extract `agentic_role.rs`'s `AgenticRole`/`AgenticCrew` traits, `RoleRef<T>`, and `KnownRole` verbatim into a new workspace crate `b00t-c0re-role` (neither `b00t-cli` nor `b00t-c0re-hierarchy` can depend on the other without a cycle, so a new shared crate is structurally required). Rename the `AppProvider` ZST to `Specialist`; `KnownRole::resolve`'s fallback for any unrecognized role name changes from bucketing as `Worker` to bucketing as `Specialist`, preserving the given name for datum lookup — `Worker` becomes a single fixed generalized name, `Specialist` becomes the open-ended stereotype family (`"appprovider"`, `"rust-specialist"`, etc. all resolve into it). `b00t-c0re-hierarchy::Role` is deleted outright; `Agent`/`Team`/`CrewMeta` switch to `KnownRole`.

**Tech Stack:** Rust 2024 edition, Cargo workspace, serde, serde_yaml (LinkML coherence tests).

## Global Constraints

- Canonical role names (lowercase, matching `_B00T_ROLE` env var / role-datum filenames): `worker`, `executive`, `operator`, `specialist`. `provider`/`appprovider` is retired as a sealed name — it becomes an ordinary specialist stereotype string, no longer matched by `KnownRole::from_str`.
- `_b00t_/linkml/schema/hive_role_vocabulary.yaml` is the declared source of truth for these names (enforced by a coherence test) — it must be updated in lockstep with the Rust enum, not after.
- No PascalCase (`"Captain"`, `"Bouncer"`, etc.) backward-compat deserialization is being added. A repo-wide search found no persisted data using `Role`'s old PascalCase serialization (`grep`/`gh search code` for literal `"role": "Captain"`-shaped JSON — zero hits outside this codebase's own type definitions and unrelated prose). If real legacy data is later discovered, that is a separate, explicit migration task — not silently built here.
- Every crate/file touched must still compile and its existing tests (adapted, never silently dropped without a stated reason) must pass. `cargo build --workspace` and `cargo test -p b00t-c0re-role -p b00t-c0re-hierarchy -p b00t-cli` are the final gate (Task 9).
- `_b00t_/linkml/schema/hive_role_vocabulary_rust` is a LinkML-codegen'd crate but is **not** a Cargo workspace member (confirmed: absent from root `Cargo.toml`'s `members`, has its own `Cargo.lock`) — it will not be caught by `cargo build --workspace`. It is hand-updated in Task 8 to stay consistent with the YAML, flagged explicitly since no codegen command was found in this repo to regenerate it automatically.

---

### Task 1: Scaffold the `b00t-c0re-role` crate (empty, compiling)

**Files:**
- Create: `b00t-c0re-role/Cargo.toml`
- Create: `b00t-c0re-role/src/lib.rs` (stub)
- Modify: `Cargo.toml:19` (root workspace `members` list — insert alphabetically near `b00t-c0re-hierarchy`)

**Interfaces:**
- Consumes: nothing (first task).
- Produces: an empty, compiling workspace member named `b00t-c0re-role` that Task 2 fills in, and that Tasks 5/6 add as a dependency.

- [ ] **Step 1: Create the crate directory and Cargo.toml**

```toml
# b00t-c0re-role/Cargo.toml
[package]
name = "b00t-c0re-role"
version.workspace = true
edition = "2024"

[dependencies]
serde = { version = "1.0", features = ["derive"] }

[dev-dependencies]
serde_yaml = "0.9"
```

- [ ] **Step 2: Create a stub lib.rs**

```rust
// b00t-c0re-role/src/lib.rs
```

- [ ] **Step 3: Add the crate to the workspace members list**

In `Cargo.toml`, find the `members = [` list (starts at line 2) and insert `"b00t-c0re-role",` immediately after the existing `"b00t-c0re-hierarchy",` entry (line 19).

- [ ] **Step 4: Verify it builds**

Run: `cargo build -p b00t-c0re-role`
Expected: builds successfully (empty crate).

- [ ] **Step 5: Commit**

```bash
git add Cargo.toml b00t-c0re-role/
git commit -m "feat(b00t-c0re-role): scaffold new crate for unified role taxonomy"
```

---

### Task 2: Move and adapt the ZST role system into `b00t-c0re-role`

**Files:**
- Create: `b00t-c0re-role/src/lib.rs` (full content, replacing Task 1's stub)
- Test: same file, `#[cfg(test)] mod tests` at the bottom

**Interfaces:**
- Consumes: nothing new.
- Produces: `pub trait AgenticRole`, `pub trait AgenticCrew`, `pub struct RoleRef<T>`, ZST types `Worker`/`Executive`/`Operator`/`Specialist`, `pub struct HiveCrew`, `pub enum KnownRole { Worker(RoleRef<Worker>), Executive(RoleRef<Executive>), Operator(RoleRef<Operator>), Specialist(RoleRef<Specialist>) }` (now also `PartialEq`, `Serialize`, `Deserialize`), `pub fn resolve_role(Option<String>) -> KnownRole`, convenience constructors `KnownRole::worker()`, `KnownRole::executive()`, `KnownRole::operator()`, `KnownRole::specialist()`, `KnownRole::specialist_named(&str)`. Tasks 5, 6, 7 depend on all of these exact names.

- [ ] **Step 1: Write the full lib.rs**

This is `b00t-cli/src/agentic_role.rs` moved verbatim, with: `AppProvider` renamed to `Specialist` (`NAME` changes from `"provider"` to `"specialist"`), `KnownRole::resolve`'s fallback changed to bucket under `Specialist` instead of `Worker`, `PartialEq`/`Serialize`/`Deserialize` added to `KnownRole`, convenience constructors added, and the `Operator` struct's "NOT the same role as..." collision doc comment removed (the collision this documented is resolved by this crate's existence).

```rust
//! AgenticRole trait + AgenticCrew — type-level role invariants.
//!
//! Each role is a ZST implementing `AgenticRole`, carrying its semantic
//! position within an `AgenticCrew`.  `RoleRef<T>` wraps a string value
//! at the type level — the concrete role type T is the invariant lock,
//! guaranteeing the string is a known role at compile time.
//!
//! Crew relationships are encoded as associated types: each role knows
//! its parent, peers, and delegated sub-roles through the crew graph.
//!
//! Unifies what were previously two parallel role systems (this one, and
//! `b00t-c0re-hierarchy::Role`) per #905/#909 "no parallel vocabularies" —
//! see `_b00t_/linkml/schema/hive_role_vocabulary.yaml` for the canonical
//! name list this crate is checked against.

use serde::{Deserialize, Serialize};
use std::borrow::Cow;
use std::marker::PhantomData;

// ── Module-level sealed trait pattern ────────────────────────────────────────

mod sealed {
    /// Sealed trait — only impls inside this module.
    pub trait Sealed {}
}

/// A semantic role within an `AgenticCrew`.
///
/// Implementations are zero-sized types (ZSTs) that carry role metadata
/// as `const` / `fn` items.  The type itself is the invariant: if a
/// function returns `RoleRef<Worker>`, callers know statically the role
/// is "worker" within the `HiveCrew`.
pub trait AgenticRole: sealed::Sealed + Sized + 'static {
    /// Canonical name — matches role datum filenames (lowercase).
    const NAME: &'static str;

    /// Human-readable hint for the role.
    fn hint() -> &'static str;

    /// The crew this role belongs to.
    type Crew: AgenticCrew;

    /// Peer roles within the same crew (siblings in the role graph).
    fn peers() -> Vec<&'static str>;
}

/// A crew is a collection of roles with defined semantic relationships.
pub trait AgenticCrew: sealed::Sealed + Sized + 'static {
    const NAME: &'static str;
    fn known_roles() -> Vec<&'static str>;
}

// ── Zero-cost role reference ─────────────────────────────────────────────────

/// Trait-locked invariant: wraps a `Cow<'static, str>` at the type level with
/// a phantom `AgenticRole` marker.  The type `T` guarantees the string
/// is a valid role at construction time.
///
/// Uses `Cow::Borrowed` for known roles (zero allocation) and
/// `Cow::Owned` for env-var overrides / unknown role names.
///
/// # Invariants
/// - Constructed only via `RoleRef::new(name)` which validates against
///   `T::NAME` or via `RoleRef::from_env()` / `RoleRef::new_owned()`.
/// - The inner string is guaranteed to be a known role for type `T`.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct RoleRef<T: AgenticRole> {
    inner: Cow<'static, str>,
    _marker: PhantomData<T>,
}

impl<T: AgenticRole> RoleRef<T> {
    /// Create from a verified `&'static str` role name.  Returns `None` if
    /// the name does not match `T::NAME` — this is a compile-time hint
    /// that the caller may have the wrong role type.
    ///
    /// Uses `Cow::Borrowed` — zero allocation for known static strings.
    pub fn new(name: &'static str) -> Option<Self> {
        if name == T::NAME {
            Some(Self {
                inner: Cow::Borrowed(name),
                _marker: PhantomData,
            })
        } else {
            None
        }
    }

    /// Create from an owned string (env override, unknown role name).
    /// Allocates once into `Cow::Owned`.
    pub fn new_owned(name: String) -> Self {
        Self {
            inner: Cow::Owned(name.to_lowercase()),
            _marker: PhantomData,
        }
    }

    /// Read from `_B00T_ROLE` env var, falling back to `T::NAME`.
    pub fn from_env() -> Self {
        let name = std::env::var("_B00T_ROLE")
            .ok()
            .filter(|r| !r.is_empty())
            .unwrap_or_else(|| T::NAME.to_string());
        Self::new_owned(name)
    }
}

impl<T: AgenticRole> std::ops::Deref for RoleRef<T> {
    type Target = str;
    fn deref(&self) -> &str {
        self.inner.as_ref()
    }
}

impl<T: AgenticRole> AsRef<str> for RoleRef<T> {
    fn as_ref(&self) -> &str {
        self.inner.as_ref()
    }
}

impl<T: AgenticRole> PartialEq<&str> for RoleRef<T> {
    fn eq(&self, other: &&str) -> bool {
        self.inner.as_ref() == *other
    }
}

impl<T: AgenticRole> From<RoleRef<T>> for String {
    fn from(r: RoleRef<T>) -> String {
        r.inner.into_owned()
    }
}

// ── Default role ─────────────────────────────────────────────────────────────

/// The default role when nothing is overridden.
#[derive(Debug, Clone)]
pub struct Worker;
/// Executive decision authority.
#[derive(Debug, Clone)]
pub struct Executive;
/// Crew dispatch, recruitment, and specialist routing — spins typed crews
/// via k0mmand3r; also administrative privileges (scouts/finds agents,
/// enlists, executes training plans). Previously two separate role systems'
/// idea of "operator" — unified here as one meaning (#905/#909).
#[derive(Debug, Clone)]
pub struct Operator;
/// Specialist — an open-ended stereotype family for domain-specific work.
/// `Specialist::NAME` ("specialist") is the one sealed/bare name; any other
/// role name (e.g. "appprovider", "rust-specialist", "security-auditor")
/// also resolves into this variant via `KnownRole::resolve`'s fallback,
/// with the specific name preserved for role-datum lookup. `Worker` stays
/// a single generalized name with no sub-stereotyping — this asymmetry is
/// deliberate, not an oversight.
#[derive(Debug, Clone)]
pub struct Specialist;

// ── Crew definitions ─────────────────────────────────────────────────────────

/// The primary hive crew — roles that operate within the b00t hive.
pub struct HiveCrew;

impl sealed::Sealed for HiveCrew {}
impl AgenticCrew for HiveCrew {
    const NAME: &'static str = "hive";
    fn known_roles() -> Vec<&'static str> {
        vec!["worker", "executive", "operator", "specialist"]
    }
}

// ── Sealed + AgenticRole impls ───────────────────────────────────────────────

impl sealed::Sealed for Worker {}
impl AgenticRole for Worker {
    const NAME: &'static str = "worker";
    fn hint() -> &'static str {
        "Default hive worker — general-purpose executor with governance safety gates"
    }
    type Crew = HiveCrew;
    fn peers() -> Vec<&'static str> {
        vec!["executive", "operator", "specialist"]
    }
}

impl sealed::Sealed for Executive {}
impl AgenticRole for Executive {
    const NAME: &'static str = "executive";
    fn hint() -> &'static str {
        "Hive-level decision authority — release gate, tier routing, resource management"
    }
    type Crew = HiveCrew;
    fn peers() -> Vec<&'static str> {
        vec!["operator", "specialist", "worker"]
    }
}

impl sealed::Sealed for Operator {}
impl AgenticRole for Operator {
    const NAME: &'static str = "operator";
    fn hint() -> &'static str {
        "Crew dispatch, recruitment, and specialist routing — spins typed crews via k0mmand3r"
    }
    type Crew = HiveCrew;
    fn peers() -> Vec<&'static str> {
        vec!["executive", "specialist", "worker"]
    }
}

impl sealed::Sealed for Specialist {}
impl AgenticRole for Specialist {
    const NAME: &'static str = "specialist";
    fn hint() -> &'static str {
        "Specialist — domain-specific work; open-ended stereotype family (e.g. appprovider, rust-specialist)"
    }
    type Crew = HiveCrew;
    fn peers() -> Vec<&'static str> {
        vec!["executive", "operator", "worker"]
    }
}

// ── KnownRole — sealed ADT ───────────────────────────────────────────────────

/// A sealed ADT over all known `HiveCrew` role variants.
///
/// Each variant wraps a `RoleRef<T>` so the concrete role type is
/// preserved within the enum.  The type-level invariant is maintained:
/// a `KnownRole::Executive` guarantees the inner string is `"executive"`.
#[derive(Debug, Clone, PartialEq)]
pub enum KnownRole {
    Worker(RoleRef<Worker>),
    Executive(RoleRef<Executive>),
    Operator(RoleRef<Operator>),
    Specialist(RoleRef<Specialist>),
}

impl KnownRole {
    pub fn name(&self) -> &str {
        match self {
            KnownRole::Worker(r) => r.as_ref(),
            KnownRole::Executive(r) => r.as_ref(),
            KnownRole::Operator(r) => r.as_ref(),
            KnownRole::Specialist(r) => r.as_ref(),
        }
    }

    pub fn from_str(name: &str) -> Option<KnownRole> {
        match name {
            n if n == Worker::NAME => Some(KnownRole::Worker(RoleRef::new(Worker::NAME).unwrap())),
            n if n == Executive::NAME => {
                Some(KnownRole::Executive(RoleRef::new(Executive::NAME).unwrap()))
            }
            n if n == Operator::NAME => {
                Some(KnownRole::Operator(RoleRef::new(Operator::NAME).unwrap()))
            }
            n if n == Specialist::NAME => Some(KnownRole::Specialist(
                RoleRef::new(Specialist::NAME).unwrap(),
            )),
            _ => None,
        }
    }

    pub fn resolve(override_role: Option<String>) -> KnownRole {
        let name = override_role
            .filter(|r| !r.trim().is_empty())
            .or_else(|| std::env::var("_B00T_ROLE").ok())
            .map(|r| r.to_lowercase())
            .unwrap_or_else(|| Worker::NAME.to_string());

        KnownRole::from_str(&name).unwrap_or_else(|| {
            // Unknown/overridden name -> Specialist. Worker is reserved for
            // the exact literal "worker"; anything else is a specialist
            // stereotype, name preserved for datum lookup.
            KnownRole::Specialist(RoleRef::new_owned(name))
        })
    }

    /// Bare `worker` role.
    pub fn worker() -> Self {
        KnownRole::Worker(RoleRef::new(Worker::NAME).unwrap())
    }

    /// Bare `executive` role.
    pub fn executive() -> Self {
        KnownRole::Executive(RoleRef::new(Executive::NAME).unwrap())
    }

    /// Bare `operator` role.
    pub fn operator() -> Self {
        KnownRole::Operator(RoleRef::new(Operator::NAME).unwrap())
    }

    /// Bare `specialist` role (no specific stereotype).
    pub fn specialist() -> Self {
        KnownRole::Specialist(RoleRef::new(Specialist::NAME).unwrap())
    }

    /// A specialist role with a specific stereotype name (e.g.
    /// "appprovider", "rust-specialist"), preserved verbatim.
    pub fn specialist_named(name: &str) -> Self {
        KnownRole::Specialist(RoleRef::new_owned(name.to_string()))
    }
}

impl PartialEq<&str> for KnownRole {
    fn eq(&self, other: &&str) -> bool {
        self.name() == *other
    }
}

impl std::fmt::Display for KnownRole {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.name())
    }
}

impl Serialize for KnownRole {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(self.name())
    }
}

impl<'de> Deserialize<'de> for KnownRole {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let name = String::deserialize(deserializer)?;
        Ok(KnownRole::from_str(&name)
            .unwrap_or_else(|| KnownRole::Specialist(RoleRef::new_owned(name))))
    }
}

// ── Dynamic dispatch — role-agnostic helpers ─────────────────────────────────

/// Resolve a role override + env to a `KnownRole`.
///
/// Defaults to `Worker` (the hive default).  Returns a `KnownRole` sealed
/// ADT; callers can pattern-match or call `.name()` for display/datum
/// lookup.
pub fn resolve_role(role_override: Option<String>) -> KnownRole {
    KnownRole::resolve(role_override)
}

/// Create a `RoleRef<Worker>` from a role name (for callers that
/// don't care about the concrete type — they just need the string).
pub fn worker_role(name: &str) -> RoleRef<Worker> {
    if name == Worker::NAME {
        RoleRef::new(Worker::NAME).unwrap()
    } else {
        RoleRef::new_owned(name.to_string())
    }
}

/// Format a crew relationship diagram for display.
pub fn format_crew_hierarchy() -> String {
    let mut out = String::from("HiveCrew:\n");
    for role in HiveCrew::known_roles() {
        out.push_str(&format!("  └─→ {role}\n"));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_worker_role_default() {
        let role = resolve_role(None);
        assert_eq!(role.name(), "worker");
        assert_eq!(Worker::NAME, "worker");
        assert_eq!(
            Worker::hint(),
            "Default hive worker — general-purpose executor with governance safety gates"
        );
    }

    #[test]
    fn test_role_override_honoured() {
        let role = resolve_role(Some("executive".to_string()));
        assert_eq!(role.name(), "executive");
        assert_eq!(role, "executive");
    }

    #[test]
    fn test_role_new_valid() {
        let r = RoleRef::<Worker>::new("worker");
        assert!(r.is_some());
        let r = RoleRef::<Worker>::new("executive");
        assert!(r.is_none(), "Worker::new should reject 'executive'");
    }

    #[test]
    fn test_crew_known_roles() {
        let roles = HiveCrew::known_roles();
        assert!(roles.contains(&"worker"));
        assert!(roles.contains(&"executive"));
        assert!(roles.contains(&"operator"));
        assert!(roles.contains(&"specialist"));
        assert_eq!(HiveCrew::NAME, "hive");
    }

    #[test]
    fn test_role_hints_differ() {
        assert_ne!(Worker::hint(), Executive::hint());
        assert_ne!(Operator::hint(), Specialist::hint());
    }

    #[test]
    fn test_peer_relationships() {
        let worker_peers = Worker::peers();
        assert!(worker_peers.contains(&"executive"));
        let exec_peers = Executive::peers();
        assert!(exec_peers.contains(&"worker"));
    }

    #[test]
    fn test_role_ref_deref_to_str() {
        let r = RoleRef::<Worker>::from_env();
        let s: &str = &r;
        assert_eq!(s, "worker");
    }

    #[test]
    fn test_role_ref_deref_inline() {
        let r = RoleRef::<Worker>::from_env();
        assert_eq!(&*r, "worker");
    }

    #[test]
    fn test_role_ref_as_ref() {
        let r: RoleRef<Worker> = worker_role("operator");
        assert_eq!(r.as_ref(), "operator");
    }

    #[test]
    fn test_role_ref_into_string() {
        let r = RoleRef::<Worker>::from_env();
        let s: String = r.into();
        assert_eq!(s, "worker");
    }

    #[test]
    fn test_crew_hierarchy_format() {
        let hierarchy = format_crew_hierarchy();
        assert!(hierarchy.contains("HiveCrew"));
        assert!(hierarchy.contains("worker"));
    }

    #[test]
    fn test_role_from_env_not_set() {
        let r = RoleRef::<Worker>::from_env();
        assert_eq!(r.as_ref(), "worker");
    }

    /// "provider" is no longer a sealed/known name (Specialist::NAME is
    /// "specialist") — it now falls into the Specialist fallback bucket
    /// like any other stereotype, name preserved.
    #[test]
    fn test_provider_is_a_specialist_stereotype_not_a_sealed_role() {
        let r = resolve_role(Some("provider".to_string()));
        assert_eq!(r.name(), "provider");
        assert_eq!(r, "provider");
        assert!(matches!(r, KnownRole::Specialist(_)));
    }

    #[test]
    fn test_known_role_from_str() {
        assert!(KnownRole::from_str("worker").is_some());
        assert!(KnownRole::from_str("executive").is_some());
        assert!(KnownRole::from_str("operator").is_some());
        assert!(KnownRole::from_str("specialist").is_some());
        assert!(KnownRole::from_str("provider").is_none());
        assert!(KnownRole::from_str("captain").is_none());
    }

    #[test]
    fn test_specialist_role_definition() {
        assert_eq!(Specialist::NAME, "specialist");
        assert!(Specialist::peers().contains(&"executive"));
    }

    #[test]
    fn test_known_role_display() {
        assert_eq!(format!("{}", resolve_role(None)), "worker");
        assert_eq!(
            format!("{}", resolve_role(Some("executive".to_string()))),
            "executive"
        );
    }

    /// Regression for whoami --role=<custom> loading the wrong AGENTS/--role=*.md
    /// file: any role name outside the 4 sealed HiveCrew variants (worker,
    /// executive, operator, specialist) — e.g. "reviewer", "podman",
    /// "ux-designer" — falls into KnownRole::resolve's fallback branch,
    /// which wraps it as KnownRole::Specialist(RoleRef::new_owned(name)). The
    /// actual name IS preserved inside that RoleRef; .name() must read it,
    /// not return a hardcoded constant.
    #[test]
    fn test_custom_role_name_is_preserved_in_specialist_bucket() {
        let role = resolve_role(Some("reviewer".to_string()));
        assert_eq!(role.name(), "reviewer");
        assert_eq!(role, "reviewer");
        assert_eq!(format!("{}", role), "reviewer");
        assert!(matches!(role, KnownRole::Specialist(_)));
    }

    #[test]
    fn test_unknown_role_name_resolves_to_specialist_bucket() {
        let role = resolve_role(Some("rust-specialist".to_string()));
        assert!(matches!(role, KnownRole::Specialist(_)));
        assert_eq!(role.name(), "rust-specialist");
    }

    #[test]
    fn test_known_role_serde_round_trip() {
        for role in [
            KnownRole::worker(),
            KnownRole::executive(),
            KnownRole::operator(),
            KnownRole::specialist(),
            KnownRole::specialist_named("rust-specialist"),
        ] {
            let json = serde_json::to_string(&role).unwrap();
            let back: KnownRole = serde_json::from_str(&json).unwrap();
            assert_eq!(role, back);
        }
    }

    /// Phase 2 (ScopeStore+LinkML epic, #905/#909 "no parallel vocabularies")
    /// coherence check: _b00t_/linkml/schema/hive_role_vocabulary.yaml is
    /// declared the source of truth for HiveCrew's role vocabulary. This
    /// reads the actual schema file (not a hand-copied duplicate of its
    /// contents, which would just reintroduce the drift problem this
    /// schema exists to prevent) and asserts its `enums.HiveRole`
    /// permissible values are exactly `HiveCrew::known_roles()` -- so a
    /// future edit to either side that silently drifts from the other
    /// fails CI instead of accumulating as undetected fragmentation.
    #[test]
    fn hive_crew_roles_match_linkml_schema_source_of_truth() {
        let schema_yaml = include_str!("../../_b00t_/linkml/schema/hive_role_vocabulary.yaml");
        let schema: serde_yaml::Value =
            serde_yaml::from_str(schema_yaml).expect("hive_role_vocabulary.yaml must parse as YAML");

        let permissible_values = schema
            .get("enums")
            .and_then(|e| e.get("HiveRole"))
            .and_then(|r| r.get("permissible_values"))
            .and_then(|pv| pv.as_mapping())
            .expect("schema must have enums.HiveRole.permissible_values as a mapping");

        let mut schema_roles: Vec<String> = permissible_values
            .keys()
            .map(|k| k.as_str().expect("permissible_value keys must be strings").to_string())
            .collect();
        schema_roles.sort();

        let mut rust_roles: Vec<String> = HiveCrew::known_roles().iter().map(|s| s.to_string()).collect();
        rust_roles.sort();

        assert_eq!(
            schema_roles, rust_roles,
            "HiveCrew::known_roles() has drifted from _b00t_/linkml/schema/hive_role_vocabulary.yaml \
             (the declared source of truth) -- update whichever side is stale"
        );
    }

    /// Each individual role's NAME constant must also appear in the schema
    /// (not just the aggregate known_roles() list) -- catches the case
    /// where known_roles() and a role's own NAME const independently drift
    /// from each other, which the aggregate-only check above wouldn't.
    #[test]
    fn each_agentic_role_name_const_is_in_the_schema() {
        let schema_yaml = include_str!("../../_b00t_/linkml/schema/hive_role_vocabulary.yaml");
        let schema: serde_yaml::Value =
            serde_yaml::from_str(schema_yaml).expect("hive_role_vocabulary.yaml must parse as YAML");
        let permissible_values = schema
            .get("enums")
            .and_then(|e| e.get("HiveRole"))
            .and_then(|r| r.get("permissible_values"))
            .and_then(|pv| pv.as_mapping())
            .expect("schema must have enums.HiveRole.permissible_values as a mapping");

        for name in [Worker::NAME, Executive::NAME, Operator::NAME, Specialist::NAME] {
            assert!(
                permissible_values.contains_key(name),
                "{name:?} is a real AgenticRole::NAME but missing from \
                 hive_role_vocabulary.yaml's permissible_values"
            );
        }
    }
}
```

Note: `test_known_role_serde_round_trip` needs `serde_json` — add it alongside `serde_yaml` in `[dev-dependencies]` (Task 1's Cargo.toml): `serde_json = "1.0"`.

- [ ] **Step 2: Update Task 1's Cargo.toml dev-dependencies**

```toml
[dev-dependencies]
serde_yaml = "0.9"
serde_json = "1.0"
```

- [ ] **Step 3: Run the crate's tests**

Run: `cargo test -p b00t-c0re-role`
Expected: all tests pass, including the two LinkML coherence tests — which will only pass once Task 3 (below) updates the YAML. If running this step before Task 3, expect `hive_crew_roles_match_linkml_schema_source_of_truth` and `each_agentic_role_name_const_is_in_the_schema` to fail with a drift message naming `"provider"` vs `"specialist"` — that failure is expected at this point and confirms the coherence check itself works; proceed to Task 3 to fix it.

- [ ] **Step 4: Commit**

```bash
git add b00t-c0re-role/
git commit -m "feat(b00t-c0re-role): unified ZST role system (Worker/Executive/Operator/Specialist)"
```

---

### Task 3: Update the LinkML schema (source of truth)

**Files:**
- Modify: `_b00t_/linkml/schema/hive_role_vocabulary.yaml`

**Interfaces:**
- Consumes: nothing.
- Produces: the YAML Task 2's two coherence tests read via `include_str!`.

- [ ] **Step 1: Replace the file content**

```yaml
id: https://promptexecution.com/b00t/linkml/hive-role-vocabulary
name: hive-role-vocabulary
description: >-
  Canonical HiveCrew role identifiers — source of truth for
  b00t-c0re-role's Worker/Executive/Operator/Specialist ZST role system
  (moved here from b00t-cli/src/agentic_role.rs). Phase 2 ("harmonization")
  of the ScopeStore+LinkML epic — _b00t_ issues #905/#909's "no parallel
  vocabularies" goal is now resolved: b00t-c0re-hierarchy::Role and
  agentic_role.rs's ZST system have been merged into this one taxonomy,
  consumed by both b00t-cli and b00t-c0re-hierarchy.

  RESOLVED COLLISION: the "operator" value below previously named two
  different things — agentic_role.rs's "crew dispatch and specialist
  routing" and b00t-c0re-hierarchy::Role::Operator's "recruitment +
  training". Both are now the same role, one meaning: administrative
  authority that recruits, trains, and dispatches other agents.

  "provider"/"appprovider" is no longer a sealed role name. It survives
  only as a common stereotype string within "specialist" (see that
  value's description) — Specialist is an open-ended stereotype family,
  Worker stays a single generalized name with no sub-stereotyping.

prefixes:
  linkml: https://w3id.org/linkml/
  b00t: https://promptexecution.com/b00t/

default_range: string
default_prefix: b00t

imports:
  - linkml:types

enums:
  HiveRole:
    description: >-
      Canonical role identifiers within HiveCrew (b00t-c0re-role, the
      single unified role taxonomy for both b00t-cli and
      b00t-c0re-hierarchy). This schema is the source of truth going
      forward — AgenticRole::NAME constants must match these permissible
      value names exactly, checked by a coherence test in
      b00t-c0re-role's own test module.
    permissible_values:
      worker:
        description: >-
          Default hive worker — general-purpose executor with
          governance safety gates. A single generalized name, no
          sub-stereotyping (contrast with specialist, below).
      executive:
        description: >-
          Hive-level decision authority — release gate, tier routing,
          resource management.
      operator:
        description: >-
          Crew dispatch, recruitment, and specialist routing — spins
          typed crews via k0mmand3r; administrative privileges (recruits,
          trains, enlists agents). Previously two separate roles across
          two parallel role systems; unified as one (#905/#909).
      specialist:
        description: >-
          Domain-specific work — an open-ended stereotype family, not a
          single fixed meaning. The bare "specialist" name covers
          unspecified domain work; specific stereotypes (e.g.
          "appprovider" for MCP surface/desktop control-plane/service
          hosting, "rust-specialist", "security-auditor") resolve into
          this same role type with their specific name preserved for
          role-datum lookup, rather than each needing its own sealed
          enum variant.
```

- [ ] **Step 2: Verify Task 2's coherence tests now pass**

Run: `cargo test -p b00t-c0re-role`
Expected: `hive_crew_roles_match_linkml_schema_source_of_truth` and `each_agentic_role_name_const_is_in_the_schema` both pass. Full crate test suite passes.

- [ ] **Step 3: Commit**

```bash
git add _b00t_/linkml/schema/hive_role_vocabulary.yaml
git commit -m "docs(linkml): hive_role_vocabulary reflects unified role taxonomy"
```

---

### Task 4: Delete `b00t-cli/src/agentic_role.rs`, wire `b00t-c0re-role` into `b00t-cli`

**Files:**
- Delete: `b00t-cli/src/agentic_role.rs`
- Modify: `b00t-cli/src/lib.rs:40` (remove `pub mod agentic_role;`)
- Modify: `b00t-cli/Cargo.toml` (add `b00t-c0re-role` dependency)
- Modify: `b00t-cli/src/whoami.rs:1` (import from the new crate)

**Interfaces:**
- Consumes: `b00t_c0re_role::{resolve_role, KnownRole}` from Task 2.
- Produces: `b00t-cli` no longer has its own role system; `whoami.rs::resolve_role` now returns the shared `KnownRole`.

- [ ] **Step 1: Delete the old module file**

```bash
git rm b00t-cli/src/agentic_role.rs
```

- [ ] **Step 2: Remove the module declaration**

In `b00t-cli/src/lib.rs`, delete line 40: `pub mod agentic_role;`

- [ ] **Step 3: Add the dependency**

In `b00t-cli/Cargo.toml`, near the existing `b00t-c0re-hierarchy = { path = "../b00t-c0re-hierarchy" }` line, add:

```toml
b00t-c0re-role = { path = "../b00t-c0re-role" }
```

- [ ] **Step 4: Update whoami.rs's import**

In `b00t-cli/src/whoami.rs`, change line 1 from:

```rust
use crate::agentic_role::resolve_role;
```

to:

```rust
use b00t_c0re_role::resolve_role;
```

- [ ] **Step 5: Verify whoami.rs's own role tests still pass**

`whoami.rs` has its own tests (`test_resolve_role_prefers_override`, `test_resolve_role_empty_override_falls_back`, `test_resolve_role_defaults_to_worker`, `test_resolve_role_uses_env_var`, around lines 1742-1790) that call `resolve_role` — these need no logic changes, only the import above. Run: `cargo test -p b00t-cli whoami::tests` (adjust the test-path filter to match this crate's actual module test path if `cargo test` reports a different one) and confirm all four pass.

- [ ] **Step 6: Verify the crate builds (it will not fully build yet — Task 5 also touches b00t-cli)**

Run: `cargo build -p b00t-cli 2>&1 | head -50`
Expected: errors only from `crew_handler.rs` and `wow.rs` (still referencing the old `Role`/path) — confirms nothing else in `b00t-cli` broke. Proceed to Task 5.

- [ ] **Step 7: Commit**

```bash
git add -A b00t-cli/
git commit -m "refactor(b00t-cli): use b00t-c0re-role instead of local agentic_role module"
```

---

### Task 5: Update `b00t-c0re-hierarchy` — delete `Role`, switch to `KnownRole`

**Files:**
- Modify: `b00t-c0re-hierarchy/Cargo.toml` (add `b00t-c0re-role` dependency)
- Modify: `b00t-c0re-hierarchy/src/roles.rs` (delete `Role` enum, `Agent`/`Team` field changes)

**Interfaces:**
- Consumes: `b00t_c0re_role::KnownRole` from Task 2.
- Produces: `pub struct Agent { pub role: KnownRole, ... }`, `pub struct Team { pub executive_id: String, pub worker_ids: Vec<String>, pub operator_ids: Vec<String>, pub specialist_ids: Vec<String>, pub player_ids: Vec<String> }` (no `captain_id`/`executor_ids`/`bouncer_ids`), `Team::{new, add_worker, remove_worker, add_specialist, remove_specialist, add_operator, remove_operator, add_player, remove_player}` (no `add_bouncer`/`remove_bouncer`/`add_executor`/`remove_executor`). Tasks 6, 7, 8 depend on these exact names.

- [ ] **Step 1: Add the dependency**

In `b00t-c0re-hierarchy/Cargo.toml`, add alongside the existing `b00t-c0re-gov`/`b00t-council` deps:

```toml
b00t-c0re-role = { path = "../b00t-c0re-role" }
```

- [ ] **Step 2: Rewrite roles.rs's Role/Agent/Team section**

Replace lines 1-152 of `b00t-c0re-hierarchy/src/roles.rs` (from the `use serde` line through the end of `impl Team`) with:

```rust
use b00t_c0re_role::KnownRole;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Agent {
    pub id: String,
    pub role: KnownRole,
    pub skills: Vec<String>,
    pub cake_balance: f64,
    pub is_alive: bool,
    pub manager_id: Option<String>, // Executive or Operator who recruited them
    pub is_player: bool,            // true if this Agent represents a human user
}

/// First behavioral use of `is_player` in this codebase — previously set at
/// construction but never read. Lets hive messaging (`b00t-council`) tag
/// traffic as human- vs. software-originated.
impl b00t_council::Player for Agent {
    fn player_id(&self) -> &str {
        &self.id
    }

    fn is_human(&self) -> bool {
        self.is_player
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Team {
    pub executive_id: String,
    pub worker_ids: Vec<String>,
    pub operator_ids: Vec<String>,
    pub specialist_ids: Vec<String>,
    #[serde(default)]
    pub player_ids: Vec<String>, // human users (not agent software roles)
}

impl Team {
    /// Create a new Team with the given executive.
    pub fn new(executive_id: &str) -> Self {
        Self {
            executive_id: executive_id.to_string(),
            worker_ids: Vec::new(),
            operator_ids: Vec::new(),
            specialist_ids: Vec::new(),
            player_ids: Vec::new(),
        }
    }

    /// Add an agent ID to the worker roster.
    pub fn add_worker(&mut self, id: &str) {
        if !self.worker_ids.iter().any(|m| m == id) {
            self.worker_ids.push(id.to_string());
        }
    }

    /// Remove an agent ID from the worker roster.
    pub fn remove_worker(&mut self, id: &str) {
        self.worker_ids.retain(|m| m != id);
    }

    /// Add an agent ID to the specialist roster.
    pub fn add_specialist(&mut self, id: &str) {
        if !self.specialist_ids.iter().any(|s| s == id) {
            self.specialist_ids.push(id.to_string());
        }
    }

    /// Remove an agent ID from the specialist roster.
    pub fn remove_specialist(&mut self, id: &str) {
        self.specialist_ids.retain(|s| s != id);
    }

    /// Add an agent ID to the operator roster.
    pub fn add_operator(&mut self, id: &str) {
        if !self.operator_ids.iter().any(|o| o == id) {
            self.operator_ids.push(id.to_string());
        }
    }

    /// Remove an agent ID from the operator roster.
    pub fn remove_operator(&mut self, id: &str) {
        self.operator_ids.retain(|o| o != id);
    }

    /// Add a player ID to the player roster (human user).
    pub fn add_player(&mut self, id: &str) {
        if !self.player_ids.iter().any(|p| p == id) {
            self.player_ids.push(id.to_string());
        }
    }

    /// Remove a player ID from the player roster.
    pub fn remove_player(&mut self, id: &str) {
        self.player_ids.retain(|p| p != id);
    }
}
```

(The rest of `roles.rs` — `MissionTopic`, `TopicStatus` — is unchanged, leave as-is below this point.)

- [ ] **Step 3: Verify the crate does not yet build (expected)**

Run: `cargo build -p b00t-c0re-hierarchy 2>&1 | head -50`
Expected: errors in `cake_economy.rs`, `governance_bridge.rs`, `recruitment.rs` (still reference deleted `Role`) — confirms `roles.rs` itself is internally consistent. Proceed to Task 6.

- [ ] **Step 4: Commit**

```bash
git add b00t-c0re-hierarchy/Cargo.toml b00t-c0re-hierarchy/src/roles.rs
git commit -m "refactor(b00t-c0re-hierarchy): Agent/Team use unified KnownRole, drop local Role enum"
```

---

### Task 6: Fix `cake_economy.rs`, `governance_bridge.rs`, `recruitment.rs`

**Files:**
- Modify: `b00t-c0re-hierarchy/src/cake_economy.rs` (doctest + one test helper)
- Modify: `b00t-c0re-hierarchy/src/governance_bridge.rs` (one test helper)
- Modify: `b00t-c0re-hierarchy/src/recruitment.rs` (production `HireAction.role` field + test helper + 12 call sites)

**Interfaces:**
- Consumes: `Team::{add_worker, ...}` and `Agent.role: KnownRole` from Task 5, `KnownRole::{worker, executive, specialist}` from Task 2.
- Produces: `pub struct HireAction { pub captain_id: String, pub agent_id: String, pub role: KnownRole }` (field name `captain_id` on `HireAction`/`RecruitRequest` is a request-payload field, not a `Role` variant — left unchanged, it's just a string label for "who is requesting," unrelated to the type unification).

- [ ] **Step 1: Fix cake_economy.rs's module-doc doctest**

In `b00t-c0re-hierarchy/src/cake_economy.rs`, replace lines 14-21 (the `//!` doctest body, from `use b00t_c0re_hierarchy::cake_economy...` through the closing of the `alice` struct literal):

```rust
//! use b00t_c0re_hierarchy::cake_economy::{CakeLedger, CakeTransaction};
//! use b00t_c0re_hierarchy::roles::Agent;
//! use b00t_c0re_role::KnownRole;
//!
//! let mut ledger = CakeLedger::new();
//! let mut alice = Agent {
//!     id: "alice".into(), role: KnownRole::executive(), skills: vec![],
//!     cake_balance: 0.0, is_alive: true, manager_id: None, is_player: false,
//! };
```

- [ ] **Step 2: Fix cake_economy.rs's test helper**

In the same file's `#[cfg(test)] mod tests`, change:

```rust
    use super::*;
    use crate::roles::Role;

    fn make_agent(id: &str, balance: f64, alive: bool) -> Agent {
        Agent {
            id: id.to_string(),
            role: Role::Executor,
```

to:

```rust
    use super::*;
    use b00t_c0re_role::KnownRole;

    fn make_agent(id: &str, balance: f64, alive: bool) -> Agent {
        Agent {
            id: id.to_string(),
            role: KnownRole::worker(),
```

- [ ] **Step 3: Fix governance_bridge.rs's test helper**

In `b00t-c0re-hierarchy/src/governance_bridge.rs`, change:

```rust
    use super::*;
    use crate::roles::{Agent, MissionTopic, Role, TopicStatus};

    fn make_agent(id: &str) -> Agent {
        Agent {
            id: id.to_string(),
            role: Role::Specialist,
```

to:

```rust
    use super::*;
    use crate::roles::{Agent, MissionTopic, TopicStatus};
    use b00t_c0re_role::KnownRole;

    fn make_agent(id: &str) -> Agent {
        Agent {
            id: id.to_string(),
            role: KnownRole::specialist(),
```

- [ ] **Step 4: Fix recruitment.rs's production struct field**

In `b00t-c0re-hierarchy/src/recruitment.rs`, change the top import and the `HireAction` struct:

```rust
use crate::roles::*;
use b00t_c0re_role::KnownRole;
```

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HireAction {
    pub captain_id: String,
    pub agent_id: String,
    /// Expected hireable role for the recruited agent.
    /// Recruitment normally assigns operational roles such as
    /// `KnownRole::worker()` or `KnownRole::specialist()`; callers should
    /// avoid passing unrelated roles unless the processing path explicitly
    /// supports them.
    pub role: KnownRole,
}
```

- [ ] **Step 5: Fix recruitment.rs's test helper and all 12 call sites**

Change the test helper:

```rust
    fn make_agent(id: &str, role: Role, skills: &[&str], alive: bool) -> Agent {
```

to:

```rust
    fn make_agent(id: &str, role: KnownRole, skills: &[&str], alive: bool) -> Agent {
```

Then replace every `Role::Executor` call-site argument with `KnownRole::worker()` and every `Role::Specialist` with `KnownRole::specialist()` — this is every line reported by `grep -n "Role::" b00t-c0re-hierarchy/src/recruitment.rs` (lines 110, 129-131, 151-152, 161, 165, 169, 183-184, 201 as of this plan being written; re-grep before editing in case line numbers shifted from earlier tasks). Example of the pattern (`test_hire_updates_role`, originally lines 66-77):

```rust
    #[test]
    fn test_hire_updates_role() {
        let mut agent = make_agent("a1", KnownRole::worker(), &["rust"], true);
        let action = HireAction {
            captain_id: "cap1".to_string(),
            agent_id: "a1".to_string(),
            role: KnownRole::specialist(),
        };

        hire_agent(&mut agent, &action);
        assert_eq!(agent.role, KnownRole::specialist());
        assert_eq!(agent.manager_id, Some("cap1".to_string()));
    }
```

- [ ] **Step 6: Verify the crate builds and tests pass**

Run: `cargo test -p b00t-c0re-hierarchy 2>&1 | tail -60`
Expected: compiles; all tests in `cake_economy.rs`, `governance_bridge.rs`, `recruitment.rs` pass. (`hierarchy_test.rs` in the `tests/` dir is fixed separately in Task 8 — expect that integration test binary to still fail to compile at this point.)

- [ ] **Step 7: Commit**

```bash
git add b00t-c0re-hierarchy/src/cake_economy.rs b00t-c0re-hierarchy/src/governance_bridge.rs b00t-c0re-hierarchy/src/recruitment.rs
git commit -m "refactor(b00t-c0re-hierarchy): cake_economy/governance_bridge/recruitment use KnownRole"
```

---

### Task 7: Fix `crew_handler.rs`

**Files:**
- Modify: `b00t-cli/src/commands/crew_handler.rs`

**Interfaces:**
- Consumes: `KnownRole::{worker, specialist}` (Task 2), `Team`/`Agent` (Task 5).
- Produces: `struct CrewMeta { role: KnownRole, ... }`; `handle_hire`/`handle_roster` operate on `KnownRole`.

- [ ] **Step 1: Update the import and CrewMeta struct**

Change line 12-13's imports and the `CrewMeta` struct (originally lines 19-36):

```rust
use b00t_c0re_hierarchy::recruitment::*;
use b00t_c0re_hierarchy::roles::*;
use b00t_c0re_role::KnownRole;
use serde::{Deserialize, Serialize};
use url::Url;

use crate::commands::crew::CrewCommand;

/// Metadata for crew-specific fields not present in AgentCard.
#[derive(Debug, Clone, Serialize, Deserialize)]
struct CrewMeta {
    role: KnownRole,
    manager_id: Option<String>,
    cake_balance: f64,
    is_alive: bool,
    is_player: bool,
}

impl Default for CrewMeta {
    fn default() -> Self {
        Self {
            role: KnownRole::worker(),
            manager_id: None,
            cake_balance: 100.0,
            is_alive: true,
            is_player: false,
        }
    }
}
```

- [ ] **Step 2: Fix the four demo-agent `CrewMeta` literals**

Replace every `role: Role::Executor,` (originally lines 32, 137, 168, 199 — the `rc_meta`, `de_meta`, `db_meta` demo-agent constructions and any sibling not shown in the earlier grep excerpt) with `role: KnownRole::worker(),`. Re-grep `grep -n "role: Role::Executor" b00t-cli/src/commands/crew_handler.rs` before editing to confirm the current line numbers and catch all occurrences.

- [ ] **Step 3: Fix handle_hire's role parsing**

Change (originally lines 325-330):

```rust
fn handle_hire(store: &AgentStore, agent_id: &str, role: Option<&str>) {
    let target_role = match role {
        Some("executor") => Role::Executor,
        Some("specialist") => Role::Specialist,
        _ => Role::Executor,
    };
```

to (deliberately keeping `"executor"` as an accepted input alongside the new `"worker"`, so existing scripts/muscle-memory using `--role executor` keep working — a small, cheap back-compat accommodation, not a silent break):

```rust
fn handle_hire(store: &AgentStore, agent_id: &str, role: Option<&str>) {
    let target_role = match role {
        Some("worker") | Some("executor") => KnownRole::worker(),
        Some("specialist") => KnownRole::specialist(),
        _ => KnownRole::worker(),
    };
```

- [ ] **Step 4: Fix handle_roster's bucketing match**

Change (originally lines 344-361):

```rust
    // Separate by role
    let mut captains = Vec::new();
    let mut executors = Vec::new();
    let mut operators = Vec::new();
    let mut specialists = Vec::new();
    let mut bouncers = Vec::new();

    for agent in &agents {
        match agent.role {
            Role::Captain => captains.push(agent),
            Role::Executor => executors.push(agent),
            Role::Operator => operators.push(agent),
            Role::Specialist => specialists.push(agent),
            Role::Bouncer => bouncers.push(agent),
            Role::Mate | Role::Player => specialists.push(agent),
        }
    }
```

to:

```rust
    // Separate by role
    let mut executives = Vec::new();
    let mut workers = Vec::new();
    let mut operators = Vec::new();
    let mut specialists = Vec::new();

    for agent in &agents {
        match &agent.role {
            KnownRole::Executive(_) => executives.push(agent),
            KnownRole::Worker(_) => workers.push(agent),
            KnownRole::Operator(_) => operators.push(agent),
            KnownRole::Specialist(_) => specialists.push(agent),
        }
    }
```

- [ ] **Step 5: Fix the roster's display section**

Below the match, find the print blocks for `captains`/`executors`/`bouncers` (originally starting around line 363) and rename to match: `captains` → `executives` (heading `"  Captain:"` → `"  Executive:"`), `executors` → `workers` (heading `"  Executors:"` → `"  Workers:"`). Delete the entire `"  Bouncers:"` print block (its `if bouncers.is_empty() { ... } else { for a in &bouncers { ... } }` section) — there is no `bouncers` variable anymore. Leave the `"  Operators:"` and any `"  Specialists:"` blocks' variable names unchanged (`operators`, `specialists` already match).

- [ ] **Step 6: Verify the crate builds and existing crew_handler behavior is intact**

Run: `cargo build -p b00t-cli 2>&1 | head -80`
Expected: errors only from `wow.rs` remain (fixed in Task 8). Everything in `crew_handler.rs` compiles clean.

- [ ] **Step 7: Commit**

```bash
git add b00t-cli/src/commands/crew_handler.rs
git commit -m "refactor(b00t-cli): crew_handler uses unified KnownRole (Bouncer/Mate/Player retired)"
```

---

### Task 8: Fix `wow.rs`'s `KnownRoleCheck`, then `hierarchy_test.rs`

**Files:**
- Modify: `b00t-cli/src/wow.rs:183-222` (`KnownRoleCheck`)
- Modify: `b00t-c0re-hierarchy/tests/hierarchy_test.rs`

**Interfaces:**
- Consumes: `b00t-c0re-role/src/lib.rs`'s exact `KnownRole` variant source text (Task 2), `Team`/`Agent`/`KnownRole` (Tasks 2, 5).
- Produces: a working `wow` integrity check and a passing `hierarchy_test.rs` integration test binary — the last two pieces before the full-workspace gate in Task 9.

- [ ] **Step 1: Repoint and update KnownRoleCheck**

In `b00t-cli/src/wow.rs`, change the path construction (originally lines 189-194) from `b00t-cli/src/agentic_role.rs` to the new crate's location:

```rust
        let path = if let Ok(manifest) = std::env::var("CARGO_MANIFEST_DIR") {
            std::path::Path::new(&manifest)
                .join("..")
                .join("b00t-c0re-role/src/lib.rs")
        } else {
            std::path::PathBuf::from("b00t-c0re-role/src/lib.rs")
        };
```

And update the error message on the next few lines from `"read agentic_role.rs: {e}"` to `"read b00t-c0re-role/src/lib.rs: {e}"`.

Then update the four substring checks (originally lines 208-211) from:

```rust
        let has_worker = src.contains("Worker(RoleRef<Worker>)");
        let has_exec = src.contains("Executive(RoleRef<Executive>)");
        let has_op = src.contains("Operator(RoleRef<Operator>)");
        let has_provider = src.contains("AppProvider(RoleRef<AppProvider>)");
        let all = has_worker && has_exec && has_op && has_provider;
```

to:

```rust
        let has_worker = src.contains("Worker(RoleRef<Worker>)");
        let has_exec = src.contains("Executive(RoleRef<Executive>)");
        let has_op = src.contains("Operator(RoleRef<Operator>)");
        let has_specialist = src.contains("Specialist(RoleRef<Specialist>)");
        let all = has_worker && has_exec && has_op && has_specialist;
```

And the failure-detail format string's `provider={has_provider}` becomes `specialist={has_specialist}`.

- [ ] **Step 2: Verify**

Run: `cargo build -p b00t-cli 2>&1 | tail -40`
Expected: `b00t-cli` now compiles fully (the last remaining error source is fixed).

Run: `cargo test -p b00t-cli wow::` (adjust filter to whatever module path the WOW tests actually live under — check `b00t-cli/src/wow.rs` around line 542 for the test referencing `KnownRoleCheck` if the filter above finds nothing)
Expected: passes, `"4 variants present"`.

- [ ] **Step 3: Commit the wow.rs fix**

```bash
git add b00t-cli/src/wow.rs
git commit -m "fix(b00t-cli): wow KnownRoleCheck points at b00t-c0re-role, checks Specialist"
```

- [ ] **Step 4: Rewrite hierarchy_test.rs**

Replace the full content of `b00t-c0re-hierarchy/tests/hierarchy_test.rs` with:

```rust
use b00t_c0re_hierarchy::recruitment::*;
use b00t_c0re_hierarchy::roles::*;
use b00t_c0re_role::KnownRole;
use serde_json::json;

fn make_agent(id: &str, role: KnownRole, skills: &[&str], alive: bool, is_player: bool) -> Agent {
    Agent {
        id: id.to_string(),
        role,
        skills: skills.iter().map(|s| s.to_string()).collect(),
        cake_balance: 100.0,
        is_alive: alive,
        manager_id: None,
        is_player,
    }
}

#[test]
fn test_executive_creates_team() {
    let executive = make_agent("exec1", KnownRole::executive(), &["leadership"], true, false);
    let specialist = make_agent("spec1", KnownRole::specialist(), &["navigation"], true, false);
    let worker = make_agent("w1", KnownRole::worker(), &["rust"], true, false);

    let mut team = Team::new(&executive.id);
    team.add_specialist(&specialist.id);
    team.add_worker(&worker.id);

    assert_eq!(team.executive_id, "exec1");
    assert_eq!(team.specialist_ids.len(), 1);
    assert_eq!(team.worker_ids.len(), 1);
}

#[test]
fn test_recruit_request_ranks_by_skill() {
    let request = RecruitRequest {
        captain_id: "cap1".to_string(),
        required_skills: vec![
            "rust".to_string(),
            "python".to_string(),
            "docker".to_string(),
        ],
        max_players: 2,
        bounty_share: 10.0,
    };

    let agents = vec![
        make_agent("a1", KnownRole::worker(), &["rust"], true, false),
        make_agent(
            "a2",
            KnownRole::worker(),
            &["rust", "python", "docker"],
            true,
            false,
        ),
        make_agent("a3", KnownRole::worker(), &["python", "docker"], true, false),
    ];

    let response = process_recruit_request(&request, &agents, "op1");

    assert_eq!(response.candidates.len(), 2);
    assert_eq!(response.candidates[0].id, "a2");
    assert_eq!(response.candidates[1].id, "a3");
    assert_eq!(response.operator_fee_pct, 20.0);
}

#[test]
fn test_hire_updates_role() {
    let mut agent = make_agent("a1", KnownRole::worker(), &["rust"], true, false);
    let action = HireAction {
        captain_id: "cap1".to_string(),
        agent_id: "a1".to_string(),
        role: KnownRole::specialist(),
    };

    hire_agent(&mut agent, &action);
    assert_eq!(agent.role, KnownRole::specialist());
    assert_eq!(agent.manager_id, Some("cap1".to_string()));
}

#[test]
fn test_agent_death_detection() {
    let alive = make_agent("a1", KnownRole::worker(), &["rust"], true, false);
    let dead = make_agent("a2", KnownRole::worker(), &["rust"], false, false);

    assert!(!is_dead(&alive));
    assert!(is_dead(&dead));
}

#[test]
fn test_recruit_no_candidates_returns_empty() {
    let request = RecruitRequest {
        captain_id: "cap1".to_string(),
        required_skills: vec!["java".to_string(), "scala".to_string()],
        max_players: 3,
        bounty_share: 10.0,
    };

    let agents = vec![
        make_agent("a1", KnownRole::worker(), &["rust"], true, false),
        make_agent("a2", KnownRole::worker(), &["python"], true, false),
    ];

    let response = process_recruit_request(&request, &agents, "op1");
    assert!(response.candidates.is_empty());
}

#[test]
fn test_team_deserializes_without_player_ids() {
    let legacy_team = json!({
        "executive_id": "exec1",
        "worker_ids": [],
        "operator_ids": [],
        "specialist_ids": []
    });

    let team: Team = serde_json::from_value(legacy_team).expect("legacy payload must deserialize");
    assert!(team.player_ids.is_empty());
}
```

Note what's deliberately removed, not just adapted: `test_legacy_role_variants_still_deserialize` (asserted `Role::Mate`/`Role::Player` deserialize from `"Mate"`/`"Player"` strings) is deleted outright — those variants no longer exist, and a repo-wide search (see this plan's Global Constraints) found no real persisted data depending on that PascalCase deserialization, so no replacement compatibility shim is being added. `test_team_deserializes_without_player_ids`'s JSON payload drops its old `"bouncer_id"`-adjacent `"bouncer_ids": []` key and `"captain_id"`/`"executor_ids"` keys, renamed to match `Team`'s new field names — its actual purpose (proving `player_ids` has `#[serde(default)]` and doesn't need to be present) is unchanged.

- [ ] **Step 5: Run the integration test**

Run: `cargo test -p b00t-c0re-hierarchy --test hierarchy_test`
Expected: all 6 tests pass.

- [ ] **Step 6: Commit**

```bash
git add b00t-c0re-hierarchy/tests/hierarchy_test.rs
git commit -m "test(b00t-c0re-hierarchy): hierarchy_test uses KnownRole, drops Bouncer/Mate/Player coverage"
```

---

### Task 9: Hand-update the generated LinkML Rust crate, full-workspace verification

**Files:**
- Modify: `_b00t_/linkml/schema/hive_role_vocabulary_rust/src/lib.rs`

**Interfaces:**
- Consumes: nothing new — final consistency pass.
- Produces: a fully green `cargo build --workspace` / `cargo test` for every crate this plan touched.

- [ ] **Step 1: Update the generated HiveRole enum**

`_b00t_/linkml/schema/hive_role_vocabulary_rust` is **not** a Cargo workspace member (verified: absent from root `Cargo.toml`'s `members` list, has its own `Cargo.lock`), so it will not be caught by any build/test command below — no automatic regeneration command was found in this repo (no `justfile`/script reference to this schema). This is a manual, best-effort mirror of Task 3's YAML change, done because the file is checked into the repo and would otherwise silently drift the moment anyone looks at it.

In `_b00t_/linkml/schema/hive_role_vocabulary_rust/src/lib.rs`, find the `HiveRole` enum (starts around line 53) and change:

```rust
#[cfg_attr(feature = "serde", serde(rename = "provider"))]
    Provider,
```

to:

```rust
#[cfg_attr(feature = "serde", serde(rename = "specialist"))]
    Specialist,
```

(Leave `Worker`/`Executive`/`Operator` variants unchanged — only the fourth variant's name and rename attribute change.) Check the rest of the file (it's LinkML-codegen boilerplate — `poly.rs`/`poly_containers.rs`/`serde_utils.rs` etc. are generic infrastructure, not role-specific) for any other literal occurrence of `Provider`/`"provider"` tied to this specific enum with `grep -n "Provider\|\"provider\"" _b00t_/linkml/schema/hive_role_vocabulary_rust/src/*.rs` and update any found (e.g. a `Display`/`FromStr` impl generated alongside the enum, if present).

- [ ] **Step 2: Commit**

```bash
git add _b00t_/linkml/schema/hive_role_vocabulary_rust/src/lib.rs
git commit -m "chore(linkml-rust): hand-sync generated HiveRole enum with hive_role_vocabulary.yaml"
```

- [ ] **Step 3: Full workspace build**

Run: `cargo build --workspace 2>&1 | tail -100`
Expected: exit 0, no errors. If anything outside the files this plan touched fails, it's a call site this plan's `gh search`/`grep` passes missed — fix it following the same `Role::X` → `KnownRole::Y` / renamed-field pattern established in Tasks 5-8, then re-run.

- [ ] **Step 4: Full workspace test for every touched crate**

Run: `cargo test -p b00t-c0re-role -p b00t-c0re-hierarchy -p b00t-cli 2>&1 | tail -150`
Expected: exit 0, all tests pass (including the doctest in `cake_economy.rs`, run automatically as part of `cargo test -p b00t-c0re-hierarchy`).

- [ ] **Step 5: Broader regression check**

Run: `cargo test --workspace 2>&1 | tail -200`
Expected: exit 0. This is slower and covers crates this plan didn't explicitly analyze — if something unexpected fails here, treat it as a real finding (a call site missed by the earlier `gh search`/`grep` passes), not noise to suppress.

- [ ] **Step 6: Final commit (only if Step 3-5 required fixes)**

If Steps 3-5 were clean on the first try, there is nothing to commit here — Task 9's only change was Step 1-2's hand-sync. If fixes were needed, commit them with a message describing exactly what was missed, e.g.:

```bash
git add -A
git commit -m "fix: address workspace-wide fallout from role taxonomy unification"
```
