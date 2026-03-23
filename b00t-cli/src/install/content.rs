use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use crate::install::manifest::B00tInstallManifest;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ContentPackId {
    Skills,
    Agents,
    Hooks,
    DatumLifecycle,
}

impl ContentPackId {
    pub fn display_name(&self) -> &'static str {
        match self {
            ContentPackId::Skills        => "Skills & commands",
            ContentPackId::Agents        => "Agents",
            ContentPackId::Hooks         => "Hooks (statusline, update-check, context-monitor, datum-guard)",
            ContentPackId::DatumLifecycle => "Datum lifecycle (b00t install/b00t uninstall)",
        }
    }

    pub fn all() -> Vec<ContentPackId> {
        vec![
            ContentPackId::Skills,
            ContentPackId::Agents,
            ContentPackId::Hooks,
            ContentPackId::DatumLifecycle,
        ]
    }
}

/// ContentPack: one responsibility — install/uninstall a named set of files
pub trait ContentPack: Send + Sync {
    fn id(&self) -> ContentPackId;
    fn source_dir(&self) -> PathBuf;
    /// Install files into `target`, recording each in `manifest`
    fn install_into(&self, target: &Path, manifest: &mut B00tInstallManifest) -> Result<()>;
    /// Uninstall: delete b00t-owned files (SHA256 match), back up user-modified files
    fn uninstall_from(&self, manifest: &mut B00tInstallManifest) -> Result<()>;
}

/// Generic file-copy content pack: copies all files from source_dir into target/<subdir>
pub struct FileCopyPack {
    pub id: ContentPackId,
    pub source_dir: PathBuf,
    pub target_subdir: String,  // e.g. "skills", "agents"
}

impl ContentPack for FileCopyPack {
    fn id(&self) -> ContentPackId { self.id.clone() }

    fn source_dir(&self) -> PathBuf { self.source_dir.clone() }

    fn install_into(&self, target: &Path, manifest: &mut B00tInstallManifest) -> Result<()> {
        if !self.source_dir.exists() {
            return Ok(()); // no content for this runtime yet — silent skip
        }
        let dest = target.join(&self.target_subdir);
        std::fs::create_dir_all(&dest)?;

        for entry in walkdir::WalkDir::new(&self.source_dir) {
            let entry = entry?;
            if entry.file_type().is_file() {
                let rel = entry.path().strip_prefix(&self.source_dir)?;
                let dest_path = dest.join(rel);
                if let Some(parent) = dest_path.parent() {
                    std::fs::create_dir_all(parent)?;
                }
                let content = std::fs::read(entry.path())?;
                std::fs::write(&dest_path, &content)?;
                manifest.record_file(&dest_path, &content);
            }
        }
        Ok(())
    }

    fn uninstall_from(&self, manifest: &mut B00tInstallManifest) -> Result<()> {
        let to_remove: Vec<PathBuf> = manifest.files.keys()
            .filter(|p| p.to_string_lossy().contains(&self.target_subdir))
            .cloned()
            .collect();

        for path in to_remove {
            if manifest.file_owned(&path) {
                std::fs::remove_file(&path).ok();
                manifest.files.remove(&path);
            } else {
                // User modified — back up instead of deleting
                let backup = path.with_extension("b00t-backup");
                eprintln!("⚠️  {} was modified — backing up to {:?}", path.display(), backup);
                std::fs::copy(&path, &backup).ok();
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;
    use crate::install::adapter::{RuntimeId, InstallScope};

    #[test]
    fn test_content_pack_id_all() {
        let all = ContentPackId::all();
        assert_eq!(all.len(), 4);
        assert!(all.contains(&ContentPackId::DatumLifecycle));
    }

    #[test]
    fn test_file_copy_pack_install_and_uninstall() {
        let source_dir = TempDir::new().unwrap();
        let target_dir = TempDir::new().unwrap();

        // Create source content
        std::fs::create_dir(source_dir.path().join("b00t-greet")).unwrap();
        std::fs::write(source_dir.path().join("b00t-greet/SKILL.md"), b"# Greet").unwrap();

        let pack = FileCopyPack {
            id: ContentPackId::Skills,
            source_dir: source_dir.path().to_path_buf(),
            target_subdir: "skills".to_string(),
        };

        let mut manifest = crate::install::manifest::B00tInstallManifest::new(
            RuntimeId::Claude, InstallScope::Global
        );

        // Install
        pack.install_into(target_dir.path(), &mut manifest).unwrap();
        let installed = target_dir.path().join("skills/b00t-greet/SKILL.md");
        assert!(installed.exists());
        assert_eq!(manifest.files.len(), 1);

        // Uninstall
        pack.uninstall_from(&mut manifest).unwrap();
        assert!(!installed.exists());
        assert_eq!(manifest.files.len(), 0);
    }

    #[test]
    fn test_file_copy_pack_backs_up_modified_files() {
        let source_dir = TempDir::new().unwrap();
        let target_dir = TempDir::new().unwrap();
        std::fs::write(source_dir.path().join("hook.js"), b"original").unwrap();

        let pack = FileCopyPack {
            id: ContentPackId::Hooks,
            source_dir: source_dir.path().to_path_buf(),
            target_subdir: "hooks".to_string(),
        };
        let mut manifest = crate::install::manifest::B00tInstallManifest::new(
            RuntimeId::Claude, InstallScope::Global
        );
        pack.install_into(target_dir.path(), &mut manifest).unwrap();

        // User modifies the file
        let installed = target_dir.path().join("hooks/hook.js");
        std::fs::write(&installed, b"user modified").unwrap();

        // Uninstall — should back up, not delete
        pack.uninstall_from(&mut manifest).unwrap();
        let backup = installed.with_extension("b00t-backup");
        assert!(backup.exists(), "Modified file should be backed up");
        assert!(installed.exists(), "Modified file should not be deleted");
    }
}
