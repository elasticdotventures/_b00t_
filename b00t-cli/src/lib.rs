#![allow(dead_code, async_fn_in_trait)]
#![recursion_limit = "256"]

// 🤓 write_event moved to b00t-c0re-lib::events — unified telemetry writer
pub use b00t_c0re_lib::write_event;

/// ANSI color helpers — auto-disable when stdout is not a terminal.
pub mod ansi {
    pub fn enabled() -> bool {
        use std::io::IsTerminal;
        std::io::stdout().is_terminal()
    }
    pub fn green(s: &str) -> String { if enabled() { format!("\x1b[32m{}\x1b[0m", s) } else { s.to_string() } }
    pub fn yellow(s: &str) -> String { if enabled() { format!("\x1b[33m{}\x1b[0m", s) } else { s.to_string() } }
    pub fn red(s: &str) -> String { if enabled() { format!("\x1b[31m{}\x1b[0m", s) } else { s.to_string() } }
    pub fn cyan(s: &str) -> String { if enabled() { format!("\x1b[36m{}\x1b[0m", s) } else { s.to_string() } }
    pub fn dim(s: &str) -> String { if enabled() { format!("\x1b[2m{}\x1b[0m", s) } else { s.to_string() } }
    pub fn bold(s: &str) -> String { if enabled() { format!("\x1b[1m{}\x1b[0m", s) } else { s.to_string() } }
}

pub mod exit_code {
    /// Generic / unknown error
    pub const ERROR: i32 = 1;
    /// Datum or resource not found
    pub const NOT_FOUND: i32 = 2;
    /// Invalid arguments or syntax
    pub const USAGE: i32 = 3;
    /// Permission / auth / credential failure
    pub const ACCESS: i32 = 4;
    /// Gate precondition not satisfied (command/env/file missing)
    pub const GATE: i32 = 10;
    /// Dependency resolution failure
    pub const DEP: i32 = 11;
    /// MCP server not found or install failed
    pub const MCP: i32 = 20;
    /// Network / connectivity failure
    pub const NETWORK: i32 = 30;
}

pub mod agentic_role;
pub mod ansible;
pub mod blessing;
pub mod virtfs;
pub mod datum_schema;
pub mod bootstrap;
pub mod budget_controller;
pub mod cloud_sync;
pub mod assimilate;
pub mod commands;
pub mod datum_ai;
pub mod datum_ai_model;
pub mod datum_api;
pub mod datum_apt;
pub mod datum_bash;
pub mod datum_claude_plugin;
pub mod datum_cli;
pub mod datum_config;
pub mod datum_database;
pub mod datum_docker;
pub mod datum_guard;
pub mod datum_gemini;
pub mod training_examples;
pub mod datum_job;
pub mod datum_justfile;
pub mod datum_pipeline;
pub mod datum_k8s;
pub mod datum_podman;
pub mod datum_mcp;
pub mod datum_repo;
pub mod datum_skill;
pub mod datum_stack;
pub mod datum_triples;
pub mod datum_proof;
pub mod datum_store;
pub mod datum_utils;
pub mod datum_vscode;
pub mod query_sources;
#[cfg(feature = "dbus")]
pub mod dbus_dispatch;
pub mod dependency_resolver;
pub mod entanglement;
pub mod erp;
pub mod errors;
pub mod governance;
pub mod guards;
pub mod hive;
pub mod hook_engine;
pub mod install;
pub mod inventory;
pub mod job_executor;
pub mod job_ipc;
pub mod job_state;
pub mod just_ast;
pub mod k0mmand3r;
pub mod k8s;
pub mod memory_provider;
pub mod model_manager;
pub mod model_registry;
pub mod orchestrator;
pub mod runtime_sandbox;
pub mod scheduler;
pub mod session_memory;
pub mod skill_resolver;
pub mod semantic_patch;
pub mod soul_writer;
pub mod step;
pub mod traits;
pub mod utils;
pub mod variant;
pub mod viz;
pub mod whoami;
pub mod wow;
pub mod calorie_tracker;
pub mod cake_ledger;
pub mod a2a_gates;
pub mod pipeline_cache;
pub mod pipeline_costs;
pub mod pipeline_checkpoint;
pub mod pipeline_executor;
pub mod pipeline_flowctl;
pub mod pipeline_logs;
pub mod pipeline_secrets;
pub mod pipeline_types;
pub mod pipeline_viz;
pub mod pipeline_kerml;
pub mod pipeline_scheduler;
pub mod pipeline_dataframe;
pub mod pipeline_nats;
pub mod pipeline_auth;
pub mod pipeline_k8s;
pub mod pipeline_statemachine;
pub mod pipeline_store_nats;
pub mod pipeline_transitions;
pub mod stage_registry;
pub mod transmogrifier;
#[cfg(feature = "rpa")]
pub mod rpa_cdp;
#[cfg(feature = "rpa")]
pub mod rpa_tui;
#[cfg(any(feature = "rpa", feature = "rpa-playwright"))]
pub mod rpa_backend;
#[cfg(feature = "rpa")]
pub mod rpa_rhai;
pub use traits::*;

