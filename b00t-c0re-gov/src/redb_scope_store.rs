//! RedbScopeStore — first concrete ScopeStore backend (#893 checklist item).
//!
//! One redb::Database file per scope root, one fixed table of
//! JSON-serialized values keyed by string. redb is a pure-Rust embedded
//! DB (no server, no network) — the natural fit for repo/node-local
//! scopes; the eventual redis backend (#902's ADR territory) is for the
//! global/distributed case, not this one.

use crate::errors::{ScopeError, ScopeResult};
use crate::scope_store::{ScopeEnvelope, ScopeId, ScopeOp, ScopeOpResult, ScopeStore, TransactionalScopeStore};
use chrono::Utc;
use redb::{Database, ReadableDatabase, ReadableTable, Table, TableDefinition};
use serde_json::Value;
use std::path::Path;
use std::sync::Arc;

const KV_TABLE: TableDefinition<&str, &str> = TableDefinition::new("scope_kv");

/// A ScopeStore backed by a local redb database file.
pub struct RedbScopeStore {
    db: Arc<Database>,
    id: ScopeId,
    parent: Option<ScopeId>,
}

impl RedbScopeStore {
    /// Open (creating if absent) a redb database at `path` for the given
    /// scope identity.
    pub fn open(
        path: impl AsRef<Path>,
        id: ScopeId,
        parent: Option<ScopeId>,
    ) -> ScopeResult<Self> {
        let db = Database::create(path.as_ref())
            .map_err(|e| ScopeError::BackendUnavailable(e.to_string()))?;
        Ok(Self {
            db: Arc::new(db),
            id,
            parent,
        })
    }
}

fn read_envelope(
    table: &Table<&str, &str>,
    key: &str,
) -> ScopeResult<Option<ScopeEnvelope>> {
    let Some(guard) = table
        .get(key)
        .map_err(|e| ScopeError::BackendUnavailable(e.to_string()))?
    else {
        return Ok(None);
    };
    let env: ScopeEnvelope = serde_json::from_str(guard.value())?;
    Ok(Some(env))
}

impl ScopeStore for RedbScopeStore {
    fn get_raw(&self, key: &str) -> ScopeResult<Option<Value>> {
        let txn = self
            .db
            .begin_read()
            .map_err(|e| ScopeError::BackendUnavailable(e.to_string()))?;

        let table = match txn.open_table(KV_TABLE) {
            Ok(t) => t,
            // No writes have happened yet -- an empty scope, not an error.
            Err(redb::TableError::TableDoesNotExist(_)) => return Ok(None),
            Err(e) => return Err(ScopeError::BackendUnavailable(e.to_string())),
        };

        let Some(guard) = table
            .get(key)
            .map_err(|e| ScopeError::BackendUnavailable(e.to_string()))?
        else {
            return Ok(None);
        };

        let raw = guard.value();
        let value: Value = serde_json::from_str(raw)?;
        Ok(Some(value))
    }

    fn set_raw(&mut self, key: &str, val: Value) -> ScopeResult<()> {
        let serialized = serde_json::to_string(&val)?;

        let txn = self
            .db
            .begin_write()
            .map_err(|e| ScopeError::BackendUnavailable(e.to_string()))?;
        {
            let mut table = txn
                .open_table(KV_TABLE)
                .map_err(|e| ScopeError::BackendUnavailable(e.to_string()))?;
            table
                .insert(key, serialized.as_str())
                .map_err(|e| ScopeError::BackendUnavailable(e.to_string()))?;
        }
        txn.commit()
            .map_err(|e| ScopeError::BackendUnavailable(e.to_string()))?;
        Ok(())
    }

    fn scope_id(&self) -> &ScopeId {
        &self.id
    }

    fn parent(&self) -> Option<&ScopeId> {
        self.parent.as_ref()
    }
}

