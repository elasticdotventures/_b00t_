//! pi coding agent runtime adapter.
//!
//! pi is an **agent harness**, not a conventional CLI: its extension points are
//! TypeScript extensions, skills, prompt templates, themes, and pi-managed
//! packages installed through pi's OWN package manager (`pi install npm:…`),
//! never `npm install -g`. This adapter is the registered handler set that lets
//! `b00t install --runtimes pi` provision all of it.
//!
//! Layout (global; `InstallScope::Local(p)` uses `p/.pi` instead):
//!   ~/.pi/agent/settings.json   packages[], extensions[], theme, trust
//!   ~/.pi/agent/extensions/     *.ts or */index.ts   ← pi's hook mechanism
//!   ~/.pi/agent/skills/         skill dirs + root .md
//!   ~/.pi/agent/prompts/        *.md prompt templates
//!   ~/.pi/agent/mcp.json        pi-owned MCP override (pi-mcp-adapter reads it)
//!   ~/.pi/agent/npm/            pi-managed package store
//!
//! `PI_CODING_AGENT_DIR` overrides the global directory.
//!
//! 🤓 pi has NO built-in MCP client. b00t-mcp is reachable only through the
//!    pi-mcp-adapter package, which this adapter installs and wires.

use crate::install::adapter::*;
use crate::install::content::{ContentPack, ContentPackId, FileCopyPack};
use crate::install::manifest::{B00tInstallManifest, remove_managed_block};
use anyhow::{Context, Result};
use std::path::{Path, PathBuf};
use std::sync::Arc;

/// Keys this adapter injects into `settings.json`; removed on uninstall.
const MANAGED_SETTINGS_KEYS: &[&str] = &["_b00t"];

/// MCP servers this adapter wires into `mcp.json`.
/// 🤓 Single source of truth for both directions: `register_hooks` merges them
///    (via mcp_fragment.json) and `uninstall` strips exactly these — never a
///    server the operator added themselves.
pub const WIRED_MCP_SERVERS: &[&str] = &["b00t-mcp", "codebase-memory"];

pub struct PiConfig {
    pub target_dir: PathBuf,
}

impl RuntimeConfig for PiConfig {
    fn settings_path(&self) -> PathBuf {
        self.target_dir.join("settings.json")
    }
    /// pi's hook mechanism IS the extension directory — there is no separate
    /// hooks dir. Extensions subscribe to lifecycle events (tool_call,
    /// session_start, …) and register tools/commands.
    fn hooks_dir(&self) -> PathBuf {
        self.target_dir.join("extensions")
    }
    /// pi has no distinct "agents" dir; agent-shaped extensions live in
    /// extensions/ too. Kept separate from hooks_dir() only so manifest
    /// content_pack_id stays meaningful.
    fn agents_dir(&self) -> PathBuf {
        self.target_dir.join("extensions")
    }
    fn skills_dir(&self) -> PathBuf {
        self.target_dir.join("skills")
    }
}

#[derive(Default)]
pub struct PiAdapter;

impl RuntimeAdapterTyped for PiAdapter {
    type Config = PiConfig;
    fn config_from_scope(&self, scope: &InstallScope) -> Result<PiConfig> {
        Ok(PiConfig {
            target_dir: self.target_dir(scope)?,
        })
    }
}

/// Read a JSON object file, or an empty object when absent/unparseable.
///
/// Unparseable user config is left untouched rather than silently overwritten —
/// we bail so the operator repairs it instead of losing settings.
fn read_json_object(path: &Path) -> Result<serde_json::Value> {
    if !path.exists() {
        return Ok(serde_json::Value::Object(Default::default()));
    }
    let content = std::fs::read_to_string(path)
        .with_context(|| format!("read {}", path.display()))?;
    serde_json::from_str(&content)
        .with_context(|| format!("{} is not valid JSON — refusing to overwrite", path.display()))
}

fn write_json_object(path: &Path, value: &serde_json::Value) -> Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let mut out = serde_json::to_string_pretty(value)?;
    out.push('\n');
    std::fs::write(path, out)
        .with_context(|| format!("write {}", path.display()))
}

