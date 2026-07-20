use crate::install::adapter::*;
use crate::install::manifest::B00tInstallManifest;
use anyhow::{Context, Result};
use std::path::PathBuf;
use std::sync::Arc;

pub struct OpenCodeConfig {
    pub target_dir: PathBuf,
}

impl RuntimeConfig for OpenCodeConfig {
    fn settings_path(&self) -> PathBuf {
        self.target_dir.join("opencode.json")
    }
    fn hooks_dir(&self) -> PathBuf {
        self.target_dir.join("hooks")
    }
    fn agents_dir(&self) -> PathBuf {
        self.target_dir.join("agents")
    }
    fn skills_dir(&self) -> PathBuf {
        self.target_dir.join("skills")
    }
}

#[derive(Default)]
pub struct OpenCodeAdapter;

impl RuntimeAdapterTyped for OpenCodeAdapter {
    type Config = OpenCodeConfig;
    fn config_from_scope(&self, scope: &InstallScope) -> Result<OpenCodeConfig> {
        Ok(OpenCodeConfig {
            target_dir: self.target_dir(scope)?,
        })
    }
}

impl RuntimeAdapter for OpenCodeAdapter {
    fn id(&self) -> RuntimeId {
        RuntimeId::OpenCode
    }
    fn target_dir(&self, scope: &InstallScope) -> Result<PathBuf> {
        match scope {
            InstallScope::Global => {
                Ok(super::require_home_dir("OpenCode")?.join(".config/opencode"))
            }
            InstallScope::Local(p) => Ok(p.join(".config/opencode")),
        }
    }
    fn detect(&self) -> bool {
        dirs::home_dir()
            .map(|h| h.join(".config/opencode").exists())
            .unwrap_or(false)
    }
    fn default_config(&self, scope: &InstallScope) -> Result<Arc<dyn RuntimeConfig>> {
        Ok(Arc::new(self.config_from_scope(scope)?))
    }
    fn install(&self, _ctx: &InstallContext) -> Result<B00tInstallManifest> {
        Err(anyhow::anyhow!(
            "{} runtime installer not yet implemented",
            self.id().display_name()
        ))
    }
    fn uninstall(&self, _manifest: &B00tInstallManifest) -> Result<()> {
        Ok(())
    }
    fn register_hooks(
        &self,
        ctx: &InstallContext,
        manifest: &mut B00tInstallManifest,
    ) -> Result<()> {
        // Copy rustfmt-post-edit.ts plugin to the opencode global plugins dir.
        // Relies on settings_fragment.json registering "{{PLUGINS_DIR}}/rustfmt-post-edit.ts"
        // in the plugin array, merged into ~/.config/opencode/opencode.json.
        let fragment_path = ctx.source_root.join("settings_fragment.json");
        if !fragment_path.exists() {
            eprintln!(
                "⚠️  No settings_fragment.json for OpenCode runtime — skipping hook registration"
            );
            return Ok(());
        }

        let plugins_src = ctx.source_root.join("plugins");
        let plugin_file = "rustfmt-post-edit.ts";
        let src_plugin = plugins_src.join(plugin_file);
        if !src_plugin.exists() {
            eprintln!(
                "⚠️  Plugin source not found: {} — skipping",
                src_plugin.display()
            );
            return Ok(());
        }

        // Global plugins dir: ~/.config/opencode/plugins/
        let plugins_dir = ctx
            .config
            .hooks_dir()
            .parent()
            .map(|p| p.join("plugins"))
            .unwrap_or_else(|| ctx.config.hooks_dir().join("../plugins"));
        std::fs::create_dir_all(&plugins_dir)
            .with_context(|| format!("Failed to create plugins dir: {}", plugins_dir.display()))?;

        let dest_plugin = plugins_dir.join(plugin_file);
        std::fs::copy(&src_plugin, &dest_plugin)
            .with_context(|| format!("Failed to copy plugin to {}", dest_plugin.display()))?;

        // Merge fragment into opencode.json — substitute {{PLUGINS_DIR}} then array-merge plugin[].
        let settings_path = ctx.config.settings_path();
        let fragment_str = std::fs::read_to_string(&fragment_path)?
            .replace("{{PLUGINS_DIR}}", &plugins_dir.display().to_string());

        let fragment: serde_json::Value =
            serde_json::from_str(&fragment_str).with_context(|| {
                format!(
                    "Failed to parse settings_fragment.json: {}",
                    fragment_path.display()
                )
            })?;

        let mut settings: serde_json::Value = if settings_path.exists() {
            let content = std::fs::read_to_string(&settings_path)?;
            serde_json::from_str(&content).unwrap_or(serde_json::Value::Object(Default::default()))
        } else {
            serde_json::Value::Object(Default::default())
        };

        // Merge: for the `plugin` array, append (not replace) so existing plugins are preserved.
        if let (serde_json::Value::Object(s_map), serde_json::Value::Object(f_map)) =
            (&mut settings, &fragment)
        {
            for (key, fval) in f_map {
                if key == "_note" {
                    continue;
                }
                if key == "plugin" {
                    // Array-merge: extend existing plugin list without duplicates.
                    let existing = s_map
                        .entry("plugin")
                        .or_insert_with(|| serde_json::Value::Array(vec![]));
                    if let (serde_json::Value::Array(arr), serde_json::Value::Array(new_items)) =
                        (existing, fval)
                    {
                        for item in new_items {
                            if !arr.contains(item) {
                                arr.push(item.clone());
                            }
                        }
                    }
                } else {
                    s_map.insert(key.clone(), fval.clone());
                }
            }
        }

        if let Some(parent) = settings_path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(&settings_path, serde_json::to_string_pretty(&settings)?)?;

        manifest.managed_blocks.push(dest_plugin);
        Ok(())
    }
}
