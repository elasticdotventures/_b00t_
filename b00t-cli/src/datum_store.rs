//! Chalk-style DatumStore: abstract datum storage with O(1) identity + goal-oriented query.
//!
//! # Design (Chalk Interner analogy)
//!
//! | Chalk              | b00t                   |
//! |--------------------|------------------------|
//! | `Ty<I>`            | `InternedDatum`        |
//! | `TyKind<I>`        | `BootDatum`            |
//! | `Interner` trait   | `DatumStore` trait     |
//! | `InternedTy`       | `Arc<str>` key         |
//! | `Box` strategy     | `HashMapStore`         |
//!
//! `InternedDatum` equality is O(1) key-compare, not O(n) field-diff.
//! `DatumStore` is object-safe: swappable over TOML-files, SQLite, or Qdrant.
//!
//! # Goal-oriented query
//! ```rust
//! let clis: Vec<InternedDatum> = store
//!     .query()
//!     .proves_cli()
//!     .with_type_tag("transferable")
//!     .depends_on("rust.cli")
//!     .run();
//! ```

use crate::datum_proof::{
    AsAgentDatum, AsAiDatum, AsAiModelDatum, AsAptDatum, AsApiDatum, AsBashDatum,
    AsCliDatum, AsConfigDatum, AsDatabaseDatum, AsDockerDatum, AsHiveProfileDatum,
    AsJobDatum, AsJustfileDatum, AsK8sDatum, AsMcpDatum, AsNixDatum, AsRepoDatum,
    AsRoleDatum, AsSkillDatum, AsStackDatum, AsUnknownDatum, AsVscodeDatum,
    DatumProofError, Provable,
};
use crate::datum_utils::get_all_datums;
use crate::{BootDatum, DatumType};
use anyhow::Result;
use std::collections::{HashMap, HashSet};
use std::fmt;
use std::sync::Arc;

// ── InternedDatum ─────────────────────────────────────────────────────────────

/// Handle to a canonically-stored datum. Equality is O(1) key comparison.
/// Clone is cheap (Arc). Analogous to Chalk's `Ty<I>`.
#[derive(Clone, Debug)]
pub struct InternedDatum {
    /// Canonical key (e.g. "git.cli", "kaizen.skill"). Arc for O(1) clone.
    pub key: Arc<str>,
    /// Datum body. Arc so multiple queries share the same allocation.
    pub datum: Arc<BootDatum>,
}

impl PartialEq for InternedDatum {
    fn eq(&self, other: &Self) -> bool {
        // O(1): compare interned keys, not all datum fields.
        self.key == other.key
    }
}
impl Eq for InternedDatum {}

impl std::hash::Hash for InternedDatum {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.key.hash(state);
    }
}

impl InternedDatum {
    pub fn new(key: impl Into<Arc<str>>, datum: BootDatum) -> Self {
        Self { key: key.into(), datum: Arc::new(datum) }
    }

    /// Prove structural invariants for the datum's declared type (delegates to BootDatum).
    pub fn prove_by_type(&self) -> Result<(), crate::datum_proof::DatumProofError> {
        self.datum.prove_by_type()
    }
}

// ── LearnResult ───────────────────────────────────────────────────────────────

/// Discriminated result of a `DatumStore::learn(topic)` call.
///
/// Replaces the ambiguous `Option<InternedDatum>` + implicit prove() pattern
/// with a typed enum so callers can route on the exact failure mode.
#[derive(Debug)]
pub enum LearnResult {
    /// Topic found in store and passes `prove_by_type()`.
    Found(InternedDatum),
    /// Topic absent from store — candidate for auto-research or DWIW fanout.
    NotFound,
    /// Topic found but `prove_by_type()` fails — datum is malformed.
    Malformed(InternedDatum, DatumProofError),
    /// Store could not be loaded at all (I/O error, parse failure, etc).
    StoreError(String),
}

impl LearnResult {
    pub fn is_found(&self) -> bool { matches!(self, Self::Found(_)) }
    pub fn is_not_found(&self) -> bool { matches!(self, Self::NotFound) }
    pub fn is_malformed(&self) -> bool { matches!(self, Self::Malformed(..)) }
    pub fn is_store_error(&self) -> bool { matches!(self, Self::StoreError(_)) }

