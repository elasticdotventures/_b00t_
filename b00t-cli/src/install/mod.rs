pub mod adapter;
pub mod content;
pub mod manifest;
pub mod runtimes;
pub mod tui;

pub use adapter::{
    AdapterRegistry, InstallContext, InstallScope, RuntimeAdapter, RuntimeAdapterTyped,
    RuntimeConfig, RuntimeId,
};

use crate::install::runtimes::*;
use anyhow::Result;
use std::path::PathBuf;

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

/// Source root for runtime content.
///
/// Defaults to `_b00t_/runtimes/` relative to the workspace root. Tests and
/// controlled installers may override with `B00T_RUNTIMES_SOURCE_ROOT`.
pub fn runtimes_source_root() -> Result<PathBuf> {
    if let Ok(source_root) = std::env::var("B00T_RUNTIMES_SOURCE_ROOT") {
        let root = PathBuf::from(source_root);
        if root.exists() {
            return Ok(root);
        }
        anyhow::bail!(
            "runtimes source directory not found: {}",
            root.display()
        );
    }

    let root = PathBuf::from(crate::utils::get_workspace_root()).join("_b00t_/runtimes");
    if !root.exists() {
        anyhow::bail!(
            "runtimes source directory not found: {}. Run from within the b00t repository.",
            root.display()
        );
    }
    Ok(root)
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
        let runtimes =
            runtimes_arg.unwrap_or_else(|| registry.detected().iter().map(|a| a.id()).collect());
        let scope = scope_arg.unwrap_or(InstallScope::Global);
        let sel = tui::headless_selection(runtimes, scope, content::ContentPackId::all());
        // In headless (non-interactive) mode without --yes, require explicit confirmation
        if !yes {
            let runtime_names: Vec<&str> =
                sel.runtimes.iter().map(RuntimeId::display_name).collect();
            let scope_str = match &sel.scope {
                InstallScope::Global => "globally".to_string(),
                InstallScope::Local(p) => format!("locally in {}", p.display()),
            };
            let confirmed = inquire::Confirm::new(&format!(
                "Install b00t for [{}] {}? (pass --yes to skip this prompt)",
                runtime_names.join(", "), scope_str
            ))
            .with_default(false)
            .prompt()
            .map_err(|e| anyhow::anyhow!("Confirmation prompt failed (no TTY available?). Pass --yes to skip confirmation in non-interactive environments. Details: {}", e))?;
            if !confirmed {
                anyhow::bail!("Installation cancelled.");
            }
        }
        sel
    };

    let source_root = runtimes_source_root()?;

    for runtime_id in &selection.runtimes {
        let adapter = registry
            .get(runtime_id)
            .ok_or_else(|| anyhow::anyhow!("No adapter for {:?}", runtime_id))?;

        let config = adapter.default_config(&selection.scope)?;
        let runtime_source = source_root.join(runtime_id.source_dir_name());

        let ctx = InstallContext {
            scope: selection.scope.clone(),
            config,
            content_packs: selection.content_packs.clone(),
            source_root: runtime_source,
        };

        println!("Installing b00t for {}...", runtime_id.display_name());
        let manifest = adapter.install(&ctx)?;
        println!(
            "{} installed ({} files)",
            runtime_id.display_name(),
            manifest.files.len()
        );
    }

    println!("\nb00t installation complete!");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    static ENV_MUTEX: Mutex<()> = Mutex::new(());

    /// RAII guard that removes an env var on drop, ensuring cleanup even on panic.
    struct EnvVarGuard(&'static str);
    impl Drop for EnvVarGuard {
        fn drop(&mut self) {
            unsafe {
                std::env::remove_var(self.0);
            }
        }
    }

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

    #[test]
    fn test_runtimes_source_root_from_workspace() {
        // When run from within the repo, runtimes_source_root() is anchored at the
        // git workspace root. Verify the returned path ends with _b00t_/runtimes.
        let _lock = ENV_MUTEX.lock().unwrap();
        unsafe {
            std::env::remove_var("B00T_RUNTIMES_SOURCE_ROOT");
        }
        let root = crate::utils::get_workspace_root();
        let expected = PathBuf::from(&root).join("_b00t_/runtimes");
        if expected.exists() {
            let result = runtimes_source_root();
            assert!(result.is_ok(), "expected Ok, got {:?}", result);
            assert_eq!(result.unwrap(), expected);
        } else {
            // The test environment has no runtimes dir; verify the function errors.
            let result = runtimes_source_root();
            assert!(result.is_err());
            let msg = result.unwrap_err().to_string();
            assert!(
                msg.contains("runtimes source directory not found"),
                "unexpected error: {}",
                msg
            );
        }
    }

    #[test]
    fn test_runtimes_source_root_errors_when_missing() {
        // Override workspace root to a temp dir that has no _b00t_/runtimes.
        let tmp = tempfile::tempdir().unwrap();
        let _lock = ENV_MUTEX.lock().unwrap();
        unsafe {
            std::env::remove_var("B00T_RUNTIMES_SOURCE_ROOT");
            std::env::set_var("_B00T_TEST_ROOT", tmp.path().to_str().unwrap());
        }
        let _cleanup = EnvVarGuard("_B00T_TEST_ROOT");
        let result = runtimes_source_root();
        assert!(result.is_err());
        let msg = result.unwrap_err().to_string();
        assert!(
            msg.contains("runtimes source directory not found"),
            "unexpected error: {}",
            msg
        );
    }

    #[test]
    fn test_runtimes_source_root_explicit_override() {
        let tmp = tempfile::tempdir().unwrap();
        let source_root = tmp.path().join("_b00t_/runtimes");
        std::fs::create_dir_all(&source_root).unwrap();

        let _lock = ENV_MUTEX.lock().unwrap();
        unsafe {
            std::env::set_var("B00T_RUNTIMES_SOURCE_ROOT", &source_root);
        }
        let _cleanup = EnvVarGuard("B00T_RUNTIMES_SOURCE_ROOT");

        let result = runtimes_source_root().unwrap();
        assert_eq!(result, source_root);
    }
}
