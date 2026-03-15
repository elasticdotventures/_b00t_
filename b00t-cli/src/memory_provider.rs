// b00t-cli/src/memory_provider.rs
use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;

/// Minimal memory provider trait — read/write/sync
pub trait MemoryProvider: Send + Sync {
    fn read(&self, key: &str) -> Result<Option<String>>;
    fn write(&self, key: &str, val: &str) -> Result<()>;
    fn sync(&self) -> Result<()>;
}

/// File-backed memory (always available, zero deps)
pub struct FileMemory {
    path: PathBuf,
}

impl FileMemory {
    pub fn new(path: PathBuf) -> Self {
        Self { path }
    }
}

#[derive(Serialize, Deserialize, Default)]
struct FileStore {
    data: HashMap<String, String>,
}

impl MemoryProvider for FileMemory {
    fn read(&self, key: &str) -> Result<Option<String>> {
        if !self.path.exists() {
            return Ok(None);
        }
        let raw = std::fs::read_to_string(&self.path)?;
        // Strip .tomllm comment lines before TOML parsing
        let content: String = raw
            .lines()
            .filter(|l| !l.trim_start().starts_with('#'))
            .collect::<Vec<_>>()
            .join("\n");
        let store: FileStore = toml::from_str(&content).unwrap_or_default();
        Ok(store.data.get(key).cloned())
    }

    fn write(&self, key: &str, val: &str) -> Result<()> {
        // Ensure parent dir exists (~/._b00t_/)
        if let Some(parent) = self.path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let mut store: FileStore = if self.path.exists() {
            let raw = std::fs::read_to_string(&self.path)?;
            let stripped: String = raw
                .lines()
                .filter(|l| !l.trim_start().starts_with('#'))
                .collect::<Vec<_>>()
                .join("\n");
            toml::from_str(&stripped).unwrap_or_default()
        } else {
            FileStore::default()
        };
        store.data.insert(key.to_string(), val.to_string());
        // Write with .tomllm header + b00t:map tail
        let toml_body = toml::to_string(&store)?;
        let output = format!(
            "# b00t SOUL — agentic identity & persistent memory\n\
             # @tribal: soul persists across sessions; write via `b00t soul set`, never edit directly\n\
             \n\
             {toml_body}\n\
             # b00t:map v1\n\
             # summary: agent soul — accumulated identity, memory, lessons\n\
             # tags: soul, memory, identity, session\n\
             # tier: sm0l\n"
        );
        std::fs::write(&self.path, output)?;
        Ok(())
    }

    fn sync(&self) -> Result<()> {
        Ok(()) // local file — no-op
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

/// Canonical SOUL file path: ~/._b00t_/SOUL.tomllm
/// 🤓 ._b00t_ (dot-underscore-b00t-underscore) is the soul directory — separate from .b00t (runtime)
pub fn soul_path() -> PathBuf {
    dirs::home_dir()
        .unwrap_or_else(|| PathBuf::from("/tmp"))
        .join("._b00t_")
        .join("SOUL.tomllm")
}

/// Detect best available memory provider (copaw > redis > file fallback)
pub fn detect_provider() -> Box<dyn MemoryProvider> {
    // Future: if is_copaw_available() { return Box::new(CopawMemory::new()); }
    // Future: if redis_ping_ok() { return Box::new(RedisMemory::new()); }

    // File fallback — soul file at ~/._b00t_/SOUL.tomllm
    Box::new(FileMemory::new(soul_path()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_memory_provider_file_write_read() {
        let dir = tempfile::tempdir().unwrap();
        let mem = FileMemory::new(dir.path().join("mem.toml"));
        mem.write("key1", "val1").unwrap();
        assert_eq!(mem.read("key1").unwrap(), Some("val1".to_string()));
    }

    #[test]
    fn test_memory_provider_missing_key_returns_none() {
        let dir = tempfile::tempdir().unwrap();
        let mem = FileMemory::new(dir.path().join("mem.toml"));
        assert_eq!(mem.read("missing").unwrap(), None);
    }

    #[test]
    fn test_memory_provider_overwrite() {
        let dir = tempfile::tempdir().unwrap();
        let mem = FileMemory::new(dir.path().join("mem.toml"));
        mem.write("k", "v1").unwrap();
        mem.write("k", "v2").unwrap();
        assert_eq!(mem.read("k").unwrap(), Some("v2".to_string()));
    }

    #[test]
    fn test_detect_provider_no_panic() {
        let provider = detect_provider();
        // File fallback always works
        provider.write("test", "value").unwrap();
        assert_eq!(provider.read("test").unwrap(), Some("value".to_string()));
    }

    #[test]
    fn test_is_copaw_available_no_panic() {
        // Result depends on session state; just verify no panic
        let _ = is_copaw_available();
    }
}