    /// Return the interned datum regardless of proof status, if present.
    pub fn datum(&self) -> Option<&InternedDatum> {
        match self {
            Self::Found(d) | Self::Malformed(d, _) => Some(d),
            Self::NotFound | Self::StoreError(_) => None,
        }
    }
}

impl fmt::Display for LearnResult {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Found(d) => write!(f, "found:{}", d.key),
            Self::NotFound => write!(f, "not_found"),
            Self::Malformed(d, e) => write!(f, "malformed:{}:{}", d.key, e),
            Self::StoreError(e) => write!(f, "store_error:{}", e),
        }
    }
}

// ── DatumStore trait ──────────────────────────────────────────────────────────

/// Abstract over how datums are stored: HashMap, SQLite, Qdrant, etc.
/// Analogous to Chalk's `Interner` trait.
pub trait DatumStore: Send + Sync {
    /// Intern a datum by key; returns the canonical `InternedDatum`.
    fn intern(&mut self, key: &str, datum: BootDatum) -> InternedDatum;

    /// Look up an interned datum by key. O(1).
    fn get(&self, key: &str) -> Option<InternedDatum>;

    /// Iterate all interned datums.
    fn iter(&self) -> Box<dyn Iterator<Item = InternedDatum> + '_>;

    /// Total number of datums in store.
    fn len(&self) -> usize;

    fn is_empty(&self) -> bool { self.len() == 0 }

    /// Discriminated learn lookup: returns Found/NotFound/Malformed.
    /// Use this instead of `get()` + manual `prove_by_type()` to get actionable failure modes.
    fn learn(&self, topic: &str) -> LearnResult {
        match self.get(topic) {
            None => LearnResult::NotFound,
            Some(d) => match d.prove_by_type() {
                Ok(()) => LearnResult::Found(d),
                Err(e) => LearnResult::Malformed(d, e),
            }
        }
    }

    /// Begin a goal-oriented query over this store.
    #[must_use = "call .run() or .run_keys() to evaluate the query"]
    fn query(&self) -> DatumQuery<'_> where Self: Sized {
        DatumQuery { store: self, predicates: vec![] }
    }

    /// Phase 2 — Coherence: validate cross-datum references (Chalk coherence analogy).
    /// Reports: missing depends_on targets, self-deps, empty dep strings, missing members.
    /// Override in bulk-query backends (e.g. SQL JOIN) for efficiency.
    fn validate_references(&self) -> Vec<ReferenceError> {
        let key_set: HashSet<String> = self.iter().map(|d| d.key.as_ref().to_owned()).collect();
        let mut errors = Vec::new();
        for d in self.iter() {
            let key = d.key.as_ref();
            if let Some(deps) = &d.datum.depends_on {
                for dep in deps {
                    if dep.is_empty() {
                        errors.push(ReferenceError::EmptyDependency { datum: key.to_string() });
                    } else if dep == key {
                        errors.push(ReferenceError::SelfDependency { datum: key.to_string() });
                    } else if !key_set.contains(dep.as_str()) {
                        errors.push(ReferenceError::MissingDependency {
                            datum: key.to_string(),
                            missing_key: dep.clone(),
                        });
                    }
                }
            }
            if let Some(members) = &d.datum.members {
                for m in members {
                    if !m.is_empty() && !key_set.contains(m.as_str()) {
                        errors.push(ReferenceError::MissingMember {
                            stack: key.to_string(),
                            missing_member: m.clone(),
                        });
                    }
                }
            }
        }
        errors
    }

    /// Like `validate_references` but ignores missing deps/members in `allowlist`
    /// (for sub-graphs that intentionally reference datums from other stores/dirs).
    fn validate_references_with_allowlist(&self, allowlist: &HashSet<&str>) -> Vec<ReferenceError> {
        self.validate_references()
            .into_iter()
            .filter(|e| match e {
                ReferenceError::MissingDependency { missing_key, .. } =>
                    !allowlist.contains(missing_key.as_str()),
                ReferenceError::MissingMember { missing_member, .. } =>
                    !allowlist.contains(missing_member.as_str()),
                _ => true,
            })
            .collect()
    }
}

// ── ReferenceError ────────────────────────────────────────────────────────────

