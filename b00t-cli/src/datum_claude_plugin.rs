//! Claude Code plugin discovery — harmonizes b00t's skill resolver with
//! plugins installed via Claude Code's own `/plugin install` marketplace
//! mechanism (e.g. `superpowers@claude-plugins-official`).
//!
//! b00t does not install or manage the lifecycle of these plugins — that's
//! Claude Code's job, driven by `~/.claude/plugins/installed_plugins.json`.
//! This module only *reads* that state so plugin-bundled `skills/*/SKILL.md`
//! directories become visible to `SkillResolver` (and therefore `b00t skill
//! list/search/load/activate`) alongside project-local and other global
//! skill sources.
//!
//! # File format (`~/.claude/plugins/installed_plugins.json`)
//! ```json
//! {
//!   "version": 2,
//!   "plugins": {
//!     "superpowers@claude-plugins-official": [
//!       {
//!         "scope": "user",
//!         "installPath": "/home/user/.claude/plugins/cache/claude-plugins-official/superpowers/6.2.0",
//!         "version": "6.2.0",
//!         "installedAt": "2026-08-08T00:00:00.000Z",
//!         "lastUpdated": "2026-08-08T00:00:00.000Z"
//!       }
//!     ]
//!   }
//! }
//! ```
//! Each plugin key may have multiple entries (e.g. different scopes); all
//! are surfaced — a stale/duplicate `installPath` just means the same
//! `skills/` dir is scanned twice, which `SkillResolver::list()` already
//! dedupes by skill name.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};

/// A single install record for a plugin (one plugin key can have several,
/// e.g. one per scope).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InstalledPluginEntry {
    #[serde(default)]
    pub scope: String,
    #[serde(rename = "installPath")]
    pub install_path: String,
    #[serde(default)]
    pub version: String,
    #[serde(default, rename = "installedAt")]
    pub installed_at: String,
    #[serde(default, rename = "lastUpdated")]
    pub last_updated: String,
}

#[derive(Debug, Clone, Deserialize)]
struct InstalledPluginsFile {
    #[serde(default)]
    #[allow(dead_code)]
    version: u32,
    #[serde(default)]
    plugins: HashMap<String, Vec<InstalledPluginEntry>>,
}

/// Read `<claude_dir>/plugins/installed_plugins.json`, returning
/// `(plugin_key, entry)` pairs — one per install record.
///
/// Tolerant by design: a missing file, unreadable file, or malformed JSON
/// all return an empty list rather than erroring, matching this codebase's
/// convention for optional/global discovery paths (e.g. `find_b00t_dir`).
pub fn read_installed_plugins(claude_dir: &Path) -> Vec<(String, InstalledPluginEntry)> {
    let path = claude_dir.join("plugins").join("installed_plugins.json");
    let Ok(content) = std::fs::read_to_string(&path) else {
        return Vec::new();
    };
    let Ok(parsed) = serde_json::from_str::<InstalledPluginsFile>(&content) else {
        return Vec::new();
    };

    let mut out = Vec::new();
    for (key, entries) in parsed.plugins {
        for entry in entries {
            out.push((key.clone(), entry));
        }
    }
    out
}

