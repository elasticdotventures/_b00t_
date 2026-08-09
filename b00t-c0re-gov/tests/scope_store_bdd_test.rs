//! BDD-style end-to-end test suite for ScopeStore (issue #893, §8 checklist
//! items: "BDD: discovery chain across >=2 providers in a synthetic
//! submodule tree" plus general end-to-end coverage of get/set, chain
//! traversal, credential guard, and audit logging).
//!
//! Exercises the full user-facing seam through a real backend
//! (`RedbScopeStore`, not the in-memory test double `scope_chain_view.rs`'s
//! own unit tests use) assembled into a `ScopeChainView`, end-to-end:
//! get/set, repo -> node -> global chain resolution, the credential guard
//! (#899), audit logging (#900), and discovery-chain traversal (#898) across
//! a synthetic submodule tree.
//!
//! This crate has no cucumber/gherkin dependency (checked: no BDD framework
//! is used anywhere else in this repo either), so "BDD-style" here follows
//! the project's existing convention (see `scope_chain_view.rs`'s own test
//! names): one `mod` per Feature, one `#[test]` per Scenario, each with
//! Given/When/Then comments -- structured narrative over plain `#[test]`,
//! not a literal feature-file framework.

use b00t_c0re_gov::discovery::walk_lazy_chain;
use b00t_c0re_gov::errors::ScopeError;
use b00t_c0re_gov::redb_scope_store::RedbScopeStore;
use b00t_c0re_gov::scope_audit::AuditLogger;
use b00t_c0re_gov::scope_chain_view::{Queryable, ScopeChainView};
use b00t_c0re_gov::scope_store::{ScopeId, ScopeStore};
use serde_json::json;
use tempfile::TempDir;

/// Test fixture: a real repo -> node -> global chain, each backed by its
/// own `RedbScopeStore` file inside a fresh tempdir -- mirrors how a real
/// caller assembles a chain (three independent files, not three in-memory
/// HashMaps), so this suite proves the actual backend interacts correctly
/// with `ScopeChainView`'s resolution/guard/audit logic, not just the
/// resolution algorithm in isolation.
struct Fixture {
    _dir: TempDir, // keeps the tempdir alive for the fixture's lifetime
}

impl Fixture {
    fn chain(repo: &str, node: &str) -> (Self, ScopeChainView) {
        let dir = TempDir::new().unwrap();
        let repo_store = RedbScopeStore::open(
            dir.path().join("repo.redb"),
            ScopeId::Repo(repo.to_string()),
            Some(ScopeId::Node(node.to_string())),
        )
        .unwrap();
        let node_store = RedbScopeStore::open(
            dir.path().join("node.redb"),
            ScopeId::Node(node.to_string()),
            Some(ScopeId::Global),
        )
        .unwrap();
        let global_store =
            RedbScopeStore::open(dir.path().join("global.redb"), ScopeId::Global, None).unwrap();

        let chain = ScopeChainView::new(vec![
            Box::new(repo_store) as Box<dyn ScopeStore>,
            Box::new(node_store),
            Box::new(global_store),
        ]);

        (Self { _dir: dir }, chain)
    }
}

// ---------------------------------------------------------------------
// Feature: get/set within a scope chain
// ---------------------------------------------------------------------
mod get_set {
    use super::*;

    #[test]
    fn scenario_set_then_get_round_trips_the_same_value() {
        // Given a fresh repo -> node -> global chain
        let (_fx, mut chain) = Fixture::chain("myrepo", "myhost");

        // When a value is set at an explicit scope
        chain
            .set_raw(&ScopeId::Global, "greeting", json!("hello"))
            .unwrap();

        // Then reading the same key returns exactly what was written
        assert_eq!(chain.get_raw("greeting").unwrap(), Some(json!("hello")));
    }

    #[test]
    fn scenario_get_on_a_never_set_key_returns_none_not_an_error() {
        // Given an empty chain
        let (_fx, chain) = Fixture::chain("myrepo", "myhost");

        // When reading a key nobody ever wrote
        let result = chain.get_raw("nope").unwrap();

        // Then the caller sees an absence, not an error
        assert_eq!(result, None);
    }

    #[test]
    fn scenario_set_persists_in_the_targeted_backend_file() {
        // Given a chain whose repo scope is backed by a real redb file
        let (_fx, mut chain) = Fixture::chain("myrepo", "myhost");

        // When a value is written to the repo scope specifically
        chain
            .set_raw(&ScopeId::Repo("myrepo".into()), "k", json!(42))
            .unwrap();

        // Then the resolving scope for that key is the repo scope, not a
        // fallback -- proving the write landed in the real backend file
        // ScopeChainView routed it to, not a shared in-memory map.
        assert_eq!(
            chain.resolving_scope("k").unwrap(),
            Some(&ScopeId::Repo("myrepo".into()))
        );
    }
}

