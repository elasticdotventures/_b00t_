# b00t Datum Uninstall — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add `uninstall`/`hook_uninstall` fields to `BootDatum` and implement `b00t uninstall <name>` top-level command.

**Architecture:** Mirror the existing `install_datum()` pattern in `commands/install.rs`. New `commands/uninstall.rs` handles datum lookup, script execution, optional Rhai hook, and `--purge` manifest removal. `Commands::Uninstall` added to top-level enum in `main.rs`.

**Tech Stack:** Rust stable, `duct` (shell execution), `rhai` (hook scripts), `anyhow`, `clap`, `toml`, `tempfile` (tests)

---

## File Map

| File | Action | Purpose |
|------|--------|---------|
| `b00t-cli/src/lib.rs:228-232` | Modify | Add `uninstall` + `hook_uninstall` fields to `BootDatum` |
| `b00t-cli/src/commands/uninstall.rs` | **Create** | `uninstall_datum()` function + tests |
| `b00t-cli/src/commands/mod.rs` | Modify | `pub mod uninstall; pub use uninstall::uninstall_datum;` (use content anchor, not line number) |
| `b00t-cli/src/main.rs:311` | Modify | Add `Commands::Uninstall` variant after `Commands::Install` |
| `b00t-cli/src/main.rs:~1285` | Modify | Handle `Commands::Uninstall` in match block |

All work is in `/home/brianh/.b00t/b00t-cli/`. Run tests from `/home/brianh/.b00t/` with `cargo test -p b00t-cli -- --nocapture`.

---

### Task 1: Extend `BootDatum` with uninstall fields

**Files:**
- Modify: `b00t-cli/src/lib.rs:228-232`

- [ ] **Step 1.1: Write the failing TOML deserialization test**

Add to the existing `#[cfg(test)]` block in `src/lib.rs` (or `src/commands/uninstall.rs` once created):

```rust
#[test]
fn test_bootdatum_uninstall_fields_deserialize() {
    let toml_str = r#"
[b00t]
name = "ripgrep"
type = "cli"
hint = "fast grep"
install = "apt-get install -y ripgrep"
uninstall = "apt-get remove -y ripgrep"
hook_uninstall = "// Rhai: post-uninstall cleanup\nlet x = 1;"
"#;
    let config: crate::UnifiedConfig = toml::from_str(toml_str).unwrap();
    assert_eq!(config.b00t.uninstall, Some("apt-get remove -y ripgrep".to_string()));
    assert!(config.b00t.hook_uninstall.is_some());
}

#[test]
fn test_bootdatum_uninstall_fields_default_none() {
    let toml_str = r#"
[b00t]
name = "docker"
type = "cli"
hint = "containers"
"#;
    let config: crate::UnifiedConfig = toml::from_str(toml_str).unwrap();
    assert!(config.b00t.uninstall.is_none());
    assert!(config.b00t.hook_uninstall.is_none());
}

// 🤓 key generation uses format!("{:?}", DatumType::X).to_lowercase() — matches install.rs pattern
// but is fragile for multi-word types (HiveProfile → "hiveprofile" not "hive_profile").
// Test with cli type only since that's the primary uninstall use case.
// If lookup ever fails for mcp/ai_model datums, fix key generation to use Display not Debug.
```

- [ ] **Step 1.2: Run test to verify it fails**

```bash
cd /home/brianh/.b00t && cargo test -p b00t-cli test_bootdatum_uninstall -- --nocapture
```

Expected: FAIL — `no field 'uninstall'` deserialization error

- [ ] **Step 1.3: Add fields to `BootDatum` in `src/lib.rs`**

After line 231 (`pub hook_learn: Option<String>,`), add:

```rust
    // Uninstall lifecycle
    // 🤓 hook_uninstall: runs after uninstall script; EvalAltResult aborts (fatal); Warn/Redirect continue
    pub uninstall: Option<String>,
    pub hook_uninstall: Option<String>,
```

- [ ] **Step 1.4: Run test to verify it passes**

```bash
cd /home/brianh/.b00t && cargo test -p b00t-cli test_bootdatum_uninstall -- --nocapture
```

Expected: PASS — 2 tests pass

- [ ] **Step 1.5: Verify full test suite still passes**

```bash
cd /home/brianh/.b00t && cargo test -p b00t-cli -- --nocapture 2>&1 | tail -5
```

Expected: `test result: ok`

- [ ] **Step 1.6: Commit**

```bash
cd /home/brianh/.b00t && git add b00t-cli/src/lib.rs && git commit -m "feat(datum): add uninstall + hook_uninstall fields to BootDatum"
```

