use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
use crate::install::adapter::{RuntimeId, InstallScope};
use anyhow::Result;

pub const MANIFEST_FILENAME: &str = "b00t-manifest.json";
pub const MANAGED_BLOCK_START: &str = "# BEGIN B00T MANAGED BLOCK";
pub const MANAGED_BLOCK_END: &str = "# END B00T MANAGED BLOCK";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct B00tInstallManifest {
    pub b00t_version: String,
    pub installed_at: String,
    pub runtime: String,
    pub scope: String,
    pub files: HashMap<PathBuf, String>,
    pub managed_blocks: Vec<PathBuf>,
}

impl B00tInstallManifest {
    pub fn new(runtime: RuntimeId, scope: InstallScope) -> Self {
        Self {
            b00t_version: env!("CARGO_PKG_VERSION").to_string(),
            installed_at: chrono::Utc::now().to_rfc3339(),
            runtime: format!("{:?}", runtime).to_lowercase(),
            scope: match scope {
                InstallScope::Global => "global".into(),
                InstallScope::Local(p) => p.display().to_string()
            },
            files: HashMap::new(),
            managed_blocks: Vec::new(),
        }
    }

    pub fn file_owned(&self, path: &std::path::Path) -> bool {
        if let Some(recorded) = self.files.get(path) {
            if let Ok(content) = std::fs::read(path) {
                use sha2::{Digest, Sha256};
                let digest = format!("{:x}", Sha256::digest(&content));
                return digest == *recorded;
            }
        }
        false
    }
}

pub fn inject_managed_block(_file_path: &std::path::Path, _content: &str) -> Result<()> {
    todo!("implement in Task 3")
}

pub fn remove_managed_block(_file_path: &std::path::Path) -> Result<()> {
    todo!("implement in Task 3")
}