/// Cross-datum coherence error (Phase 2 — analogous to Chalk trait coherence).
#[derive(Debug, Clone, PartialEq)]
pub enum ReferenceError {
    /// A `depends_on` key references a datum not in this store.
    MissingDependency { datum: String, missing_key: String },
    /// A datum depends on itself.
    SelfDependency { datum: String },
    /// Empty string in `depends_on` list.
    EmptyDependency { datum: String },
    /// A `members` key references a datum not in this store (stacks).
    MissingMember { stack: String, missing_member: String },
}

impl fmt::Display for ReferenceError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MissingDependency { datum, missing_key } =>
                write!(f, "'{datum}' depends_on '{missing_key}' which is not in this store"),
            Self::SelfDependency { datum } =>
                write!(f, "'{datum}' depends_on itself"),
            Self::EmptyDependency { datum } =>
                write!(f, "'{datum}' has empty string in depends_on"),
            Self::MissingMember { stack, missing_member } =>
                write!(f, "stack '{stack}' member '{missing_member}' is not in this store"),
        }
    }
}

// ── HashMapStore ──────────────────────────────────────────────────────────────

/// Concrete store backed by an in-memory HashMap. Equivalent to Chalk's Box strategy.
#[derive(Default)]
pub struct HashMapStore {
    inner: HashMap<String, InternedDatum>,
}

impl HashMapStore {
    /// Load all datums from a `_b00t_` directory path (standard TOML scan).
    pub fn from_path(b00t_path: &str) -> Result<Self> {
        let raw = get_all_datums(b00t_path)?;
        let mut store = Self::default();
        for (key, datum) in raw {
            store.intern(&key, datum);
        }
        Ok(store)
    }
}

impl DatumStore for HashMapStore {
    fn intern(&mut self, key: &str, datum: BootDatum) -> InternedDatum {
        let interned = InternedDatum::new(Arc::from(key), datum);
        self.inner.insert(key.to_string(), interned.clone());
        interned
    }

    fn get(&self, key: &str) -> Option<InternedDatum> {
        self.inner.get(key).cloned()
    }

    fn iter(&self) -> Box<dyn Iterator<Item = InternedDatum> + '_> {
        Box::new(self.inner.values().cloned())
    }

    fn len(&self) -> usize { self.inner.len() }
}

// ── DatumQuery ────────────────────────────────────────────────────────────────

/// Goal-oriented query builder. Chains predicates; call `.run()` to evaluate.
/// Each predicate is a Horn-clause "goal" — all must hold (conjunction).
/// Annotated `#[must_use]` — a query that is not `.run()` is always a bug.
#[must_use = "call .run() or .run_keys() to evaluate the query"]
pub struct DatumQuery<'a> {
    store: &'a dyn DatumStore,
    predicates: Vec<Box<dyn Fn(&InternedDatum) -> bool + 'a>>,
}

impl<'a> DatumQuery<'a> {
    fn with(mut self, pred: impl Fn(&InternedDatum) -> bool + 'a) -> Self {
        self.predicates.push(Box::new(pred));
        self
    }

    // ── proves_*() — full parity with BootDatum::prove_*() (22 variants) ──────

