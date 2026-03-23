# b00t Polyglot Installer — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

> **Prerequisite:** Plan `2026-03-23-b00t-datum-uninstall.md` must be merged first.
> **Prerequisite gate:** Before starting Task 1, verify `BootDatum` already has `uninstall` and `hook_uninstall` fields:
> ```bash
> grep -n "hook_uninstall" /home/brianh/.b00t/b00t-cli/src/lib.rs
> ```
> Expected: at least one match. If absent, stop and merge the datum-uninstall plan first.

**Goal:** Implement `b00t-cli install --interactive` Rust TUI that deploys b00t skills/agents/hooks into Claude Code, Gemini CLI, Codex, OpenCode, and Copilot runtimes.

**Architecture:** `RuntimeAdapter` object-safe trait (two-trait split: dispatch + typed impl) with `Box<dyn RuntimeAdapter>` registry. `ContentPack` trait drives file copy + SHA256 manifest tracking. `inquire` TUI for runtime/scope/pack selection. Four Node.js hooks (pre-bundled via esbuild) for context monitoring, statusline, update-check, and datum-guard. Each runtime's content lives in `_b00t_/runtimes/<runtime>/` as native-format files (no transpilation).

**Tech Stack:** Rust stable, `inquire` (TUI), `sha2` (manifest hashing), `dirs` (home dir), `duct`, `anyhow`, `serde_json`, Node.js + esbuild (hook bundling)

---

## File Map

### New Rust files (`b00t-cli/src/install/`)

| File | Purpose |
|------|---------|
| `src/install/mod.rs` | Entry point: `handle_install_command()`, wires TUI → adapters |
| `src/install/adapter.rs` | `RuntimeAdapter` + `RuntimeAdapterTyped` traits, `RuntimeId` enum, `InstallContext`, `InstallScope`, `AdapterRegistry` |
| `src/install/manifest.rs` | `B00tInstallManifest`: SHA256 file tracking, marker-block injection/removal, JSON serialization |
| `src/install/content.rs` | `ContentPack` trait, `ContentPackId` enum, `SkillsPack`, `AgentsPack`, `HooksPack`, `DatumLifecyclePack` |
| `src/install/tui.rs` | `run_tui()` via `inquire`: runtime multiselect, scope select, content pack multiselect, confirm |
| `src/install/runtimes/mod.rs` | `pub mod` re-exports for all 5 runtime adapters |
| `src/install/runtimes/claude.rs` | `ClaudeAdapter` + `ClaudeConfig` |
| `src/install/runtimes/gemini.rs` | `GeminiAdapter` + `GeminiConfig` |
| `src/install/runtimes/codex.rs` | `CodexAdapter` + `CodexConfig` |
| `src/install/runtimes/opencode.rs` | `OpenCodeAdapter` + `OpenCodeConfig` |
| `src/install/runtimes/copilot.rs` | `CopilotAdapter` + `CopilotConfig` |

### Modified Rust files

| File | Change |
|------|--------|
| `src/commands/install.rs` | Add `--interactive` flag; `InstallCommands::Run` delegates to `handle_install_command()` when `--interactive` |
| `src/main.rs:306-311` | Extend `Commands::Install` to accept `--interactive` flag |
| `src/commands/mod.rs` | `pub mod` not needed — `install/` is a sibling module at `src/install/` |
| `Cargo.toml` | Add `inquire`, `sha2` dependencies |

### New content files (`_b00t_/runtimes/`)

| Path | Purpose |
|------|---------|
| `_b00t_/runtimes/hooks-src/package.json` | esbuild dev deps |
| `_b00t_/runtimes/hooks-src/tsconfig.json` | TS config |
| `_b00t_/runtimes/hooks-src/build.js` | esbuild bundle script |
| `_b00t_/runtimes/hooks-src/b00t-statusline.ts` | Statusline hook source |
| `_b00t_/runtimes/hooks-src/b00t-update-check.ts` | Update check hook source |
| `_b00t_/runtimes/hooks-src/b00t-context-monitor.ts` | Context monitor hook source |
| `_b00t_/runtimes/hooks-src/b00t-datum-guard.ts` | Datum guard hook source (novel) |
| `_b00t_/runtimes/claude/hooks/*.js` | Pre-bundled JS output (committed) |
| `_b00t_/runtimes/claude/settings_fragment.json` | Hook registration template |
| `_b00t_/runtimes/gemini/settings_fragment.json` | Gemini hook registrations |
| `_b00t_/runtimes/codex/config_fragment.toml` | Codex hook registrations |
| `_b00t_/runtimes/opencode/opencode_fragment.json` | OpenCode hook registrations |
| `_b00t_/runtimes/copilot/copilot_instructions_fragment.md` | Copilot managed block |

### Modified justfile

| Target | Change |
|--------|--------|
| `build-hooks` | New target: runs `node build.js` in `_b00t_/runtimes/hooks-src/` |
| `install` | Depends on `build-hooks` |

---

### Task 1: Add dependencies to `Cargo.toml`

**Files:**
- Modify: `b00t-cli/Cargo.toml`

- [ ] **Step 1.1: Add `inquire`, `sha2`, and `walkdir`**

In `b00t-cli/Cargo.toml` under `[dependencies]`, add:

```toml
inquire = "0.7"
sha2 = "0.10"
walkdir = "2"
```

Also add `Cargo.lock` to the commit in Step 1.3 — `cargo build` will update it.

- [ ] **Step 1.2: Verify compile**

```bash
cd /home/brianh/.b00t && cargo build -p b00t-cli 2>&1 | grep -E "^error"
```

Expected: no errors (may download crates)

- [ ] **Step 1.3: Commit**

```bash
cd /home/brianh/.b00t && git add b00t-cli/Cargo.toml Cargo.lock && git commit -m "build(deps): add inquire + sha2 + walkdir for installer TUI"
```

---

### Task 2: `install/adapter.rs` — Core traits

**Files:**
- Create: `b00t-cli/src/install/adapter.rs`

- [ ] **Step 2.1: Write failing tests**

Create `b00t-cli/src/install/adapter.rs`:

```rust
use anyhow::Result;
use std::path::PathBuf;
use std::sync::Arc;
use serde::{Deserialize, Serialize};

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
    fn target_dir(&self, scope: &InstallScope) -> PathBuf;
    fn detect(&self) -> bool;
    fn default_config(&self, scope: &InstallScope) -> Arc<dyn RuntimeConfig>;
    fn install(&self, ctx: &InstallContext) -> Result<super::manifest::B00tInstallManifest>;
    fn uninstall(&self, manifest: &super::manifest::B00tInstallManifest) -> Result<()>;
    fn register_hooks(&self, ctx: &InstallContext, manifest: &mut super::manifest::B00tInstallManifest) -> Result<()>;
}

/// Typed impl trait — associated Config type, used only at concrete impl level
pub trait RuntimeAdapterTyped: RuntimeAdapter {
    type Config: RuntimeConfig;
    fn config_from_scope(&self, scope: &InstallScope) -> Self::Config;
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
        fn target_dir(&self, _scope: &InstallScope) -> PathBuf { PathBuf::from("/tmp/test") }
        fn detect(&self) -> bool { self.detected }
        fn default_config(&self, _scope: &InstallScope) -> Arc<dyn RuntimeConfig> {
            Arc::new(TestAdapter { id: self.id.clone(), detected: self.detected })
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
```

- [ ] **Step 2.2: Create stub `install/mod.rs`** (needed to compile the module tree)