/// For every installed plugin, resolve `<installPath>/skills` when it
/// exists as a directory. Returns `(plugin_key, skills_dir)` pairs.
///
/// A plugin with no `skills/` directory (e.g. `rust-analyzer-lsp`, which
/// only registers an MCP server) is silently excluded — not every Claude
/// Code plugin bundles skills.
pub fn plugin_skill_dirs(claude_dir: &Path) -> Vec<(String, PathBuf)> {
    read_installed_plugins(claude_dir)
        .into_iter()
        .filter_map(|(key, entry)| {
            let skills_dir = PathBuf::from(&entry.install_path).join("skills");
            skills_dir.is_dir().then_some((key, skills_dir))
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    /// Build a fake `<root>/.claude/` tree with one installed plugin entry
    /// pointing at `<root>/plugin-install/`, optionally containing a
    /// `skills/<name>/SKILL.md`.
    fn make_fake_claude_dir(root: &Path, plugin_key: &str, install_path: &Path, with_skill: bool) {
        let plugins_dir = root.join("plugins");
        std::fs::create_dir_all(&plugins_dir).unwrap();

        if with_skill {
            let skill_dir = install_path.join("skills").join("brainstorming");
            std::fs::create_dir_all(&skill_dir).unwrap();
            let mut f = std::fs::File::create(skill_dir.join("SKILL.md")).unwrap();
            f.write_all(
                b"---\nname: brainstorming\ndescription: Explore ideas before implementing.\n---\n# Brainstorming\nAsk questions first.\n",
            )
            .unwrap();
        } else {
            std::fs::create_dir_all(install_path).unwrap();
        }

        let manifest = serde_json::json!({
            "version": 2,
            "plugins": {
                plugin_key: [
                    {
                        "scope": "user",
                        "installPath": install_path.to_string_lossy(),
                        "version": "6.2.0",
                        "installedAt": "2026-08-08T00:00:00.000Z",
                        "lastUpdated": "2026-08-08T00:00:00.000Z",
                    }
                ]
            }
        });
        std::fs::write(
            plugins_dir.join("installed_plugins.json"),
            serde_json::to_string_pretty(&manifest).unwrap(),
        )
        .unwrap();
    }

    #[test]
    fn missing_file_returns_empty() {
        let tmp = tempfile::tempdir().unwrap();
        let claude_dir = tmp.path().join(".claude");
        // Note: no plugins/installed_plugins.json written at all.
        assert!(read_installed_plugins(&claude_dir).is_empty());
        assert!(plugin_skill_dirs(&claude_dir).is_empty());
    }

    #[test]
    fn malformed_json_returns_empty() {
        let tmp = tempfile::tempdir().unwrap();
        let claude_dir = tmp.path().join(".claude");
        std::fs::create_dir_all(claude_dir.join("plugins")).unwrap();
        std::fs::write(
            claude_dir.join("plugins").join("installed_plugins.json"),
            "{ not valid json",
        )
        .unwrap();

        assert!(read_installed_plugins(&claude_dir).is_empty());
        assert!(plugin_skill_dirs(&claude_dir).is_empty());
    }

    #[test]
    fn reads_plugin_with_skills_dir() {
        let tmp = tempfile::tempdir().unwrap();
        let claude_dir = tmp.path().join(".claude");
        let install_path = tmp.path().join("cache").join("superpowers").join("6.2.0");
        make_fake_claude_dir(
            &claude_dir,
            "superpowers@claude-plugins-official",
            &install_path,
            true,
        );

        let entries = read_installed_plugins(&claude_dir);
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].0, "superpowers@claude-plugins-official");
        assert_eq!(entries[0].1.version, "6.2.0");

        let dirs = plugin_skill_dirs(&claude_dir);
        assert_eq!(dirs.len(), 1);
        assert_eq!(dirs[0].0, "superpowers@claude-plugins-official");
        assert_eq!(dirs[0].1, install_path.join("skills"));
    }

    #[test]
    fn plugin_without_skills_dir_is_excluded() {
        let tmp = tempfile::tempdir().unwrap();
        let claude_dir = tmp.path().join(".claude");
        let install_path = tmp
            .path()
            .join("cache")
            .join("rust-analyzer-lsp")
            .join("1.0.0");
        make_fake_claude_dir(
            &claude_dir,
            "rust-analyzer-lsp@claude-plugins-official",
            &install_path,
            false,
        );

        // The plugin itself is still readable...
        assert_eq!(read_installed_plugins(&claude_dir).len(), 1);
        // ...but contributes no skill directories.
        assert!(plugin_skill_dirs(&claude_dir).is_empty());
    }

    #[test]
    fn multiple_entries_for_same_plugin_key_all_returned() {
        let tmp = tempfile::tempdir().unwrap();
        let claude_dir = tmp.path().join(".claude");
        let plugins_dir = claude_dir.join("plugins");
        std::fs::create_dir_all(&plugins_dir).unwrap();

        let path_a = tmp.path().join("a");
        let path_b = tmp.path().join("b");
        std::fs::create_dir_all(path_a.join("skills")).unwrap();
        std::fs::create_dir_all(path_b.join("skills")).unwrap();

        let manifest = serde_json::json!({
            "version": 2,
            "plugins": {
                "dual-scope@marketplace": [
                    { "scope": "user", "installPath": path_a.to_string_lossy(), "version": "1.0.0", "installedAt": "", "lastUpdated": "" },
                    { "scope": "project", "installPath": path_b.to_string_lossy(), "version": "1.0.0", "installedAt": "", "lastUpdated": "" },
                ]
            }
        });
        std::fs::write(
            plugins_dir.join("installed_plugins.json"),
            serde_json::to_string_pretty(&manifest).unwrap(),
        )
        .unwrap();

        let dirs = plugin_skill_dirs(&claude_dir);
        assert_eq!(dirs.len(), 2);
    }
}