// ── New domain modules (split from monolith, Issue #718) ─────────────────
pub mod datum_types;
pub mod boot_datum;
pub mod config_types;
pub mod compose;
pub mod polyseme;
pub mod gates;
pub mod hooks;
pub mod dispatch;
pub mod lifecycle;

pub use datum_types::*;
pub use boot_datum::*;
pub use config_types::*;
pub use compose::*;
pub use polyseme::*;
pub use gates::*;
pub use dispatch::*;
pub use lifecycle::*;

pub const PRUNE_DIRS: &[&str] = &[
    "node_modules",
    ".cache",
    ".cargo",
    ".rustup",
    "target",
    ".git",
    "vendor",
    "_archive_",
    ".local",
    ".npm",
    ".pnpm-store",
    ".mozilla",
    ".vscode",
    ".codeium",
    ".config",
    "snap",
];

pub fn sweep_backup_files(root: &std::path::Path) -> usize {
    use walkdir::WalkDir;
    let mut removed = 0usize;
    for entry in WalkDir::new(root)
        .into_iter()
        .filter_entry(|e| {
            let name = e.file_name().to_str().unwrap_or("");
            !PRUNE_DIRS.contains(&name)
        })
        .filter_map(|e| e.ok())
    {
        let path = entry.path();
        let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
        if path.is_file() && name.ends_with('~') {
            if let Err(e) = std::fs::remove_file(path) {
                eprintln!("  ⚠️  {}: {e}", path.display());
            } else {
                removed += 1;
            }
        }
    }
    removed
}

// ── Crate-wide test env lock ────────────────────────────────────────────

/// Crate-wide lock for tests that mutate process-wide environment variables.
/// A per-module lock cannot prevent concurrent mutations from tests in *other*
/// modules; this single static is the authoritative guard for all env-var
/// manipulation across the entire `b00t_cli` test suite.
#[cfg(test)]
pub mod test_env {
    use once_cell::sync::Lazy;
    use std::sync::Mutex;
    pub static ENV_LOCK: Lazy<Mutex<()>> = Lazy::new(|| Mutex::new(()));
}

#[cfg(test)]
mod tests {
    use crate::InstallSpec;
    use serde::Deserialize;
    use std::path::PathBuf;
    use std::sync::{Mutex, MutexGuard};

    static HOME_LOCK: Mutex<()> = Mutex::new(());

    struct TempHome {
        _guard: MutexGuard<'static, ()>,
        old_home: Option<String>,
        _temp_dir: tempfile::TempDir,
        b00t_dir: PathBuf,
    }

    impl TempHome {
        fn new() -> Self {
            let guard = HOME_LOCK.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
            let temp_dir = tempfile::tempdir().unwrap();
            let b00t_dir = temp_dir.path().join(".b00t");
            std::fs::create_dir_all(&b00t_dir).unwrap();
            let old_home = std::env::var("HOME").ok();
            unsafe {
                std::env::set_var("HOME", temp_dir.path().to_str().unwrap());
            }

            Self {
                _guard: guard,
                old_home,
                _temp_dir: temp_dir,
                b00t_dir,
            }
        }
    }

    impl Drop for TempHome {
        fn drop(&mut self) {
            if let Some(old) = &self.old_home {
                unsafe { std::env::set_var("HOME", old); }
            } else {
                unsafe { std::env::remove_var("HOME"); }
            }
        }
    }

