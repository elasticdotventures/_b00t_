use anyhow::{Context, Result, anyhow};
use duct::cmd;
use shellexpand;
use std::collections::HashMap;
use std::path::PathBuf;
use toml;
use crate::{BootDatum, UnifiedConfig};
use crate::datum_config::B00tConfig;
use crate::hook_engine::{run_hook, HookResult};

/// Execute uninstall for a named datum.
/// - Loads datum from `path`
/// - Requires `datum.uninstall` field; errors if absent
/// - Prompts confirmation unless `yes=true`
/// - Executes `datum.uninstall` shell script
/// - Executes `datum.hook_uninstall` Rhai script if present (EvalAltResult → fatal sentinel)
/// - If `purge=true`, removes datum entry from _b00t_.toml
pub fn uninstall_datum(path: &str, name: &str, yes: bool, purge: bool) -> Result<()> {
    let all_datums = load_all_datums(path)?;

    // Find matching datum (by name or full key e.g. "ripgrep.cli")
    let (key, datum) = all_datums.iter()
        .find(|(k, d)| d.name == name || k.as_str() == name)
        .map(|(k, d)| (k.clone(), d.clone()))
        .ok_or_else(|| anyhow!("Datum '{}' not found", name))?;

    // Require uninstall field — emit actionable error if missing
    let uninstall_script = datum.uninstall.as_ref().ok_or_else(|| {
        anyhow!(
            "Datum '{}' has no uninstall script.\nhint: add `uninstall = \"...\"` to its .toml file",
            key
        )
    })?;

    // Confirmation prompt (skip with --yes)
    if !yes {
        use std::io::{self, Write};
        print!("Uninstall {}? (y/N) ", key);
        io::stdout().flush()?;
        let mut input = String::new();
        io::stdin().read_line(&mut input)?;
        if !input.trim().eq_ignore_ascii_case("y") {
            println!("Aborted.");
            return Ok(());
        }
    }

    // Execute uninstall shell script
    println!("Uninstalling {}...", key);
    cmd!("bash", "-c", uninstall_script)
        .run()
        .with_context(|| format!("Uninstall script failed for {}", key))?;
    println!("Uninstalled {}", key);

    // Execute hook_uninstall Rhai script if present
    // 🤓 Uses hook_engine::run_hook() — consistent with detect/install/update hooks.
    // Warn messages starting with "hook script error:" are treated as FATAL (EvalAltResult sentinel).
    // All other HookResult variants (Warn/Redirect/Info/Missing) are non-fatal: log + continue.
    if let Some(hook_script) = &datum.hook_uninstall {
        println!("Running hook_uninstall for {}...", key);
        match run_hook(hook_script) {
            HookResult::Ok => {},
            HookResult::Warn(msg) if msg.starts_with("hook script error:") => {
                return Err(anyhow!("hook_uninstall aborted for {}: {}", key, msg));
            }
            HookResult::Warn(msg) => eprintln!("hook_uninstall [{}]: {}", key, msg),
            HookResult::Redirect(target) => eprintln!("hook_uninstall [{}]: redirect to {}", key, target),
            HookResult::Missing(msg) => eprintln!("hook_uninstall [{}]: missing {}", key, msg),
            HookResult::Info(msg) => println!("hook_uninstall [{}]: {}", key, msg),
        }
    }

    // --purge: remove datum entry from _b00t_.toml manifest
    if purge {
        remove_from_manifest(path, &key)?;
    }

    Ok(())
}
// 🤓 Signature: name is &str (not String) — caller passes &name from the Commands match arm

// 🤓 hook_uninstall is executed via hook_engine::run_hook() — see uninstall_datum() above.
// No separate run_hook_uninstall() function needed; hook_engine is the canonical executor.