// ---------------------------------------------------------------------
// Feature: repo -> node -> global chain resolution
// ---------------------------------------------------------------------
mod chain_traversal {
    use super::*;

    #[test]
    fn scenario_most_specific_scope_wins_when_all_three_have_the_key() {
        // Given all three scopes have set the same key to different values
        let (_fx, mut chain) = Fixture::chain("myrepo", "myhost");
        chain
            .set_raw(&ScopeId::Global, "k", json!("global"))
            .unwrap();
        chain
            .set_raw(&ScopeId::Node("myhost".into()), "k", json!("node"))
            .unwrap();
        chain
            .set_raw(&ScopeId::Repo("myrepo".into()), "k", json!("repo"))
            .unwrap();

        // When reading through the chain
        let value = chain.get_raw("k").unwrap();

        // Then the most-specific (repo) value wins
        assert_eq!(value, Some(json!("repo")));
    }

    #[test]
    fn scenario_falls_back_to_global_when_repo_and_node_are_silent() {
        // Given only the global scope has the key
        let (_fx, mut chain) = Fixture::chain("myrepo", "myhost");
        chain
            .set_raw(&ScopeId::Global, "k", json!("global-default"))
            .unwrap();

        // When reading through the full chain
        let value = chain.get_raw("k").unwrap();

        // Then resolution falls through to global
        assert_eq!(value, Some(json!("global-default")));
    }

    #[test]
    fn scenario_writes_never_implicitly_shadow_a_more_specific_scope() {
        // Given a value already set at the node scope
        let (_fx, mut chain) = Fixture::chain("myrepo", "myhost");
        chain
            .set_raw(&ScopeId::Node("myhost".into()), "k", json!("node-value"))
            .unwrap();

        // When a *different* write explicitly targets global (not repo/node)
        chain
            .set_raw(&ScopeId::Global, "k", json!("global-value"))
            .unwrap();

        // Then the node-scope value still wins on read -- global's write did
        // not silently clobber a more specific scope, because
        // ScopeChainView has no "write to whichever scope is closest"
        // method: the caller had to say exactly which scope to write.
        assert_eq!(chain.get_raw("k").unwrap(), Some(json!("node-value")));
    }

    #[test]
    fn scenario_writing_to_a_scope_outside_the_chain_is_rejected() {
        // Given a chain scoped to "myrepo"
        let (_fx, mut chain) = Fixture::chain("myrepo", "myhost");

        // When a caller tries to target a repo that isn't part of this chain
        let err = chain
            .set_raw(&ScopeId::Repo("some-other-repo".into()), "k", json!(1))
            .unwrap_err();

        // Then the write is rejected outright, not silently redirected
        assert!(matches!(err, ScopeError::InvalidScopeId(_)));
    }

    #[test]
    fn scenario_jsonpath_query_reaches_into_a_resolved_nested_document() {
        // Given a JSON document stored under one key
        let (_fx, mut chain) = Fixture::chain("myrepo", "myhost");
        chain
            .set_raw(
                &ScopeId::Global,
                "config",
                json!({"b00t": {"name": "widget", "tags": ["a", "b"]}}),
            )
            .unwrap();

        // When querying into it with a JSONPath expression
        let matches = chain.query("config", "$.b00t.name").unwrap();

        // Then the query resolves the key through the chain first, then
        // evaluates the path against the resolved value
        assert_eq!(matches, vec![json!("widget")]);
    }

    #[test]
    fn scenario_query_on_a_key_missing_everywhere_returns_empty_not_error() {
        // Given an empty chain
        let (_fx, chain) = Fixture::chain("myrepo", "myhost");

        // When querying a key that was never set anywhere
        let matches = chain.query("nope", "$.anything").unwrap();

        // Then the result is an empty match set, not an error
        assert_eq!(matches, Vec::<serde_json::Value>::new());
    }
}

// ---------------------------------------------------------------------
// Feature: credential guard (#899) -- rejected at every scope
// ---------------------------------------------------------------------
mod credential_guard {
    use super::*;

    #[test]
    fn scenario_credential_shaped_key_is_rejected_at_every_scope_in_the_chain() {
        // Given a chain with all three scopes available as write targets
        for target in [
            ScopeId::Repo("myrepo".into()),
            ScopeId::Node("myhost".into()),
            ScopeId::Global,
        ] {
            let (_fx, mut chain) = Fixture::chain("myrepo", "myhost");

            // When a caller tries to write a credential-shaped key to any of them
            let err = chain
                .set_raw(
                    &target,
                    "openai.credential",
                    json!("sk-should-never-land-here"),
                )
                .unwrap_err();

            // Then the write is rejected before it ever reaches the backend
            // -- "at every scope", not just repo-scope (#899's reframing).
            assert!(
                matches!(err, ScopeError::WriteRejected(_)),
                "expected WriteRejected for {target:?}, got {err:?}"
            );
        }
    }