impl TransactionalScopeStore for RedbScopeStore {
    fn transaction(&mut self, ops: Vec<ScopeOp>) -> ScopeResult<Vec<ScopeOpResult>> {
        let now = Utc::now();
        let txn = self
            .db
            .begin_write()
            .map_err(|e| ScopeError::BackendUnavailable(e.to_string()))?;
        let mut results = Vec::with_capacity(ops.len());
        {
            let mut table: Table<&str, &str> = txn
                .open_table(KV_TABLE)
                .map_err(|e| ScopeError::BackendUnavailable(e.to_string()))?;

            // Pass 1: CAS pre-check — abort before any write on mismatch.
            for op in &ops {
                let (key, expect_gen) = match op {
                    ScopeOp::Put { key, expect_gen, .. } => (key, *expect_gen),
                    ScopeOp::Delete { key, expect_gen } => (key, *expect_gen),
                    ScopeOp::Get { .. } => continue,
                };
                if let Some(expected) = expect_gen {
                    let current_gen = read_envelope(&table, key)?.map(|e| e.generation).unwrap_or(0);
                    if current_gen != expected {
                        return Err(ScopeError::WriteRejected(format!(
                            "CAS mismatch on {key}: expected generation {expected}, found {current_gen}"
                        )));
                    }
                }
            }

            // Pass 2: apply.
            for op in ops {
                match op {
                    ScopeOp::Get { key } => {
                        let env = read_envelope(&table, &key)?.filter(|e| !e.is_expired(now));
                        results.push(ScopeOpResult::Value {
                            value: env.as_ref().map(|e| e.v.clone()),
                            generation: env.map(|e| e.generation).unwrap_or(0),
                        });
                    }
                    ScopeOp::Put { key, value, expires_at, .. } => {
                        let current_gen = read_envelope(&table, &key)?.map(|e| e.generation).unwrap_or(0);
                        let new_gen = current_gen + 1;
                        let env = ScopeEnvelope {
                            v: value,
                            generation: new_gen,
                            expires_at: expires_at.map(|e| e.timestamp()),
                        };
                        let serialized = serde_json::to_string(&env)?;
                        table
                            .insert(key.as_str(), serialized.as_str())
                            .map_err(|e| ScopeError::BackendUnavailable(e.to_string()))?;
                        results.push(ScopeOpResult::Written { generation: new_gen });
                    }
                    ScopeOp::Delete { key, .. } => {
                        table
                            .remove(key.as_str())
                            .map_err(|e| ScopeError::BackendUnavailable(e.to_string()))?;
                        results.push(ScopeOpResult::Deleted);
                    }
                }
            }
        }
        txn.commit()
            .map_err(|e| ScopeError::BackendUnavailable(e.to_string()))?;
        Ok(results)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;
    use crate::scope_store::{ScopeOp, ScopeOpResult, TransactionalScopeStore};
    use chrono::Duration;

    #[test]
    fn empty_scope_returns_none_not_error() {
        let dir = tempdir().unwrap();
        let store = RedbScopeStore::open(dir.path().join("scope.redb"), ScopeId::Global, None)
            .unwrap();
        assert_eq!(store.get_raw("nope").unwrap(), None);
    }

    #[test]
    fn get_set_round_trips() {
        let dir = tempdir().unwrap();
        let mut store = RedbScopeStore::open(
            dir.path().join("scope.redb"),
            ScopeId::Repo("abc".into()),
            Some(ScopeId::Node("host1".into())),
        )
        .unwrap();

        store
            .set_raw("greeting", Value::String("hello".into()))
            .unwrap();
        assert_eq!(
            store.get_raw("greeting").unwrap(),
            Some(Value::String("hello".into()))
        );
    }

    #[test]
    fn overwrite_replaces_value() {
        let dir = tempdir().unwrap();
        let mut store =
            RedbScopeStore::open(dir.path().join("scope.redb"), ScopeId::Global, None).unwrap();

        store.set_raw("k", Value::from(1)).unwrap();
        store.set_raw("k", Value::from(2)).unwrap();
        assert_eq!(store.get_raw("k").unwrap(), Some(Value::from(2)));
    }

    #[test]
    fn persists_across_reopen() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("scope.redb");

        {
            let mut store =
                RedbScopeStore::open(&path, ScopeId::Global, None).unwrap();
            store
                .set_raw("durable", Value::String("yes".into()))
                .unwrap();
        }
        // store dropped, database file closed

        let store = RedbScopeStore::open(&path, ScopeId::Global, None).unwrap();
        assert_eq!(
            store.get_raw("durable").unwrap(),
            Some(Value::String("yes".into()))
        );
    }

