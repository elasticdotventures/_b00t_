use anyhow::Result;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use crate::install::adapter::{RuntimeId, InstallScope};

pub const MANIFEST_FILENAME: &str = "b00t-manifest.json";
pub const MANAGED_BLOCK_START: &str = "# BEGIN B00T MANAGED BLOCK";
pub const MANAGED_BLOCK_END: &str = "# END B00T MANAGED BLOCK";

/// Per-file metadata stored in the manifest: allows safe, pack-scoped uninstall
/// without fragile substring path matching.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ManifestFileEntry {
    /// SHA256 hex digest of the installed content (for change detection)
    pub sha256: String,
    /// Stable key identifying which content pack owns this file (e.g. "skills")
    pub content_pack_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct B00tInstallManifest {
    pub b00t_version: String,
    pub installed_at: String,
    pub runtime: String,
    pub scope: String,
    /// Absolute paths → per-file metadata (SHA256 + owning content pack id)
    pub files: HashMap<PathBuf, ManifestFileEntry>,
    /// Absolute paths to files containing managed blocks
    pub managed_blocks: Vec<PathBuf>,
}

impl B00tInstallManifest {
    pub fn new(runtime: RuntimeId, scope: InstallScope) -> Self {
        Self {
            b00t_version: env!("CARGO_PKG_VERSION").to_string(),
            installed_at: chrono::Utc::now().to_rfc3339(),
            runtime: format!("{:?}", runtime).to_lowercase(),
            scope: match scope { InstallScope::Global => "global".into(), InstallScope::Local(p) => p.display().to_string() },
            files: HashMap::new(),
            managed_blocks: Vec::new(),
        }
    }

    /// Record a file as b00t-owned (absolute path, SHA256 of content, owning pack id)
    pub fn record_file(&mut self, path: &Path, content: &[u8], pack_id: &str) {
        let sha256 = format!("{:x}", Sha256::digest(content));
        self.files.insert(path.to_path_buf(), ManifestFileEntry {
            sha256,
            content_pack_id: pack_id.to_string(),
        });
    }

    /// Return true if the file at `path` matches the recorded SHA256
    pub fn file_owned(&self, path: &Path) -> bool {
        if let Some(entry) = self.files.get(path) {
            if let Ok(content) = std::fs::read(path) {
                let digest = format!("{:x}", Sha256::digest(&content));
                return digest == entry.sha256;
            }
        }
        false
    }

    pub fn save(&self, target_dir: &Path) -> Result<()> {
        let path = target_dir.join(MANIFEST_FILENAME);
        let json = serde_json::to_string_pretty(self)?;
        std::fs::write(&path, json)?;
        Ok(())
    }

    pub fn load(target_dir: &Path) -> Result<Self> {
        let path = target_dir.join(MANIFEST_FILENAME);
        let json = std::fs::read_to_string(&path)?;
        Ok(serde_json::from_str(&json)?)
    }
}

/// Inject `content` between managed block markers in `file_path`.
/// Creates the managed block if absent; replaces existing block if present.
/// User content outside markers is preserved.
pub fn inject_managed_block(file_path: &Path, content: &str) -> Result<()> {
    let existing = if file_path.exists() {
        std::fs::read_to_string(file_path)?
    } else {
        String::new()
    };

    let block = format!("{}\n{}\n{}", MANAGED_BLOCK_START, content, MANAGED_BLOCK_END);

    let result = if existing.contains(MANAGED_BLOCK_START) {
        // Replace existing block
        let before = existing.split(MANAGED_BLOCK_START).next().unwrap_or("").to_string();
        let after = existing.split(MANAGED_BLOCK_END).nth(1).unwrap_or("").to_string();
        format!("{}{}{}", before, block, after)
    } else {
        // Append block
        format!("{}\n{}\n", existing.trim_end(), block)
    };

    if let Some(parent) = file_path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(file_path, result)?;
    Ok(())
}