    #[test]
    fn scenario_rejected_credential_write_never_touches_the_backend() {
        // Given a chain
        let (_fx, mut chain) = Fixture::chain("myrepo", "myhost");

        // When a credential-shaped write is attempted and rejected
        let _ = chain
            .set_raw(&ScopeId::Global, "aws.credentials", json!("secret"))
            .unwrap_err();

        // Then no value was actually persisted -- a read finds nothing
        assert_eq!(chain.get_raw("aws.credentials").unwrap(), None);
    }

    #[test]
    fn scenario_ordinary_keys_are_unaffected_by_the_guard() {
        // Given a chain
        let (_fx, mut chain) = Fixture::chain("myrepo", "myhost");

        // When writing a key that merely mentions credentials in its name,
        // not in the guarded ".credential"/".credentials" suffix shape
        chain
            .set_raw(&ScopeId::Global, "openai.cli", json!("gpt-5"))
            .unwrap();

        // Then it's written normally
        assert_eq!(chain.get_raw("openai.cli").unwrap(), Some(json!("gpt-5")));
    }
}

// ---------------------------------------------------------------------
// Feature: audit logging (#900) -- boundaries_crossed, not a boolean
// ---------------------------------------------------------------------
mod audit_logging {
    use super::*;

    #[test]
    fn scenario_audited_read_records_every_boundary_crossed_before_the_hit() {
        // Given a chain where only the global scope holds the key
        let (_fx, mut chain) = Fixture::chain("myrepo", "myhost");
        chain.set_raw(&ScopeId::Global, "k", json!("v")).unwrap();
        let audit_dir = TempDir::new().unwrap();
        let logger = AuditLogger::open(audit_dir.path().join("audit.jsonl"));

        // When reading through the chain with auditing enabled
        let result = chain.get_raw_with_audit("k", &logger).unwrap();

        // Then the value resolves correctly...
        assert_eq!(result, Some(json!("v")));

        // ...and the audit trail shows both boundaries crossed
        // (repo->node, node->global) on the way to the global hit.
        let events = logger.read_all().unwrap();
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].boundaries_crossed.len(), 2);
        assert_eq!(events[0].resolved_at, Some(ScopeId::Global));
    }

    #[test]
    fn scenario_audited_read_hit_on_the_most_specific_scope_crosses_nothing() {
        // Given a chain where the repo scope (most specific) has the key
        let (_fx, mut chain) = Fixture::chain("myrepo", "myhost");
        chain
            .set_raw(&ScopeId::Repo("myrepo".into()), "k", json!("v"))
            .unwrap();
        let audit_dir = TempDir::new().unwrap();
        let logger = AuditLogger::open(audit_dir.path().join("audit.jsonl"));

        // When reading with auditing enabled
        chain.get_raw_with_audit("k", &logger).unwrap();

        // Then zero boundaries were crossed, and that zero is itself
        // recorded -- not indistinguishable from "never checked" (#900).
        let events = logger.read_all().unwrap();
        assert_eq!(events.len(), 1);
        assert!(events[0].boundaries_crossed.is_empty());
        assert_eq!(
            events[0].resolved_at,
            Some(ScopeId::Repo("myrepo".into()))
        );
    }

    #[test]
    fn scenario_audit_log_survives_multiple_reads_across_reopened_loggers() {
        // Given a chain with a value set, and two separate reads
        let (_fx, mut chain) = Fixture::chain("myrepo", "myhost");
        chain
            .set_raw(&ScopeId::Node("myhost".into()), "k", json!("v"))
            .unwrap();
        let audit_dir = TempDir::new().unwrap();
        let audit_path = audit_dir.path().join("audit.jsonl");

        // When two reads happen through two separately-opened loggers
        // pointed at the same file (simulating two agent processes sharing
        // one scope root's audit log)
        {
            let logger = AuditLogger::open(&audit_path);
            chain.get_raw_with_audit("k", &logger).unwrap();
        }
        {
            let logger = AuditLogger::open(&audit_path);
            chain.get_raw_with_audit("missing-key", &logger).unwrap();
        }

        // Then both events are present, in order, in the shared append-only
        // log -- nothing was truncated or overwritten by the second open.
        let logger = AuditLogger::open(&audit_path);
        let events = logger.read_all().unwrap();
        assert_eq!(events.len(), 2);
        assert_eq!(events[0].key, "k");
        assert_eq!(events[1].key, "missing-key");
    }
}