    #[test]
    fn test_datum_type_from_filename_accepts_typed_toml_extensions() {
        assert_eq!(
            crate::DatumType::from_filename("b00t.cli"),
            crate::DatumType::Cli
        );
        assert_eq!(
            crate::DatumType::from_filename("b00t.cli.toml"),
            crate::DatumType::Cli
        );
        assert_eq!(
            crate::DatumType::from_filename("executive.role.tomllmd"),
            crate::DatumType::Role
        );
        assert_eq!(
            crate::DatumType::from_filename("executive.role.tomllm"),
            crate::DatumType::Role
        );
        assert_eq!(
            crate::DatumType::from_filename("irontology.mcp.toml"),
            crate::DatumType::Mcp
        );
        // 🤓 hardware datums use dotted SoC.subsystem namespace
        assert_eq!(
            crate::DatumType::from_filename("rk3588.npu.hardware.tomllmd"),
            crate::DatumType::Hardware
        );
        assert_eq!(
            crate::DatumType::from_filename("rtx3090.hardware.toml"),
            crate::DatumType::Hardware
        );
        // 🤓 overlay datums carry node-local state in git enclave
        assert_eq!(
            crate::DatumType::from_filename("models.overlay.toml"),
            crate::DatumType::Overlay
        );
        assert_eq!(
            crate::DatumType::from_filename("unknown.toml"),
            crate::DatumType::Unknown
        );
    }

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
        assert_eq!(
            config.b00t.uninstall,
            Some("apt-get remove -y ripgrep".to_string())
        );
        assert_eq!(
            config.b00t.hook_uninstall,
            Some("// Rhai: post-uninstall cleanup\nlet x = 1;".to_string())
        );
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

    // ── get_config tomllmd precedence ─────────────────────────────────────────

    #[test]
    fn test_get_config_prefers_tomllmd_over_tomllm_and_toml() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().to_str().unwrap();

        // Write all three extension variants for the same datum
        std::fs::write(
            dir.path().join("mytool.cli.toml"),
            "[b00t]\nname = \"mytool-toml\"\ntype = \"cli\"\nhint = \"toml\"\n",
        )
        .unwrap();
        std::fs::write(
            dir.path().join("mytool.cli.tomllm"),
            "[b00t]\nname = \"mytool-tomllm\"\ntype = \"cli\"\nhint = \"tomllm\"\n",
        )
        .unwrap();
        std::fs::write(
            dir.path().join("mytool.cli.tomllmd"),
            "[b00t]\nname = \"mytool-tomllmd\"\ntype = \"cli\"\nhint = \"tomllmd\"\n",
        )
        .unwrap();

