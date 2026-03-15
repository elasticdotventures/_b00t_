// b00t-cli/src/memory_provider.rs
use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;

/// Minimal memory provider trait — read/write/sync
/// Intentionally synchronous; async providers wrap with block_on at boundary.
/// 🤓 Interface mirrors moltis MemoryStore KV subset so MoltisMemory_🥾 shim
///    can delegate to any registered b00t provider without API drift.
pub trait MemoryProvider: Send + Sync {
    fn read(&self, key: &str) -> Result<Option<String>>;
    fn write(&self, key: &str, val: &str) -> Result<()>;
    fn delete(&self, key: &str) -> Result<()>;
    fn list_keys(&self, prefix: &str) -> Result<Vec<String>>;
    fn sync(&self) -> Result<()>;
}

// ─── File-backed (.tomllm) ────────────────────────────────────────────────────

/// File-backed soul memory — always available, zero deps.
/// Persists to ~/._b00t_/SOUL.tomllm with .tomllm header + b00t:map tail.
pub struct FileMemory {
    path: PathBuf,
}

impl FileMemory {
    pub fn new(path: PathBuf) -> Self {
        Self { path }
    }

    fn load_store(&self) -> Result<FileStore> {
        if !self.path.exists() {
            return Ok(FileStore::default());
        }
        let raw = std::fs::read_to_string(&self.path)?;
        let stripped: String = raw
            .lines()
            .filter(|l| !l.trim_start().starts_with('#'))
            .collect::<Vec<_>>()
            .join("\n");
        Ok(toml::from_str(&stripped).unwrap_or_default())
    }

    fn save_store(&self, store: &FileStore) -> Result<()> {
        if let Some(parent) = self.path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let toml_body = toml::to_string(store)?;
        let output = format!(
            "# b00t SOUL — agentic identity & persistent memory\n\
             # @tribal: soul persists across sessions; write via `b00t soul set`, never edit directly\n\
             \n\
             {toml_body}\n\
             # b00t:map v1\n\
             # summary: agent soul — accumulated identity, memory, lessons\n\
             # tags: soul, memory, identity, session\n\
             # tier: small\n"
        );
        std::fs::write(&self.path, output)?;
        Ok(())
    }
}

#[derive(Serialize, Deserialize, Default)]
struct FileStore {
    data: HashMap<String, String>,
}

impl MemoryProvider for FileMemory {
    fn read(&self, key: &str) -> Result<Option<String>> {
        Ok(self.load_store()?.data.get(key).cloned())
    }

    fn write(&self, key: &str, val: &str) -> Result<()> {
        let mut store = self.load_store()?;
        store.data.insert(key.to_string(), val.to_string());
        self.save_store(&store)
    }

    fn delete(&self, key: &str) -> Result<()> {
        let mut store = self.load_store()?;
        store.data.remove(key);
        self.save_store(&store)
    }

    fn list_keys(&self, prefix: &str) -> Result<Vec<String>> {
        let store = self.load_store()?;
        let keys = store
            .data
            .keys()
            .filter(|k| k.starts_with(prefix))
            .cloned()
            .collect();
        Ok(keys)
    }

    fn sync(&self) -> Result<()> {
        Ok(()) // local file — no-op
    }
}

// ─── SQLite-backed ────────────────────────────────────────────────────────────

/// SQLite-backed soul memory.
/// Provides durable K/V + richer query surface for future pgvector/neumann
/// integrations. Schema: `soul_kv(key TEXT PRIMARY KEY, value TEXT, updated_at INTEGER)`.
/// 🤓 This is the target backend; FileMemory is the fallback.
pub struct SqliteMemoryStore {
    db_path: PathBuf,
}

impl SqliteMemoryStore {
    pub fn new(db_path: PathBuf) -> Self {
        Self { db_path }
    }

    /// Default soul DB path: ~/._b00t_/soul.db
    pub fn default_path() -> PathBuf {
        dirs::home_dir()
            .unwrap_or_else(|| PathBuf::from("/tmp"))
            .join("._b00t_")
            .join("soul.db")
    }

    fn conn(&self) -> Result<rusqlite::Connection> {
        if let Some(parent) = self.db_path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let conn = rusqlite::Connection::open(&self.db_path)?;
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS soul_kv (
                key        TEXT PRIMARY KEY,
                value      TEXT NOT NULL,
                updated_at INTEGER NOT NULL DEFAULT (unixepoch())
             );",
        )?;
        Ok(conn)
    }
}

impl MemoryProvider for SqliteMemoryStore {
    fn read(&self, key: &str) -> Result<Option<String>> {
        let conn = self.conn()?;
        let mut stmt = conn.prepare("SELECT value FROM soul_kv WHERE key = ?1")?;
        let val = match stmt.query_row([key], |row| row.get::<_, String>(0)) {
            Ok(v) => Some(v),
            Err(rusqlite::Error::QueryReturnedNoRows) => None,
            Err(e) => return Err(e.into()),
        };
        Ok(val)
    }

    fn write(&self, key: &str, val: &str) -> Result<()> {
        let conn = self.conn()?;
        conn.execute(
            "INSERT INTO soul_kv(key, value, updated_at) VALUES(?1, ?2, unixepoch())
             ON CONFLICT(key) DO UPDATE SET value=excluded.value, updated_at=excluded.updated_at",
            rusqlite::params![key, val],
        )?;
        Ok(())
    }

    fn delete(&self, key: &str) -> Result<()> {
        let conn = self.conn()?;
        conn.execute("DELETE FROM soul_kv WHERE key = ?1", [key])?;
        Ok(())
    }

