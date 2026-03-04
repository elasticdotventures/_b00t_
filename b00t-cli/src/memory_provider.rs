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
        let content = std::fs::read_to_string(&self.path)?;
        let store: FileStore = toml::from_str(&content).unwrap_or_default();
        Ok(store.data.get(key).cloned())
    }

    fn write(&self, key: &str, val: &str) -> Result<()> {
        let mut store: FileStore = if self.path.exists() {
            toml::from_str(&std::fs::read_to_string(&self.path)?).unwrap_or_default()
        } else {
            FileStore::default()
        };
        store.data.insert(key.to_string(), val.to_string());
        std::fs::write(&self.path, toml::to_string(&store)?)?;
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

/// Detect best available memory provider (copaw > redis > file fallback)
pub fn detect_provider() -> Box<dyn MemoryProvider> {
    // Future: if is_copaw_available() { return Box::new(CopawMemory::new()); }
    // Future: if redis_ping_ok() { return Box::new(RedisMemory::new()); }

    // File fallback — always available
    let path = dirs::home_dir()
        .unwrap_or_else(|| PathBuf::from("/tmp"))
        .join(".b00t")
        .join("memory.toml");
    Box::new(FileMemory::new(path))
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
