# b00t Polyglot Installer — Design Spec

**Date:** 2026-03-23
**Status:** Approved
**Inspired by:** [gsd-build/get-shit-done](https://github.com/gsd-build/get-shit-done)

---

## Summary

Implement a Rust-native interactive installer for b00t that:

1. Deploys b00t skills, agents, hooks, and datum-lifecycle tooling into 5 agent runtimes (Claude Code, Gemini CLI, Codex, OpenCode, Copilot)
2. Extends `BootDatum` with `uninstall` + `hook_uninstall` fields and adds `b00t cli uninstall` command
3. Ports 4 GSD hooks as b00t-branded equivalents (including a new `b00t-datum-guard` hook)
4. Exposes `just install` as a thin wrapper over `b00t-cli install --interactive`

---

## Scope

**In scope:**
- `b00t-cli install --interactive` Rust TUI (inquire crate)
- `RuntimeAdapter<Config>` trait + per-runtime adapter structs (5 runtimes)
- `ContentPack` trait + install/uninstall lifecycle
- `B00tInstallManifest` (SHA256 file tracking, marker-based config injection)
- `BootDatum` struct extension: `uninstall`, `hook_uninstall`
- Top-level `Commands::Uninstall { name, purge, yes }` variant (not CLI-type-scoped)
- `_b00t_/runtimes/<runtime>/` content directory structure
- 4 Node.js hooks (pre-bundled via esbuild): statusline, update-check, context-monitor, datum-guard
- `just install` justfile target

**Out of scope:**
- Transpilation between runtime formats (each runtime maintains its own files)
- Cursor and Antigravity runtimes (future extension)
- Remote datum registry / index fetching
- npx/npm packaging of the installer

---

## Architecture

### Directory Layout

```
b00t-cli/src/
  install/
    mod.rs              ← InstallCommand entry point, TUI orchestration
    adapter.rs          ← RuntimeAdapter<C: RuntimeConfig> trait + AdapterRegistry
    manifest.rs         ← B00tInstallManifest (SHA256 tracking)
    runtimes/
      claude.rs         ← ClaudeAdapter
      gemini.rs         ← GeminiAdapter
      codex.rs          ← CodexAdapter
      opencode.rs       ← OpenCodeAdapter
      copilot.rs        ← CopilotAdapter
    content.rs          ← ContentPack trait + ContentPackId enum
    hooks.rs            ← Hook descriptor structs
    tui.rs              ← Inquire TUI (runtime selection, scope, content packs)
  commands/
    cli.rs              ← CliCommands: unchanged
    install.rs          ← existing InstallCommands::Run superseded; Run now delegates to
                          src/install/mod.rs --interactive path (backward-compat wrapper kept)
  main.rs / b00t.rs     ← top-level Commands enum: add Uninstall variant here

_b00t_/runtimes/
  claude/
    skills/b00t-*/SKILL.md
    agents/b00t-*.md
    hooks/
      b00t-statusline.js
      b00t-update-check.js
      b00t-context-monitor.js
      b00t-datum-guard.js
    settings_fragment.json
  gemini/
    skills/b00t-*/SKILL.md
    agents/b00t-*.md
    settings_fragment.json
  codex/
    skills/b00t-*/SKILL.md
    agents/b00t-*.toml
    config_fragment.toml
  opencode/
    commands/b00t-*.md
    agents/b00t-*.md
    opencode_fragment.json
  copilot/
    skills/b00t-*/SKILL.md
    agents/b00t-*.agent.md
    copilot_instructions_fragment.md
```

---

## Core Traits (Advanced Rust)

### `RuntimeAdapter` — two-trait pattern (C1 fix)

Associated types prevent `Box<dyn Trait>` object safety AND conflict with `enum_dispatch`.
Solution: split into an **object-safe dispatch trait** + a **typed impl trait**.

```rust
/// Object-safe trait — no associated types, used for Box<dyn> and enum_dispatch registry
pub trait RuntimeAdapter: Send + Sync {
    fn id(&self) -> RuntimeId;
    fn target_dir(&self, scope: InstallScope) -> PathBuf;
    fn detect(&self) -> bool;
    fn install(&self, ctx: &InstallContext) -> Result<B00tInstallManifest>;
    fn uninstall(&self, manifest: &B00tInstallManifest) -> Result<()>;
    fn register_hooks(&self, ctx: &InstallContext, manifest: &mut B00tInstallManifest) -> Result<()>;
}

/// Typed impl trait — carries Config associated type; used only at the concrete impl level
pub trait RuntimeAdapterTyped: RuntimeAdapter {
    type Config: RuntimeConfig;
    fn typed_install(&self, ctx: &InstallContext<Self::Config>) -> Result<B00tInstallManifest>;
}

/// InstallContext carries Arc<dyn RuntimeConfig> for object-safe dispatch path
pub struct InstallContext {
    pub scope: InstallScope,
    pub config: Arc<dyn RuntimeConfig>,
    pub content_packs: Vec<ContentPackId>,
    pub source_root: PathBuf,   // _b00t_/runtimes/<runtime>/
}

/// RuntimeConfig is object-safe (no generics); each runtime implements it
pub trait RuntimeConfig: Send + Sync {
    fn settings_path(&self) -> PathBuf;
    fn hooks_dir(&self) -> PathBuf;
    fn agents_dir(&self) -> PathBuf;
    fn skills_dir(&self) -> PathBuf;
}
```

`AdapterRegistry` stores `Vec<Box<dyn RuntimeAdapter>>`. `enum_dispatch` is **not used** — the five concrete adapter structs are dispatched via `Box<dyn RuntimeAdapter>` (dispatch cost is negligible for an interactive installer). The `RuntimeAdapterTyped` supertrait is used only within each concrete impl for type-checked config access.

### `ContentPack`

```rust
pub trait ContentPack: Send + Sync {
    fn id(&self) -> ContentPackId;
    fn source_dir(&self) -> PathBuf;
    fn install_into(&self, target: &Path, manifest: &mut B00tInstallManifest) -> Result<()>;
    /// manifest is &mut to allow marking backed-up files (changed SHA256) during uninstall
    fn uninstall_from(&self, manifest: &mut B00tInstallManifest) -> Result<()>;
}

pub enum ContentPackId {
    Skills,
    Agents,
    Hooks,
    DatumLifecycle,
}
```

### `RuntimeConfig` (per-runtime concrete types)

Each adapter declares its own config struct implementing `RuntimeConfig`
(e.g. `ClaudeConfig { settings_json_path, hooks_dir, ... }`).
Config is constructed from `InstallScope` at TUI completion time and stored as `Arc<dyn RuntimeConfig>` in `InstallContext`.

---

## BootDatum Extension

### Struct changes (`lib.rs`)

```rust
pub struct BootDatum {
    // ... existing fields unchanged ...

    /// Shell script to remove this tool from the system
    pub uninstall: Option<String>,

    /// Rhai lifecycle hook executed after uninstall completes
    pub hook_uninstall: Option<String>,
}
```

### New top-level subcommand (`main.rs` / top-level `Commands` enum)

`Uninstall` is NOT added to `CliCommands` (which is CLI-type-scoped). It goes in the
top-level `Commands` enum so `b00t uninstall ripgrep.cli`, `b00t uninstall sequential-thinking.mcp`,
`b00t uninstall my-stack.stack` all work regardless of datum type.

```rust
// top-level Commands enum (main.rs or b00t.rs)
pub enum Commands {
    // ... existing: Cli, Mcp, Ai, Stack, Grok, Datum, ... ...
    Uninstall {
        /// Datum name with type suffix, e.g. "ripgrep.cli", "sequential-thinking.mcp"
        name: String,
        /// Also remove datum entry from _b00t_.toml after uninstall
        #[arg(long, default_value_t = false)]
        purge: bool,
        /// Skip confirmation prompt
        #[arg(long, short = 'y')]
        yes: bool,
    },
    Install(InstallArgs),  // supersedes commands::install::InstallCommands::Run
}
```

### Uninstall execution flow

1. Load datum by name; error if not found
2. Check `uninstall` field present; emit actionable error if missing (`hint: add uninstall = "..." to <name>.toml`)
3. Prompt `"Uninstall <name>? (y/N)"` unless `--yes`
4. Execute `uninstall` shell script (same executor as `install`)
5. Run `hook_uninstall` Rhai script if present
   - `HookResult::Warn` → log warning, **continue** uninstall (non-fatal)
   - `HookResult::Redirect` → log the redirect message, **continue** (soft advisory only)
   - `HookResult::Info` → log, continue
   - `EvalAltResult` (Rhai panic/error) → log error with script name + line, **abort** uninstall,
     return `Err` — hook failure is fatal to prevent partial teardown
6. If `--purge`: call `B00tConfig::remove_datum(name)` to drop from `_b00t_.toml`
7. Emit success or structured error

---

## TUI Flow (`b00t cli install --interactive`)

Uses the `inquire` crate.

```
🥾 b00t installer

? Which runtimes to configure?
  ❯ ◉ Claude Code  (~/.claude)          [detected]
    ◉ Gemini CLI   (~/.gemini)          [detected]
    ○ Codex        (~/.codex)           [not detected]
    ○ OpenCode     (~/.config/opencode) [not detected]
    ○ Copilot      (~/.copilot)         [not detected]

? Install scope?
  ❯ Global (user home dirs)
    Local (current directory)

? Content packs?
  ❯ ◉ Skills & commands
    ◉ Agents
    ◉ Hooks (statusline, update-check, context-monitor, datum-guard)
    ◉ Datum lifecycle (b00t cli install/uninstall from within sessions)

? Ready to install for [Claude Code, Gemini CLI] globally? (y/n)
```

Non-interactive mode: `b00t cli install --runtimes claude,gemini --scope global --yes`

---

## Hooks (b00t-branded GSD ports)

All hooks are Node.js scripts pre-bundled via esbuild. They NEVER block tool execution (always exit 0); behavior is injected via `additionalContext`.

| Hook | Event trigger | Purpose |
|------|--------------|---------|
| `b00t-statusline.js` | `statusLine` | Shows model / b00t version / context% / active role |
| `b00t-update-check.js` | `SessionStart` | Checks for newer b00t-cli; result cached in `~/.b00t/cache/update-check.json` |
| `b00t-context-monitor.js` | `PostToolUse` (Bash\|Edit\|Write\|Agent) | Injects `⚠️ CONTEXT WARNING` (≤35%) or `🚨 CONTEXT CRITICAL` (≤25%) |
| `b00t-datum-guard.js` | `PreToolUse` (Bash) | Soft-redirects `pip install`, `npm install -g`, `apt install` to `b00t cli install <datum>` |

`b00t-datum-guard.js` is the novel addition beyond GSD — teaches agents to route all package installs through the datum lifecycle system.

---

## B00tInstallManifest

Written to `<target>/b00t-manifest.json`. Used for idempotent reinstall and clean uninstall.

```json
{
  "b00t_version": "0.4.2",
  "installed_at": "2026-03-23T00:00:00Z",
  "runtime": "claude",
  "scope": "global",
  "files": {
    "/home/user/.claude/skills/b00t-install/SKILL.md": "sha256:abc123...",
    "/home/user/.claude/hooks/b00t-datum-guard.js":    "sha256:def456..."
  },
  "managed_blocks": [
    "/home/user/.claude/settings.json"
  ]
}
```

**Note:** All paths stored as **absolute paths** (via `std::fs::canonicalize` or `dirs::home_dir()` expansion at install time). Tilde-prefixed strings are NOT used — they cannot be reliably compared against filesystem paths during SHA256 verification.

**Uninstall behavior:**
- Files with matching SHA256 → deleted
- Files with changed SHA256 → backed up to `b00t-local-patches/`, not deleted
- `managed_blocks` entries → marker block removed, surrounding user content preserved

---

## Testing Strategy

**Rust:**
- Unit tests per `RuntimeAdapter` impl: verify `target_dir()`, `install()` file output, `register_hooks()` config injection
- Integration test: install to a temp dir, verify manifest written (absolute paths), verify uninstall removes only b00t-owned files (changed-SHA256 file backed up, not deleted)
- `BootDatum` deserialization test: confirm `uninstall` + `hook_uninstall` round-trip through TOML
- `b00t uninstall` test: mock datum with `uninstall` script, verify execution order (script → hook → purge), verify `EvalAltResult` aborts and returns `Err`
- TUI: tested via non-interactive `--yes` flag in CI

**Node.js hooks** (run via `node --test` or jest, located in `_b00t_/runtimes/claude/hooks/`):
- `b00t-datum-guard.js`: assert that `pip install foo` input → `additionalContext` contains `b00t cli install`; assert exit code is always 0; assert valid input with no package manager → no output
- `b00t-context-monitor.js`: assert ≤35% remaining → WARNING injected; assert ≤25% → CRITICAL injected; assert >35% → no output
- `b00t-update-check.js`: assert cache hit (fresh timestamp) → no output; assert cache miss → version check attempted
- `b00t-statusline.js`: assert output is valid JSON with `statusLine` string field

---

## Dependencies to Add

**Rust crates:**

| Crate | Use |
|-------|-----|
| `inquire` | Interactive TUI prompts |
| `sha2` | SHA256 for manifest file hashing |
| `dirs` | Cross-platform home dir resolution (absolute paths in manifest) |

`enum_dispatch` is **not used** (incompatible with object-safe dispatch; see C1 fix above).

**JS build pipeline (hooks):**

Hook JS source lives in `_b00t_/runtimes/hooks-src/` (TypeScript preferred).
Pre-built bundles are committed to `_b00t_/runtimes/claude/hooks/` et al. — agents use them at runtime without any Node.js build step.

Build pipeline:
```
_b00t_/runtimes/hooks-src/
  package.json          ← devDependencies: esbuild, @types/node
  tsconfig.json
  b00t-statusline.ts
  b00t-update-check.ts
  b00t-context-monitor.ts
  b00t-datum-guard.ts
  build.js              ← esbuild bundle script
```

Justfile target drives the JS build:
```just
# Bundle hook JS for all runtimes
build-hooks:
    cd _b00t_/runtimes/hooks-src && node build.js
```

`just install` depends on `build-hooks`:
```just
install: build-hooks
    b00t-cli install --interactive
```

CI runs `just build-hooks` before `cargo test` to ensure committed bundles match source.

---

## b00t:map v1
```
# summary: polyglot installer — b00t-cli TUI deploys skills/agents/hooks to 5 runtimes
# tags: installer, runtime, datum, uninstall, hooks, tui, polyglot
# tier: frontier
# cmds: just install, b00t cli install --interactive, b00t cli uninstall <name>
# complexity: 7
```