    fn list_keys(&self, prefix: &str) -> Result<Vec<String>> {
        let conn = self.conn()?;
        let pattern = format!("{prefix}%");
        let mut stmt = conn.prepare("SELECT key FROM soul_kv WHERE key LIKE ?1 ORDER BY key")?;
        let keys = stmt
            .query_map([pattern], |row| row.get(0))?
            .collect::<std::result::Result<Vec<String>, _>>()?;
        Ok(keys)
    }

    fn sync(&self) -> Result<()> {
        // SQLite writes are synchronous by default; WAL checkpoint on demand
        let conn = self.conn()?;
        conn.execute_batch("PRAGMA wal_checkpoint(PASSIVE);")?;
        Ok(())
    }
}

// ─── Path helpers ─────────────────────────────────────────────────────────────

/// Canonical SOUL file path: ~/._b00t_/SOUL.tomllm (FileMemory backend)
/// 🤓 ._b00t_ is the soul directory; separate from .b00t (runtime state)
pub fn soul_path() -> PathBuf {
    dirs::home_dir()
        .unwrap_or_else(|| PathBuf::from("/tmp"))
        .join("._b00t_")
        .join("SOUL.tomllm")
}

/// Detect best available memory provider.
/// Priority: SqliteMemoryStore > FileMemory (SOUL.tomllm)
/// Future slots: MoltisMemory_🥾 | RedisMemory | PgvectorMemory | NeumannMemory
pub fn detect_provider() -> Box<dyn MemoryProvider> {
    // Prefer SQLite — richer interface, better durability
    Box::new(SqliteMemoryStore::new(SqliteMemoryStore::default_path()))
}

/// Fallback file provider — used when SQLite unavailable or for SOUL.tomllm compat
pub fn file_provider() -> FileMemory {
    FileMemory::new(soul_path())
}

// ─── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_memory_provider_file_write_read() {
        let dir = tempfile::tempdir().unwrap();
        let mem = FileMemory::new(dir.path().join("mem.tomllm"));
        mem.write("key1", "val1").unwrap();
        assert_eq!(mem.read("key1").unwrap(), Some("val1".to_string()));
    }

    #[test]
    fn test_memory_provider_missing_key_returns_none() {
        let dir = tempfile::tempdir().unwrap();
        let mem = FileMemory::new(dir.path().join("mem.tomllm"));
        assert_eq!(mem.read("missing").unwrap(), None);
    }

    #[test]
    fn test_memory_provider_overwrite() {
        let dir = tempfile::tempdir().unwrap();
        let mem = FileMemory::new(dir.path().join("mem.tomllm"));
        mem.write("k", "v1").unwrap();
        mem.write("k", "v2").unwrap();
        assert_eq!(mem.read("k").unwrap(), Some("v2".to_string()));
    }

    #[test]
    fn test_memory_provider_delete() {
        let dir = tempfile::tempdir().unwrap();
        let mem = FileMemory::new(dir.path().join("mem.tomllm"));
        mem.write("k", "v").unwrap();
        mem.delete("k").unwrap();
        assert_eq!(mem.read("k").unwrap(), None);
    }

    #[test]
    fn test_memory_provider_list_keys_prefix() {
        let dir = tempfile::tempdir().unwrap();
        let mem = FileMemory::new(dir.path().join("mem.tomllm"));
        mem.write("agent.name", "claude").unwrap();
        mem.write("agent.role", "executive").unwrap();
        mem.write("session.id", "abc").unwrap();
        let keys = mem.list_keys("agent.").unwrap();
        assert!(keys.contains(&"agent.name".to_string()));
        assert!(keys.contains(&"agent.role".to_string()));
        assert!(!keys.contains(&"session.id".to_string()));
    }

    #[test]
    fn test_sqlite_write_read() {
        let dir = tempfile::tempdir().unwrap();
        let store = SqliteMemoryStore::new(dir.path().join("soul.db"));
        store.write("k", "v").unwrap();
        assert_eq!(store.read("k").unwrap(), Some("v".to_string()));
    }

    #[test]
    fn test_sqlite_delete() {
        let dir = tempfile::tempdir().unwrap();
        let store = SqliteMemoryStore::new(dir.path().join("soul.db"));
        store.write("k", "v").unwrap();
        store.delete("k").unwrap();
        assert_eq!(store.read("k").unwrap(), None);
    }

    #[test]
    fn test_sqlite_list_keys() {
        let dir = tempfile::tempdir().unwrap();
        let store = SqliteMemoryStore::new(dir.path().join("soul.db"));
        store.write("agent.name", "x").unwrap();
        store.write("agent.role", "y").unwrap();
        store.write("other", "z").unwrap();
        let keys = store.list_keys("agent.").unwrap();
        assert_eq!(keys.len(), 2);
    }

    #[test]
    fn test_detect_provider_no_panic() {
        let provider = detect_provider();
        provider.write("test", "value").unwrap();
        assert_eq!(provider.read("test").unwrap(), Some("value".to_string()));
    }

    #[test]
    fn test_is_copaw_available_no_panic() {
        let _ = is_copaw_available();
    }
}

/// Check if copaw is validated in session memory
pub fn is_copaw_available() -> bool {
    crate::session_memory::SessionMemory::load()
        .ok()
        .and_then(|s| s.get("tutorial.completed").cloned())
        .map(|c| c.split(',').any(|x| x == "copaw"))
        .unwrap_or(false)
}
