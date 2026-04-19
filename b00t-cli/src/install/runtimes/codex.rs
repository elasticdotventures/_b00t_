use crate::install::adapter::*;
use crate::install::manifest::B00tInstallManifest;
use anyhow::Result;
use std::path::PathBuf;
use std::sync::Arc;

pub struct CodexConfig {
    pub target_dir: PathBuf,
}

impl RuntimeConfig for CodexConfig {
    fn settings_path(&self) -> PathBuf {
        self.target_dir.join("config.toml")
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
pub struct CodexAdapter;

impl RuntimeAdapterTyped for CodexAdapter {
    type Config = CodexConfig;
    fn config_from_scope(&self, scope: &InstallScope) -> Result<CodexConfig> {
        Ok(CodexConfig {
            target_dir: self.target_dir(scope)?,
        })
    }
}

impl RuntimeAdapter for CodexAdapter {
    fn id(&self) -> RuntimeId {
        RuntimeId::Codex
    }
    fn target_dir(&self, scope: &InstallScope) -> Result<PathBuf> {
        match scope {
            InstallScope::Global => Ok(super::require_home_dir("Codex")?.join(".codex")),
            InstallScope::Local(p) => Ok(p.join(".codex")),
        }
    }
    fn detect(&self) -> bool {
        std::process::Command::new("which")
            .arg("codex")
            .output()
            .map(|o| o.status.success())
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