// ---------------------------------------------------------------------
// Feature: discovery chain (#898) across a synthetic submodule tree --
// #893 §8's explicit outstanding checklist item: "BDD: discovery chain
// across >=2 providers in a synthetic submodule tree".
// ---------------------------------------------------------------------
mod discovery_chain {
    use super::*;
    use std::collections::HashMap;

    /// Synthetic submodule tree: each entry maps a repo id to the parent
    /// repo(s) its `$.b00t.manifest` publishes -- mirrors #893's "each
    /// discovery unlocks the next" walk, reusing `discovery::walk_lazy_chain`
    /// (the same generic walker `blessing --manifest` uses) rather than a
    /// second hand-rolled graph walk.
    fn submodule_manifest() -> HashMap<String, Vec<String>> {
        let mut m = HashMap::new();
        m.insert("app".to_string(), vec!["vendor-lib".to_string()]);
        m.insert("vendor-lib".to_string(), vec!["monorepo".to_string()]);
        m.insert("monorepo".to_string(), vec![]); // top of the submodule tree
        m
    }

    #[test]
    fn scenario_discovery_walk_orders_repos_most_specific_first() {
        // Given a synthetic 3-level submodule tree (app -> vendor-lib -> monorepo)
        let manifest = submodule_manifest();

        // When walking the discovery chain starting from the innermost
        // checked-out repo ("app")
        let order = walk_lazy_chain(["app".to_string()], 10, |id| {
            manifest.get(id).cloned().unwrap_or_default()
        });

        // Then discovery visits most-specific (app) first, walking outward
        // toward the submodule root -- exactly the order ScopeChainView
        // needs for most-specific-wins resolution.
        assert_eq!(
            order,
            vec![
                "app".to_string(),
                "vendor-lib".to_string(),
                "monorepo".to_string(),
            ]
        );
    }

    #[test]
    fn scenario_two_or_more_discovered_providers_assemble_into_a_working_chain() {
        // Given a discovery walk across >= 2 providers (app, vendor-lib,
        // monorepo -- three, comfortably over the >=2 bar from #893's own
        // checklist wording)
        let manifest = submodule_manifest();
        let order = walk_lazy_chain(["app".to_string()], 10, |id| {
            manifest.get(id).cloned().unwrap_or_default()
        });
        assert!(order.len() >= 2, "fixture must exercise >=2 providers");

        let dir = TempDir::new().unwrap();
        let mut stores: Vec<Box<dyn ScopeStore>> = Vec::new();
        for (i, repo_id) in order.iter().enumerate() {
            let parent = order.get(i + 1).map(|p| ScopeId::Repo(p.clone()));
            let store = RedbScopeStore::open(
                dir.path().join(format!("{repo_id}.redb")),
                ScopeId::Repo(repo_id.clone()),
                parent,
            )
            .unwrap();
            stores.push(Box::new(store));
        }
        let mut chain = ScopeChainView::new(stores);

        // When each discovered provider writes its own value for the same key
        for repo_id in &order {
            chain
                .set_raw(
                    &ScopeId::Repo(repo_id.clone()),
                    "shared-key",
                    json!(repo_id),
                )
                .unwrap();
        }

        // Then resolution honors discovery order: the innermost ("app")
        // provider wins, exactly like the non-discovery chain tests above.
        assert_eq!(chain.get_raw("shared-key").unwrap(), Some(json!("app")));
        assert_eq!(
            chain.resolving_scope("shared-key").unwrap(),
            Some(&ScopeId::Repo("app".to_string()))
        );
    }

    #[test]
    fn scenario_diamond_shaped_submodule_graph_visits_each_provider_once() {
        // Given a diamond: a leaf depends on two submodules that share a
        // common ancestor (a realistic shape -- two vendored libs sharing
        // one upstream submodule)
        let mut manifest = HashMap::new();
        manifest.insert(
            "leaf".to_string(),
            vec!["lib-a".to_string(), "lib-b".to_string()],
        );
        manifest.insert("lib-a".to_string(), vec!["shared-upstream".to_string()]);
        manifest.insert("lib-b".to_string(), vec!["shared-upstream".to_string()]);
        manifest.insert("shared-upstream".to_string(), vec![]);

        // When discovering from the leaf
        let order = walk_lazy_chain(["leaf".to_string()], 10, |id| {
            manifest.get(id).cloned().unwrap_or_default()
        });

        // Then the shared ancestor is discovered exactly once, not twice,
        // and every provider in the diamond appears (4 total: leaf, lib-a,
        // lib-b, shared-upstream) -- a real submodule graph can easily be a
        // diamond, and re-visiting shared-upstream twice would double-count
        // it in the eventual scope chain.
        assert_eq!(order.len(), 4);
        assert_eq!(
            order
                .iter()
                .filter(|id| id.as_str() == "shared-upstream")
                .count(),
            1
        );
    }
}
