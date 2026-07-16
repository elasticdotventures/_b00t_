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
        let stripped = strip_tomllm_comments(&raw);
        Ok(toml::from_str(&stripped).unwrap_or_default())
    }

    fn save_store(&self, store: &FileStore) -> Result<()> {
        if let Some(parent) = self.path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let mut doc = if self.path.exists() {
            let raw = std::fs::read_to_string(&self.path)?;
            let stripped = strip_tomllm_comments(&raw);
            stripped.parse::<toml::Table>().unwrap_or_default()
        } else {
            toml::Table::new()
        };
        let data = toml::Value::try_from(&store.data)?;
        doc.insert("data".to_string(), data);
        let toml_body = toml::to_string_pretty(&doc)?;
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

fn strip_tomllm_comments(raw: &str) -> String {
    raw.lines()
        .filter(|l| !l.trim_start().starts_with('#'))
        .collect::<Vec<_>>()
        .join("\n")
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
///
/// Priority:
/// 1. `<cwd>/._b00t_/soul.db`  — repo-local soul (if `._b00t_/` dir exists)
/// 2. `~/._b00t_/soul.db`       — global soul (default)
///
/// Future slots: MoltisMemory_🥾 | RedisMemory | PgvectorMemory | NeumannMemory
pub fn detect_provider() -> Box<dyn MemoryProvider> {
    Box::new(SqliteMemoryStore::new(active_soul_db_path()))
}

/// Active soul DB path — local workspace if `._b00t_/` exists, else global.
pub fn active_soul_db_path() -> PathBuf {
    std::env::current_dir()
        .ok()
        .map(|d| d.join("._b00t_"))
        .filter(|p| p.is_dir())
        .map(|d| d.join("soul.db"))
        .unwrap_or_else(SqliteMemoryStore::default_path)
}

/// Active SOUL.tomllm path — local workspace if `._b00t_/` exists, else global.
pub fn active_soul_path() -> PathBuf {
    std::env::current_dir()
        .ok()
        .map(|d| d.join("._b00t_"))
        .filter(|p| p.is_dir())
        .map(|d| d.join("SOUL.tomllm"))
        .unwrap_or_else(soul_path)
}

/// Fallback file provider — used when SQLite unavailable or for SOUL.tomllm compat
pub fn file_provider() -> FileMemory {
    FileMemory::new(soul_path())
}

// ─── Node identity summary (context-efficient) ────────────────────────────────

/// Compose a compressed one-line node identity from soul `node.*` keys.
///
/// Emits only the highest-signal facts (board · soc arch | ram | NPU | GPU | os).
/// Returns `None` when no node identity is recorded, so callers (e.g. `whoami`)
/// leave their output unchanged on nodes without soul node data.
///
/// 🤓 context-efficiency: agents run `b00t soul get node.npu` for full detail
///    instead of receiving a full key dump in every whoami.
pub fn compose_node_summary(mem: &dyn MemoryProvider) -> Option<String> {
    let get = |k: &str| mem.read(k).ok().flatten();
    // board is the anchor — without it there is no node identity to summarise.
    let board = get("node.board")?;
    let soc = get("node.soc");
    let arch = get("node.arch");
    let ram = get("node.ram_gb");
    let npu = get("node.npu");
    let gpu = get("node.gpu");
    let os = get("node.os");

    let mut head = board;
    if let Some(s) = &soc {
        head.push_str(&format!(" · {s}"));
    }
    if let Some(a) = &arch {
        head.push_str(&format!(" {a}"));
    }
    if let Some(r) = &ram {
        head.push_str(&format!(" | {r}GB"));
    }
    if let Some(n) = &npu {
        head.push_str(&format!(" | NPU: {}", shorten_accel(n)));
    }
    if let Some(g) = &gpu {
        head.push_str(&format!(" | GPU: {}", shorten_accel(g)));
    }
    if let Some(o) = &os {
        head.push_str(&format!(" | {o}"));
    }
    Some(head)
}

/// Read the global SOUL.tomllm and return the node summary, if recorded.
pub fn node_summary_from_soul() -> Option<String> {
    compose_node_summary(&file_provider())
}

/// Shorten an accelerator description for a one-line summary.
/// "RKNPU2 2.3.0 + rknn_server + python3-rknnlite2" → "RKNPU2 2.3.0"
fn shorten_accel(s: &str) -> String {
    s.split('+').next().unwrap_or(s).trim().to_string()
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
    fn test_file_memory_write_preserves_dataframerr_registry() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("SOUL.tomllm");
        std::fs::write(
            &path,
            r#"
[data]
existing = "keep"

[soul.tables.repro_t]
name = "repro_t"
next_id = 2

[[soul.tables.repro_t.columns]]
name = "a"
type = "text"
nullable = false

[[soul.tables.repro_t.rows]]
id = 1

[soul.tables.repro_t.rows.fields]
a = { Text = "row1" }

[soul.cursors.repro_cursor]
table = "repro_t"
next_id = 1
"#,
        )
        .unwrap();

        let mem = FileMemory::new(path.clone());
        mem.write("fresh", "value").unwrap();

        let raw = std::fs::read_to_string(path).unwrap();
        let doc: toml::Table = strip_tomllm_comments(&raw).parse().unwrap();
        assert_eq!(
            doc.get("data")
                .and_then(|v| v.as_table())
                .and_then(|t| t.get("fresh"))
                .and_then(|v| v.as_str()),
            Some("value")
        );
        assert!(doc.contains_key("soul"), "DataFramerr registry was dropped");
        assert_eq!(
            doc.get("soul")
                .and_then(|v| v.get("tables"))
                .and_then(|v| v.get("repro_t"))
                .and_then(|v| v.get("name"))
                .and_then(|v| v.as_str()),
            Some("repro_t")
        );
        assert_eq!(
            doc.get("soul")
                .and_then(|v| v.get("cursors"))
                .and_then(|v| v.get("repro_cursor"))
                .and_then(|v| v.get("table"))
                .and_then(|v| v.as_str()),
            Some("repro_t")
        );
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

    // ─── node summary composition ──────────────────────────────────────────────

    #[test]
    fn test_node_summary_full() {
        let dir = tempfile::tempdir().unwrap();
        let mem = FileMemory::new(dir.path().join("SOUL.tomllm"));
        for (k, v) in [
            ("node.board", "rock-5c"),
            ("node.soc", "rk3588"),
            ("node.arch", "aarch64"),
            ("node.ram_gb", "16"),
            ("node.npu", "RKNPU2 2.3.0 + rknn_server + python3-rknnlite2"),
            ("node.gpu", "Mali (/dev/mali0, dri renderD128/129)"),
            ("node.os", "armbian-25.11.2 (ubuntu-24.04 noble)"),
        ] {
            mem.write(k, v).unwrap();
        }
        let s = compose_node_summary(&mem).unwrap();
        // highest-signal facts present, accelerator descriptions shortened
        assert!(s.starts_with("rock-5c · rk3588 aarch64 | 16GB"));
        assert!(s.contains("NPU: RKNPU2 2.3.0"));
        assert!(s.contains("GPU: Mali (/dev/mali0, dri renderD128/129)"));
        assert!(s.contains("armbian-25.11.2"));
        // the trailing tail of the NPU value must NOT leak unshortened
        assert!(!s.contains("rknn_server"));
    }

    #[test]
    fn test_node_summary_none_without_board() {
        let dir = tempfile::tempdir().unwrap();
        let mem = FileMemory::new(dir.path().join("SOUL.tomllm"));
        // no node.board → None (whoami output unchanged)
        mem.write("node.soc", "rk3588").unwrap();
        assert!(compose_node_summary(&mem).is_none());
    }

    #[test]
    fn test_node_summary_minimal() {
        let dir = tempfile::tempdir().unwrap();
        let mem = FileMemory::new(dir.path().join("SOUL.tomllm"));
        mem.write("node.board", "pi5").unwrap();
        // only board present → just the board, no separators
        assert_eq!(compose_node_summary(&mem).as_deref(), Some("pi5"));
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
