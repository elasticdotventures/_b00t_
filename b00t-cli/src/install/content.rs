use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use anyhow::Result;
use crate::install::manifest::B00tInstallManifest;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ContentPackId {
    Skills,
    Agents,
    Hooks,
    DatumLifecycle,
}

pub trait ContentPack: Send + Sync {
    fn id(&self) -> ContentPackId;
    fn source_dir(&self) -> PathBuf;
    fn install_into(&self, target: &std::path::Path, manifest: &mut B00tInstallManifest) -> Result<()>;
    fn uninstall_from(&self, manifest: &mut B00tInstallManifest) -> Result<()>;
}