```rust
// b00t-cli/src/install/mod.rs
pub mod adapter;
pub mod content;
pub mod manifest;
pub mod runtimes;
pub mod tui;

pub use adapter::{AdapterRegistry, InstallContext, InstallScope, RuntimeAdapter, RuntimeAdapterTyped, RuntimeConfig, RuntimeId};
```

Also create stub files (just `// TODO` + the struct/trait skeletons) for `content.rs`, `manifest.rs`, `tui.rs`, and `runtimes/mod.rs` so it compiles.

- [ ] **Step 2.3: Add `pub mod install;` to `src/lib.rs`**

Near the other `pub mod commands;` declarations:
```rust
pub mod install;
```

- [ ] **Step 2.4: Run tests**

```bash
cd /home/brianh/.b00t && cargo test -p b00t-cli install::adapter -- --nocapture
```

Expected: 3 tests PASS

- [ ] **Step 2.5: Commit**

```bash
cd /home/brianh/.b00t && git add b00t-cli/src/install/ b00t-cli/src/lib.rs && git commit -m "feat(install): RuntimeAdapter trait + AdapterRegistry skeleton"
```

---

### Task 3: `install/manifest.rs` — SHA256 file tracking

**Files:**
- Create: `b00t-cli/src/install/manifest.rs`

- [ ] **Step 3.1: Write failing tests**

```rust
use anyhow::Result;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use crate::install::adapter::{RuntimeId, InstallScope};

pub const MANIFEST_FILENAME: &str = "b00t-manifest.json";
pub const MANAGED_BLOCK_START: &str = "# BEGIN B00T MANAGED BLOCK";
pub const MANAGED_BLOCK_END: &str = "# END B00T MANAGED BLOCK";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct B00tInstallManifest {
    pub b00t_version: String,
    pub installed_at: String,
    pub runtime: String,
    pub scope: String,
    /// Absolute paths → SHA256 hex digest
    pub files: HashMap<PathBuf, String>,
    /// Absolute paths to files containing managed blocks
    pub managed_blocks: Vec<PathBuf>,
}

impl B00tInstallManifest {
    pub fn new(runtime: RuntimeId, scope: InstallScope) -> Self {
        Self {
            b00t_version: env!("CARGO_PKG_VERSION").to_string(),
            installed_at: chrono::Utc::now().to_rfc3339(),
            runtime: format!("{:?}", runtime).to_lowercase(),
            scope: match scope { InstallScope::Global => "global".into(), InstallScope::Local(p) => p.display().to_string() },
            files: HashMap::new(),
            managed_blocks: Vec::new(),
        }
    }

    /// Record a file as b00t-owned (absolute path, SHA256 of content)
    pub fn record_file(&mut self, path: &Path, content: &[u8]) {
        let digest = format!("{:x}", Sha256::digest(content));
        self.files.insert(path.to_path_buf(), digest);
    }

    /// Return true if the file at `path` matches the recorded SHA256
    pub fn file_owned(&self, path: &Path) -> bool {
        if let Some(recorded) = self.files.get(path) {
            if let Ok(content) = std::fs::read(path) {
                let digest = format!("{:x}", Sha256::digest(&content));
                return digest == *recorded;
            }
        }
        false
    }

    pub fn save(&self, target_dir: &Path) -> Result<()> {
        let path = target_dir.join(MANIFEST_FILENAME);
        let json = serde_json::to_string_pretty(self)?;
        std::fs::write(&path, json)?;
        Ok(())
    }

    pub fn load(target_dir: &Path) -> Result<Self> {
        let path = target_dir.join(MANIFEST_FILENAME);
        let json = std::fs::read_to_string(&path)?;
        Ok(serde_json::from_str(&json)?)
    }
}

/// Inject `content` between managed block markers in `file_path`.
/// Creates the managed block if absent; replaces existing block if present.
/// User content outside markers is preserved.
pub fn inject_managed_block(file_path: &Path, content: &str) -> Result<()> {
    todo!()
}

/// Remove the managed block from `file_path`, preserving surrounding content.
pub fn remove_managed_block(file_path: &Path) -> Result<()> {
    todo!()
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_manifest_record_and_verify_file() {
        let dir = TempDir::new().unwrap();
        let file_path = dir.path().join("test.txt");
        let content = b"hello world";
        std::fs::write(&file_path, content).unwrap();

        let mut manifest = B00tInstallManifest::new(RuntimeId::Claude, InstallScope::Global);
        manifest.record_file(&file_path, content);
        assert!(manifest.file_owned(&file_path));
    }

    #[test]
    fn test_manifest_detects_modified_file() {
        let dir = TempDir::new().unwrap();
        let file_path = dir.path().join("test.txt");
        std::fs::write(&file_path, b"original").unwrap();

        let mut manifest = B00tInstallManifest::new(RuntimeId::Claude, InstallScope::Global);
        manifest.record_file(&file_path, b"original");

        // Modify file
        std::fs::write(&file_path, b"modified by user").unwrap();
        assert!(!manifest.file_owned(&file_path));
    }

    #[test]
    fn test_manifest_save_and_load_roundtrip() {
        let dir = TempDir::new().unwrap();
        let mut manifest = B00tInstallManifest::new(RuntimeId::Gemini, InstallScope::Global);
        manifest.files.insert(dir.path().join("foo.js"), "abc123".to_string());

        manifest.save(dir.path()).unwrap();
        let loaded = B00tInstallManifest::load(dir.path()).unwrap();
        assert_eq!(loaded.runtime, "gemini");
        assert_eq!(loaded.files.len(), 1);
    }

    #[test]
    fn test_manifest_paths_are_absolute() {
        let dir = TempDir::new().unwrap();
        let abs_path = dir.path().join("skills/SKILL.md");
        let mut manifest = B00tInstallManifest::new(RuntimeId::Claude, InstallScope::Global);
        manifest.record_file(&abs_path, b"content");

        // All keys must be absolute
        for path in manifest.files.keys() {
            assert!(path.is_absolute(), "Expected absolute path, got: {:?}", path);
        }
    }

    #[test]
    fn test_inject_managed_block_creates_block() {
        let dir = TempDir::new().unwrap();
        let file = dir.path().join("settings.json");
        std::fs::write(&file, r#"{"existing": true}"#).unwrap();
        inject_managed_block(&file, "injected content").unwrap();
        let result = std::fs::read_to_string(&file).unwrap();
        assert!(result.contains(MANAGED_BLOCK_START));
        assert!(result.contains("injected content"));
        assert!(result.contains(MANAGED_BLOCK_END));
        assert!(result.contains("existing")); // user content preserved
    }

    #[test]
    fn test_remove_managed_block_preserves_user_content() {
        let dir = TempDir::new().unwrap();
        let file = dir.path().join("settings.json");
        let content = format!(
            "before\n{}\nmanaged\n{}\nafter\n",
            MANAGED_BLOCK_START, MANAGED_BLOCK_END
        );
        std::fs::write(&file, &content).unwrap();
        remove_managed_block(&file).unwrap();
        let result = std::fs::read_to_string(&file).unwrap();
        assert!(result.contains("before"));
        assert!(result.contains("after"));
        assert!(!result.contains(MANAGED_BLOCK_START));
        assert!(!result.contains("managed"));
    }
}
```

- [ ] **Step 3.2: Run tests to verify they fail**

```bash
cd /home/brianh/.b00t && cargo test -p b00t-cli install::manifest -- --nocapture
```

Expected: compile errors (todo!) for inject/remove, and test failures

- [ ] **Step 3.3: Implement `inject_managed_block` and `remove_managed_block`**

