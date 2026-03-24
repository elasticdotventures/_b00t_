use anyhow::{Context, Result};
use std::path::PathBuf;
use std::sync::Arc;
use crate::install::adapter::*;
use crate::install::content::{ContentPackId, FileCopyPack, ContentPack};
use crate::install::manifest::{B00tInstallManifest, remove_managed_block};

pub struct ClaudeConfig {
    pub target_dir: PathBuf,
}

impl RuntimeConfig for ClaudeConfig {
    fn settings_path(&self) -> PathBuf { self.target_dir.join("settings.json") }
    fn hooks_dir(&self) -> PathBuf { self.target_dir.join("hooks") }
    fn agents_dir(&self) -> PathBuf { self.target_dir.join("agents") }
    fn skills_dir(&self) -> PathBuf { self.target_dir.join("skills") }
}

/// Remove specific keys from a JSON settings file, writing back valid JSON.
/// No-op if file does not exist. Keys missing from the file are silently skipped.
fn remove_from_json_settings(path: &std::path::Path, keys: &[&str]) -> Result<()> {
    if !path.exists() { return Ok(()); }
    let content = std::fs::read_to_string(path)?;
    let mut value: serde_json::Value = match serde_json::from_str(&content) {
        Ok(v) => v,
        Err(_) => return Ok(()), // already invalid — leave it alone
    };
    if let serde_json::Value::Object(ref mut map) = value {
        for key in keys {
            map.remove(*key);
        }
    }
    std::fs::write(path, serde_json::to_string_pretty(&value)?)?;
    Ok(())
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
        // Remove managed blocks first — JSON files use key-deletion, others use text-marker removal
        for block_path in &manifest.managed_blocks {
            if block_path.extension().and_then(|e| e.to_str()) == Some("json") {
                remove_from_json_settings(block_path, &["hooks", "statusLine"])?;
            } else {
                remove_managed_block(block_path)?;
            }
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
        let fragment_str = std::fs::read_to_string(&fragment_path)?
            .replace("{{HOOKS_DIR}}", &hooks_dir.display().to_string());

        let fragment: serde_json::Value = serde_json::from_str(&fragment_str)
            .with_context(|| format!("Failed to parse settings_fragment.json: {}", fragment_path.display()))?;

        // Read existing settings or start with empty object
        let mut settings: serde_json::Value = if settings_path.exists() {
            let content = std::fs::read_to_string(&settings_path)?;
            serde_json::from_str(&content).unwrap_or(serde_json::Value::Object(Default::default()))
        } else {
            serde_json::Value::Object(Default::default())
        };

        // Merge fragment keys into settings (top-level merge)
        if let (serde_json::Value::Object(settings_map), serde_json::Value::Object(fragment_map))
            = (&mut settings, fragment)
        {
            for (key, value) in fragment_map {
                settings_map.insert(key, value);
            }
        }

        // Write back valid JSON
        if let Some(parent) = settings_path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(&settings_path, serde_json::to_string_pretty(&settings)?)?;

        // Track in manifest for uninstall (JSON-aware cleanup via remove_from_json_settings)
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

    #[test]
    fn test_register_hooks_produces_valid_json() {
        let tmp = TempDir::new().unwrap();
        let hooks_src = tmp.path().join("hooks");
        std::fs::create_dir_all(&hooks_src).unwrap();

        // Minimal fragment with a top-level key that uses the placeholder
        let fragment = r#"{"statusLine": "node {{HOOKS_DIR}}/b00t-statusline.js"}"#;
        std::fs::write(tmp.path().join("settings_fragment.json"), fragment).unwrap();

        let adapter = ClaudeAdapter;
        let config = Arc::new(ClaudeConfig { target_dir: tmp.path().to_path_buf() });
        let ctx = InstallContext {
            scope: InstallScope::Local(tmp.path().to_path_buf()),
            config,
            content_packs: vec![ContentPackId::Hooks],
            source_root: tmp.path().to_path_buf(),
        };

        let mut manifest = B00tInstallManifest::new(RuntimeId::Claude, InstallScope::Local(tmp.path().to_path_buf()));
        adapter.register_hooks(&ctx, &mut manifest).unwrap();

        // settings.json must be valid JSON
        let content = std::fs::read_to_string(tmp.path().join("settings.json")).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&content)
            .expect("settings.json must be valid JSON");

        assert!(parsed.get("statusLine").is_some(), "statusLine key must be present");

        // {{HOOKS_DIR}} placeholder must be substituted
        let status_line = parsed["statusLine"].as_str().unwrap();
        assert!(!status_line.contains("{{HOOKS_DIR}}"), "placeholder must be substituted");
        assert!(status_line.contains(&hooks_src.display().to_string()));

        // manifest must track the settings path for uninstall
        assert_eq!(manifest.managed_blocks.len(), 1);
    }

    #[test]
    fn test_register_hooks_merges_into_existing_settings() {
        let tmp = TempDir::new().unwrap();

        // Pre-existing settings with a user key
        std::fs::write(
            tmp.path().join("settings.json"),
            r#"{"enabledPlugins": {"my-plugin": true}}"#,
        ).unwrap();

        let fragment = r#"{"statusLine": "node {{HOOKS_DIR}}/b00t-statusline.js"}"#;
        std::fs::write(tmp.path().join("settings_fragment.json"), fragment).unwrap();

        let adapter = ClaudeAdapter;
        let config = Arc::new(ClaudeConfig { target_dir: tmp.path().to_path_buf() });
        let ctx = InstallContext {
            scope: InstallScope::Local(tmp.path().to_path_buf()),
            config,
            content_packs: vec![ContentPackId::Hooks],
            source_root: tmp.path().to_path_buf(),
        };

        let mut manifest = B00tInstallManifest::new(RuntimeId::Claude, InstallScope::Local(tmp.path().to_path_buf()));
        adapter.register_hooks(&ctx, &mut manifest).unwrap();

        let content = std::fs::read_to_string(tmp.path().join("settings.json")).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&content)
            .expect("settings.json must be valid JSON after merge");

        // Original user key preserved
        assert!(parsed.get("enabledPlugins").is_some(), "existing user keys must be preserved");
        // Fragment key injected
        assert!(parsed.get("statusLine").is_some(), "statusLine must be injected");
    }

    #[test]
    fn test_remove_from_json_settings_deletes_keys() {
        let tmp = TempDir::new().unwrap();
        let file = tmp.path().join("settings.json");
        std::fs::write(&file, r#"{"hooks": {}, "statusLine": "foo", "keep": true}"#).unwrap();

        remove_from_json_settings(&file, &["hooks", "statusLine"]).unwrap();

        let content = std::fs::read_to_string(&file).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&content).unwrap();
        assert!(parsed.get("hooks").is_none());
        assert!(parsed.get("statusLine").is_none());
        assert!(parsed.get("keep").is_some(), "unrelated keys must be preserved");
    }
}