/// Recursively strip documentation keys from a parsed fragment.
///
/// 🤓 JSON has no comments, so fragments document themselves with `_`-prefixed
/// and `//`-prefixed keys. Stripping once at load time — rather than filtering
/// at each merge level — is what keeps nested objects clean: a naive top-level
/// filter still leaks `_why` inside `mcpServers.<name>` when the whole server
/// object is inserted verbatim.
fn strip_comment_keys(value: &mut serde_json::Value) {
    match value {
        serde_json::Value::Object(map) => {
            map.retain(|k, _| !k.starts_with('_') && !k.starts_with("//"));
            for v in map.values_mut() {
                strip_comment_keys(v);
            }
        }
        serde_json::Value::Array(items) => {
            for v in items {
                strip_comment_keys(v);
            }
        }
        _ => {}
    }
}

/// Append-merge `items` into `map[key]` without duplicates.
///
/// 🤓 Append, never replace: `packages` and `extensions` are user-owned lists.
/// Replacing them would deregister every package the operator added by hand.
/// Map-level so callers can hold a `&mut Map` borrow without re-borrowing the
/// whole document.
fn merge_array_into(
    map: &mut serde_json::Map<String, serde_json::Value>,
    key: &str,
    items: &[serde_json::Value],
) -> usize {
    let entry = map
        .entry(key.to_string())
        .or_insert_with(|| serde_json::Value::Array(vec![]));
    let Some(arr) = entry.as_array_mut() else {
        return 0;
    };
    let mut added = 0;
    for item in items {
        if !arr.contains(item) {
            arr.push(item.clone());
            added += 1;
        }
    }
    added
}

/// Object-merge `fragment` into `target`, append-merging the listed array keys.
///
/// `fragment` must already have been through [`strip_comment_keys`].
fn merge_fragment(
    target: &mut serde_json::Value,
    fragment: &serde_json::Value,
    array_keys: &[&str],
) {
    let (Some(t_map), Some(f_map)) = (target.as_object_mut(), fragment.as_object()) else {
        return;
    };
    for (key, fval) in f_map {
        if array_keys.contains(&key.as_str()) {
            if let Some(items) = fval.as_array() {
                merge_array_into(t_map, key, items);
                continue;
            }
        }
        // Nested objects (e.g. mcpServers, settings) merge recursively so a
        // pre-existing server or setting is never clobbered.
        if let (Some(existing), Some(incoming)) =
            (t_map.get_mut(key).and_then(|v| v.as_object_mut()), fval.as_object())
        {
            for (k2, v2) in incoming {
                existing.entry(k2.clone()).or_insert_with(|| v2.clone());
            }
            continue;
        }
        t_map.entry(key.clone()).or_insert_with(|| fval.clone());
    }
}

/// Enforce the b00t-mcp `--stdio` invariant on a merged MCP config.
///
/// ⚠️需 With `args: []` b00t-mcp takes the HTTP path: it syncs the official MCP
/// registry over the network, spawns `npx` bridges for every stdio entry
/// (recursively bridging pi itself), prints usage, and exits — so the MCP
/// handshake never happens. Any wired b00t-mcp entry MUST carry `--stdio`.
/// Returns the number of entries repaired.
fn enforce_stdio(mcp: &mut serde_json::Value) -> usize {
    let Some(servers) = mcp
        .as_object_mut()
        .and_then(|m| m.get_mut("mcpServers"))
        .and_then(|s| s.as_object_mut())
    else {
        return 0;
    };
    let mut repaired = 0;
    for (name, entry) in servers.iter_mut() {
        if !name.contains("b00t-mcp") {
            continue;
        }
        let Some(obj) = entry.as_object_mut() else { continue };
        let args_ok = obj
            .get("args")
            .and_then(|a| a.as_array())
            .is_some_and(|a| a.iter().any(|v| v.as_str() == Some("--stdio")));
        if !args_ok {
            obj.insert(
                "args".to_string(),
                serde_json::json!(["--stdio"]),
            );
            repaired += 1;
        }
    }
    repaired
}

