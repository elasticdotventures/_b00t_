//! RedbScopeStore — first concrete ScopeStore backend (#893 checklist item).
//!
//! One redb::Database file per scope root, one fixed table of
//! JSON-serialized values keyed by string. redb is a pure-Rust embedded
//! DB (no server, no network) — the natural fit for repo/node-local
//! scopes; the eventual redis backend (#902's ADR territory) is for the
//! global/distributed case, not this one.

use crate::errors::{ScopeError, ScopeResult};
use crate::scope_store::{ScopeId, ScopeStore};
use redb::{Database, ReadableDatabase, TableDefinition};
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

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

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
}
