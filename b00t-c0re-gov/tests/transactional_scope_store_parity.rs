//! Cross-backend parity for `TransactionalScopeStore` — proves RedbScopeStore
//! and RedisScopeStore agree on transaction outcomes, per ADR #902
//! (docs/architecture/SCOPESTORE_CONCURRENCY_ADR.md) and issue #897.

use b00t_c0re_gov::errors::ScopeError;
use b00t_c0re_gov::redb_scope_store::RedbScopeStore;
use b00t_c0re_gov::redis_scope_store::RedisScopeStore;
use b00t_c0re_gov::scope_store::{ScopeId, ScopeOp, ScopeOpResult, TransactionalScopeStore};
use b00t_c0re_lib::redis::RedisConfig;
use serde_json::Value;

fn cas_mismatch_rejects_whole_batch(store: &mut dyn TransactionalScopeStore) {
    store
        .transaction(vec![ScopeOp::Put {
            key: "parity:k".into(),
            value: Value::String("v1".into()),
            expect_gen: None,
            expires_at: None,
        }])
        .unwrap();

    let err = store
        .transaction(vec![
            ScopeOp::Put {
                key: "parity:k".into(),
                value: Value::String("v2".into()),
                expect_gen: Some(99),
                expires_at: None,
            },
            ScopeOp::Put {
                key: "parity:other".into(),
                value: Value::String("should-not-land".into()),
                expect_gen: None,
                expires_at: None,
            },
        ])
        .unwrap_err();

    assert!(
        matches!(err, ScopeError::WriteRejected(_)),
        "expected WriteRejected on CAS mismatch, got: {err:?}"
    );
    assert_eq!(
        store.get_raw("parity:other").unwrap(),
        None,
        "second op in a rejected batch must not have landed"
    );
}

fn successful_cas_advances_generation(store: &mut dyn TransactionalScopeStore) {
    let r1 = store
        .transaction(vec![ScopeOp::Put {
            key: "parity:generation".into(),
            value: Value::String("v1".into()),
            expect_gen: None,
            expires_at: None,
        }])
        .unwrap();
    assert_eq!(r1, vec![ScopeOpResult::Written { generation: 1 }]);

    let r2 = store
        .transaction(vec![ScopeOp::Put {
            key: "parity:generation".into(),
            value: Value::String("v2".into()),
            expect_gen: Some(1),
            expires_at: None,
        }])
        .unwrap();
    assert_eq!(r2, vec![ScopeOpResult::Written { generation: 2 }]);
}

#[test]
fn redb_cas_mismatch_rejects_whole_batch() {
    let dir = tempfile::tempdir().unwrap();
    let mut store =
        RedbScopeStore::open(dir.path().join("scope.redb"), ScopeId::Global, None).unwrap();
    cas_mismatch_rejects_whole_batch(&mut store);
}

#[test]
fn redb_successful_cas_advances_generation() {
    let dir = tempfile::tempdir().unwrap();
    let mut store =
        RedbScopeStore::open(dir.path().join("scope.redb"), ScopeId::Global, None).unwrap();
    successful_cas_advances_generation(&mut store);
}

#[test]
fn redis_cas_mismatch_rejects_whole_batch_when_available() {
    let mut store = RedisScopeStore::open(RedisConfig::default(), ScopeId::Global, None).unwrap();
    if !store.is_available() {
        eprintln!("skipping: no Redis reachable in this environment");
        return;
    }
    cas_mismatch_rejects_whole_batch(&mut store);
}

#[test]
fn redis_successful_cas_advances_generation_when_available() {
    let mut store = RedisScopeStore::open(RedisConfig::default(), ScopeId::Global, None).unwrap();
    if !store.is_available() {
        eprintln!("skipping: no Redis reachable in this environment");
        return;
    }
    successful_cas_advances_generation(&mut store);
}