/// Remove the managed block from `file_path`, preserving surrounding content.
pub fn remove_managed_block(file_path: &Path) -> Result<()> {
    if !file_path.exists() { return Ok(()); }
    let content = std::fs::read_to_string(file_path)?;
    if !content.contains(MANAGED_BLOCK_START) { return Ok(()); }

    let before = content.split(MANAGED_BLOCK_START).next().unwrap_or("").trim_end().to_string();
    let after = content.split(MANAGED_BLOCK_END).nth(1).unwrap_or("").trim_start().to_string();
    std::fs::write(file_path, format!("{}\n{}", before, after))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_manifest_record_and_verify_file() {
        let dir = TempDir::new().unwrap();
        let file_path = dir.path().join("test.txt");
        let content = b"hello world";
        std::fs::write(&file_path, content).unwrap();

        let mut manifest = B00tInstallManifest::new(RuntimeId::Claude, InstallScope::Global);
        manifest.record_file(&file_path, content, "skills");
        assert!(manifest.file_owned(&file_path));
    }

    #[test]
    fn test_manifest_detects_modified_file() {
        let dir = TempDir::new().unwrap();
        let file_path = dir.path().join("test.txt");
        std::fs::write(&file_path, b"original").unwrap();

        let mut manifest = B00tInstallManifest::new(RuntimeId::Claude, InstallScope::Global);
        manifest.record_file(&file_path, b"original", "hooks");

        // Modify file
        std::fs::write(&file_path, b"modified by user").unwrap();
        assert!(!manifest.file_owned(&file_path));
    }

    #[test]
    fn test_manifest_save_and_load_roundtrip() {
        let dir = TempDir::new().unwrap();
        let mut manifest = B00tInstallManifest::new(RuntimeId::Gemini, InstallScope::Global);
        manifest.files.insert(dir.path().join("foo.js"), ManifestFileEntry {
            sha256: "abc123".to_string(),
            content_pack_id: "skills".to_string(),
        });

        manifest.save(dir.path()).unwrap();
        let loaded = B00tInstallManifest::load(dir.path()).unwrap();
        assert_eq!(loaded.runtime, "gemini");
        assert_eq!(loaded.files.len(), 1);
    }

    #[test]
    fn test_manifest_paths_are_absolute() {
        let dir = TempDir::new().unwrap();
        let abs_path = dir.path().join("skills/SKILL.md");
        let mut manifest = B00tInstallManifest::new(RuntimeId::Claude, InstallScope::Global);
        manifest.record_file(&abs_path, b"content", "skills");

        // All keys must be absolute
        for path in manifest.files.keys() {
            assert!(path.is_absolute(), "Expected absolute path, got: {:?}", path);
        }
    }

    #[test]
    fn test_inject_managed_block_creates_block() {
        let dir = TempDir::new().unwrap();
        // 🤓 use .tomllm (not .json); # markers corrupt JSON but are valid in .tomllm
        let file = dir.path().join("settings.tomllm");
        std::fs::write(&file, "existing = true\n").unwrap();
        inject_managed_block(&file, "injected content").unwrap();
        let result = std::fs::read_to_string(&file).unwrap();
        assert!(result.contains(MANAGED_BLOCK_START));
        assert!(result.contains("injected content"));
        assert!(result.contains(MANAGED_BLOCK_END));
        assert!(result.contains("existing")); // user content preserved
    }

    #[test]
    fn test_remove_managed_block_preserves_user_content() {
        let dir = TempDir::new().unwrap();
        let file = dir.path().join("settings.tomllm");
        let content = format!(
            "before\n{}\nmanaged\n{}\nafter\n",
            MANAGED_BLOCK_START, MANAGED_BLOCK_END
        );
        std::fs::write(&file, &content).unwrap();
        remove_managed_block(&file).unwrap();
        let result = std::fs::read_to_string(&file).unwrap();
        assert!(result.contains("before"));
        assert!(result.contains("after"));
        assert!(!result.contains(MANAGED_BLOCK_START));
        assert!(!result.contains("managed"));
    }
}