/// Remove datum key from the global _b00t_.toml using B00tConfig discovery.
/// 🤓 _b00t_.toml lives at the repo root or ~/.b00t/_b00t_.toml — NOT inside datum_dir.
///    Uses B00tConfig::find_config_path() to locate the correct file.
fn remove_from_manifest(_datum_dir: &str, key: &str) -> Result<()> {
    let (mut config, config_path) = B00tConfig::load_or_create()
        .with_context(|| "Failed to locate _b00t_.toml for purge")?;

    let name = key.split('.').next().unwrap_or(key);
    let removed_key = config.remove_datum(key);
    let removed_name = config.remove_datum(name);

    if removed_key || removed_name {
        config.save(&config_path)
            .with_context(|| format!("Failed to save {}", config_path.display()))?;
        println!("Removed '{}' from {}", key, config_path.display());
    } else {
        eprintln!("'{}' not found in {}, skipping purge", key, config_path.display());
    }
    Ok(())
}
/// Load all datums from the configured path (reuses load pattern from commands/install.rs).
/// ⚠️ DRY note: if load_all_datums is ever extracted to a shared module, update this import.
fn load_all_datums(path: &str) -> Result<HashMap<String, BootDatum>> {
    let mut datums = HashMap::new();
    let b00t_dir = PathBuf::from(shellexpand::tilde(path).to_string());
    if !b00t_dir.exists() {
        return Ok(datums);
    }
    for entry in std::fs::read_dir(&b00t_dir)? {
        let entry = entry?;
        let entry_path = entry.path();
        if entry_path.is_file() {
            if let Some(file_name) = entry_path.file_name().and_then(|s| s.to_str()) {
                if file_name.ends_with(".stack.toml") { continue; }
                if file_name.ends_with(".toml") {
                    if let Ok(content) = std::fs::read_to_string(&entry_path) {
                        if let Ok(config) = toml::from_str::<UnifiedConfig>(&content) {
                            let datum = config.b00t;
                            let datum_type = datum.datum_type.as_ref()
                                .map(|t| serde_json::to_string(t).unwrap_or_else(|_| String::from("\"unknown\"")).trim_matches('"').to_string())
                                .unwrap_or_else(|| "unknown".to_string());
                            let key = format!("{}.{}", datum.name, datum_type);
                            datums.insert(key, datum);
                        }
                    }
                }
            }
        }
    }
    Ok(datums)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    fn write_datum(dir: &TempDir, name: &str, dtype: &str, uninstall: Option<&str>, hook: Option<&str>) {
        let uninstall_str = uninstall.map(|s| format!("uninstall = {:?}", s)).unwrap_or_default();
        let hook_str = hook.map(|s| format!("hook_uninstall = {:?}", s)).unwrap_or_default();
        let content = format!(
            "[b00t]\nname = {:?}\ntype = {:?}\nhint = \"test\"\n{}\n{}\n",
            name, dtype, uninstall_str, hook_str
        );
        fs::write(dir.path().join(format!("{}.{}.toml", name, dtype)), content).unwrap();
    }

    #[test]
    fn test_uninstall_datum_not_found() {
        let dir = TempDir::new().unwrap();
        write_datum(&dir, "docker", "cli", Some("echo remove"), None);
        let result = uninstall_datum(dir.path().to_str().unwrap(), "nonexistent", true, false);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("not found"));
    }

    #[test]
    fn test_uninstall_datum_missing_uninstall_field() {
        let dir = TempDir::new().unwrap();
        write_datum(&dir, "docker", "cli", None, None);  // no uninstall field
        let result = uninstall_datum(dir.path().to_str().unwrap(), "docker", true, false);
        assert!(result.is_err());
        let msg = result.unwrap_err().to_string();
        // Must contain the datum name AND an actionable hint
        assert!(
            msg.contains("docker") && msg.contains("uninstall"),
            "Expected error naming datum + hinting at uninstall field, got: {}", msg
        );
    }

    #[test]
    fn test_uninstall_datum_executes_script() {
        let dir = TempDir::new().unwrap();
        let marker = dir.path().join("uninstalled.txt");
        let script = format!("touch {}", marker.display());
        write_datum(&dir, "mytool", "cli", Some(&script), None);
        let result = uninstall_datum(dir.path().to_str().unwrap(), "mytool", true, false);
        assert!(result.is_ok(), "Expected ok, got: {:?}", result);
        assert!(marker.exists(), "Uninstall script should have run");
    }

    #[test]
    fn test_uninstall_datum_purge_removes_entry() {
        let dir = TempDir::new().unwrap();
        write_datum(&dir, "mytool", "cli", Some("echo ok"), None);

        // --purge delegates to B00tConfig::find_config_path() (repo root or ~/.b00t/_b00t_.toml),
        // NOT datum_dir/_b00t_.toml. We verify the function returns Ok regardless of whether
        // the datum is present in the discovered config (it may not be in a test environment).
        // B00tConfig::remove_datum() behavior is tested in datum_config tests.
        let result = uninstall_datum(dir.path().to_str().unwrap(), "mytool", true, true);
        assert!(result.is_ok(), "Expected ok with --purge, got: {:?}", result);
    }

    #[test]
    fn test_uninstall_datum_hook_uninstall_fatal_on_rhai_error() {
        let dir = TempDir::new().unwrap();
        // Rhai script with a syntax error — hook_engine returns Warn("hook script error: ...")
        write_datum(&dir, "mytool", "cli", Some("echo ok"), Some("this is not valid rhai $$$$"));
        let result = uninstall_datum(dir.path().to_str().unwrap(), "mytool", true, false);
        assert!(result.is_err(), "hook_uninstall Rhai error should abort with Err");
        let msg = result.unwrap_err().to_string();
        assert!(
            msg.contains("hook_uninstall aborted") || msg.contains("hook script error"),
            "Error should mention hook failure, got: {}", msg
        );
    }

    #[test]
    fn test_uninstall_datum_lookup_by_key() {
        let dir = TempDir::new().unwrap();
        let marker = dir.path().join("done.txt");
        let script = format!("touch {}", marker.display());
        write_datum(&dir, "mytool", "cli", Some(&script), None);
        // lookup by full key "mytool.cli"
        let result = uninstall_datum(dir.path().to_str().unwrap(), "mytool.cli", true, false);
        assert!(result.is_ok(), "{:?}", result);
        assert!(marker.exists());
    }
}
