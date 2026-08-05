//! AgenticRole trait + AgenticCrew — type-level role invariants.
//!
//! Each role is a ZST implementing `AgenticRole`, carrying its semantic
//! position within an `AgenticCrew`.  `RoleRef<T>` wraps a string value
//! at the type level — the concrete role type T is the invariant lock,
//! guaranteeing the string is a known role at compile time.
//!
//! Crew relationships are encoded as associated types: each role knows
//! its parent, peers, and delegated sub-roles through the crew graph.

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
/// Crew dispatch and specialist routing — spins typed crews via k0mmand3r.
///
/// ⚠️ NOT the same role as `b00t_c0re_hierarchy::roles::Role::Operator`
/// ("recruitment + training — scouts/finds agents, enlists, executes
/// training plans"). Both use the bare string `"operator"` with no
/// disambiguating prefix — a known, tracked, currently-unresolved naming
/// collision. See `_b00t_/linkml/schema/hive_role_vocabulary.yaml`'s
/// top-level description for the full context (Phase 2 of the
/// ScopeStore+LinkML epic, #905/#909 "no parallel vocabularies").
#[derive(Debug, Clone)]
pub struct Operator;
// 🦨 Orchestrator renamed to AppProvider
/// App provider — service hosting, MCP surface, desktop control-plane.
#[derive(Debug, Clone)]
pub struct AppProvider;

// ── Crew definitions ─────────────────────────────────────────────────────────

/// The primary hive crew — roles that operate within the b00t hive.
pub struct HiveCrew;

impl sealed::Sealed for HiveCrew {}
impl AgenticCrew for HiveCrew {
    const NAME: &'static str = "hive";
    fn known_roles() -> Vec<&'static str> {
        vec!["worker", "executive", "operator", "provider"]
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
        vec!["executive", "operator", "provider"]
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
        vec!["operator", "provider", "worker"]
    }
}

impl sealed::Sealed for Operator {}
impl AgenticRole for Operator {
    const NAME: &'static str = "operator";
    fn hint() -> &'static str {
        "Crew dispatch and specialist routing — spins typed crews via k0mmand3r"
    }
    type Crew = HiveCrew;
    fn peers() -> Vec<&'static str> {
        vec!["executive", "provider", "worker"]
    }
}

impl sealed::Sealed for AppProvider {}
impl AgenticRole for AppProvider {
    const NAME: &'static str = "provider";
    fn hint() -> &'static str {
        "Provider — MCP surface, desktop control-plane, service hosting"
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
#[derive(Debug, Clone)]
pub enum KnownRole {
    Worker(RoleRef<Worker>),
    Executive(RoleRef<Executive>),
    Operator(RoleRef<Operator>),
    AppProvider(RoleRef<AppProvider>),
}

impl KnownRole {
    pub fn name(&self) -> &str {
        match self {
            KnownRole::Worker(r) => r.as_ref(),
            KnownRole::Executive(r) => r.as_ref(),
            KnownRole::Operator(r) => r.as_ref(),
            KnownRole::AppProvider(r) => r.as_ref(),
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
            n if n == AppProvider::NAME => Some(KnownRole::AppProvider(
                RoleRef::new(AppProvider::NAME).unwrap(),
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
            // Unknown/overridden name defaults to Worker type — the name
            // is preserved for datum lookup, the type is Worker (safe fallback).
            KnownRole::Worker(RoleRef::new_owned(name))
        })
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
        assert!(roles.contains(&"provider"));
        assert_eq!(HiveCrew::NAME, "hive");
    }

    #[test]
    fn test_role_hints_differ() {
        assert_ne!(Worker::hint(), Executive::hint());
        assert_ne!(Operator::hint(), AppProvider::hint());
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

    #[test]
    fn test_known_role_resolve_provider() {
        let r = resolve_role(Some("provider".to_string()));
        assert_eq!(r.name(), "provider");
        assert_eq!(r, "provider");
    }

    #[test]
    fn test_known_role_from_str() {
        assert!(KnownRole::from_str("worker").is_some());
        assert!(KnownRole::from_str("executive").is_some());
        assert!(KnownRole::from_str("operator").is_some());
        assert!(KnownRole::from_str("provider").is_some());
        assert!(KnownRole::from_str("captain").is_none());
    }

    #[test]
    fn test_provider_role_definition() {
        assert_eq!(AppProvider::NAME, "provider");
        assert!(AppProvider::peers().contains(&"executive"));
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
    /// executive, operator, provider) — e.g. "reviewer", "podman",
    /// "ux-designer" — falls into KnownRole::resolve's fallback branch,
    /// which wraps it as KnownRole::Worker(RoleRef::new_owned(name)). The
    /// actual name IS preserved inside that RoleRef, but .name() used to
    /// match on the enum variant and return the hardcoded Worker::NAME
    /// constant instead of reading it — silently collapsing every custom
    /// role's .name() to "worker".
    #[test]
    fn test_custom_role_name_is_preserved_not_collapsed_to_worker() {
        let role = resolve_role(Some("reviewer".to_string()));
        assert_eq!(role.name(), "reviewer");
        assert_eq!(role, "reviewer");
        assert_eq!(format!("{}", role), "reviewer");
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

        for name in [Worker::NAME, Executive::NAME, Operator::NAME, AppProvider::NAME] {
            assert!(
                permissible_values.contains_key(name),
                "{name:?} is a real AgenticRole::NAME but missing from \
                 hive_role_vocabulary.yaml's permissible_values"
            );
        }
    }
}
