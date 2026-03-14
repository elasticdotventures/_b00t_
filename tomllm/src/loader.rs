//! Idiomatic `.tomllm`-first file loader.
//!
//! `.tomllm` is a superset of `.toml` — always prefer it, fall back to `.toml`.
//! This module eliminates the `.tomllm`/`.toml` boilerplate from every call site.
//!
//! ## Usage
//! ```rust,ignore
//! // Instead of:
//! //   try "foo.role.tomllm", then "foo.role.toml", then "foo.toml" ...
//! // Just:
//! let doc = tomllm::loader::load_first("~/.b00t/_b00t_", "executive", ".role")?;
//!
//! // Or deserialize directly into your type:
//! let cfg: MyConfig = tomllm::loader::load_typed("~/.b00t/_b00t_", "executive", ".role")?;
//! ```

use std::path::{Path, PathBuf};

use crate::{Result, TomllmError};

/// Resolve the first existing file for `{dir}/{name}{base}.tomllm` or `{dir}/{name}{base}.toml`.
/// 🤓 .tomllm always wins — richer tribal context; .toml is the legacy fallback.
pub fn resolve_path(dir: impl AsRef<Path>, name: &str, base: &str) -> Option<PathBuf> {
    let dir = dir.as_ref();
    for ext in [".tomllm", ".toml"] {
        let path = dir.join(format!("{}{}{}", name, base, ext));
        if path.exists() {
            return Some(path);
        }
    }
    None
}

/// Read the first existing `.tomllm`-or-`.toml` file as a raw string.
pub fn load_first(dir: impl AsRef<Path>, name: &str, base: &str) -> Result<(String, PathBuf)> {
    let path = resolve_path(dir, name, base)
        .ok_or_else(|| TomllmError::Io(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            format!("no {}{}.tomllm or {}{}.toml found", name, base, name, base),
        )))?;
    let content = std::fs::read_to_string(&path)?;
    Ok((content, path))
}

/// Deserialize `{dir}/{name}{base}.(tomllm|toml)` into `T`.
/// Tries `.tomllm` first; `.toml` is the fallback.
pub fn load_typed<T: serde::de::DeserializeOwned>(
    dir: impl AsRef<Path>,
    name: &str,
    base: &str,
) -> Result<T> {
    let (content, _) = load_first(dir, name, base)?;
    let value: T = toml::from_str(&content)?;
    Ok(value)
}

/// Try multiple bases in order, returning the first match.
/// 🤓 Use when you don't know the datum type ahead of time.
///    Mirrors `get_config` base resolution order from b00t-cli.
pub fn load_any_typed<T: serde::de::DeserializeOwned>(
    dir: impl AsRef<Path>,
    name: &str,
    bases: &[&str],
) -> Result<T> {
    let dir = dir.as_ref();
    for base in bases {
        if let Some(path) = resolve_path(dir, name, base) {
            let content = std::fs::read_to_string(&path)?;
            if let Ok(value) = toml::from_str::<T>(&content) {
                return Ok(value);
            }
        }
    }
    Err(TomllmError::Io(std::io::Error::new(
        std::io::ErrorKind::NotFound,
        format!("'{}' not found with any of {:?}", name, bases),
    )))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;
    use serde::Deserialize;

    #[derive(Deserialize, PartialEq, Debug)]
    struct Cfg { value: String }

    #[test]
    fn test_resolve_prefers_tomllm() {
        let dir = TempDir::new().unwrap();
        fs::write(dir.path().join("foo.role.toml"), "value = \"toml\"").unwrap();
        fs::write(dir.path().join("foo.role.tomllm"), "value = \"tomllm\"").unwrap();
        let path = resolve_path(dir.path(), "foo", ".role").unwrap();
        assert!(path.to_str().unwrap().ends_with(".tomllm"));
    }

    #[test]
    fn test_resolve_falls_back_to_toml() {
        let dir = TempDir::new().unwrap();
        fs::write(dir.path().join("foo.role.toml"), "value = \"toml\"").unwrap();
        let path = resolve_path(dir.path(), "foo", ".role").unwrap();
        assert!(path.to_str().unwrap().ends_with(".toml"));
    }

    #[test]
    fn test_load_typed() {
        let dir = TempDir::new().unwrap();
        fs::write(dir.path().join("bar.cli.tomllm"), "# @tribal: test\nvalue = \"ok\"").unwrap();
        let cfg: Cfg = load_typed(dir.path(), "bar", ".cli").unwrap();
        assert_eq!(cfg.value, "ok");
    }

    #[test]
    fn test_load_any_typed_first_match() {
        let dir = TempDir::new().unwrap();
        fs::write(dir.path().join("x.agent.toml"), "value = \"agent\"").unwrap();
        let cfg: Cfg = load_any_typed(dir.path(), "x", &[".role", ".agent", ".mcp"]).unwrap();
        assert_eq!(cfg.value, "agent");
    }
}