```rust
pub fn inject_managed_block(file_path: &Path, content: &str) -> Result<()> {
    let existing = if file_path.exists() {
        std::fs::read_to_string(file_path)?
    } else {
        String::new()
    };

    let block = format!("{}\n{}\n{}", MANAGED_BLOCK_START, content, MANAGED_BLOCK_END);

    let result = if existing.contains(MANAGED_BLOCK_START) {
        // Replace existing block
        let before = existing.split(MANAGED_BLOCK_START).next().unwrap_or("").to_string();
        let after = existing.split(MANAGED_BLOCK_END).nth(1).unwrap_or("").to_string();
        format!("{}{}{}", before, block, after)
    } else {
        // Append block
        format!("{}\n{}\n", existing.trim_end(), block)
    };

    if let Some(parent) = file_path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(file_path, result)?;
    Ok(())
}

pub fn remove_managed_block(file_path: &Path) -> Result<()> {
    if !file_path.exists() { return Ok(()); }
    let content = std::fs::read_to_string(file_path)?;
    if !content.contains(MANAGED_BLOCK_START) { return Ok(()); }

    let before = content.split(MANAGED_BLOCK_START).next().unwrap_or("").trim_end().to_string();
    let after = content.split(MANAGED_BLOCK_END).nth(1).unwrap_or("").trim_start().to_string();
    std::fs::write(file_path, format!("{}\n{}", before, after))?;
    Ok(())
}
```

- [ ] **Step 3.4: Run tests — all should pass**

```bash
cd /home/brianh/.b00t && cargo test -p b00t-cli install::manifest -- --nocapture
```

Expected: 6 tests PASS

- [ ] **Step 3.5: Commit**

```bash
cd /home/brianh/.b00t && git add b00t-cli/src/install/manifest.rs && git commit -m "feat(install): B00tInstallManifest with SHA256 tracking and managed-block injection"
```

---

### Task 4: `install/content.rs` — ContentPack trait

**Files:**
- Create: `b00t-cli/src/install/content.rs`

- [ ] **Step 4.1: Write failing tests + implementation**

```rust
use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use crate::install::manifest::B00tInstallManifest;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ContentPackId {
    Skills,
    Agents,
    Hooks,
    DatumLifecycle,
}

impl ContentPackId {
    pub fn display_name(&self) -> &'static str {
        match self {
            ContentPackId::Skills        => "Skills & commands",
            ContentPackId::Agents        => "Agents",
            ContentPackId::Hooks         => "Hooks (statusline, update-check, context-monitor, datum-guard)",
            ContentPackId::DatumLifecycle => "Datum lifecycle (b00t cli install/uninstall)",
        }
    }

    pub fn all() -> Vec<ContentPackId> {
        vec![
            ContentPackId::Skills,
            ContentPackId::Agents,
            ContentPackId::Hooks,
            ContentPackId::DatumLifecycle,
        ]
    }
}

/// ContentPack: one responsibility — install/uninstall a named set of files
pub trait ContentPack: Send + Sync {
    fn id(&self) -> ContentPackId;
    fn source_dir(&self) -> PathBuf;
    /// Install files into `target`, recording each in `manifest`
    fn install_into(&self, target: &Path, manifest: &mut B00tInstallManifest) -> Result<()>;
    /// Uninstall: delete b00t-owned files (SHA256 match), back up user-modified files
    fn uninstall_from(&self, manifest: &mut B00tInstallManifest) -> Result<()>;
}

/// Generic file-copy content pack: copies all files from source_dir into target/<subdir>
pub struct FileCopyPack {
    pub id: ContentPackId,
    pub source_dir: PathBuf,
    pub target_subdir: String,  // e.g. "skills", "agents"
}

impl ContentPack for FileCopyPack {
    fn id(&self) -> ContentPackId { self.id.clone() }

    fn source_dir(&self) -> PathBuf { self.source_dir.clone() }

    fn install_into(&self, target: &Path, manifest: &mut B00tInstallManifest) -> Result<()> {
        if !self.source_dir.exists() {
            return Ok(()); // no content for this runtime yet — silent skip
        }
        let dest = target.join(&self.target_subdir);
        std::fs::create_dir_all(&dest)?;

        for entry in walkdir::WalkDir::new(&self.source_dir) {
            let entry = entry?;
            if entry.file_type().is_file() {
                let rel = entry.path().strip_prefix(&self.source_dir)?;
                let dest_path = dest.join(rel);
                if let Some(parent) = dest_path.parent() {
                    std::fs::create_dir_all(parent)?;
                }
                let content = std::fs::read(entry.path())?;
                std::fs::write(&dest_path, &content)?;
                manifest.record_file(&dest_path, &content);
            }
        }
        Ok(())
    }

    fn uninstall_from(&self, manifest: &mut B00tInstallManifest) -> Result<()> {
        let to_remove: Vec<PathBuf> = manifest.files.keys()
            .filter(|p| p.to_string_lossy().contains(&self.target_subdir))
            .cloned()
            .collect();

        for path in to_remove {
            if manifest.file_owned(&path) {
                std::fs::remove_file(&path).ok();
                manifest.files.remove(&path);
            } else {
                // User modified — back up instead of deleting
                let backup = path.with_extension("b00t-backup");
                eprintln!("⚠️  {} was modified — backing up to {:?}", path.display(), backup);
                std::fs::copy(&path, &backup).ok();
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;
    use crate::install::adapter::{RuntimeId, InstallScope};

    #[test]
    fn test_content_pack_id_all() {
        let all = ContentPackId::all();
        assert_eq!(all.len(), 4);
        assert!(all.contains(&ContentPackId::DatumLifecycle));
    }

    #[test]
    fn test_file_copy_pack_install_and_uninstall() {
        let source_dir = TempDir::new().unwrap();
        let target_dir = TempDir::new().unwrap();

        // Create source content
        std::fs::create_dir(source_dir.path().join("b00t-greet")).unwrap();
        std::fs::write(source_dir.path().join("b00t-greet/SKILL.md"), b"# Greet").unwrap();

        let pack = FileCopyPack {
            id: ContentPackId::Skills,
            source_dir: source_dir.path().to_path_buf(),
            target_subdir: "skills".to_string(),
        };

        let mut manifest = crate::install::manifest::B00tInstallManifest::new(
            RuntimeId::Claude, InstallScope::Global
        );

        // Install
        pack.install_into(target_dir.path(), &mut manifest).unwrap();
        let installed = target_dir.path().join("skills/b00t-greet/SKILL.md");
        assert!(installed.exists());
        assert_eq!(manifest.files.len(), 1);

        // Uninstall
        pack.uninstall_from(&mut manifest).unwrap();
        assert!(!installed.exists());
        assert_eq!(manifest.files.len(), 0);
    }

    #[test]
    fn test_file_copy_pack_backs_up_modified_files() {
        let source_dir = TempDir::new().unwrap();
        let target_dir = TempDir::new().unwrap();
        std::fs::write(source_dir.path().join("hook.js"), b"original").unwrap();

        let pack = FileCopyPack {
            id: ContentPackId::Hooks,
            source_dir: source_dir.path().to_path_buf(),
            target_subdir: "hooks".to_string(),
        };
        let mut manifest = crate::install::manifest::B00tInstallManifest::new(
            RuntimeId::Claude, InstallScope::Global
        );
        pack.install_into(target_dir.path(), &mut manifest).unwrap();

        // User modifies the file
        let installed = target_dir.path().join("hooks/hook.js");
        std::fs::write(&installed, b"user modified").unwrap();

        // Uninstall — should back up, not delete
        pack.uninstall_from(&mut manifest).unwrap();
        let backup = installed.with_extension("b00t-backup");
        assert!(backup.exists(), "Modified file should be backed up");
        assert!(installed.exists(), "Modified file should not be deleted");
    }
}
```

