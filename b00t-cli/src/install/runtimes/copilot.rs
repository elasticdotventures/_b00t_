use crate::install::adapter::*;
use crate::install::manifest::B00tInstallManifest;
use anyhow::Result;
use std::path::PathBuf;
use std::sync::Arc;

pub struct CopilotConfig {
    pub target_dir: PathBuf,
}

impl RuntimeConfig for CopilotConfig {
    fn settings_path(&self) -> PathBuf {
        self.target_dir.join("copilot-instructions.md")
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
pub struct CopilotAdapter;

impl RuntimeAdapterTyped for CopilotAdapter {
    type Config = CopilotConfig;
    fn config_from_scope(&self, scope: &InstallScope) -> Result<CopilotConfig> {
        Ok(CopilotConfig {
            target_dir: self.target_dir(scope)?,
        })
    }
}

impl RuntimeAdapter for CopilotAdapter {
    fn id(&self) -> RuntimeId {
        RuntimeId::Copilot
    }
    fn target_dir(&self, scope: &InstallScope) -> Result<PathBuf> {
        match scope {
            InstallScope::Global => Ok(super::require_home_dir("Copilot")?.join(".copilot")),
            InstallScope::Local(p) => Ok(p.join(".copilot")),
        }
    }
    fn detect(&self) -> bool {
        dirs::home_dir()
            .map(|h| h.join(".copilot").exists())
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