---

### Task 2: Implement `commands/uninstall.rs`

**Files:**
- Create: `b00t-cli/src/commands/uninstall.rs`

- [ ] **Step 2.1: Write the failing tests first**

Create `b00t-cli/src/commands/uninstall.rs` with tests only:

```rust
use anyhow::{Context, Result, anyhow};
use duct::cmd;
use shellexpand;
use std::collections::HashMap;
use std::path::PathBuf;
use toml;
use crate::{BootDatum, UnifiedConfig};
use crate::hook_engine::{run_hook, HookResult};

/// Execute uninstall for a named datum.
/// - Loads datum from `path`
/// - Requires `datum.uninstall` field; errors if absent
/// - Prompts confirmation unless `yes=true`
/// - Executes `datum.uninstall` shell script
/// - Executes `datum.hook_uninstall` Rhai script if present (failure is fatal)
/// - If `purge=true`, removes datum entry from _b00t_.toml
pub fn uninstall_datum(path: &str, name: &str, yes: bool, purge: bool) -> Result<()> {
    todo!("implement in step 2.3")
}
// 🤓 Signature: name is &str (not String) — caller passes &name from the Commands match arm

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
                                .map(|t| format!("{:?}", t).to_lowercase())
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

        // Create a minimal _b00t_.toml that lists the datum
        let manifest_path = dir.path().join("_b00t_.toml");
        fs::write(&manifest_path, r#"datums = ["mytool.cli", "other.cli"]"#).unwrap();

        let result = uninstall_datum(dir.path().to_str().unwrap(), "mytool", true, true);
        assert!(result.is_ok(), "Expected ok, got: {:?}", result);

        // mytool must be removed from the manifest
        let content = fs::read_to_string(&manifest_path).unwrap();
        assert!(!content.contains("mytool"), "mytool should be purged from _b00t_.toml, got: {}", content);
        // other entry must be preserved
        assert!(content.contains("other.cli"), "other.cli should be preserved");
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
```

- [ ] **Step 2.2: Run tests to verify they fail**

```bash
cd /home/brianh/.b00t && cargo test -p b00t-cli uninstall -- --nocapture
```

Expected: FAIL — `todo!` panics

- [ ] **Step 2.3: Implement `uninstall_datum()`**

Replace the `todo!()` stub:

```rust
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
    println!("🗑️  Uninstalling {}...", key);
    cmd!("bash", "-c", uninstall_script)
        .run()
        .with_context(|| format!("Uninstall script failed for {}", key))?;
    println!("✅ Uninstalled {}", key);

    // Execute hook_uninstall Rhai script if present
    // 🤓 Uses hook_engine::run_hook() — consistent with detect/install/update hooks.
    // Divergence from hook_engine default: here Warn messages starting with "hook script error:"
    // are treated as FATAL (the spec requires EvalAltResult to abort uninstall).
    // All other HookResult variants (Warn/Redirect/Info/Missing) are non-fatal: log + continue.
    if let Some(hook_script) = &datum.hook_uninstall {
        println!("🪝  Running hook_uninstall for {}...", key);
        match run_hook(hook_script) {
            HookResult::Ok => {},
            HookResult::Warn(msg) if msg.starts_with("hook script error:") => {
                return Err(anyhow!("hook_uninstall aborted for {}: {}", key, msg));
            }
            HookResult::Warn(msg) => eprintln!("⚠️  hook_uninstall [{}]: {}", key, msg),
            HookResult::Redirect(target) => eprintln!("↪️  hook_uninstall [{}]: redirect to {}", key, target),
            HookResult::Missing(msg) => eprintln!("⚠️  hook_uninstall [{}]: missing {}", key, msg),
            HookResult::Info(msg) => println!("ℹ️  hook_uninstall [{}]: {}", key, msg),
        }
    }

    // --purge: remove datum entry from _b00t_.toml manifest
    if purge {
        remove_from_manifest(path, &key)?;
    }

    Ok(())
}

// 🤓 hook_uninstall is executed via hook_engine::run_hook() — see uninstall_datum() above.
// No separate run_hook_uninstall() function needed; hook_engine is the canonical executor.

/// Remove datum key from global _b00t_.toml `datums = [...]` list.
/// NOTE: `_b00t_.toml` lives at the repo root or `~/.b00t/_b00t_.toml` — it is **not** inside the datum dir.
/// 🤓 Prefer delegating to `B00tConfig::remove_datum()`; if not available, use `B00tConfig::find_config_path()` as below.
fn remove_from_manifest(path: &str, key: &str) -> Result<()> {
    // Use the same discovery logic as the rest of b00t; do NOT assume `path/_b00t_.toml`.
    let Some(b00t_toml) = B00tConfig::find_config_path() else {
        eprintln!("⚠️  _b00t_.toml not found in repo or ~/.b00t, skipping purge");
        return Ok(());
    };

    if !b00t_toml.exists() {
        eprintln!("⚠️  _b00t_.toml not found at {}, skipping purge", b00t_toml.display());
        return Ok(());
    }

    let content = std::fs::read_to_string(&b00t_toml)
        .with_context(|| format!("Failed to read {}", b00t_toml.display()))?;

    // Remove the datum entry from the datums list (simple string replace — preserves formatting)
    let name = key.split('.').next().unwrap_or(key);
    let cleaned = content
        .lines()
        .filter(|line| !line.contains(&format!("\"{}\"", name)) && !line.contains(&format!("\"{}\"", key)))
        .collect::<Vec<_>>()
        .join("\n");

    std::fs::write(&b00t_toml, cleaned)
        .with_context(|| format!("Failed to write {}", b00t_toml.display()))?;

    println!("🗑️  Removed '{}' from _b00t_.toml", key);
    Ok(())
}
```

