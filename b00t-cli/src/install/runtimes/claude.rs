use anyhow::Result;
use std::path::PathBuf;
use std::sync::Arc;
use crate::install::adapter::*;
use crate::install::content::{ContentPackId, FileCopyPack, ContentPack};
use crate::install::manifest::{B00tInstallManifest, inject_managed_block, remove_managed_block};

pub struct ClaudeConfig {
    pub target_dir: PathBuf,
}

impl RuntimeConfig for ClaudeConfig {
    fn settings_path(&self) -> PathBuf { self.target_dir.join("settings.json") }
    fn hooks_dir(&self) -> PathBuf { self.target_dir.join("hooks") }
    fn agents_dir(&self) -> PathBuf { self.target_dir.join("agents") }
    fn skills_dir(&self) -> PathBuf { self.target_dir.join("skills") }
}

#[derive(Default)]
pub struct ClaudeAdapter;

impl RuntimeAdapterTyped for ClaudeAdapter {
    type Config = ClaudeConfig;
    fn config_from_scope(&self, scope: &InstallScope) -> ClaudeConfig {
        ClaudeConfig { target_dir: self.target_dir(scope) }
    }
}

impl RuntimeAdapter for ClaudeAdapter {
    fn id(&self) -> RuntimeId { RuntimeId::Claude }

    fn target_dir(&self, scope: &InstallScope) -> PathBuf {
        match scope {
            InstallScope::Global => dirs::home_dir()
                .unwrap_or_else(|| PathBuf::from("~"))
                .join(".claude"),
            InstallScope::Local(p) => p.join(".claude"),
        }
    }

    fn detect(&self) -> bool {
        dirs::home_dir()
            .map(|h| h.join(".claude").exists())
            .unwrap_or(false)
    }

    fn default_config(&self, scope: &InstallScope) -> Arc<dyn RuntimeConfig> {
        Arc::new(self.config_from_scope(scope))
    }

    fn install(&self, ctx: &InstallContext) -> Result<B00tInstallManifest> {
        let target = ctx.config.skills_dir().parent().unwrap().to_path_buf();
        std::fs::create_dir_all(&target)?;

        let mut manifest = B00tInstallManifest::new(RuntimeId::Claude, ctx.scope.clone());

        for pack_id in &ctx.content_packs {
            let source = ctx.source_root.join(match pack_id {
                ContentPackId::Skills => "skills",
                ContentPackId::Agents => "agents",
                ContentPackId::Hooks  => "hooks",
                ContentPackId::DatumLifecycle => "skills", // datum lifecycle delivered as a skill
            });
            let pack = FileCopyPack {
                id: pack_id.clone(),
                source_dir: source,
                target_subdir: match pack_id {
                    ContentPackId::Skills | ContentPackId::DatumLifecycle => "skills".into(),
                    ContentPackId::Agents => "agents".into(),
                    ContentPackId::Hooks  => "hooks".into(),
                },
            };
            pack.install_into(&target, &mut manifest)?;
        }

        self.register_hooks(ctx, &mut manifest)?;
        manifest.save(&target)?;
        Ok(manifest)
    }

    fn uninstall(&self, manifest: &B00tInstallManifest) -> Result<()> {
        let mut m = manifest.clone();
        // Remove managed blocks first
        for block_path in &manifest.managed_blocks {
            remove_managed_block(block_path)?;
        }
        // Remove b00t-owned files
        let paths: Vec<PathBuf> = m.files.keys().cloned().collect();
        for path in paths {
            if m.file_owned(&path) {
                std::fs::remove_file(&path).ok();
                m.files.remove(&path);
            } else {
                let backup = path.with_extension("b00t-backup");
                eprintln!("⚠️  {} was modified — backing up to {:?}", path.display(), backup);
                std::fs::copy(&path, &backup).ok();
            }
        }
        Ok(())
    }

    fn register_hooks(&self, ctx: &InstallContext, manifest: &mut B00tInstallManifest) -> Result<()> {
        if !ctx.content_packs.contains(&ContentPackId::Hooks) { return Ok(()); }

        let hooks_dir = ctx.config.hooks_dir();
        let settings_path = ctx.config.settings_path();

        // Read hook template from source
        let fragment_path = ctx.source_root.join("settings_fragment.json");
        if !fragment_path.exists() {
            eprintln!("⚠️  No settings_fragment.json for Claude runtime — skipping hook registration");
            return Ok(());
        }
        let fragment = std::fs::read_to_string(&fragment_path)?;
        // Substitute hooks_dir path into fragment
        let fragment = fragment.replace("{{HOOKS_DIR}}", &hooks_dir.display().to_string());

        inject_managed_block(&settings_path, &fragment)?;
        manifest.managed_blocks.push(settings_path);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_claude_target_dir_global() {
        let adapter = ClaudeAdapter;
        let target = adapter.target_dir(&InstallScope::Global);
        assert!(target.ends_with(".claude"));
        assert!(target.is_absolute());
    }

    #[test]
    fn test_claude_target_dir_local() {
        let adapter = ClaudeAdapter;
        let project = PathBuf::from("/tmp/myproject");
        let target = adapter.target_dir(&InstallScope::Local(project.clone()));
        assert_eq!(target, project.join(".claude"));
    }

    #[test]
    fn test_claude_install_creates_manifest() {
        let source_dir = TempDir::new().unwrap();
        let target_dir = TempDir::new().unwrap();

        // Create minimal source content
        let skills_dir = source_dir.path().join("skills/b00t-test");
        std::fs::create_dir_all(&skills_dir).unwrap();
        std::fs::write(skills_dir.join("SKILL.md"), b"# Test Skill").unwrap();

        let adapter = ClaudeAdapter;
        let config = Arc::new(ClaudeConfig { target_dir: target_dir.path().to_path_buf() });
        let ctx = InstallContext {
            scope: InstallScope::Global,
            config,
            content_packs: vec![ContentPackId::Skills],
            source_root: source_dir.path().to_path_buf(),
        };

        let manifest = adapter.install(&ctx).unwrap();
        assert_eq!(manifest.files.len(), 1);
        assert!(target_dir.path().join("b00t-manifest.json").exists());
    }
}