impl PiAdapter {
    /// Install pi packages that are declared in settings.json but not yet
    /// present in `pi list`. This is the "pi install" handler: pi owns
    /// ~/.pi/agent/npm/ and records packages in settings.json, so provisioning
    /// a pi extension is `pi install <spec>`, never `npm install -g`.
    fn pi_install_packages(&self, settings_path: &Path) -> Result<Vec<String>> {
        let settings = read_json_object(settings_path)?;
        let declared: Vec<String> = settings
            .get("packages")
            .and_then(|p| p.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|v| v.as_str().map(|s| s.to_string()))
                    .collect()
            })
            .unwrap_or_default();
        if declared.is_empty() {
            return Ok(vec![]);
        }

        // `pi list` prints installed package specs; treat any failure as
        // "unknown state" and let `pi install` be the idempotent authority.
        let listed = std::process::Command::new("pi")
            .arg("list")
            .output()
            .ok()
            .map(|o| String::from_utf8_lossy(&o.stdout).to_string())
            .unwrap_or_default();

        let mut installed = vec![];
        for spec in declared {
            // Spec form is `npm:<pkg>[@version]` / `git:<…>` / a path. Match on
            // the package identity, ignoring the pinned version, so a version
            // bump in the fragment does not look like a missing package.
            let identity = spec
                .split('@')
                .take(2)
                .collect::<Vec<_>>()
                .join("@")
                .trim_start_matches("npm:")
                .to_string();
            let bare = spec.trim_start_matches("npm:").split('@').next().unwrap_or(&spec);
            if !listed.is_empty() && (listed.contains(&identity) || listed.contains(bare)) {
                continue;
            }
            eprintln!("📦 pi install {}", spec);
            let status = std::process::Command::new("pi")
                .args(["install", &spec])
                .status();
            match status {
                Ok(s) if s.success() => installed.push(spec),
                Ok(s) => eprintln!(
                    "⚠️  pi install {} exited with {} — continuing",
                    spec, s
                ),
                Err(e) => eprintln!(
                    "⚠️  pi install {} failed to spawn ({}) — is pi on PATH?",
                    spec, e
                ),
            }
        }
        Ok(installed)
    }
}

impl RuntimeAdapter for PiAdapter {
    fn id(&self) -> RuntimeId {
        RuntimeId::Pi
    }

    fn target_dir(&self, scope: &InstallScope) -> Result<PathBuf> {
        match scope {
            InstallScope::Global => {
                // PI_CODING_AGENT_DIR wins, matching pi's own resolution order.
                if let Ok(dir) = std::env::var("PI_CODING_AGENT_DIR") {
                    if !dir.is_empty() {
                        return Ok(PathBuf::from(shellexpand::tilde(&dir).to_string()));
                    }
                }
                Ok(super::require_home_dir("pi")?.join(".pi/agent"))
            }
            InstallScope::Local(p) => Ok(p.join(".pi")),
        }
    }

    fn detect(&self) -> bool {
        self.target_dir(&InstallScope::Global)
            .map(|d| d.exists())
            .unwrap_or(false)
    }

    fn default_config(&self, scope: &InstallScope) -> Result<Arc<dyn RuntimeConfig>> {
        Ok(Arc::new(self.config_from_scope(scope)?))
    }

    fn install(&self, ctx: &InstallContext) -> Result<B00tInstallManifest> {
        let target = ctx.config.skills_dir().parent().unwrap().to_path_buf();
        std::fs::create_dir_all(&target)?;
        // Extensions are pi's hook mechanism — always ensure the dir exists so
        // /reload and auto-discovery work even with zero content packs.
        std::fs::create_dir_all(ctx.config.hooks_dir())?;

        let mut manifest = B00tInstallManifest::new(RuntimeId::Pi, ctx.scope.clone());

        for pack_id in &ctx.content_packs {
            // pi routes both Agents and Hooks content into extensions/; skills
            // and datum-lifecycle skills into skills/.
            let (source, target_subdir) = match pack_id {
                ContentPackId::Skills => (ctx.source_root.join("skills"), "skills"),
                ContentPackId::DatumLifecycle => (ctx.source_root.join("skills"), "skills"),
                ContentPackId::Agents => (ctx.source_root.join("extensions"), "extensions"),
                ContentPackId::Hooks => (ctx.source_root.join("extensions"), "extensions"),
            };
            let pack = FileCopyPack {
                id: pack_id.clone(),
                source_dir: source,
                target_subdir: target_subdir.into(),
                // Extensions/skills are optional for pi — a bare runtime install
                // that only wires packages + MCP is still useful.
                required: false,
            };
            pack.install_into(&target, &mut manifest)?;
        }

        self.register_hooks(ctx, &mut manifest)?;

        // Provision declared pi packages through pi's own package manager.
        match self.pi_install_packages(&ctx.config.settings_path()) {
            Ok(specs) if !specs.is_empty() => {
                eprintln!("✅ pi packages installed: {}", specs.join(", "));
            }
            Ok(_) => {}
            Err(e) => eprintln!("⚠️  pi package provisioning failed: {e}"),
        }

        manifest.save(&target)?;
        Ok(manifest)
    }