- [ ] **Step 2.4: Run tests to verify they pass**

```bash
cd /home/brianh/.b00t && cargo test -p b00t-cli uninstall -- --nocapture
```

Expected: 5 tests PASS

- [ ] **Step 2.5: Wire `pub mod uninstall` in `commands/mod.rs` NOW** (prevents dangling module between commits)

In `b00t-cli/src/commands/mod.rs`, after `pub mod tutorial;` (content anchor):
```rust
pub mod uninstall;
```
After `pub use tutorial::TutorialCommands;` (content anchor):
```rust
pub use uninstall::uninstall_datum;
```

- [ ] **Step 2.6: Compile check**

```bash
cd /home/brianh/.b00t && cargo build -p b00t-cli 2>&1 | grep -E "^error"
```

Expected: no errors (Commands::Uninstall not yet added — `uninstall.rs` compiles as a module even without the CLI variant)

- [ ] **Step 2.7: Commit**

```bash
cd /home/brianh/.b00t && git add b00t-cli/src/commands/uninstall.rs b00t-cli/src/commands/mod.rs && git commit -m "feat(uninstall): implement uninstall_datum with Rhai hook support"
```

---

### Task 3: Wire into `commands/mod.rs` and `main.rs`

**Files:**
- Modify: `b00t-cli/src/commands/mod.rs:34`
- Modify: `b00t-cli/src/main.rs`

> `commands/mod.rs` was already updated in Task 2 Step 2.5. Skip any mod.rs changes here.

- [ ] **Step 3.1: Write the failing CLI integration test**

Add to `src/integration_tests.rs`:

```rust
#[cfg(test)]
mod uninstall_integration {
    use assert_cmd::Command;
    use tempfile::TempDir;
    use std::fs;

    fn write_uninstall_datum(dir: &TempDir, name: &str, script: &str) {
        let content = format!(
            "[b00t]\nname = {:?}\ntype = \"cli\"\nhint = \"test\"\nuninstall = {:?}\n",
            name, script
        );
        fs::write(dir.path().join(format!("{}.cli.toml", name)), content).unwrap();
    }

    #[test]
    fn test_uninstall_command_not_found() {
        let dir = TempDir::new().unwrap();
        let mut cmd = Command::cargo_bin("b00t-cli").unwrap();
        cmd.args(["--path", dir.path().to_str().unwrap(), "uninstall", "--yes", "nonexistent"]);
        cmd.assert().failure().stderr(predicates::str::contains("not found"));
    }

    #[test]
    fn test_uninstall_command_executes() {
        let dir = TempDir::new().unwrap();
        let marker = dir.path().join("removed.txt");
        write_uninstall_datum(&dir, "mytool", &format!("touch {}", marker.display()));
        let mut cmd = Command::cargo_bin("b00t-cli").unwrap();
        cmd.args(["--path", dir.path().to_str().unwrap(), "uninstall", "--yes", "mytool"]);
        cmd.assert().success();
        assert!(marker.exists());
    }
}
```

Note: `predicates` crate is already a transitive dep via `assert_cmd`. If it's not available directly, use `stderr.contains("not found")` pattern from existing integration tests.

- [ ] **Step 3.2: Run test to verify it fails**