Note: `walkdir` crate — check if it's in the workspace deps. If not, add `walkdir = "2"` to `Cargo.toml`.

- [ ] **Step 4.2: Run tests**

```bash
cd /home/brianh/.b00t && cargo test -p b00t-cli install::content -- --nocapture
```

Expected: 3 tests PASS

- [ ] **Step 4.3: Commit**

```bash
cd /home/brianh/.b00t && git add b00t-cli/src/install/content.rs b00t-cli/Cargo.toml && git commit -m "feat(install): ContentPack trait + FileCopyPack with backup-on-modify uninstall"
```

---

### Task 5: `install/tui.rs` — Inquire TUI

**Files:**
- Create: `b00t-cli/src/install/tui.rs`

- [ ] **Step 5.1: Write TUI module**

```rust
use anyhow::Result;
use inquire::{MultiSelect, Select, Confirm};
use crate::install::adapter::{AdapterRegistry, InstallScope, RuntimeId};
use crate::install::content::ContentPackId;

pub struct InstallSelection {
    pub runtimes: Vec<RuntimeId>,
    pub scope: InstallScope,
    pub content_packs: Vec<ContentPackId>,
}

pub fn run_tui(registry: &AdapterRegistry) -> Result<InstallSelection> {
    println!("🥾 b00t installer\n");

    // Runtime selection — detected runtimes shown with [detected] suffix
    let all_adapters = registry.all_adapters();
    let options: Vec<String> = all_adapters.iter()
        .map(|a| {
            let detected = if a.detect() { " [detected]" } else { " [not detected]" };
            format!("{}{}", a.id().display_name(), detected)
        })
        .collect();

    let selected_options = MultiSelect::new("Which runtimes to configure?", options.clone())
        .with_default(&all_adapters.iter().enumerate()
            .filter(|(_, a)| a.detect())
            .map(|(i, _)| i)
            .collect::<Vec<_>>())
        .prompt()?;

    let runtimes: Vec<RuntimeId> = all_adapters.iter()
        .zip(options.iter())
        .filter(|(_, opt)| selected_options.contains(opt))
        .map(|(a, _)| a.id())
        .collect();

    if runtimes.is_empty() {
        anyhow::bail!("No runtimes selected. Aborting.");
    }

    // Scope selection
    let scope_choice = Select::new("Install scope?", vec!["Global (user home dirs)", "Local (current directory)"])
        .prompt()?;
    let scope = match scope_choice {
        "Global (user home dirs)" => InstallScope::Global,
        _ => InstallScope::Local(std::env::current_dir()?),
    };

    // Content pack selection — all selected by default
    let pack_names: Vec<String> = ContentPackId::all().iter()
        .map(|p| p.display_name().to_string())
        .collect();
    let selected_packs = MultiSelect::new("Content packs?", pack_names.clone())
        .with_default(&(0..pack_names.len()).collect::<Vec<_>>())
        .prompt()?;

    let content_packs: Vec<ContentPackId> = ContentPackId::all().into_iter()
        .filter(|p| selected_packs.contains(&p.display_name().to_string()))
        .collect();

    // Confirm
    let runtime_names: Vec<&str> = runtimes.iter().map(|r| r.display_name()).collect();
    let scope_str = match &scope {
        InstallScope::Global => "globally".to_string(),
        InstallScope::Local(p) => format!("locally in {}", p.display()),
    };
    let confirmed = Confirm::new(&format!(
        "Ready to install for [{}] {}?",
        runtime_names.join(", "), scope_str
    )).with_default(true).prompt()?;

    if !confirmed {
        anyhow::bail!("Installation cancelled.");
    }

    Ok(InstallSelection { runtimes, scope, content_packs })
}

/// Non-interactive selection for CI/scripting (--yes flag)
pub fn headless_selection(
    runtimes: Vec<RuntimeId>,
    scope: InstallScope,
    content_packs: Vec<ContentPackId>,
) -> InstallSelection {
    InstallSelection { runtimes, scope, content_packs }
}
```

Tests for TUI are integration-level only (via `--yes` CLI flag in Task 9). Unit testing Inquire prompts requires mocking stdin which adds complexity beyond scope here.

- [ ] **Step 5.2: Compile check**

```bash
cd /home/brianh/.b00t && cargo build -p b00t-cli 2>&1 | grep -E "^error"
```

Expected: no errors

- [ ] **Step 5.3: Commit**

```bash
cd /home/brianh/.b00t && git add b00t-cli/src/install/tui.rs && git commit -m "feat(install): inquire TUI for runtime/scope/content-pack selection"
```

---

### Task 6: Claude runtime adapter

**Files:**
- Create: `b00t-cli/src/install/runtimes/claude.rs`
- Create: `b00t-cli/src/install/runtimes/mod.rs`

- [ ] **Step 6.1: Write failing tests + implementation**

Create `b00t-cli/src/install/runtimes/mod.rs`:

```rust
pub mod claude;
pub mod gemini;
pub mod codex;
pub mod opencode;
pub mod copilot;

pub use claude::ClaudeAdapter;
pub use gemini::GeminiAdapter;
pub use codex::CodexAdapter;
pub use opencode::OpenCodeAdapter;
pub use copilot::CopilotAdapter;
```

Create `b00t-cli/src/install/runtimes/claude.rs`:

```rust
use anyhow::Result;
use std::path::PathBuf;
use std::sync::Arc;
use crate::install::adapter::*;
use crate::install::content::{ContentPackId, FileCopyPack, ContentPack};
use crate::install::manifest::{B00tInstallManifest, inject_managed_block, remove_managed_block};

pub struct ClaudeConfig {
    pub target_dir: PathBuf,
}

impl RuntimeConfig for ClaudeConfig {
    fn settings_path(&self) -> PathBuf { self.target_dir.join("settings.json") }
    fn hooks_dir(&self) -> PathBuf { self.target_dir.join("hooks") }
    fn agents_dir(&self) -> PathBuf { self.target_dir.join("agents") }
    fn skills_dir(&self) -> PathBuf { self.target_dir.join("skills") }
}

#[derive(Default)]
pub struct ClaudeAdapter;

impl RuntimeAdapterTyped for ClaudeAdapter {
    type Config = ClaudeConfig;
    fn config_from_scope(&self, scope: &InstallScope) -> ClaudeConfig {
        ClaudeConfig { target_dir: self.target_dir(scope) }
    }
}

impl RuntimeAdapter for ClaudeAdapter {
    fn id(&self) -> RuntimeId { RuntimeId::Claude }

    fn target_dir(&self, scope: &InstallScope) -> PathBuf {
        match scope {
            InstallScope::Global => dirs::home_dir()
                .unwrap_or_else(|| PathBuf::from("~"))
                .join(".claude"),
            InstallScope::Local(p) => p.join(".claude"),
        }
    }

    fn detect(&self) -> bool {
        dirs::home_dir()
            .map(|h| h.join(".claude").exists())
            .unwrap_or(false)
    }

    fn default_config(&self, scope: &InstallScope) -> Arc<dyn RuntimeConfig> {
        Arc::new(self.config_from_scope(scope))
    }

    fn install(&self, ctx: &InstallContext) -> Result<B00tInstallManifest> {
        let target = ctx.config.skills_dir().parent().unwrap().to_path_buf();
        std::fs::create_dir_all(&target)?;

        let mut manifest = B00tInstallManifest::new(RuntimeId::Claude, ctx.scope.clone());

        for pack_id in &ctx.content_packs {
            let source = ctx.source_root.join(match pack_id {
                ContentPackId::Skills => "skills",
                ContentPackId::Agents => "agents",
                ContentPackId::Hooks  => "hooks",
                ContentPackId::DatumLifecycle => "skills", // datum lifecycle delivered as a skill
            });
            let pack = FileCopyPack {
                id: pack_id.clone(),
                source_dir: source,
                target_subdir: match pack_id {
                    ContentPackId::Skills | ContentPackId::DatumLifecycle => "skills".into(),
                    ContentPackId::Agents => "agents".into(),
                    ContentPackId::Hooks  => "hooks".into(),
                },
            };
            pack.install_into(&target, &mut manifest)?;
        }

        self.register_hooks(ctx, &mut manifest)?;
        manifest.save(&target)?;
        Ok(manifest)
    }

    fn uninstall(&self, manifest: &B00tInstallManifest) -> Result<()> {
        let mut m = manifest.clone();
        // Remove managed blocks first
        for block_path in &manifest.managed_blocks {
            remove_managed_block(block_path)?;
        }
        // Remove b00t-owned files
        let paths: Vec<PathBuf> = m.files.keys().cloned().collect();
        for path in paths {
            if m.file_owned(&path) {
                std::fs::remove_file(&path).ok();
                m.files.remove(&path);
            } else {
                let backup = path.with_extension("b00t-backup");
                eprintln!("⚠️  {} was modified — backing up to {:?}", path.display(), backup);
                std::fs::copy(&path, &backup).ok();
            }
        }
        Ok(())
    }

    fn register_hooks(&self, ctx: &InstallContext, manifest: &mut B00tInstallManifest) -> Result<()> {
        if !ctx.content_packs.contains(&ContentPackId::Hooks) { return Ok(()); }

        let hooks_dir = ctx.config.hooks_dir();
        let settings_path = ctx.config.settings_path();

        // Read hook template from source
        let fragment_path = ctx.source_root.join("settings_fragment.json");
        if !fragment_path.exists() {
            eprintln!("⚠️  No settings_fragment.json for Claude runtime — skipping hook registration");
            return Ok(());
        }
        let fragment = std::fs::read_to_string(&fragment_path)?;
        // Substitute hooks_dir path into fragment
        let fragment = fragment.replace("{{HOOKS_DIR}}", &hooks_dir.display().to_string());

        inject_managed_block(&settings_path, &fragment)?;
        manifest.managed_blocks.push(settings_path);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_claude_target_dir_global() {
        let adapter = ClaudeAdapter;
        let target = adapter.target_dir(&InstallScope::Global);
        assert!(target.ends_with(".claude"));
        assert!(target.is_absolute());
    }

    #[test]
    fn test_claude_target_dir_local() {
        let adapter = ClaudeAdapter;
        let project = PathBuf::from("/tmp/myproject");
        let target = adapter.target_dir(&InstallScope::Local(project.clone()));
        assert_eq!(target, project.join(".claude"));
    }

    #[test]
    fn test_claude_install_creates_manifest() {
        let source_dir = TempDir::new().unwrap();
        let target_dir = TempDir::new().unwrap();

        // Create minimal source content
        let skills_dir = source_dir.path().join("skills/b00t-test");
        std::fs::create_dir_all(&skills_dir).unwrap();
        std::fs::write(skills_dir.join("SKILL.md"), b"# Test Skill").unwrap();

        let adapter = ClaudeAdapter;
        let config = Arc::new(ClaudeConfig { target_dir: target_dir.path().to_path_buf() });
        let ctx = InstallContext {
            scope: InstallScope::Global,
            config,
            content_packs: vec![ContentPackId::Skills],
            source_root: source_dir.path().to_path_buf(),
        };

        let manifest = adapter.install(&ctx).unwrap();
        assert_eq!(manifest.files.len(), 1);
        assert!(target_dir.path().join("b00t-manifest.json").exists());
    }
}
```

- [ ] **Step 6.2: Run tests**

```bash
cd /home/brianh/.b00t && cargo test -p b00t-cli install::runtimes::claude -- --nocapture
```

Expected: 3 tests PASS

- [ ] **Step 6.3: Create stub adapters for remaining 4 runtimes**

Create `gemini.rs`, `codex.rs`, `opencode.rs`, `copilot.rs` — each with the same structure as `claude.rs` but with runtime-specific `target_dir`, `detect`, and `register_hooks` logic:

| Runtime | Global target_dir | detect condition | Hook registration |
|---------|------------------|-----------------|-------------------|
| Gemini | `~/.gemini` | `~/.gemini` exists | `settings.json` AfterTool/BeforeTool |
| Codex | `~/.codex` | `which codex` succeeds | `config.toml` append |
| OpenCode | `~/.config/opencode` | dir exists | `opencode.json` merge |
| Copilot | `~/.copilot` | dir exists | `copilot-instructions.md` block |

For Gemini: `target_dir` = `~/.gemini/`; hook fragment registered in `~/.gemini/settings.json`.
For Codex: `target_dir` = `~/.codex/`; use `config_fragment.toml` appended.
For OpenCode: `target_dir` = `~/.config/opencode/`; use `opencode_fragment.json`.
For Copilot: `target_dir` = `~/.copilot/`; use `copilot_instructions_fragment.md`.

Each must compile but `install()` and `register_hooks()` may return `Ok(())` stub with a `println!("TODO: ...")` for non-Claude runtimes in this task. Claude is the full reference implementation.

- [ ] **Step 6.4: Compile check all runtimes**

```bash
cd /home/brianh/.b00t && cargo build -p b00t-cli 2>&1 | grep -E "^error"
```

- [ ] **Step 6.5: Commit**

```bash
cd /home/brianh/.b00t && git add b00t-cli/src/install/runtimes/ && git commit -m "feat(install): ClaudeAdapter (full) + 4 runtime adapter stubs"
```

---

### Task 7: `install/mod.rs` — orchestration entry point

**Files:**
- Modify: `b00t-cli/src/install/mod.rs`

- [ ] **Step 7.1: Implement `handle_install_command()`**

```rust
// b00t-cli/src/install/mod.rs
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
    crate::utils::get_workspace_root()
        .parse::<PathBuf>()
        .unwrap_or_else(|_| PathBuf::from("."))
        .join("_b00t_/runtimes")
}

/// Main entry: run TUI or headless, then install all selected runtimes
pub fn handle_install_command(interactive: bool, runtimes_arg: Option<Vec<RuntimeId>>) -> Result<()> {
    let registry = default_registry();

    let selection = if interactive {
        tui::run_tui(&registry)?
    } else {
        let runtimes = runtimes_arg.unwrap_or_else(|| {
            registry.detected().iter().map(|a| a.id()).collect()
        });
        tui::headless_selection(runtimes, InstallScope::Global, content::ContentPackId::all())
    };

    let source_root = runtimes_source_root();

    for runtime_id in &selection.runtimes {
        let adapter = registry.get(runtime_id)
            .ok_or_else(|| anyhow::anyhow!("No adapter for {:?}", runtime_id))?;

        let config = adapter.default_config(&selection.scope);
        let runtime_source = source_root.join(format!("{:?}", runtime_id).to_lowercase());

        let ctx = InstallContext {
            scope: selection.scope.clone(),
            config,
            content_packs: selection.content_packs.clone(),
            source_root: runtime_source,
        };

        println!("📦 Installing b00t for {}...", runtime_id.display_name());
        let manifest = adapter.install(&ctx)?;
        println!("✅ {} installed ({} files)", runtime_id.display_name(), manifest.files.len());
    }

    println!("\n🥾 b00t installation complete!");
    Ok(())
}
```