    /// Goal: datum proves Cli structural contract.
    pub fn proves_cli(self) -> Self { self.with(|d| AsCliDatum(&d.datum).prove().is_ok()) }
    /// Goal: datum proves Skill structural contract.
    pub fn proves_skill(self) -> Self { self.with(|d| AsSkillDatum(&d.datum).prove().is_ok()) }
    /// Goal: datum proves Role structural contract.
    pub fn proves_role(self) -> Self { self.with(|d| AsRoleDatum(&d.datum).prove().is_ok()) }
    /// Goal: datum proves Mcp structural contract.
    pub fn proves_mcp(self) -> Self { self.with(|d| AsMcpDatum(&d.datum).prove().is_ok()) }
    /// Goal: datum proves Docker structural contract (image/oci_uri/install present).
    pub fn proves_docker(self) -> Self { self.with(|d| AsDockerDatum(&d.datum).prove().is_ok()) }
    /// Goal: datum proves Bash structural contract (script/install present).
    pub fn proves_bash(self) -> Self { self.with(|d| AsBashDatum(&d.datum).prove().is_ok()) }
    /// Goal: datum proves Apt structural contract (install/package_name present).
    pub fn proves_apt(self) -> Self { self.with(|d| AsAptDatum(&d.datum).prove().is_ok()) }
    /// Goal: datum proves Nix structural contract (install/package_name present).
    pub fn proves_nix(self) -> Self { self.with(|d| AsNixDatum(&d.datum).prove().is_ok()) }
    /// Goal: datum proves Vscode structural contract (vsix_id/install present).
    pub fn proves_vscode(self) -> Self { self.with(|d| AsVscodeDatum(&d.datum).prove().is_ok()) }
    /// Goal: datum proves K8s structural contract (chart_path/values_file/install present).
    pub fn proves_k8s(self) -> Self { self.with(|d| AsK8sDatum(&d.datum).prove().is_ok()) }
    /// Goal: datum proves Justfile structural contract (justfile.path/install present).
    pub fn proves_justfile(self) -> Self { self.with(|d| AsJustfileDatum(&d.datum).prove().is_ok()) }
    /// Goal: datum proves Job structural contract (job metadata/script present).
    pub fn proves_job(self) -> Self { self.with(|d| AsJobDatum(&d.datum).prove().is_ok()) }
    /// Goal: datum proves Stack structural contract (stack metadata/members present).
    pub fn proves_stack(self) -> Self { self.with(|d| AsStackDatum(&d.datum).prove().is_ok()) }
    /// Goal: datum proves Agent structural contract (skills/depends_on/channel_prefix present).
    pub fn proves_agent(self) -> Self { self.with(|d| AsAgentDatum(&d.datum).prove().is_ok()) }
    /// Goal: datum proves HiveProfile structural contract (hint present).
    pub fn proves_hive_profile(self) -> Self { self.with(|d| AsHiveProfileDatum(&d.datum).prove().is_ok()) }
    /// Goal: datum proves Database structural contract (dsn/url present).
    pub fn proves_database(self) -> Self { self.with(|d| AsDatabaseDatum(&d.datum).prove().is_ok()) }
    /// Goal: datum proves Api structural contract (url/protocol/provides present).
    pub fn proves_api(self) -> Self { self.with(|d| AsApiDatum(&d.datum).prove().is_ok()) }
    /// Goal: datum proves Repo structural contract (url/clone_path present).
    pub fn proves_repo(self) -> Self { self.with(|d| AsRepoDatum(&d.datum).prove().is_ok()) }
    /// Goal: datum proves Ai structural contract (hint present).
    pub fn proves_ai(self) -> Self { self.with(|d| AsAiDatum(&d.datum).prove().is_ok()) }
    /// Goal: datum proves AiModel structural contract (hint present).
    pub fn proves_ai_model(self) -> Self { self.with(|d| AsAiModelDatum(&d.datum).prove().is_ok()) }
    /// Goal: datum proves Config structural contract (hint present).
    pub fn proves_config(self) -> Self { self.with(|d| AsConfigDatum(&d.datum).prove().is_ok()) }
    /// Goal: datum is Unknown — always passes.
    pub fn proves_unknown(self) -> Self { self.with(|d| AsUnknownDatum(&d.datum).prove().is_ok()) }
    /// Goal: datum passes prove_by_type() for its declared DatumType.
    pub fn proves_by_type(self) -> Self { self.with(|d| d.datum.prove_by_type().is_ok()) }

    // ── Structural filters ────────────────────────────────────────────────────

    /// Goal: datum has this key in its `depends_on` list.
    pub fn depends_on(self, dep: &'a str) -> Self {
        self.with(move |d| {
            d.datum.depends_on
                .as_ref()
                .map(|deps| deps.iter().any(|k| k == dep))
                .unwrap_or(false)
        })
    }

    /// Goal: datum has this `type_tag` (e.g. "transferable").
    pub fn with_type_tag(self, tag: &'a str) -> Self {
        self.with(move |d| d.datum.has_type_tag(tag))
    }

    /// Goal: datum's `datum_type` matches exactly.
    pub fn of_type(self, dt: DatumType) -> Self {
        self.with(move |d| d.datum.datum_type.as_ref() == Some(&dt))
    }

    /// Goal: key contains substring (useful for prefix/suffix match).
    pub fn key_contains(self, substr: &'a str) -> Self {
        self.with(move |d| d.key.contains(substr))
    }