    #[test]
    fn distinct_scopes_do_not_share_data() {
        let dir = tempdir().unwrap();
        let mut a = RedbScopeStore::open(
            dir.path().join("a.redb"),
            ScopeId::Repo("a".into()),
            None,
        )
        .unwrap();
        let mut b = RedbScopeStore::open(
            dir.path().join("b.redb"),
            ScopeId::Repo("b".into()),
            None,
        )
        .unwrap();

        a.set_raw("k", Value::String("a-value".into())).unwrap();
        b.set_raw("k", Value::String("b-value".into())).unwrap();

        assert_eq!(a.get_raw("k").unwrap(), Some(Value::String("a-value".into())));
        assert_eq!(b.get_raw("k").unwrap(), Some(Value::String("b-value".into())));
    }

    #[test]
    fn scope_identity_preserved() {
        let dir = tempdir().unwrap();
        let store = RedbScopeStore::open(
            dir.path().join("scope.redb"),
            ScopeId::Repo("myrepo".into()),
            Some(ScopeId::Node("myhost".into())),
        )
        .unwrap();
        assert_eq!(store.scope_id(), &ScopeId::Repo("myrepo".into()));
        assert_eq!(store.parent(), Some(&ScopeId::Node("myhost".into())));
    }

    #[test]
    fn transaction_put_then_get_round_trips_with_generation() {
        let dir = tempdir().unwrap();
        let mut store =
            RedbScopeStore::open(dir.path().join("scope.redb"), ScopeId::Global, None).unwrap();

        let results = store
            .transaction(vec![ScopeOp::Put {
                key: "k".into(),
                value: Value::String("v1".into()),
                expect_gen: None,
                expires_at: None,
            }])
            .unwrap();
        assert_eq!(results, vec![ScopeOpResult::Written { generation: 1 }]);

        let results = store.transaction(vec![ScopeOp::Get { key: "k".into() }]).unwrap();
        assert_eq!(
            results,
            vec![ScopeOpResult::Value { value: Some(Value::String("v1".into())), generation: 1 }]
        );
    }

    #[test]
    fn transaction_cas_mismatch_rejects_whole_batch_and_persists_nothing() {
        let dir = tempdir().unwrap();
        let mut store =
            RedbScopeStore::open(dir.path().join("scope.redb"), ScopeId::Global, None).unwrap();
        store
            .transaction(vec![ScopeOp::Put {
                key: "k".into(),
                value: Value::String("v1".into()),
                expect_gen: None,
                expires_at: None,
            }])
            .unwrap();

        let err = store
            .transaction(vec![
                ScopeOp::Put {
                    key: "k".into(),
                    value: Value::String("v2".into()),
                    expect_gen: Some(99),
                    expires_at: None,
                },
                ScopeOp::Put {
                    key: "other".into(),
                    value: Value::String("should-not-land".into()),
                    expect_gen: None,
                    expires_at: None,
                },
            ])
            .unwrap_err();
        assert!(matches!(err, ScopeError::WriteRejected(_)));
        assert_eq!(store.get_raw("other").unwrap(), None);
        let results = store.transaction(vec![ScopeOp::Get { key: "k".into() }]).unwrap();
        assert_eq!(
            results,
            vec![ScopeOpResult::Value { value: Some(Value::String("v1".into())), generation: 1 }]
        );
    }

    #[test]
    fn transaction_expired_value_reads_as_absent() {
        let dir = tempdir().unwrap();
        let mut store =
            RedbScopeStore::open(dir.path().join("scope.redb"), ScopeId::Global, None).unwrap();
        store
            .transaction(vec![ScopeOp::Put {
                key: "k".into(),
                value: Value::String("v1".into()),
                expect_gen: None,
                expires_at: Some(Utc::now() - Duration::seconds(1)),
            }])
            .unwrap();

        let results = store.transaction(vec![ScopeOp::Get { key: "k".into() }]).unwrap();
        assert_eq!(results, vec![ScopeOpResult::Value { value: None, generation: 0 }]);
    }

    #[test]
    fn transaction_delete_removes_key() {
        let dir = tempdir().unwrap();
        let mut store =
            RedbScopeStore::open(dir.path().join("scope.redb"), ScopeId::Global, None).unwrap();
        store
            .transaction(vec![ScopeOp::Put {
                key: "k".into(),
                value: Value::String("v1".into()),
                expect_gen: None,
                expires_at: None,
            }])
            .unwrap();
        let results = store
            .transaction(vec![ScopeOp::Delete { key: "k".into(), expect_gen: None }])
            .unwrap();
        assert_eq!(results, vec![ScopeOpResult::Deleted]);
        assert_eq!(store.get_raw("k").unwrap(), None);
    }
}