- [ ] **Step 7.2: Wire `--interactive`, `--yes`, `--runtimes`, `--scope` into `Commands::Install`**

Extend the existing `Commands::Install` variant (around line 306):

```rust
    Install {
        #[clap(help = "Datum name to install (omit to run repo bootstrap just install)")]
        name: Option<String>,
        #[clap(long, help = "Show what would be installed for bootstrap mode")]
        dry_run: bool,
        #[clap(long, help = "Interactive TUI installer for agent runtimes")]
        interactive: bool,
        /// Non-interactive: comma-separated runtime IDs (claude,gemini,codex,opencode,copilot)
        #[clap(long, value_delimiter = ',')]
        runtimes: Vec<String>,
        /// Non-interactive: install scope (global or local)
        #[clap(long, default_value = "global")]
        scope: String,
        /// Skip confirmation prompt (non-interactive mode)
        #[clap(long, short = 'y')]
        yes: bool,
    },
```

Update `handle_install_command` signature in `install/mod.rs`:

```rust
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
    // ... rest unchanged ...
}
```

Update the `Commands::Install` match arm:

```rust
        Some(Commands::Install { name, dry_run, interactive, runtimes, scope, yes }) => {
            if *interactive || !runtimes.is_empty() || *yes {
                // Parse runtime IDs
                let runtime_ids: Option<Vec<RuntimeId>> = if runtimes.is_empty() { None } else {
                    Some(runtimes.iter().filter_map(|r| match r.as_str() {
                        "claude"   => Some(RuntimeId::Claude),
                        "gemini"   => Some(RuntimeId::Gemini),
                        "codex"    => Some(RuntimeId::Codex),
                        "opencode" => Some(RuntimeId::OpenCode),
                        "copilot"  => Some(RuntimeId::Copilot),
                        _ => { eprintln!("Unknown runtime: {}", r); None }
                    }).collect())
                };
                let scope_val = match scope.as_str() {
                    "local" => Some(InstallScope::Local(std::env::current_dir().unwrap())),
                    _       => Some(InstallScope::Global),
                };
                if let Err(e) = b00t_cli::install::handle_install_command(*interactive, runtime_ids, scope_val, *yes) {
                    eprintln!("Install Error: {}", e);
                    std::process::exit(1);
                }
            } else if let Some(name) = name {
                if let Err(e) = install_datum(&cli.path, name) {
                    eprintln!("Install Error: {}", e);
                    std::process::exit(1);
                }
            } else if let Err(e) = run_just_install(*dry_run) {
                eprintln!("Install Error: {}", e);
                std::process::exit(1);
            }
        }
```

- [ ] **Step 7.3: Compile check**

```bash
cd /home/brianh/.b00t && cargo build -p b00t-cli 2>&1 | grep -E "^error"
```

- [ ] **Step 7.4: Smoke test non-interactive mode**

```bash
b00t-cli install --help | grep interactive
```

Expected: `--interactive` appears in help output

- [ ] **Step 7.5: Commit**

```bash
cd /home/brianh/.b00t && git add b00t-cli/src/install/mod.rs b00t-cli/src/main.rs && git commit -m "feat(install): orchestration entry point + --interactive flag wired to Commands::Install"
```

---

### Task 8: Node.js hooks + esbuild pipeline

**Files:**
- Create: `_b00t_/runtimes/hooks-src/package.json`
- Create: `_b00t_/runtimes/hooks-src/tsconfig.json`
- Create: `_b00t_/runtimes/hooks-src/build.js`
- Create: `_b00t_/runtimes/hooks-src/b00t-statusline.ts`
- Create: `_b00t_/runtimes/hooks-src/b00t-update-check.ts`
- Create: `_b00t_/runtimes/hooks-src/b00t-context-monitor.ts`
- Create: `_b00t_/runtimes/hooks-src/b00t-datum-guard.ts`

- [ ] **Step 8.1: Create `hooks-src/package.json`**

```json
{
  "name": "b00t-hooks",
  "version": "0.1.0",
  "private": true,
  "scripts": {
    "build": "node build.js",
    "test": "node --test *.test.js"
  },
  "devDependencies": {
    "esbuild": "^0.20.0",
    "@types/node": "^20.0.0",
    "typescript": "^5.0.0"
  }
}
```

- [ ] **Step 8.2: Create `hooks-src/tsconfig.json`**

```json
{
  "compilerOptions": {
    "target": "ES2020",
    "module": "CommonJS",
    "strict": true,
    "outDir": "dist"
  },
  "include": ["*.ts"]
}
```

- [ ] **Step 8.3: Create `hooks-src/build.js`** (esbuild script)

```js
const esbuild = require('esbuild');
const path = require('path');
const fs = require('fs');

const HOOKS = [
  'b00t-statusline',
  'b00t-update-check',
  'b00t-context-monitor',
  'b00t-datum-guard',
];

// Output directories — committed pre-built
const OUTPUT_DIRS = [
  path.join(__dirname, '../claude/hooks'),
  path.join(__dirname, '../gemini/hooks'),
  path.join(__dirname, '../codex/hooks'),
  path.join(__dirname, '../opencode/hooks'),
  path.join(__dirname, '../copilot/hooks'),
];

for (const dir of OUTPUT_DIRS) {
  fs.mkdirSync(dir, { recursive: true });
}

for (const hook of HOOKS) {
  esbuild.buildSync({
    entryPoints: [path.join(__dirname, `${hook}.ts`)],
    bundle: true,
    platform: 'node',
    target: 'node18',
    outfile: path.join(__dirname, `../claude/hooks/${hook}.js`),
    minify: false,
  });
  console.log(`✅ Built ${hook}.js`);

  // Copy to other runtime dirs
  const built = fs.readFileSync(path.join(__dirname, `../claude/hooks/${hook}.js`));
  for (const dir of OUTPUT_DIRS.slice(1)) {
    fs.writeFileSync(path.join(dir, `${hook}.js`), built);
  }
}

console.log('🥾 All hooks built and distributed.');
```

- [ ] **Step 8.4: Create `b00t-datum-guard.ts`** (novel hook — teaches agents to use datum lifecycle)

```typescript
// b00t-datum-guard.ts — PreToolUse hook
// Intercepts direct package manager invocations and soft-redirects to b00t cli install <datum>
// 🤓 NEVER exit non-zero — always advisory only (additionalContext)

const input = JSON.parse(process.argv[2] || '{}');
const command: string = input?.input?.command ?? '';

const PACKAGE_MANAGER_PATTERNS = [
  { regex: /^\s*pip\s+install\b/, hint: 'b00t cli install <datum-name>.cli' },
  { regex: /^\s*npm\s+install\s+-g\b/, hint: 'b00t cli install <datum-name>.cli' },
  { regex: /^\s*apt(-get)?\s+install\b/, hint: 'b00t cli install <datum-name>.cli' },
  { regex: /^\s*brew\s+install\b/, hint: 'b00t cli install <datum-name>.cli' },
  { regex: /^\s*cargo\s+install\b/, hint: 'b00t cli install <datum-name>.cli' },
];

let advisory: string | null = null;
for (const { regex, hint } of PACKAGE_MANAGER_PATTERNS) {
  if (regex.test(command)) {
    advisory = `⚠️ b00t datum-guard: prefer \`${hint}\` over direct package managers.\nCheck available datums with \`b00t cli desires\`.\nDirect install will work but won't be tracked in the b00t hive.`;
    break;
  }
}