    fn uninstall(&self, manifest: &B00tInstallManifest) -> Result<()> {
        let mut m = manifest.clone();
        for block_path in &manifest.managed_blocks {
            if block_path.extension().and_then(|e| e.to_str()) == Some("json") {
                // Strip only b00t-injected keys; leave user settings intact.
                let mut value = read_json_object(block_path)?;
                if let Some(obj) = value.as_object_mut() {
                    for key in MANAGED_SETTINGS_KEYS {
                        obj.remove(*key);
                    }
                    if let Some(servers) = obj
                        .get_mut("mcpServers")
                        .and_then(|s| s.as_object_mut())
                    {
                        // Remove only what this adapter wired; user-added servers stay.
                        servers.retain(|name, _| !WIRED_MCP_SERVERS.contains(&name.as_str()));
                    }
                }
                write_json_object(block_path, &value)?;
            } else {
                remove_managed_block(block_path)?;
            }
        }
        let paths: Vec<PathBuf> = m.files.keys().cloned().collect();
        for path in paths {
            if m.file_owned(&path) {
                std::fs::remove_file(&path).ok();
                m.files.remove(&path);
            } else {
                let backup = path.with_extension("b00t-backup");
                eprintln!(
                    "⚠️  {} was modified — backing up to {:?}",
                    path.display(),
                    backup
                );
                std::fs::copy(&path, &backup).ok();
            }
        }
        Ok(())
    }