    // ── Terminal operations ───────────────────────────────────────────────────

    /// Evaluate all goals; return matching `InternedDatum`s.
    #[must_use = "query result is unused"]
    pub fn run(self) -> Vec<InternedDatum> {
        self.store
            .iter()
            .filter(|d| self.predicates.iter().all(|p| p(d)))
            .collect()
    }

    /// Like `run()` but returns just keys, sorted.
    #[must_use = "query result is unused"]
    pub fn run_keys(self) -> Vec<Arc<str>> {
        let mut keys: Vec<Arc<str>> = self.run().into_iter().map(|d| d.key).collect();
        keys.sort();
        keys
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{BootDatum, DatumType};
    use std::collections::HashSet;

    fn make_store() -> HashMapStore {
        let mut store = HashMapStore::default();

        store.intern("git.cli", BootDatum {
            name: "git".to_string(),
            datum_type: Some(DatumType::Cli),
            hint: "Git VCS".to_string(),
            version: Some("git --version".to_string()),
            ..Default::default()
        });
        store.intern("kaizen.skill", BootDatum {
            name: "kaizen".to_string(),
            datum_type: Some(DatumType::Skill),
            hint: "Continuous improvement".to_string(),
            keywords: Some(vec!["improvement".to_string()]),
            type_tags: Some(vec!["transferable".to_string()]),
            ..Default::default()
        });
        store.intern("worker.role", BootDatum {
            name: "worker".to_string(),
            datum_type: Some(DatumType::Role),
            hint: "Default worker".to_string(),
            depends_on: Some(vec!["git.cli".to_string()]),
            ..Default::default()
        });
        store.intern("github.mcp", BootDatum {
            name: "github".to_string(),
            datum_type: Some(DatumType::Mcp),
            hint: "GitHub MCP".to_string(),
            command: Some("uvx".to_string()),
            ..Default::default()
        });
        store.intern("bare.cli", BootDatum {
            name: "bare".to_string(),
            datum_type: Some(DatumType::Cli),
            hint: "no install or version".to_string(),
            ..Default::default()
        });

        store
    }

    #[test]
    fn interned_equality_by_key() {
        let mut store = HashMapStore::default();
        let a = store.intern("git.cli", BootDatum { name: "git".to_string(), hint: "a".to_string(), ..Default::default() });
        let b = store.get("git.cli").unwrap();
        assert_eq!(a, b);
    }

    #[test]
    fn interned_different_keys_not_equal() {
        let mut store = HashMapStore::default();
        let a = store.intern("git.cli", BootDatum { name: "git".to_string(), hint: "a".to_string(), ..Default::default() });
        let b = store.intern("gh.cli", BootDatum { name: "gh".to_string(), hint: "b".to_string(), ..Default::default() });
        assert_ne!(a, b);
    }

    #[test]
    fn query_proves_cli_filters_bare() {
        let store = make_store();
        let keys = store.query().proves_cli().run_keys();
        // git.cli passes (has version); bare.cli fails (no install or version)
        assert!(keys.iter().any(|k| k.as_ref() == "git.cli"));
        assert!(!keys.iter().any(|k| k.as_ref() == "bare.cli"));
    }

    #[test]
    fn query_proves_skill() {
        let store = make_store();
        let keys = store.query().proves_skill().run_keys();
        assert!(keys.iter().any(|k| k.as_ref() == "kaizen.skill"));
        assert!(!keys.iter().any(|k| k.as_ref() == "git.cli"));
    }

    #[test]
    fn query_proves_role() {
        let store = make_store();
        let keys = store.query().proves_role().run_keys();
        assert!(keys.iter().any(|k| k.as_ref() == "worker.role"));
    }

    #[test]
    fn query_with_type_tag() {
        let store = make_store();
        let keys = store.query().with_type_tag("transferable").run_keys();
        assert_eq!(keys.len(), 1);
        assert_eq!(keys[0].as_ref(), "kaizen.skill");
    }

    #[test]
    fn query_depends_on() {
        let store = make_store();
        let keys = store.query().depends_on("git.cli").run_keys();
        assert!(keys.iter().any(|k| k.as_ref() == "worker.role"));
    }

    #[test]
    fn query_chain_proves_role_and_depends_on() {
        let store = make_store();
        let results = store.query().proves_role().depends_on("git.cli").run();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].key.as_ref(), "worker.role");
    }

    #[test]
    fn query_of_type_mcp() {
        let store = make_store();
        let keys = store.query().of_type(DatumType::Mcp).run_keys();
        assert!(keys.iter().any(|k| k.as_ref() == "github.mcp"));
        assert!(!keys.iter().any(|k| k.as_ref() == "git.cli"));
    }

    #[test]
    fn store_len() {
        let store = make_store();
        assert_eq!(store.len(), 5);
    }

    #[test]
    fn prove_by_type_dispatches() {
        let store = make_store();
        let git = store.get("git.cli").unwrap();
        assert!(git.prove_by_type().is_ok());
        let bare = store.get("bare.cli").unwrap();
        assert!(bare.prove_by_type().is_err());
    }

    #[test]
    fn from_path_loads_real_b00t_dir() {
        // Integration: load actual _b00t_/ directory; verify non-empty and at least one Cli proves.
        let b00t_dir = std::env::var("HOME")
            .map(|h| format!("{h}/.b00t/_b00t_"))
            .unwrap_or_else(|_| "/home/brianh/.b00t/_b00t_".to_string());

        if !std::path::Path::new(&b00t_dir).exists() {
            return; // skip on CI without _b00t_
        }

        let store = HashMapStore::from_path(&b00t_dir).expect("from_path failed");
        assert!(store.len() > 0, "expected at least one datum from {b00t_dir}");

        // At least one datum in the real directory should prove as Cli (git, just, etc.)
        let cli_count = store.query().proves_cli().run().len();
        assert!(cli_count > 0, "expected at least one provable Cli datum in real _b00t_");
    }

    #[test]
    fn from_path_nonexistent_yields_empty_store() {
        // get_all_datums returns Ok(empty) on missing path — store is empty, not Err.
        let store = HashMapStore::from_path("/tmp/b00t-nonexistent-XXXXXX-datum-store-test")
            .expect("from_path on nonexistent dir should not panic");
        assert!(store.is_empty(), "expected empty store for missing path");
    }

    // ── Coherence: validate_references ───────────────────────────────────────

    fn store_with_refs() -> HashMapStore {
        let mut s = HashMapStore::default();
        s.intern("git.cli", BootDatum { name: "git".to_string(), hint: "git".to_string(), ..Default::default() });
        s.intern("worker.role", BootDatum {
            name: "worker".to_string(), hint: "worker".to_string(),
            depends_on: Some(vec!["git.cli".to_string()]),
            ..Default::default()
        });
        s.intern("ml-stack.stack", BootDatum {
            name: "ml-stack".to_string(), hint: "ml".to_string(),
            members: Some(vec!["git.cli".to_string()]),
            ..Default::default()
        });
        s
    }

    #[test]
    fn validate_references_clean_store() {
        let store = store_with_refs();
        assert!(store.validate_references().is_empty(), "expected no errors for valid store");
    }

    #[test]
    fn validate_references_missing_dep() {
        let mut store = store_with_refs();
        store.intern("broken.role", BootDatum {
            name: "broken".to_string(), hint: "broken".to_string(),
            depends_on: Some(vec!["nonexistent.cli".to_string()]),
            ..Default::default()
        });
        let errs = store.validate_references();
        assert!(errs.iter().any(|e| matches!(e, ReferenceError::MissingDependency {
            datum, missing_key
        } if datum == "broken.role" && missing_key == "nonexistent.cli")));
    }

    #[test]
    fn validate_references_self_dep() {
        let mut store = store_with_refs();
        store.intern("self-loop.role", BootDatum {
            name: "self-loop".to_string(), hint: "loop".to_string(),
            depends_on: Some(vec!["self-loop.role".to_string()]),
            ..Default::default()
        });
        let errs = store.validate_references();
        assert!(errs.iter().any(|e| matches!(e, ReferenceError::SelfDependency { datum }
            if datum == "self-loop.role")));
    }

    #[test]
    fn validate_references_empty_dep() {
        let mut store = store_with_refs();
        store.intern("empty-dep.role", BootDatum {
            name: "empty-dep".to_string(), hint: "test".to_string(),
            depends_on: Some(vec!["".to_string()]),
            ..Default::default()
        });
        let errs = store.validate_references();
        assert!(errs.iter().any(|e| matches!(e, ReferenceError::EmptyDependency { datum }
            if datum == "empty-dep.role")));
    }

    #[test]
    fn validate_references_missing_member() {
        let mut store = store_with_refs();
        store.intern("broken-stack.stack", BootDatum {
            name: "broken-stack".to_string(), hint: "stack".to_string(),
            members: Some(vec!["phantom.cli".to_string()]),
            ..Default::default()
        });
        let errs = store.validate_references();
        assert!(errs.iter().any(|e| matches!(e, ReferenceError::MissingMember {
            stack, missing_member
        } if stack == "broken-stack.stack" && missing_member == "phantom.cli")));
    }

    #[test]
    fn validate_references_allowlist_suppresses_external() {
        let mut store = store_with_refs();
        store.intern("cross-store.role", BootDatum {
            name: "cross-store".to_string(), hint: "cross".to_string(),
            depends_on: Some(vec!["external.cli".to_string()]),
            ..Default::default()
        });
        let allowlist: HashSet<&str> = ["external.cli"].into();
        let errs = store.validate_references_with_allowlist(&allowlist);
        assert!(!errs.iter().any(|e| matches!(e, ReferenceError::MissingDependency {
            missing_key, ..
        } if missing_key == "external.cli")), "external.cli should be suppressed by allowlist");
    }

    // ── LearnResult / learn() ────────────────────────────────────────────────

    #[test]
    fn learn_found_for_valid_datum() {
        let store = make_store();
        assert!(matches!(store.learn("git.cli"), LearnResult::Found(_)));
    }

    #[test]
    fn learn_not_found_for_missing_key() {
        let store = make_store();
        assert!(matches!(store.learn("does-not-exist.cli"), LearnResult::NotFound));
    }

    #[test]
    fn learn_malformed_for_invalid_datum() {
        let store = make_store();
        // bare.cli has no install or version — fails AsCliDatum.prove()
        assert!(matches!(store.learn("bare.cli"), LearnResult::Malformed(..)));
    }

    #[test]
    fn learn_result_display() {
        let store = make_store();
        let found = store.learn("git.cli");
        assert!(found.to_string().starts_with("found:git.cli"));
        let not_found = store.learn("nope.cli");
        assert_eq!(not_found.to_string(), "not_found");
        let malformed = store.learn("bare.cli");
        assert!(malformed.to_string().starts_with("malformed:bare.cli"));
    }

    #[test]
    fn learn_result_helpers() {
        let store = make_store();
        assert!(store.learn("git.cli").is_found());
        assert!(!store.learn("git.cli").is_not_found());
        assert!(store.learn("nope.cli").is_not_found());
        assert!(store.learn("bare.cli").is_malformed());
        assert!(store.learn("git.cli").datum().is_some());
        assert!(store.learn("nope.cli").datum().is_none());
    }

    #[test]
    fn learn_result_store_error() {
        let err = LearnResult::StoreError("disk full".to_string());
        assert!(err.is_store_error());
        assert!(!err.is_found());
        assert!(err.datum().is_none());
        assert_eq!(err.to_string(), "store_error:disk full");
    }

    #[test]
    fn validate_references_integration_real_b00t() {
        let b00t_dir = std::env::var("HOME")
            .map(|h| format!("{h}/.b00t/_b00t_"))
            .unwrap_or_else(|_| "/home/brianh/.b00t/_b00t_".to_string());
        if !std::path::Path::new(&b00t_dir).exists() {
            return;
        }
        let store = HashMapStore::from_path(&b00t_dir).expect("from_path failed");
        // External deps are expected (datums from other dirs) — report but don't fail.
        let errs = store.validate_references();
        let self_deps: Vec<_> = errs.iter()
            .filter(|e| matches!(e, ReferenceError::SelfDependency { .. }))
            .collect();
        let empty_deps: Vec<_> = errs.iter()
            .filter(|e| matches!(e, ReferenceError::EmptyDependency { .. }))
            .collect();
        assert!(self_deps.is_empty(), "self-dependencies in real _b00t_: {self_deps:?}");
        assert!(empty_deps.is_empty(), "empty depends_on entries in real _b00t_: {empty_deps:?}");
    }
}