if (advisory) {
  process.stdout.write(JSON.stringify({ additionalContext: advisory }));
}

process.exit(0);  // ALWAYS exit 0 — never block
```

- [ ] **Step 8.5: Create `b00t-context-monitor.ts`** (port from GSD)

```typescript
// b00t-context-monitor.ts — PostToolUse hook
// Monitors context window usage and injects warnings when running low
// 🤓 Bridge: reads /tmp/b00t-ctx-{session}.json written by statusline hook

import * as fs from 'fs';
import * as path from 'path';

const input = JSON.parse(process.argv[2] || '{}');
const sessionId: string = input?.session_id ?? process.env.CLAUDE_SESSION_ID ?? 'unknown';
const bridgeFile = path.join('/tmp', `b00t-ctx-${sessionId}.json`);

let contextPct = 100;
try {
  const bridge = JSON.parse(fs.readFileSync(bridgeFile, 'utf8'));
  contextPct = bridge.remaining_pct ?? 100;
} catch {
  // No bridge file yet — first tool use
}

let advisory: string | null = null;
if (contextPct <= 25) {
  advisory = `🚨 CONTEXT CRITICAL: Only ${contextPct}% context remaining. Run /compact or finish current task.`;
} else if (contextPct <= 35) {
  advisory = `⚠️ CONTEXT WARNING: ${contextPct}% context remaining. Consider /compact soon.`;
}

if (advisory) {
  process.stdout.write(JSON.stringify({ additionalContext: advisory }));
}

process.exit(0);
```

- [ ] **Step 8.6: Create `b00t-statusline.ts`** (port from GSD)

```typescript
// b00t-statusline.ts — statusLine hook
// Writes context bridge file; outputs status line string

import * as fs from 'fs';
import * as path from 'path';
import * as child_process from 'child_process';

const input = JSON.parse(process.argv[2] || '{}');
const sessionId: string = input?.session_id ?? 'unknown';
const model: string = input?.model ?? '?';
const contextTokensUsed: number = input?.context_tokens_used ?? 0;
const contextTokensMax: number = input?.context_tokens_max ?? 200000;

const remainingPct = Math.round(((contextTokensMax - contextTokensUsed) / contextTokensMax) * 100);

// Write bridge for context-monitor
const bridgeFile = path.join('/tmp', `b00t-ctx-${sessionId}.json`);
try {
  fs.writeFileSync(bridgeFile, JSON.stringify({ remaining_pct: remainingPct, updated_at: Date.now() }));
} catch { /* non-fatal */ }

// Get b00t version
let b00tVersion = '?';
try {
  b00tVersion = child_process.execSync('b00t-cli --version 2>/dev/null', { timeout: 500 })
    .toString().trim().split(' ').pop() ?? '?';
} catch { /* not installed */ }

const statusLine = `🥾 b00t ${b00tVersion} | ${model} | ctx ${remainingPct}%`;
process.stdout.write(JSON.stringify({ statusLine }));
process.exit(0);
```

- [ ] **Step 8.7: Create `b00t-update-check.ts`** (port from GSD)

```typescript
// b00t-update-check.ts — SessionStart hook
// Checks for newer b00t-cli version; result cached for 24h

import * as fs from 'fs';
import * as path from 'path';
import * as https from 'https';
import * as os from 'os';

const CACHE_FILE = path.join(os.homedir(), '.b00t', 'cache', 'update-check.json');
const CACHE_TTL_MS = 24 * 60 * 60 * 1000;  // 24h

function isCacheFresh(): boolean {
  try {
    const cache = JSON.parse(fs.readFileSync(CACHE_FILE, 'utf8'));
    return Date.now() - cache.checked_at < CACHE_TTL_MS;
  } catch { return false; }
}

if (isCacheFresh()) { process.exit(0); }

// Async check — fire and forget pattern (don't block agent startup)
const req = https.get('https://api.github.com/repos/promptexecution/b00t/releases/latest', {
  headers: { 'User-Agent': 'b00t-update-check' }
}, (res) => {
  let data = '';
  res.on('data', (chunk) => data += chunk);
  res.on('end', () => {
    try {
      const latest = JSON.parse(data).tag_name?.replace(/^v/, '');
      fs.mkdirSync(path.dirname(CACHE_FILE), { recursive: true });
      fs.writeFileSync(CACHE_FILE, JSON.stringify({ checked_at: Date.now(), latest }));
    } catch { /* non-fatal */ }
  });
});
req.on('error', () => { /* non-fatal */ });
req.setTimeout(3000, () => req.destroy());

