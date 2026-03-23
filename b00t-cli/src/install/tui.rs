use anyhow::Result;
use crate::install::adapter::{AdapterRegistry, InstallScope, RuntimeId};
use crate::install::content::ContentPackId;

pub struct InstallSelection {
    pub runtimes: Vec<RuntimeId>,
    pub scope: InstallScope,
    pub content_packs: Vec<ContentPackId>,
}

pub fn run_tui(_registry: &AdapterRegistry) -> Result<InstallSelection> {
    todo!("implement in Task 5")
}

pub fn headless_selection(
    runtimes: Vec<RuntimeId>,
    scope: InstallScope,
    content_packs: Vec<ContentPackId>,
) -> InstallSelection {
    InstallSelection { runtimes, scope, content_packs }
}