        let (config, filename) = crate::get_config("mytool", path).unwrap();
        assert_eq!(
            config.b00t.name, "mytool-tomllmd",
            ".tomllmd must be returned first"
        );
        assert!(
            filename.ends_with(".tomllmd"),
            "filename must end with .tomllmd, got {}",
            filename
        );
    }

    #[test]
    fn test_get_config_falls_back_to_tomllm_when_no_tomllmd() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().to_str().unwrap();

        std::fs::write(
            dir.path().join("mytool.cli.toml"),
            "[b00t]\nname = \"mytool-toml\"\ntype = \"cli\"\nhint = \"toml\"\n",
        )
        .unwrap();
        std::fs::write(
            dir.path().join("mytool.cli.tomllm"),
            "[b00t]\nname = \"mytool-tomllm\"\ntype = \"cli\"\nhint = \"tomllm\"\n",
        )
        .unwrap();

        let (config, filename) = crate::get_config("mytool", path).unwrap();
        assert_eq!(config.b00t.name, "mytool-tomllm");
        assert!(filename.ends_with(".tomllm"), "got {}", filename);
    }

    // ── write_event re-export from c0re-lib ────────────────────────────────────

    #[test]
    fn test_write_event_reexport_from_c0re_lib() {
        use std::fs;

        let temp_home = TempHome::new();

        // write_event is re-exported from b00t_c0re_lib
        crate::write_event("mcp_list_view", "42");

        let events_path = temp_home.b00t_dir.join("events.jsonl");
        assert!(events_path.exists(), "events.jsonl should exist");
        let content = fs::read_to_string(&events_path).unwrap();
        let line = content.lines().next().unwrap();
        let parsed: serde_json::Value = serde_json::from_str(line).unwrap();
        assert_eq!(parsed["event"], "mcp_list_view");
        assert_eq!(parsed["detail"], "42");
    }

    // ── mti type_id tests ──────────────────────────────────────────────────
    use super::{BootDatum, DatumType};

    fn make_datum(name: &str, datum_type: Option<DatumType>) -> BootDatum {
        BootDatum {
            name: name.to_string(),
            datum_type,
            hint: String::new(),
            status: None, enabled: None, status_msg: None, replacement: None,
            git_attributes: Default::default(), desires: None, auto_install: None,
            skills: None, compliance: None, install: None, update: None,
            version: None, version_regex: None, requires_sudo: false,
            command: None, args: None, vsix_id: None, script: None,
            image: None, docker_args: None, oci_uri: None, resource_path: None,
            chart_path: None, namespace: None, values_file: None,
            keywords: None, package_name: None, ansible: None,
            env: None, require: None, aliases: None, k0mmand3r: None,
            knowledge: None, mcp: None, ai_provision: None, gate: None, url: None, branch: None,
            clone_path: None, entangled_agents: None, entangled_cli: None,
            entangled_mcp: None, entangled_ai_models: None, entangled_apis: None,
            entangled_docker: None, entangled_k8s: None, channel_prefix: None,
            depends_on: None, members: None, orchestration: None,
            model_hf_id: None, model_size_gb: None, model_size_4bit_gb: None,
            stack: None, job: None, skill: None, dsn: None, justfile: None, pipeline: None,
            learn: None, lfmf_category: None, usage: None, provides: None,
            protocol: None, implements: None, hook_detect: None,
            hook_install: None, hook_update: None, hook_learn: None,
            uninstall: None, hook_uninstall: None, unlocks: None,
            type_tags: None, maintenance: None, required_for_core: None,
            runtime: None, polyseme: None, trigger_words: None, compose: None,
            requires_competency: None,
        }
    }

    #[test]
    fn type_id_is_deterministic() {
        let d = make_datum("bayesian", Some(DatumType::Skill));
        assert_eq!(d.type_id(), d.type_id(), "same inputs must produce same TypeID");
    }

    #[test]
    fn type_id_prefix_inferred_from_datum_type() {
        let skill = make_datum("bayesian", Some(DatumType::Skill));
        let mcp   = make_datum("context7", Some(DatumType::Mcp));
        let cli   = make_datum("fdfind",   Some(DatumType::Cli));
        assert!(skill.type_id().starts_with("skill_"), "skill datum must have 'skill_' prefix");
        assert!(mcp.type_id().starts_with("mcp_"),   "mcp datum must have 'mcp_' prefix");
        assert!(cli.type_id().starts_with("cli_"),   "cli datum must have 'cli_' prefix");
    }

    #[test]
    fn type_id_different_names_produce_different_ids() {
        let a = make_datum("bayesian",     Some(DatumType::Skill));
        let b = make_datum("first-principles", Some(DatumType::Skill));
        assert_ne!(a.type_id(), b.type_id());
    }

    #[test]
    fn type_id_different_types_produce_different_ids() {
        let as_skill = make_datum("kaizen", Some(DatumType::Skill));
        let as_role  = make_datum("kaizen", Some(DatumType::Role));
        assert_ne!(as_skill.type_id(), as_role.type_id());
    }

    #[test]
    fn type_id_unknown_type_falls_back_to_dat_prefix() {
        let d = make_datum("mystery", None);
        assert!(d.type_id().starts_with("dat_"), "unknown type must fall back to 'dat_' prefix");
    }

    #[test]
    fn datum_type_prefix_inferred_not_hardcoded() {
        // base_suffix() → trim '.' → type_prefix() — all variants must round-trip
        for variant in DatumType::all_variants() {
            let prefix = variant.type_prefix();
            let suffix = variant.base_suffix().trim_start_matches('.');
            assert_eq!(prefix, suffix, "{variant:?}: type_prefix must equal base_suffix without leading dot");
        }
    }

    #[test]
    fn datum_nodes_covers_all_variants() {
        let nodes = DatumType::datum_nodes();
        let variant_count = DatumType::all_variants().len();
        assert_eq!(nodes.len(), variant_count, "datum_nodes() must emit one node per DatumType variant");
    }

    #[test]
    fn datum_nodes_no_duplicate_ids() {
        let nodes = DatumType::datum_nodes();
        let mut ids: std::collections::HashSet<&str> = std::collections::HashSet::new();
        for n in &nodes {
            assert!(ids.insert(n.id.as_str()), "duplicate datum_node id: {}", n.id);
        }
    }

    #[test]
    fn datum_nodes_id_format_and_kind() {
        for node in DatumType::datum_nodes() {
            assert!(node.id.starts_with("datum_type::"), "id must be datum_type::<prefix>, got: {}", node.id);
            assert_eq!(node.kind, "datum_type", "kind must be 'datum_type', got: {}", node.kind);
            assert!(node.z_layer.is_none(), "datum_nodes must have no z_layer");
            assert!(node.semantic_type.is_none(), "datum_nodes must have no semantic_type");
        }
    }

    #[test]
    fn datum_nodes_label_matches_type_prefix() {
        for (node, variant) in DatumType::datum_nodes().iter().zip(DatumType::all_variants().iter()) {
            assert_eq!(node.label, variant.type_prefix(), "label must equal type_prefix for {variant:?}");
            assert_eq!(node.id, format!("datum_type::{}", variant.type_prefix()), "id format mismatch for {variant:?}");
        }
    }

    // ── Datum dispatch resolution tests ──────────────────────────────────

    use super::{DatumDispatch, resolve_datum_dispatch, resolve_all_datum_dispatches};

    fn write_cli_datum(dir: &std::path::Path, name: &str, command: &str) {
        let toml = format!(
            "[b00t]\nname = \"{name}\"\ntype = \"cli\"\ncommand = \"{command}\"\n"
        );
        std::fs::write(dir.join(format!("{name}.cli.toml")), toml).unwrap();
    }

    fn write_runtime_datum(dir: &std::path::Path, name: &str, binary: &str) {
        let toml = format!(
            "[b00t]\nname = \"{name}\"\ntype = \"runtime\"\n\n[b00t.runtime]\nbinary = \"{binary}\"\n"
        );
        std::fs::write(dir.join(format!("{name}.runtime.toml")), toml).unwrap();
    }

    fn write_polyseme_datum(dir: &std::path::Path, name: &str) {
        let toml = format!(
            "[b00t]\nname = \"{name}\"\ntype = \"polyseme\"\n\n\
             [[b00t.polyseme.refs]]\nname = \"{name}-example\"\n\
             canonical = \"github:example/{name}\"\n\
             datum = \"{name}-example.cli\"\n\
             description = \"example {name}\"\n"
        );
        std::fs::write(dir.join(format!("{name}.polyseme.toml")), toml).unwrap();
    }

    #[test]
    fn resolve_cli_datum_returns_passthrough() {
        let dir = tempfile::tempdir().unwrap();
        write_cli_datum(dir.path(), "testcli", "echo");
        let result = resolve_datum_dispatch("testcli", dir.path().to_str().unwrap());
        assert!(result.is_some());
        match result.unwrap() {
            DatumDispatch::CliPassthrough { command, .. } => assert_eq!(command, "echo"),
            _ => panic!("expected CliPassthrough"),
        }
    }

    #[test]
    fn resolve_runtime_datum_returns_runtime() {
        let dir = tempfile::tempdir().unwrap();
        write_runtime_datum(dir.path(), "testrt", "/bin/true");
        let result = resolve_datum_dispatch("testrt", dir.path().to_str().unwrap());
        assert!(result.is_some());
        assert!(matches!(result.unwrap(), DatumDispatch::Runtime(_)));
    }

    #[test]
    fn resolve_polyseme_returns_refs() {
        let dir = tempfile::tempdir().unwrap();
        write_polyseme_datum(dir.path(), "testpoly");
        let result = resolve_datum_dispatch("testpoly", dir.path().to_str().unwrap());
        assert!(result.is_some());
        match result.unwrap() {
            DatumDispatch::Polyseme { refs, .. } => assert_eq!(refs.len(), 1),
            _ => panic!("expected Polyseme"),
        }
    }

    #[test]
    fn resolve_missing_datum_returns_none() {
        let dir = tempfile::tempdir().unwrap();
        let result = resolve_datum_dispatch("__nonexistent__", dir.path().to_str().unwrap());
        assert!(result.is_none());
    }

    #[test]
    fn package_install_spec_generates_multi_os_installer() {
        #[derive(Deserialize)]
        struct TestDatum {
            install: InstallSpec,
        }

        let datum: TestDatum = toml::from_str(r#"install = { package = "mold" }"#).unwrap();
        let command = datum.install.command_string().unwrap();

        assert!(command.contains("command -v 'mold'"));
        assert!(command.contains("sudo apt-get install -y 'mold'"));
        assert!(command.contains("sudo dnf install -y 'mold'"));
        assert!(command.contains("sudo pacman -S --needed --noconfirm 'mold'"));
        assert!(command.contains("brew install 'mold'"));
    }

    #[test]
    fn package_install_spec_supports_package_manager_overrides() {
        #[derive(Deserialize)]
        struct TestDatum {
            install: InstallSpec,
        }

        let datum: TestDatum =
            toml::from_str(r#"install = { package = "fd", binary = "fdfind", apt = "fd-find" }"#)
                .unwrap();
        let command = datum.install.command_string().unwrap();

        assert!(command.contains("command -v 'fdfind'"));
        assert!(command.contains("sudo apt-get install -y 'fd-find'"));
        assert!(command.contains("sudo dnf install -y 'fd'"));
    }

    #[test]
    fn tool_install_spec_generates_cargo_installer() {
        #[derive(Deserialize)]
        struct TestDatum {
            install: InstallSpec,
        }

        let datum: TestDatum = toml::from_str(r#"install = { cargo = "eureka" }"#).unwrap();
        let command = datum.install.command_string().unwrap();

        assert!(command.contains("command -v 'eureka'"));
        assert!(command.contains("command -v cargo"));
        assert!(command.contains("cargo install 'eureka'"));
    }

    #[test]
    fn tool_install_spec_generates_go_installer() {
        #[derive(Deserialize)]
        struct TestDatum {
            install: InstallSpec,
        }

        let datum: TestDatum = toml::from_str(
            r#"install = { go = "github.com/go-task/task/v3/cmd/task@latest" }"#,
        )
        .unwrap();
        let command = datum.install.command_string().unwrap();

        assert!(command.contains("command -v 'task'"));
        assert!(command.contains("command -v go"));
        assert!(command.contains("GO111MODULE=on go install 'github.com/go-task/task/v3/cmd/task@latest'"));
    }

    #[test]
    fn tool_install_spec_generates_npm_global_installer() {
        #[derive(Deserialize)]
        struct TestDatum {
            install: InstallSpec,
        }

        let datum: TestDatum = toml::from_str(
            r#"install = { npm_global = "@google/gemini-cli", binary = "gemini" }"#,
        )
        .unwrap();
        let command = datum.install.command_string().unwrap();

        assert!(command.contains("command -v 'gemini'"));
        assert!(command.contains("command -v npm"));
        assert!(command.contains("npm install -g '@google/gemini-cli'"));
    }

    #[test]
    fn tool_install_spec_generates_uv_tool_installer() {
        #[derive(Deserialize)]
        struct TestDatum {
            install: InstallSpec,
        }

        let datum: TestDatum = toml::from_str(r#"install = { uv_tool = "fastmcp" }"#).unwrap();
        let command = datum.install.command_string().unwrap();

        assert!(command.contains("command -v 'fastmcp'"));
        assert!(command.contains("command -v uv"));
        assert!(command.contains("uv tool install 'fastmcp'"));
    }

    #[test]
    fn install_metadata_is_not_parsed_as_tool_functor() {
        #[derive(Deserialize)]
        struct TestDatum {
            install: InstallSpec,
        }

        let datum: TestDatum =
            toml::from_str(r#"install = { requires = ["node", "npm"] }"#).unwrap();

        assert!(matches!(datum.install, InstallSpec::Metadata { .. }));
        assert!(datum.install.command_string().is_none());
    }

    #[test]
    fn resolve_prefers_runtime_over_cli() {
        let dir = tempfile::tempdir().unwrap();
        write_runtime_datum(dir.path(), "dual", "/bin/true");
        write_cli_datum(dir.path(), "dual", "echo");
        let result = resolve_datum_dispatch("dual", dir.path().to_str().unwrap());
        assert!(matches!(result.unwrap(), DatumDispatch::Runtime(_)));
    }

    #[test]
    fn resolve_all_returns_multiple_dispatches() {
        let dir = tempfile::tempdir().unwrap();
        write_cli_datum(dir.path(), "multi", "echo");
        write_polyseme_datum(dir.path(), "multi");
        let all = resolve_all_datum_dispatches("multi", dir.path().to_str().unwrap());
        assert!(all.len() >= 2);
        let has_cli = all.iter().any(|d| matches!(d, DatumDispatch::CliPassthrough { .. }));
        let has_poly = all.iter().any(|d| matches!(d, DatumDispatch::Polyseme { .. }));
        assert!(has_cli, "should have cli dispatch");
        assert!(has_poly, "should have polyseme dispatch");
    }
}
