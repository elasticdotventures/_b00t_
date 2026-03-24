use anyhow::{Context, Result};
use std::path::PathBuf;
use std::sync::Arc;
use serde::{Deserialize, Serialize};

/// Returns the current user's home directory, or a clear error if it cannot be determined.
/// Prefer this over `dirs::home_dir().unwrap_or_default()` to avoid writing into a relative
/// `~/...` path when `$HOME` is unset.
pub fn home_dir_required() -> Result<PathBuf> {
    dirs::home_dir().context("Cannot determine home directory — $HOME is not set")
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum RuntimeId {
    Claude,
    Gemini,
    Codex,
    OpenCode,
    Copilot,
}

impl RuntimeId {
    pub fn display_name(&self) -> &'static str {
        match self {
            RuntimeId::Claude   => "Claude Code",
            RuntimeId::Gemini   => "Gemini CLI",
            RuntimeId::Codex    => "Codex",
            RuntimeId::OpenCode => "OpenCode",
            RuntimeId::Copilot  => "Copilot",
        }
    }

    pub fn source_dir_name(&self) -> &'static str {
        match self {
            RuntimeId::Claude   => "claude",
            RuntimeId::Gemini   => "gemini",
            RuntimeId::Codex    => "codex",
            RuntimeId::OpenCode => "opencode",
            RuntimeId::Copilot  => "copilot",
        }
    }
}

#[derive(Debug, Clone)]
pub enum InstallScope {
    Global,
    Local(PathBuf),
}

/// Object-safe runtime config — no generics
pub trait RuntimeConfig: Send + Sync {
    fn settings_path(&self) -> PathBuf;
    fn hooks_dir(&self) -> PathBuf;
    fn agents_dir(&self) -> PathBuf;
    fn skills_dir(&self) -> PathBuf;
}

/// Context passed to every install/uninstall call
pub struct InstallContext {
    pub scope: InstallScope,
    pub config: Arc<dyn RuntimeConfig>,
    pub content_packs: Vec<super::content::ContentPackId>,
    pub source_root: PathBuf,  // _b00t_/runtimes/<runtime>/
}

/// Object-safe dispatch trait — no associated types
pub trait RuntimeAdapter: Send + Sync {
    fn id(&self) -> RuntimeId;
    fn target_dir(&self, scope: &InstallScope) -> Result<PathBuf>;
    fn detect(&self) -> bool;
    fn default_config(&self, scope: &InstallScope) -> Result<Arc<dyn RuntimeConfig>>;
    fn install(&self, ctx: &InstallContext) -> Result<super::manifest::B00tInstallManifest>;
    fn uninstall(&self, manifest: &super::manifest::B00tInstallManifest) -> Result<()>;
    fn register_hooks(&self, ctx: &InstallContext, manifest: &mut super::manifest::B00tInstallManifest) -> Result<()>;
}

/// Typed impl trait — associated Config type, used only at concrete impl level
pub trait RuntimeAdapterTyped: RuntimeAdapter {
    type Config: RuntimeConfig;
    fn config_from_scope(&self, scope: &InstallScope) -> Result<Self::Config>;
}

pub struct AdapterRegistry {
    adapters: Vec<Box<dyn RuntimeAdapter>>,
}

impl AdapterRegistry {
    pub fn new(adapters: Vec<Box<dyn RuntimeAdapter>>) -> Self {
        Self { adapters }
    }

    pub fn all_adapters(&self) -> &[Box<dyn RuntimeAdapter>] {
        &self.adapters
    }

    pub fn detected(&self) -> Vec<&dyn RuntimeAdapter> {
        self.adapters.iter()
            .filter(|a| a.detect())
            .map(|a| a.as_ref())
            .collect()
    }

    pub fn get(&self, id: &RuntimeId) -> Option<&dyn RuntimeAdapter> {
        self.adapters.iter()
            .find(|a| a.id() == *id)
            .map(|a| a.as_ref())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Minimal test adapter for unit testing
    struct TestAdapter { id: RuntimeId, detected: bool }

    impl RuntimeConfig for TestAdapter {
        fn settings_path(&self) -> PathBuf { PathBuf::from("/tmp/test/settings.json") }
        fn hooks_dir(&self) -> PathBuf { PathBuf::from("/tmp/test/hooks") }
        fn agents_dir(&self) -> PathBuf { PathBuf::from("/tmp/test/agents") }
        fn skills_dir(&self) -> PathBuf { PathBuf::from("/tmp/test/skills") }
    }

    impl RuntimeAdapter for TestAdapter {
        fn id(&self) -> RuntimeId { self.id.clone() }
        fn target_dir(&self, _scope: &InstallScope) -> Result<PathBuf> { Ok(PathBuf::from("/tmp/test")) }
        fn detect(&self) -> bool { self.detected }
        fn default_config(&self, _scope: &InstallScope) -> Result<Arc<dyn RuntimeConfig>> {
            Ok(Arc::new(TestAdapter { id: self.id.clone(), detected: self.detected }))
        }
        fn install(&self, _ctx: &InstallContext) -> Result<super::super::manifest::B00tInstallManifest> {
            Ok(super::super::manifest::B00tInstallManifest::new(self.id(), super::InstallScope::Global))
        }
        fn uninstall(&self, _manifest: &super::super::manifest::B00tInstallManifest) -> Result<()> { Ok(()) }
        fn register_hooks(&self, _ctx: &InstallContext, _manifest: &mut super::super::manifest::B00tInstallManifest) -> Result<()> { Ok(()) }
    }

    #[test]
    fn test_registry_detected() {
        let reg = AdapterRegistry::new(vec![
            Box::new(TestAdapter { id: RuntimeId::Claude, detected: true }),
            Box::new(TestAdapter { id: RuntimeId::Gemini, detected: false }),
        ]);
        let detected = reg.detected();
        assert_eq!(detected.len(), 1);
        assert_eq!(detected[0].id(), RuntimeId::Claude);
    }

    #[test]
    fn test_registry_get_by_id() {
        let reg = AdapterRegistry::new(vec![
            Box::new(TestAdapter { id: RuntimeId::Claude, detected: true }),
            Box::new(TestAdapter { id: RuntimeId::Codex, detected: false }),
        ]);
        assert!(reg.get(&RuntimeId::Claude).is_some());
        assert!(reg.get(&RuntimeId::Gemini).is_none());
    }

    #[test]
    fn test_runtime_id_display() {
        assert_eq!(RuntimeId::Claude.display_name(), "Claude Code");
        assert_eq!(RuntimeId::Gemini.display_name(), "Gemini CLI");
    }
}