    /// Merge `settings_fragment.json` into settings.json and
    /// `mcp_fragment.json` into mcp.json.
    ///
    /// Both merges are additive and preserve user content. The MCP merge also
    /// enforces the b00t-mcp `--stdio` invariant, repairing a hand-written
    /// `args: []` that would otherwise silently break the handshake.
    fn register_hooks(
        &self,
        ctx: &InstallContext,
        manifest: &mut B00tInstallManifest,
    ) -> Result<()> {
        // ── settings.json: packages[] + extensions[] (append-merge) ──────────
        let settings_path = ctx.config.settings_path();
        let fragment_path = ctx.source_root.join("settings_fragment.json");
        if fragment_path.exists() {
            let fragment_str = std::fs::read_to_string(&fragment_path)?
                .replace("{{EXTENSIONS_DIR}}", &ctx.config.hooks_dir().display().to_string())
                .replace("{{AGENT_DIR}}", &ctx.config.skills_dir().parent().unwrap_or(Path::new(".")).display().to_string());
            let mut fragment: serde_json::Value = serde_json::from_str(&fragment_str)
                .with_context(|| format!("parse {}", fragment_path.display()))?;
            strip_comment_keys(&mut fragment);
            let mut settings = read_json_object(&settings_path)?;
            merge_fragment(&mut settings, &fragment, &["packages", "extensions"]);
            write_json_object(&settings_path, &settings)?;
            manifest.managed_blocks.push(settings_path.clone());
        } else {
            eprintln!("⚠️  No settings_fragment.json for pi runtime — skipping settings merge");
        }

        // ── mcp.json: mcpServers{} (recursive merge + --stdio enforcement) ───
        let mcp_fragment_path = ctx.source_root.join("mcp_fragment.json");
        if mcp_fragment_path.exists() {
            let mcp_path = ctx
                .config
                .settings_path()
                .parent()
                .map(|p| p.join("mcp.json"))
                .unwrap_or_else(|| ctx.config.settings_path().join("mcp.json"));
            let fragment_str = std::fs::read_to_string(&mcp_fragment_path)?
                .replace("{{HOME}}", &super::require_home_dir("pi")?.display().to_string());
            let mut fragment: serde_json::Value = serde_json::from_str(&fragment_str)
                .with_context(|| format!("parse {}", mcp_fragment_path.display()))?;
            strip_comment_keys(&mut fragment);
            let mut mcp = read_json_object(&mcp_path)?;
            merge_fragment(&mut mcp, &fragment, &[]);
            let repaired = enforce_stdio(&mut mcp);
            if repaired > 0 {
                eprintln!(
                    "⚠️  repaired {repaired} b00t-mcp entr{}: args must include --stdio",
                    if repaired == 1 { "y" } else { "ies" }
                );
            }
            write_json_object(&mcp_path, &mcp)?;
            manifest.managed_blocks.push(mcp_path);
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn ctx_for(tmp: &TempDir) -> (InstallContext, PathBuf) {
        let source = tmp.path().join("src");
        std::fs::create_dir_all(&source).unwrap();
        let target = tmp.path().join("agent");
        let config = Arc::new(PiConfig {
            target_dir: target.clone(),
        });
        (
            InstallContext {
                scope: InstallScope::Local(target.clone()),
                config,
                content_packs: vec![],
                source_root: source.clone(),
            },
            target,
        )
    }

    #[test]
    fn test_pi_target_dir_local_is_dot_pi() {
        let adapter = PiAdapter;
        let project = PathBuf::from("/tmp/myproject");
        assert_eq!(
            adapter.target_dir(&InstallScope::Local(project.clone())).unwrap(),
            project.join(".pi")
        );
    }

    #[test]
    fn test_pi_target_dir_honours_env_override() {
        let adapter = PiAdapter;
        // SAFETY: single-threaded test binary section; restored below.
        unsafe { std::env::set_var("PI_CODING_AGENT_DIR", "/tmp/pi-agent-test") };
        let got = adapter.target_dir(&InstallScope::Global).unwrap();
        unsafe { std::env::remove_var("PI_CODING_AGENT_DIR") };
        assert_eq!(got, PathBuf::from("/tmp/pi-agent-test"));
    }

    #[test]
    fn test_pi_dirs_match_pi_layout() {
        let cfg = PiConfig {
            target_dir: PathBuf::from("/x/.pi/agent"),
        };
        assert_eq!(cfg.settings_path(), PathBuf::from("/x/.pi/agent/settings.json"));
        assert_eq!(cfg.hooks_dir(), PathBuf::from("/x/.pi/agent/extensions"));
        assert_eq!(cfg.skills_dir(), PathBuf::from("/x/.pi/agent/skills"));
    }

    #[test]
    fn test_merge_array_appends_without_duplicates() {
        let mut target = serde_json::json!({"packages": ["npm:a"], "theme": "dark"});
        let added = merge_array_into(
            target.as_object_mut().unwrap(),
            "packages",
            &[serde_json::json!("npm:a"), serde_json::json!("npm:b")],
        );
        assert_eq!(added, 1, "only the new package is added");
        assert_eq!(target["packages"], serde_json::json!(["npm:a", "npm:b"]));
        assert_eq!(target["theme"], serde_json::json!("dark"), "user keys preserved");
    }

    #[test]
    fn test_merge_fragment_preserves_existing_servers() {
        let mut target = serde_json::json!({
            "mcpServers": {"context7": {"command": "bunx"}},
            "settings": {"toolPrefix": "server"}
        });
        let fragment = serde_json::json!({
            "mcpServers": {"b00t-mcp": {"command": "b00t-mcp", "args": ["--stdio"]}},
            "settings": {"outputGuard": true}
        });
        merge_fragment(&mut target, &fragment, &[]);
        assert!(target["mcpServers"]["context7"].is_object(), "user server kept");
        assert_eq!(target["mcpServers"]["b00t-mcp"]["args"], serde_json::json!(["--stdio"]));
        assert_eq!(target["settings"]["toolPrefix"], serde_json::json!("server"));
        assert_eq!(target["settings"]["outputGuard"], serde_json::json!(true));
    }

    #[test]
    fn test_enforce_stdio_repairs_empty_args() {
        let mut mcp = serde_json::json!({
            "mcpServers": {
                "b00t-mcp": {"command": "b00t-mcp", "args": []},
                "other": {"command": "x", "args": []}
            }
        });
        assert_eq!(enforce_stdio(&mut mcp), 1, "only b00t-mcp is repaired");
        assert_eq!(mcp["mcpServers"]["b00t-mcp"]["args"], serde_json::json!(["--stdio"]));
        assert_eq!(mcp["mcpServers"]["other"]["args"], serde_json::json!([]), "untouched");
    }

    #[test]
    fn test_enforce_stdio_idempotent() {
        let mut mcp = serde_json::json!({
            "mcpServers": {"b00t-mcp": {"command": "b00t-mcp", "args": ["--stdio"]}}
        });
        assert_eq!(enforce_stdio(&mut mcp), 0);
    }

    #[test]
    fn test_register_hooks_writes_settings_and_mcp() {
        let tmp = TempDir::new().unwrap();
        let (mut ctx, target) = ctx_for(&tmp);
        std::fs::write(
            ctx.source_root.join("settings_fragment.json"),
            r#"{"_note":"b00t","packages":["npm:pi-mcp-adapter"]}"#,
        )
        .unwrap();
        std::fs::write(
            ctx.source_root.join("mcp_fragment.json"),
            r#"{"mcpServers":{"b00t-mcp":{"command":"b00t-mcp","args":[]}}}"#,
        )
        .unwrap();
        std::fs::create_dir_all(&target).unwrap();
        std::fs::write(target.join("settings.json"), r#"{"theme":"dark"}"#).unwrap();

        let mut manifest =
            B00tInstallManifest::new(RuntimeId::Pi, InstallScope::Local(target.clone()));
        PiAdapter.register_hooks(&ctx, &mut manifest).unwrap();

        let settings: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(target.join("settings.json")).unwrap())
                .unwrap();
        assert_eq!(settings["theme"], serde_json::json!("dark"), "user setting preserved");
        assert_eq!(settings["packages"], serde_json::json!(["npm:pi-mcp-adapter"]));

        let mcp: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(target.join("mcp.json")).unwrap())
                .unwrap();
        assert_eq!(
            mcp["mcpServers"]["b00t-mcp"]["args"],
            serde_json::json!(["--stdio"]),
            "--stdio must be enforced"
        );
        assert_eq!(manifest.managed_blocks.len(), 2);
        ctx.content_packs.clear();
    }

    #[test]
    fn test_install_creates_manifest_without_content() {
        let tmp = TempDir::new().unwrap();
        let (ctx, target) = ctx_for(&tmp);
        let manifest = PiAdapter.install(&ctx).unwrap();
        assert_eq!(manifest.runtime, "pi");
        assert!(target.join("b00t-manifest.json").exists());
        assert!(target.join("extensions").is_dir(), "pi hook dir created");
    }

    #[test]
    fn test_strip_comment_keys_is_recursive() {
        // 🤓 Regression: a top-level-only filter leaked `_why` into
        //    mcpServers.<server> because the whole server object is inserted
        //    verbatim when that server is new.
        let mut fragment = serde_json::json!({
            "_note": "top-level doc",
            "settings": {"outputGuard": true},
            "mcpServers": {
                "codebase-memory": {
                    "command": "codebase-memory-mcp",
                    "_why": "nested doc must not leak",
                    "searchKeywords": {"_doc": "deep doc", "*": ["x"]}
                }
            }
        });
        strip_comment_keys(&mut fragment);
        assert!(fragment.get("_note").is_none());
        let server = &fragment["mcpServers"]["codebase-memory"];
        assert!(server.get("_why").is_none(), "nested comment key must be stripped");
        assert_eq!(server["command"], serde_json::json!("codebase-memory-mcp"));
        assert!(
            server["searchKeywords"].get("_doc").is_none(),
            "deeply nested comment key must be stripped"
        );
        assert_eq!(server["searchKeywords"]["*"], serde_json::json!(["x"]));
        assert_eq!(fragment["settings"]["outputGuard"], serde_json::json!(true));
    }

    #[test]
    fn test_merge_fragment_skips_comment_keys() {
        let mut target = serde_json::json!({});
        let mut fragment = serde_json::json!({
            "_note": "do not merge",
            "_invariant": "do not merge either",
            "//doc": "nor this",
            "settings": {"outputGuard": true}
        });
        strip_comment_keys(&mut fragment);
        merge_fragment(&mut target, &fragment, &[]);
        assert!(target.get("_note").is_none(), "_note must not leak into config");
        assert!(target.get("_invariant").is_none(), "_-prefixed keys must not leak");
        assert!(target.get("//doc").is_none(), "//-prefixed keys must not leak");
        assert_eq!(target["settings"]["outputGuard"], serde_json::json!(true));
    }

    #[test]
    fn test_read_json_object_bails_on_invalid_json() {
        let tmp = TempDir::new().unwrap();
        let bad = tmp.path().join("settings.json");
        std::fs::write(&bad, "{not json").unwrap();
        assert!(read_json_object(&bad).is_err(), "must not silently clobber user config");
    }
}
