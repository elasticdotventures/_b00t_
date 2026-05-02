use crate::install::adapter::*;
use crate::install::manifest::B00tInstallManifest;
use anyhow::Result;
use std::path::PathBuf;
use std::sync::Arc;

pub struct GeminiConfig {
    pub target_dir: PathBuf,
}

impl RuntimeConfig for GeminiConfig {
    fn settings_path(&self) -> PathBuf {
        self.target_dir.join("settings.json")
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
pub struct GeminiAdapter;

impl RuntimeAdapterTyped for GeminiAdapter {
    type Config = GeminiConfig;
    fn config_from_scope(&self, scope: &InstallScope) -> Result<GeminiConfig> {
        Ok(GeminiConfig {
            target_dir: self.target_dir(scope)?,
        })
    }
}

impl RuntimeAdapter for GeminiAdapter {
    fn id(&self) -> RuntimeId {
        RuntimeId::Gemini
    }
    fn target_dir(&self, scope: &InstallScope) -> Result<PathBuf> {
        match scope {
            InstallScope::Global => Ok(super::require_home_dir("Gemini")?.join(".gemini")),
            InstallScope::Local(p) => Ok(p.join(".gemini")),
        }
    }
    fn detect(&self) -> bool {
        dirs::home_dir()
            .map(|h| h.join(".gemini").exists())
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
        _ctx: &InstallContext,
        _manifest: &mut B00tInstallManifest,
    ) -> Result<()> {
        Ok(())
    }
}
