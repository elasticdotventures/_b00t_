//! `SoulMemoryWriter` — structured memory write with path validation & size limits.
//!
//! b00t-native analogue of moltis `MemoryWriter` trait.
//! Writes to `~/._b00t_/` (global) or `<repo>/._b00t_/` (local workspace) only.
//!
//! Interface mirrors moltis `memory_writer::MemoryWriter` so `B00tSoulWriter`
//! in moltis-b00t can implement that trait by delegating here via HTTP.
//!
//! # b00t:map v1
//! # summary: SoulMemoryWriter — validated markdown appender for SOUL workspace files
//! # tags: soul, memory, writer, markdown, workspace
//! # tier: sm0l

use std::path::{Component, Path, PathBuf};

use anyhow::{bail, Context as _, Result};

/// Default max bytes per soul memory file (64 KiB).
pub const SOUL_FILE_MAX_BYTES: usize = 64 * 1024;

/// Result of a successful memory write.
#[derive(Debug)]
pub struct SoulWriteResult {
    /// Resolved absolute path of the written file.
    pub location: String,
    /// Total bytes in the file after write.
    pub bytes_written: usize,
}

/// Write content to soul workspace markdown files with path validation.
///
/// Allowed base directories (checked in order):
/// 1. `<cwd>/._b00t_/`  — repo-local soul (if present)
/// 2. `~/._b00t_/`       — global soul (fallback)
///
/// Only relative paths within the chosen base are accepted (no `..` escape).
pub trait SoulMemoryWriter: Send + Sync {
    /// Write or append `content` to `file` (relative path like `"SOUL.md"`).
    fn write_memory(&self, file: &str, content: &str, append: bool) -> Result<SoulWriteResult>;
}

// ─── File-backed implementation ───────────────────────────────────────────────

/// File-backed `SoulMemoryWriter` — writes to `._b00t_/` directories.
pub struct FileSoulWriter {
    base: PathBuf,
    max_bytes: usize,
}

impl FileSoulWriter {
    /// Create a writer rooted at `base` (must be an `._b00t_/` directory).
    pub fn new(base: PathBuf) -> Self {
        Self {
            base,
            max_bytes: SOUL_FILE_MAX_BYTES,
        }
    }

    /// Auto-detect the best soul base: local `._b00t_/` if it exists, else global.
    pub fn detect() -> Self {
        let local = std::env::current_dir()
            .ok()
            .map(|d| d.join("._b00t_"))
            .filter(|p| p.is_dir());
        let base = local.unwrap_or_else(global_soul_dir);
        Self::new(base)
    }

    pub fn with_max_bytes(mut self, max: usize) -> Self {
        self.max_bytes = max;
        self
    }
}

impl SoulMemoryWriter for FileSoulWriter {
    fn write_memory(&self, file: &str, content: &str, append: bool) -> Result<SoulWriteResult> {
        let resolved = validate_soul_path(&self.base, file)?;

        if let Some(parent) = resolved.parent() {
            std::fs::create_dir_all(parent).context("create soul subdirectory")?;
        }

        let final_content = if append && resolved.exists() {
            let existing = std::fs::read_to_string(&resolved).context("read existing soul file")?;
            format!("{}\n\n{}", existing.trim_end(), content)
        } else {
            content.to_owned()
        };

        if final_content.len() > self.max_bytes {
            bail!(
                "soul file would exceed {} KiB limit (got {} bytes)",
                self.max_bytes / 1024,
                final_content.len()
            );
        }

        std::fs::write(&resolved, &final_content).context("write soul memory file")?;

        Ok(SoulWriteResult {
            location: resolved.display().to_string(),
            bytes_written: final_content.len(),
        })
    }
}

// ─── Path validation ──────────────────────────────────────────────────────────

/// Validate a relative `file` path against `base`, rejecting `..` traversal.
/// Returns the resolved absolute path on success.
pub fn validate_soul_path(base: &Path, file: &str) -> Result<PathBuf> {
    if file.is_empty() {
        bail!("soul memory file path must not be empty");
    }
    let file_path = Path::new(file);
    // Reject absolute paths and `..` components
    for component in file_path.components() {
        match component {
            Component::Normal(_) | Component::CurDir => {}
            Component::ParentDir => bail!("soul file path must not contain '..'"),
            Component::Prefix(_) | Component::RootDir => {
                bail!("soul file path must be relative")
            }
        }
    }
    // Allow only safe extensions
    let ext = file_path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("md");
    if !matches!(ext, "md" | "tomllm" | "toml" | "txt" | "json") {
        bail!("soul file extension '.{ext}' not allowed (use .md, .tomllm, .toml, .txt, .json)");
    }
    Ok(base.join(file_path))
}

// ─── Soul directory helpers ───────────────────────────────────────────────────

/// Global soul directory: `~/._b00t_/`
pub fn global_soul_dir() -> PathBuf {
    dirs::home_dir()
        .unwrap_or_else(|| PathBuf::from("/tmp"))
        .join("._b00t_")
}

/// Local (workspace) soul directory: `<cwd>/._b00t_/` if it exists, else None.
pub fn local_soul_dir() -> Option<PathBuf> {
    std::env::current_dir()
        .ok()
        .map(|d| d.join("._b00t_"))
        .filter(|p| p.is_dir())
}

/// Active soul directory — local if present, global otherwise.
pub fn active_soul_dir() -> PathBuf {
    local_soul_dir().unwrap_or_else(global_soul_dir)
}

// ─── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validate_normal_path_ok() {
        let base = PathBuf::from("/tmp/soul");
        let resolved = validate_soul_path(&base, "SOUL.md").unwrap();
        assert_eq!(resolved, PathBuf::from("/tmp/soul/SOUL.md"));
    }

    #[test]
    fn validate_subdirectory_path_ok() {
        let base = PathBuf::from("/tmp/soul");
        let resolved = validate_soul_path(&base, "memory/notes.md").unwrap();
        assert_eq!(resolved, PathBuf::from("/tmp/soul/memory/notes.md"));
    }

    #[test]
    fn validate_dotdot_rejected() {
        let base = PathBuf::from("/tmp/soul");
        assert!(validate_soul_path(&base, "../etc/passwd").is_err());
    }

    #[test]
    fn validate_absolute_path_rejected() {
        let base = PathBuf::from("/tmp/soul");
        assert!(validate_soul_path(&base, "/etc/shadow").is_err());
    }

    #[test]
    fn validate_bad_extension_rejected() {
        let base = PathBuf::from("/tmp/soul");
        assert!(validate_soul_path(&base, "evil.sh").is_err());
    }

    #[test]
    fn file_writer_write_and_append() {
        let dir = tempfile::tempdir().unwrap();
        let writer = FileSoulWriter::new(dir.path().to_path_buf());
        let r1 = writer
            .write_memory("SOUL.md", "# Soul\nfirst", false)
            .unwrap();
        assert!(r1.bytes_written > 0);
        let r2 = writer.write_memory("SOUL.md", "second line", true).unwrap();
        let content = std::fs::read_to_string(&r1.location).unwrap();
        assert!(content.contains("first"));
        assert!(content.contains("second line"));
        let _ = r2;
    }

    #[test]
    fn file_writer_size_limit_enforced() {
        let dir = tempfile::tempdir().unwrap();
        let writer = FileSoulWriter::new(dir.path().to_path_buf()).with_max_bytes(10);
        let result = writer.write_memory("SOUL.md", "this is definitely more than 10 bytes", false);
        assert!(result.is_err());
    }
}