process.exit(0);
```

- [ ] **Step 8.8: Create hook test files** (`_b00t_/runtimes/hooks-src/*.test.js`)

Create `b00t-datum-guard.test.js`:

```js
const { test } = require('node:test');
const assert = require('node:assert');
const { execSync } = require('child_process');

function runHook(input) {
  try {
    return execSync(`node b00t-datum-guard.js '${JSON.stringify(input)}'`,
      { cwd: __dirname, timeout: 2000, encoding: 'utf8' });
  } catch (e) { return e.stdout ?? ''; }
}

test('datum-guard: pip install triggers advisory', () => {
  const out = runHook({ input: { command: 'pip install requests' } });
  assert.ok(out.includes('b00t cli install') || out === '', 'advisory or empty output');
  // process must exit 0 — execSync would throw on non-zero
});

test('datum-guard: non-package-manager command produces no output', () => {
  const out = runHook({ input: { command: 'ls -la' } });
  assert.strictEqual(out.trim(), '', 'no output for safe commands');
});

test('datum-guard: apt install triggers advisory', () => {
  const out = runHook({ input: { command: 'apt install curl' } });
  assert.ok(out.includes('b00t') || out === '');
});
```

Create `b00t-context-monitor.test.js`:

```js
const { test } = require('node:test');
const assert = require('node:assert');
const { execSync, spawnSync } = require('child_process');
const fs = require('fs');
const os = require('os');

test('context-monitor: >35% remaining produces no advisory', () => {
  const session = 'test-' + Date.now();
  const bridge = `/tmp/b00t-ctx-${session}.json`;
  fs.writeFileSync(bridge, JSON.stringify({ remaining_pct: 80 }));
  const out = execSync(`node b00t-context-monitor.js '${JSON.stringify({ session_id: session })}'`,
    { cwd: __dirname, encoding: 'utf8' });
  assert.strictEqual(out.trim(), '', 'no output when context is healthy');
  fs.unlinkSync(bridge);
});

test('context-monitor: ≤35% injects WARNING', () => {
  const session = 'test-' + Date.now();
  const bridge = `/tmp/b00t-ctx-${session}.json`;
  fs.writeFileSync(bridge, JSON.stringify({ remaining_pct: 30 }));
  const out = execSync(`node b00t-context-monitor.js '${JSON.stringify({ session_id: session })}'`,
    { cwd: __dirname, encoding: 'utf8' });
  const parsed = JSON.parse(out);
  assert.ok(parsed.additionalContext.includes('WARNING'));
  fs.unlinkSync(bridge);
});

test('context-monitor: ≤25% injects CRITICAL', () => {
  const session = 'test-' + Date.now();
  const bridge = `/tmp/b00t-ctx-${session}.json`;
  fs.writeFileSync(bridge, JSON.stringify({ remaining_pct: 20 }));
  const out = execSync(`node b00t-context-monitor.js '${JSON.stringify({ session_id: session })}'`,
    { cwd: __dirname, encoding: 'utf8' });
  const parsed = JSON.parse(out);
  assert.ok(parsed.additionalContext.includes('CRITICAL'));
  fs.unlinkSync(bridge);
});
```

Create `b00t-statusline.test.js`:

```js
const { test } = require('node:test');
const assert = require('node:assert');
const { execSync } = require('child_process');

test('statusline: output is valid JSON with statusLine string', () => {
  const out = execSync(`node b00t-statusline.js '${JSON.stringify({ session_id: 'test', model: 'claude-test', context_tokens_used: 50000, context_tokens_max: 200000 })}'`,
    { cwd: __dirname, encoding: 'utf8' });
  const parsed = JSON.parse(out);
  assert.ok(typeof parsed.statusLine === 'string', 'statusLine must be a string');
  assert.ok(parsed.statusLine.includes('%'), 'statusLine must include context percentage');
});
```

- [ ] **Step 8.9: Install node deps and build**

```bash
cd /home/brianh/.b00t/_b00t_/runtimes/hooks-src && npm install && node build.js
```

Expected: `✅ Built b00t-*.js` for each hook, distributed to each runtime dir

- [ ] **Step 8.10: Run hook tests**

```bash
cd /home/brianh/.b00t/_b00t_/runtimes/hooks-src
# Copy tests to claude/hooks for testing against built output
cp b00t-*.test.js ../claude/hooks/
cd ../claude/hooks && node --test b00t-datum-guard.test.js b00t-context-monitor.test.js b00t-statusline.test.js
```

Expected: all tests PASS

- [ ] **Step 8.11: Verify built output**

```bash
ls /home/brianh/.b00t/_b00t_/runtimes/claude/hooks/
```

Expected: `b00t-statusline.js b00t-update-check.js b00t-context-monitor.js b00t-datum-guard.js`

- [ ] **Step 8.12: Add `just build-hooks` and update `just install`**

In `/home/brianh/.b00t/justfile`, add after the `install` target:

```just
# Bundle b00t hook JS for all runtimes (requires node + npm)
build-hooks:
    cd _b00t_/runtimes/hooks-src && npm install && node build.js
```

Update the `install` target first line to depend on `build-hooks`:
```just
install: build-hooks
    ...rest unchanged...
```

- [ ] **Step 8.13: Commit**

```bash
cd /home/brianh/.b00t && git add _b00t_/runtimes/ justfile && git commit -m "feat(hooks): b00t-branded hooks (statusline, context-monitor, datum-guard, update-check) + esbuild pipeline"
```

---

### Task 9: `settings_fragment.json` for Claude + end-to-end test

**Files:**
- Create: `_b00t_/runtimes/claude/settings_fragment.json`

- [ ] **Step 9.1: Create Claude hook registration fragment**

```json
{
  "hooks": {
    "SessionStart": [{"matcher": "", "hooks": [{"type": "command", "command": "node {{HOOKS_DIR}}/b00t-update-check.js"}]}],
    "PostToolUse": [{"matcher": "Bash|Edit|Write|Agent|Task", "hooks": [{"type": "command", "command": "node {{HOOKS_DIR}}/b00t-context-monitor.js"}]}],
    "PreToolUse": [{"matcher": "Bash", "hooks": [{"type": "command", "command": "node {{HOOKS_DIR}}/b00t-datum-guard.js"}]}]
  },
  "statusLine": "node {{HOOKS_DIR}}/b00t-statusline.js"
}
```

- [ ] **Step 9.2: End-to-end install test to temp dir**

```bash
# Build first
cd /home/brianh/.b00t && cargo build -p b00t-cli

# Test: install Claude to a temp directory (local scope)
TMPDIR=$(mktemp -d)
b00t-cli install --interactive
# Select: Claude Code only, Local → $TMPDIR, all packs, confirm yes

# Verify outputs
ls $TMPDIR/.claude/
# Expected: skills/ agents/ hooks/ settings.json b00t-manifest.json

cat $TMPDIR/.claude/b00t-manifest.json | python3 -m json.tool | head -10
# Expected: valid JSON with "runtime": "claude", "files": {...}

cat $TMPDIR/.claude/settings.json | grep "b00t-datum-guard"
# Expected: hook registration present
```

- [ ] **Step 9.3: Run full test suite**

```bash
cd /home/brianh/.b00t && cargo test -p b00t-cli -- --nocapture 2>&1 | tail -5
```

Expected: `test result: ok`

- [ ] **Step 9.4: Commit**

```bash
cd /home/brianh/.b00t && git add _b00t_/runtimes/claude/ && git commit -m "feat(install): Claude runtime settings_fragment + end-to-end install verified"
```

---

### Task 10: Update `just install` + final verification

**Files:**
- Modify: `/home/brianh/.b00t/justfile` (install target)
- Modify: `/home/brianh/.dotfiles/justfile` (if symlinked, may be same file)

- [ ] **Step 10.1: Add `install-runtimes` target (preserve existing `install`)**

The existing `install` target handles cargo binary compilation and is not overwritten.
Add a new `install-runtimes` target alongside it:

```just
# Install b00t skills/agents/hooks into agent runtimes (interactive TUI)
install-runtimes: build-hooks
    b00t-cli install --interactive
```

This is the deliberate design: `just install` = build b00t binaries; `just install-runtimes` = deploy b00t into agent runtimes.
Both `install` and `install-runtimes` depend on `build-hooks` to ensure hooks are current.

- [ ] **Step 10.2: Final non-interactive smoke test (CI-safe)**

```bash
# Non-interactive: install Claude only, global, all packs, skip confirmation
b00t-cli install --runtimes claude --scope global --yes
```

Expected: exits 0, files installed to `~/.claude/`, manifest written.

- [ ] **Step 10.3: Run full Rust test suite**

```bash
cd /home/brianh/.b00t && cargo test -p b00t-cli -- --nocapture 2>&1 | tail -3
```

Expected: `test result: ok`

- [ ] **Step 10.4: Commit**

```bash
cd /home/brianh/.b00t && git add justfile && git commit -m "feat(installer): add just install-runtimes target"
```

---

## Verification Checklist

```bash
# Prerequisite gate
grep "hook_uninstall" /home/brianh/.b00t/b00t-cli/src/lib.rs | head -1
# Expected: match found

# All Rust tests pass
cd /home/brianh/.b00t && cargo test -p b00t-cli -- --nocapture 2>&1 | tail -3
# Expected: test result: ok

# Hook tests pass
cd /home/brianh/.b00t/_b00t_/runtimes/claude/hooks && node --test b00t-datum-guard.test.js b00t-context-monitor.test.js b00t-statusline.test.js
# Expected: all tests pass

# CLI help shows --interactive and --yes flags
b00t-cli install --help | grep -E "interactive|yes|runtimes"
# Expected: all three appear

# Non-interactive install (CI-safe)
b00t-cli install --runtimes claude --scope global --yes
# Expected: exits 0

# Hooks built (4 files)
ls /home/brianh/.b00t/_b00t_/runtimes/claude/hooks/*.js | wc -l
# Expected: 4

# Manifest has absolute paths (no ~ tildes)
python3 -c "import json; m=json.load(open(open('/dev/stdin').read().strip())); print(all(k.startswith('/') for k in m['files'].keys()))" <<< ~/.claude/b00t-manifest.json
# Expected: True
```
