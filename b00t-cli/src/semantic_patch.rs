/// Visible semantic patch: produces a unified diff before any file write.
///
/// Pattern: show diff → confirm → write. Prevents silent overwrites.
use anyhow::{Result, bail};
use std::path::{Path, PathBuf};

pub struct SemanticPatch {
    pub path: PathBuf,
    pub original: String,
    pub proposed: String,
    pub unified_diff: String,
}

impl SemanticPatch {
    pub fn new(
        path: impl Into<PathBuf>,
        original: impl Into<String>,
        proposed: impl Into<String>,
    ) -> Result<Self> {
        let path = path.into();
        let original = original.into();
        let proposed = proposed.into();

        // Guard: reject non-UTF-8 content early
        if original.contains('\0') || proposed.contains('\0') {
            bail!(
                "binary content detected in {}, cannot produce semantic patch",
                path.display()
            );
        }

        let patch = diffy::create_patch(&original, &proposed);
        let unified_diff = diffy::PatchFormatter::new().fmt_patch(&patch).to_string();

        Ok(Self {
            path,
            original,
            proposed,
            unified_diff,
        })
    }

    /// Load original from disk, diff against proposed string.
    pub fn from_disk(path: impl AsRef<Path>, proposed: impl Into<String>) -> Result<Self> {
        let path = path.as_ref();
        let original = if path.exists() {
            std::fs::read_to_string(path)?
        } else {
            String::new()
        };
        Self::new(path, original, proposed)
    }

    pub fn has_changes(&self) -> bool {
        !self.unified_diff.is_empty() && self.original != self.proposed
    }

    pub fn display(&self) {
        if self.has_changes() {
            println!("--- {}", self.path.display());
            print!("{}", self.unified_diff);
        } else {
            println!("(no changes to {})", self.path.display());
        }
    }

    pub fn apply(&self) -> Result<()> {
        std::fs::write(&self.path, &self.proposed)?;
        Ok(())
    }

    pub fn to_json(&self) -> serde_json::Value {
        serde_json::json!({
            "path": self.path.to_string_lossy(),
            "has_changes": self.has_changes(),
            "unified_diff": self.unified_diff,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write as _;
    use tempfile::NamedTempFile;

    #[test]
    fn identical_files_produce_no_diff() {
        let p = SemanticPatch::new("test.rs", "fn main() {}\n", "fn main() {}\n").unwrap();
        assert!(!p.has_changes());
    }

    #[test]
    fn changed_file_shows_unified_diff() {
        let p = SemanticPatch::new("test.rs", "fn foo() {}\n", "fn bar() {}\n").unwrap();
        assert!(p.has_changes());
        assert!(p.unified_diff.contains('-') || p.unified_diff.contains('+'));
    }

    #[test]
    fn apply_writes_proposed_content() {
        let mut f = NamedTempFile::new().unwrap();
        writeln!(f, "original content").unwrap();
        let path = f.path().to_owned();

        let p = SemanticPatch::new(&path, "original content\n", "new content\n").unwrap();
        p.apply().unwrap();

        let result = std::fs::read_to_string(&path).unwrap();
        assert_eq!(result, "new content\n");
    }

    #[test]
    fn from_disk_reads_existing_file() {
        let mut f = NamedTempFile::new().unwrap();
        writeln!(f, "existing").unwrap();

        let p = SemanticPatch::from_disk(f.path(), "existing\n").unwrap();
        assert!(!p.has_changes());
    }

    #[test]
    fn from_disk_new_file_shows_full_add() {
        let path = std::env::temp_dir().join("nonexistent_semantic_patch_test.rs");
        let _ = std::fs::remove_file(&path);

        let p = SemanticPatch::from_disk(&path, "new content\n").unwrap();
        assert!(p.has_changes());
    }
}