```bash
cd /home/brianh/.b00t && cargo test -p b00t-cli uninstall_integration -- --nocapture
```

Expected: compile error — `Commands::Uninstall` doesn't exist yet

- [ ] **Step 3.3: Add `Commands::Uninstall` variant to `main.rs`**

In `src/main.rs`, after the `Install` variant (around line 311):

```rust
    #[clap(about = "Uninstall a datum by name (use --purge to remove from _b00t_.toml)")]
    Uninstall {
        #[clap(help = "Datum name or key, e.g. 'ripgrep' or 'ripgrep.cli'")]
        name: String,
        #[clap(long, help = "Also remove datum entry from _b00t_.toml")]
        purge: bool,
        #[clap(long, short = 'y', help = "Skip confirmation prompt")]
        yes: bool,
    },
```

- [ ] **Step 3.4: Add import in `main.rs`**

Find the existing install import line (~line 37):
```rust
use b00t_cli::commands::install::{install_datum, run_just_install};
```
Add alongside it:
```rust
use b00t_cli::commands::uninstall::uninstall_datum;
```

- [ ] **Step 3.5: Handle `Commands::Uninstall` in the match block**

After the `Commands::Install` match arm (~line 1283). Note: `Commands` is matched by value, so
`name: String`, `purge: bool`, `yes: bool` — do NOT dereference `bool` values (no `*`):

```rust
        Some(Commands::Uninstall { name, purge, yes }) => {
            if let Err(e) = uninstall_datum(&cli.path, &name, yes, purge) {
                eprintln!("Uninstall Error: {}", e);
                std::process::exit(1);
            }
        }
```

Also add the import alongside the existing install import (find: `use b00t_cli::commands::install::`):
```rust
use b00t_cli::commands::uninstall::uninstall_datum;
```

- [ ] **Step 3.7: Compile check**

```bash
cd /home/brianh/.b00t && cargo build -p b00t-cli 2>&1 | grep -E "^error"
```

Expected: no errors

- [ ] **Step 3.8: Run integration test**

```bash
cd /home/brianh/.b00t && cargo test -p b00t-cli uninstall_integration -- --nocapture
```

Expected: 2 tests PASS

- [ ] **Step 3.9: Run full test suite**

```bash
cd /home/brianh/.b00t && cargo test -p b00t-cli -- --nocapture 2>&1 | tail -5
```

Expected: `test result: ok`

- [ ] **Step 3.10: Commit**

```bash
cd /home/brianh/.b00t && git add b00t-cli/src/commands/mod.rs b00t-cli/src/main.rs && git commit -m "feat(cli): wire b00t uninstall command to top-level Commands enum"
```

---

### Task 4: Update an example datum with `uninstall` field

Demonstrate the new capability with a real datum file to validate end-to-end.

**Files:**
- Modify: `/home/brianh/.b00t/_b00t_/ripgrep.cli.toml` (or whichever cli datum is simplest)

- [ ] **Step 4.1: Find an appropriate datum to update**

```bash
ls /home/brianh/.b00t/_b00t_/*.cli.toml | head -5
```

Pick a simple one (e.g. `ripgrep.cli.toml` or `fdfind.cli.toml`).

- [ ] **Step 4.2: Add `uninstall` field to the chosen datum**

Open the file and add after the `install` field:

```toml
uninstall = "sudo apt-get remove -y ripgrep"
```

- [ ] **Step 4.3: Verify `b00t uninstall --dry-run` behavior**

Since we don't have `--dry-run` yet, do a manual smoke test that the datum is found:

```bash
b00t-cli --path ~/.b00t/_b00t_ uninstall --yes nonexistent 2>&1 | grep "not found"
```

Expected: `Datum 'nonexistent' not found`

- [ ] **Step 4.4: Commit**

```bash
cd /home/brianh/.b00t && git add _b00t_/ripgrep.cli.toml && git commit -m "docs(datum): add uninstall field to ripgrep.cli example"
```

---

## Verification

```bash
# All tests pass
cd /home/brianh/.b00t && cargo test -p b00t-cli -- --nocapture 2>&1 | tail -3

# CLI help shows uninstall command
b00t-cli --help | grep uninstall

# Uninstall errors correctly when datum missing
b00t-cli --path /tmp uninstall --yes nonexistent
# → "Datum 'nonexistent' not found"

# Uninstall errors correctly when field missing
b00t-cli --path ~/.b00t/_b00t_ uninstall --yes docker
# → "has no uninstall script" (if docker has no uninstall field)
```
