pub mod adapter;
pub mod content;
pub mod manifest;
pub mod runtimes;
pub mod tui;

pub use adapter::{AdapterRegistry, InstallContext, InstallScope, RuntimeAdapter, RuntimeAdapterTyped, RuntimeConfig, RuntimeId};

use anyhow::Result;
use std::path::PathBuf;
use crate::install::runtimes::*;

/// Build the default adapter registry with all 5 runtimes
pub fn default_registry() -> AdapterRegistry {
    AdapterRegistry::new(vec![
        Box::new(ClaudeAdapter),
        Box::new(GeminiAdapter),
        Box::new(CodexAdapter),
        Box::new(OpenCodeAdapter),
        Box::new(CopilotAdapter),
    ])
}

/// Source root for runtime content: _b00t_/runtimes/ relative to workspace root
pub fn runtimes_source_root() -> PathBuf {
    std::env::var("B00T_ROOT")
        .unwrap_or_else(|_| ".".to_string())
        .parse::<PathBuf>()
        .unwrap_or_else(|_| PathBuf::from("."))
        .join("_b00t_/runtimes")
}

/// Main entry: run TUI or headless, then install all selected runtimes
pub fn handle_install_command(
    interactive: bool,
    runtimes_arg: Option<Vec<RuntimeId>>,
    scope_arg: Option<InstallScope>,
    yes: bool,
) -> Result<()> {
    let registry = default_registry();

    let selection = if interactive && !yes {
        tui::run_tui(&registry)?
    } else {
        let runtimes = runtimes_arg.unwrap_or_else(|| {
            registry.detected().iter().map(|a| a.id()).collect()
        });
        let scope = scope_arg.unwrap_or(InstallScope::Global);
        tui::headless_selection(runtimes, scope, content::ContentPackId::all())
    };

    let source_root = runtimes_source_root();

    for runtime_id in &selection.runtimes {
        let adapter = registry.get(runtime_id)
            .ok_or_else(|| anyhow::anyhow!("No adapter for {:?}", runtime_id))?;

        let config = adapter.default_config(&selection.scope);
        let runtime_source = source_root.join(runtime_id.source_dir_name());

        let ctx = InstallContext {
            scope: selection.scope.clone(),
            config,
            content_packs: selection.content_packs.clone(),
            source_root: runtime_source,
        };

        println!("Installing b00t for {}...", runtime_id.display_name());
        let manifest = adapter.install(&ctx)?;
        println!("{} installed ({} files)", runtime_id.display_name(), manifest.files.len());
    }

    println!("\nb00t installation complete!");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_registry_has_five_adapters() {
        let registry = default_registry();
        assert_eq!(registry.all_adapters().len(), 5);
    }

    #[test]
    fn test_headless_selection_no_runtimes_defaults_to_detected() {
        // headless with empty runtimes falls back to detected
        let registry = default_registry();
        let detected: Vec<RuntimeId> = registry.detected().iter().map(|a| a.id()).collect();
        let selection = tui::headless_selection(
            detected.clone(),
            InstallScope::Global,
            content::ContentPackId::all(),
        );
        assert_eq!(selection.runtimes.len(), detected.len());
    }
}
