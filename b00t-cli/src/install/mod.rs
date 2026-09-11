pub mod adapter;
pub mod capability;
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

/// Build the default adapter registry with all 6 runtimes
pub fn default_registry() -> AdapterRegistry {
    AdapterRegistry::new(vec![
        Box::new(ClaudeAdapter),
        Box::new(GeminiAdapter),
        Box::new(CodexAdapter),
        Box::new(OpenCodeAdapter),
        Box::new(CopilotAdapter),
        Box::new(PiAdapter),
    ])
}

/// Source root for runtime content.
///
/// Defaults to `_b00t_/runtimes/` relative to the workspace root. Tests and
/// controlled installers may override with `B00T_RUNTIMES_SOURCE_ROOT`.
pub fn runtimes_source_root() -> Result<PathBuf> {
    if let Ok(source_root) = std::env::var("B00T_RUNTIMES_SOURCE_ROOT") {
        let root = PathBuf::from(source_root);
        if root.is_dir() {
            return Ok(root);
        }
        if root.exists() {
            anyhow::bail!(
                "runtimes source path exists but is not a directory: {}",
                root.display()
            );
        }
        anyhow::bail!("runtimes source directory not found: {}", root.display());
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

/// Main entry: remove b00t-managed runtime content using the install manifest.
///
/// Inverse of [`handle_install_command`]. This is what makes
/// `RuntimeAdapter::uninstall()` reachable from the CLI — previously the trait
/// method existed but nothing ever called it, so b00t-managed runtime files
/// (copied skills/extensions, injected settings.json keys, wired MCP servers)
/// could be installed but never cleanly removed.
///
/// Removal is manifest-scoped and therefore safe: adapters delete only files
/// whose SHA256 still matches the manifest, back up anything the user modified,
/// and strip only b00t-injected JSON keys rather than deleting whole configs.
pub fn handle_uninstall_command(
    runtimes_arg: Option<Vec<RuntimeId>>,
    scope_arg: Option<InstallScope>,
    yes: bool,
) -> Result<()> {
    let registry = default_registry();

    // Mirror handle_install_command: explicit --runtimes wins, else detected.
    let runtimes = runtimes_arg.unwrap_or_else(|| {
        registry
            .detected()
            .iter()
            .map(|adapter| adapter.id())
            .collect::<Vec<_>>()
    });
    if runtimes.is_empty() {
        anyhow::bail!(
            "no runtimes given and none detected — pass --runtimes <{}>",
            RuntimeId::all_tokens_joined()
        );
    }
    let scope = scope_arg.unwrap_or(InstallScope::Global);

    if !yes {
        let names: Vec<&str> = runtimes.iter().map(RuntimeId::display_name).collect();
        let scope_str = match &scope {
            InstallScope::Global => "globally".to_string(),
            InstallScope::Local(p) => format!("locally in {}", p.display()),
        };
        let confirmed = inquire::Confirm::new(&format!(
            "Uninstall b00t from [{}] {}? (pass --yes to skip this prompt)",
            names.join(", "),
            scope_str
        ))
        .with_default(false)
        .prompt()
        .map_err(|e| anyhow::anyhow!("Confirmation prompt failed (no TTY available?). Pass --yes to skip confirmation in non-interactive environments. Details: {}", e))?;
        if !confirmed {
            anyhow::bail!("Uninstall cancelled.");
        }
    }

    let mut uninstalled = 0usize;
    for runtime_id in &runtimes {
        let adapter = registry
            .get(runtime_id)
            .ok_or_else(|| anyhow::anyhow!("No adapter for {:?}", runtime_id))?;
        let target = adapter.target_dir(&scope)?;
        let manifest_path = target.join(manifest::MANIFEST_FILENAME);

        if !manifest_path.exists() {
            eprintln!(
                "⚠️  no {} for {} at {} — nothing b00t-managed to remove",
                manifest::MANIFEST_FILENAME,
                runtime_id.display_name(),
                target.display()
            );
            continue;
        }

        let loaded = manifest::B00tInstallManifest::load(&target).map_err(|e| {
            anyhow::anyhow!(
                "failed to read {}: {} — refusing to guess what b00t owns",
                manifest_path.display(),
                e
            )
        })?;

        println!(
            "Uninstalling b00t from {} ({} managed files, {} managed blocks)...",
            runtime_id.display_name(),
            loaded.files.len(),
            loaded.managed_blocks.len()
        );
        adapter.uninstall(&loaded)?;
        // The manifest describes state that no longer exists; drop it so a
        // later uninstall cannot act on stale SHA256 records.
        std::fs::remove_file(&manifest_path).ok();
        println!("✅ {} uninstalled", runtime_id.display_name());
        uninstalled += 1;
    }

    if uninstalled == 0 {
        anyhow::bail!(
            "nothing to uninstall — no b00t install manifest found for: {}",
            runtimes
                .iter()
                .map(RuntimeId::display_name)
                .collect::<Vec<_>>()
                .join(", ")
        );
    }
    println!("\nb00t runtime uninstall complete!");
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
    fn test_default_registry_covers_every_runtime_variant() {
        // 🤓 Derived from RuntimeId::all_variants() rather than a hardcoded count
        //    so adding a runtime cannot silently leave it unregistered.
        let registry = default_registry();
        let registered: Vec<RuntimeId> =
            registry.all_adapters().iter().map(|a| a.id()).collect();
        assert_eq!(
            registered.len(),
            RuntimeId::all_variants().len(),
            "default_registry must register exactly one adapter per RuntimeId"
        );
        for id in RuntimeId::all_variants() {
            assert!(
                registered.contains(&id),
                "runtime {:?} is missing from default_registry",
                id
            );
            assert!(
                registry.get(&id).is_some(),
                "registry.get({:?}) must resolve",
                id
            );
        }
    }

    #[test]
    fn test_runtime_token_roundtrip() {
        // `--runtimes <token>` parsing must accept every registered runtime and
        // reject unknown tokens, so main.rs never needs a per-variant match arm.
        for id in RuntimeId::all_variants() {
            let token = id.token();
            assert_eq!(
                RuntimeId::from_token(token),
                Some(id),
                "token {:?} must roundtrip",
                token
            );
        }
        assert_eq!(RuntimeId::from_token("PI"), Some(RuntimeId::Pi), "case-insensitive");
        assert_eq!(RuntimeId::from_token(" pi "), Some(RuntimeId::Pi), "whitespace-trimmed");
        assert_eq!(RuntimeId::from_token("cursor"), None, "unknown token rejected");
        assert!(
            RuntimeId::all_tokens_joined().contains("pi"),
            "help/error text must list pi"
        );
    }

    // ── handle_uninstall_command: the inverse of handle_install_command ────────
    // 🤓 PiAdapter honours PI_CODING_AGENT_DIR, so these tests exercise the real
    //    manifest-aware uninstall path against a throwaway agent dir.

    #[test]
    fn test_handle_uninstall_command_strips_wired_servers_and_keeps_user_ones() {
        let _mutex = ENV_MUTEX.lock().unwrap_or_else(|e| e.into_inner());
        let tmp = tempfile::TempDir::new().unwrap();
        let agent_dir = tmp.path().join("agent");
        std::fs::create_dir_all(&agent_dir).unwrap();
        let _env = EnvVarRestoreGuard::set("PI_CODING_AGENT_DIR", agent_dir.to_str().unwrap());

        // Simulate a prior install: wired servers alongside a user-added one.
        std::fs::write(
            agent_dir.join("mcp.json"),
            r#"{"mcpServers":{"b00t-mcp":{"command":"b00t-mcp","args":["--stdio"]},"codebase-memory":{"command":"codebase-memory-mcp"},"context7":{"command":"bunx"}},"settings":{"outputGuard":true}}"#,
        )
        .unwrap();
        let mut m = manifest::B00tInstallManifest::new(RuntimeId::Pi, InstallScope::Global);
        m.managed_blocks.push(agent_dir.join("mcp.json"));
        m.save(&agent_dir).unwrap();

        handle_uninstall_command(Some(vec![RuntimeId::Pi]), Some(InstallScope::Global), true)
            .expect("uninstall must succeed against a valid manifest");

        let after: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(agent_dir.join("mcp.json")).unwrap())
                .unwrap();
        for wired in WIRED_MCP_SERVERS {
            assert!(
                after["mcpServers"].get(*wired).is_none(),
                "b00t-wired server {} must be removed",
                wired
            );
        }
        assert!(
            after["mcpServers"].get("context7").is_some(),
            "a server the operator added must survive uninstall"
        );
        assert_eq!(
            after["settings"]["outputGuard"],
            serde_json::json!(true),
            "unrelated settings must not be touched"
        );
        assert!(
            !agent_dir.join(manifest::MANIFEST_FILENAME).exists(),
            "manifest must be consumed so a second uninstall cannot act on stale SHA256 records"
        );
    }

    #[test]
    fn test_handle_uninstall_command_errors_when_no_manifest() {
        let _mutex = ENV_MUTEX.lock().unwrap_or_else(|e| e.into_inner());
        let tmp = tempfile::TempDir::new().unwrap();
        let agent_dir = tmp.path().join("agent-empty");
        std::fs::create_dir_all(&agent_dir).unwrap();
        let _env = EnvVarRestoreGuard::set("PI_CODING_AGENT_DIR", agent_dir.to_str().unwrap());

        let err = handle_uninstall_command(Some(vec![RuntimeId::Pi]), Some(InstallScope::Global), true)
            .expect_err("uninstall without a manifest must fail rather than guess");
        assert!(
            err.to_string().contains("nothing to uninstall"),
            "expected an explicit nothing-to-uninstall error, got: {}",
            err
        );
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

    /// RAII guard that saves the current value of an env var and restores it on drop.
    struct EnvVarRestoreGuard {
        name: &'static str,
        original: Option<String>,
    }

    impl EnvVarRestoreGuard {
        fn remove(name: &'static str) -> Self {
            let original = std::env::var(name).ok();
            unsafe {
                std::env::remove_var(name);
            }
            Self { name, original }
        }

        fn set(name: &'static str, value: &str) -> Self {
            let original = std::env::var(name).ok();
            unsafe {
                std::env::set_var(name, value);
            }
            Self { name, original }
        }
    }

    impl Drop for EnvVarRestoreGuard {
        fn drop(&mut self) {
            unsafe {
                match &self.original {
                    Some(val) => std::env::set_var(self.name, val),
                    None => std::env::remove_var(self.name),
                }
            }
        }
    }

    #[test]
    fn test_runtimes_source_root_from_workspace() {
        // When run from within the repo, runtimes_source_root() is anchored at the
        // git workspace root. Verify the returned path ends with _b00t_/runtimes.
        let _lock = ENV_MUTEX.lock().unwrap();
        let _restore = EnvVarRestoreGuard::remove("B00T_RUNTIMES_SOURCE_ROOT");
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
        let _restore = EnvVarRestoreGuard::remove("B00T_RUNTIMES_SOURCE_ROOT");
        unsafe {
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
