//! Rhai hook engine for b00t datum lifecycle hooks.
//!
//! Hooks are inline Rhai scripts stored in datum fields (e.g. `hook_detect`).
//! They run at specific lifecycle points and return a `HookResult` that the
//! caller acts on.
//!
//! ## Available functions in hook scripts
//! - `which(name)` → `String`  — path to binary or `""` if not found
//! - `exec(cmd)`   → `String`  — stdout of shell command (best-effort, `""` on error)
//! - `which_capability(name)` → `String` — "yes" if capability exists in registry, `""` if not
//! - `capability_depends(name)` → `String` — comma-separated depends_on or `""` if none/missing
//!
//! ## Hook return protocol (string-based, LLM-friendly)
//! - `"ok"`                  — proceed normally
//! - `"warn: <message>"`     — print warning, proceed
//! - `"redirect:<datum>"`    — use a different datum instead (e.g. `"redirect:opentofu"`)
//! - `"missing: <message>"`  — binary not found; message shown as install hint
//! - anything else           — treated as informational message, printed verbatim
//!
//! ## Example datum hook
//! ```toml
//! hook_detect = '''
//! let tf = which("terraform");
//! if tf == "" { "missing: install opentofu — b00t cli install opentofu" }
//! else if exec("terraform version") =~ "OpenTofu" { "ok" }
//! else { "warn: HashiCorp Terraform (BSL) detected — prefer: b00t cli install opentofu" }
//! '''
//! ```

use rhai::{Engine, EvalAltResult, ImmutableString};
use serde::Deserialize;
use std::path::PathBuf;
use std::sync::LazyLock;
use toml::{Value, map::Map};

/// Capability info extracted from capability-registry.toml
#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct CapabilityInfo {
    pub name: String,
    #[serde(rename = "type")]
    pub capability_type: String,
    #[serde(default)]
    pub depends_on: Vec<String>,
    #[serde(default)]
    pub tags: Vec<String>,
}

/// Registry file parsed into a lookup structure
#[derive(Debug, Clone, Default, Deserialize)]
struct CapabilityRegistry {
    #[serde(default)]
    pub skills: Map<String, Value>,
    #[serde(default)]
    pub roles: Map<String, Value>,
    #[serde(default)]
    pub datums: Map<String, Value>,
    #[serde(default)]
    pub mcp: Map<String, Value>,
}

/// Global cache for the parsed capability registry
static CAPABILITY_REGISTRY: LazyLock<Option<CapabilityRegistry>> =
    LazyLock::new(|| load_capability_registry().ok());

/// Get the path to the capability registry file
fn capability_registry_path() -> Option<PathBuf> {
    // Try _B00T_Path env var first, fallback to workspace root, then default
    let candidates: Vec<String> = vec![
        std::env::var("_B00T_Path").ok().map(|p| {
            let expanded = shellexpand::tilde(&p);
            expanded.into_owned()
        }),
        // Workspace-relative path for dev/CI environments (git root)
        crate::utils::get_workspace_root()
            .parse::<String>()
            .ok()
            .map(|r| format!("{}/_b00t_", r)),
        // Legacy default
        Some(shellexpand::tilde("~/.b00t/_b00t_").into_owned()),
    ]
    .into_iter()
    .flatten()
    .collect();

    candidates.into_iter().find_map(|base| {
        let p = PathBuf::from(&base).join("capability-registry.toml");
        if p.exists() { Some(p) } else { None }
    })
}

/// Load and parse the capability registry TOML file
fn load_capability_registry() -> Result<CapabilityRegistry, Box<dyn std::error::Error>> {
    let path = capability_registry_path().ok_or("capability-registry.toml not found")?;
    let content = std::fs::read_to_string(path)?;
    let registry: CapabilityRegistry = toml::from_str(&content)?;
    Ok(registry)
}

/// Look up a capability by name across all sections (skills, roles, datums, mcp)
fn lookup_capability(name: &str) -> Option<CapabilityInfo> {
    let registry = CAPABILITY_REGISTRY.as_ref()?;

    // Search skills section
    if let Some(val) = registry.skills.get(name) {
        return parse_capability_entry(name, "skill", val);
    }
    // Search roles section
    if let Some(val) = registry.roles.get(name) {
        return parse_capability_entry(name, "role", val);
    }
    // Search datums section
    if let Some(val) = registry.datums.get(name) {
        return parse_capability_entry(name, "datum", val);
    }
    // Search mcp section
    if let Some(val) = registry.mcp.get(name) {
        return parse_capability_entry(name, "mcp", val);
    }

    None
}

/// Parse a capability entry from a TOML value
fn parse_capability_entry(name: &str, section_type: &str, val: &Value) -> Option<CapabilityInfo> {
    let table = val.as_table()?;

    let capability_type = table
        .get("type")
        .and_then(|v| v.as_str())
        .unwrap_or(section_type)
        .to_string();

    let depends_on = table
        .get("depends_on")
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|v| v.as_str().map(String::from))
                .collect()
        })
        .unwrap_or_default();

    let tags = table
        .get("tags")
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|v| v.as_str().map(String::from))
                .collect()
        })
        .unwrap_or_default();

    Some(CapabilityInfo {
        name: name.to_string(),
        capability_type,
        depends_on,
        tags,
    })
}

/// Query the capability registry for a capability by name.
/// Returns Some(CapabilityInfo) if found, None otherwise.
pub fn query_capability_registry(capability_name: &str) -> Option<CapabilityInfo> {
    lookup_capability(capability_name)
}

/// Result returned by a datum hook script.
#[derive(Debug, Clone, PartialEq)]
pub enum HookResult {
    /// Proceed normally.
    Ok,
    /// Print warning, then proceed.
    Warn(String),
    /// Suggest a different datum by name.
    Redirect(String),
    /// Binary not found; message is the install hint.
    Missing(String),
    /// Informational output — print and proceed.
    Info(String),
}

impl HookResult {
    /// Parse the string return value from a Rhai hook script.
    pub fn from_str(s: &str) -> Self {
        let s = s.trim();
        if s == "ok" || s.is_empty() {
            HookResult::Ok
        } else if let Some(msg) = s.strip_prefix("warn:") {
            HookResult::Warn(msg.trim().to_string())
        } else if let Some(name) = s.strip_prefix("redirect:") {
            HookResult::Redirect(name.trim().to_string())
        } else if let Some(msg) = s.strip_prefix("missing:") {
            HookResult::Missing(msg.trim().to_string())
        } else {
            HookResult::Info(s.to_string())
        }
    }
}

/// Run a Rhai hook script and return the parsed `HookResult`.
///
/// Registers `which(name)`, `exec(cmd)`, `which_capability(name)`, and `capability_depends(name)` as custom functions.
/// If the script starts with `"gates"` or `"@gates"`, it's resolved to `_b00t_/scripts/gates.rhai` — the reusable
/// gate evaluation module. The module is prepended so `evaluate_gates()`, `derive_gates()`, and `is_installed()`
/// are available in scope.
///
/// Errors in the script are returned as `HookResult::Warn` (non-fatal).
pub fn run_hook(script: &str) -> HookResult {
    let engine = build_engine();

    // Resolve "gates" or "@gates" shorthand → load reusable gate module
    let resolved = if script.trim() == "gates" || script.trim() == "@gates" {
        // Try workspace _b00t_/scripts/gates.rhai, then ~/.dotfiles/_b00t_/scripts/
        let candidates = [
            crate::utils::get_workspace_root() + "/_b00t_/scripts/gates.rhai",
            dirs::home_dir()
                .map(|h| h.join(".dotfiles").join("_b00t_").join("scripts").join("gates.rhai"))
                .unwrap_or_default()
                .to_string_lossy().to_string(),
        ];
        let content = candidates.iter().find_map(|p| std::fs::read_to_string(p).ok());
        match content {
            Some(src) => src,
            None => return HookResult::Warn("gates.rhai not found (searched workspace and ~/.dotfiles)".into()),
        }
    } else {
        script.to_string()
    };

    // For gate hooks, the script returns a boolean (gate result). Wrap in hook protocol.
    let full_script = if script.trim() == "gates" || script.trim() == "@gates" {
        format!(
            r#"{}
            let __datum_file = get_env("_B00T_DATUM_FILE");
            let __name = get_env("_B00T_DATUM_NAME");
            if __datum_file != "" && __name != "" {{
                let __content = read_file(__datum_file);
                let __pass = evaluate_gates(__name, __content);
                if __pass {{ "ok" }} else {{ "warn: gates blocked" }}
            }} else {{
                "missing: _B00T_DATUM_FILE and _B00T_DATUM_NAME must be set"
            }}"#,
            resolved
        )
    } else {
        resolved
    };

    match engine.eval::<ImmutableString>(&full_script) {
        Ok(result) => HookResult::from_str(result.as_str()),
        Err(e) => HookResult::Warn(format!("hook script error: {e}")),
    }
}

fn build_engine() -> Engine {
    let mut engine = Engine::new();

    // which(name: &str) → String — path to binary or "" if absent
    engine.register_fn("which", |name: ImmutableString| -> ImmutableString {
        match std::process::Command::new("which")
            .arg(name.as_str())
            .output()
        {
            Ok(o) if o.status.success() => {
                String::from_utf8_lossy(&o.stdout).trim().to_string().into()
            }
            _ => ImmutableString::from(""),
        }
    });

    // exec(cmd: &str) → String — stdout of shell command (stderr discarded, "" on error)
    engine.register_fn("exec", |cmd: ImmutableString| -> ImmutableString {
        match std::process::Command::new("sh")
            .args(["-c", cmd.as_str()])
            .output()
        {
            Ok(o) => String::from_utf8_lossy(&o.stdout).trim().to_string().into(),
            Err(_) => ImmutableString::from(""),
        }
    });

    // which_capability(name: &str) → String — "yes" if capability exists, "" if not
    engine.register_fn(
        "which_capability",
        |name: ImmutableString| -> ImmutableString {
            if query_capability_registry(name.as_str()).is_some() {
                ImmutableString::from("yes")
            } else {
                ImmutableString::from("")
            }
        },
    );

    // capability_depends(name: &str) → String — comma-separated depends_on or "" if none/missing
    engine.register_fn(
        "capability_depends",
        |name: ImmutableString| -> ImmutableString {
            match query_capability_registry(name.as_str()) {
                Some(info) if !info.depends_on.is_empty() => info.depends_on.join(",").into(),
                _ => ImmutableString::from(""),
            }
        },
    );

    // Gate evaluation functions — reused from rhai_engine.rs
    engine.register_fn("gate_check", |kind: &str, spec: &str| -> Result<bool, Box<EvalAltResult>> {
        let result = match kind {
            "command" => std::process::Command::new("which").arg(spec).output().map(|o| o.status.success()).unwrap_or(false),
            "file" => {
                let expanded = if spec.starts_with('~') {
                    let home = std::env::var("HOME").unwrap_or_default();
                    std::path::Path::new(&home).join(spec.strip_prefix("~/").unwrap_or(spec))
                } else {
                    std::path::Path::new(spec).to_path_buf()
                };
                expanded.exists()
            }
            "env" => {
                let direct = std::env::var(spec);
                if direct.is_ok() && !direct.unwrap_or_default().is_empty() {
                    true
                } else {
                    let ws = std::env::var("WORKSPACE_ROOT").or_else(|_| std::env::var("HOME")).unwrap_or_default();
                    let env_path = std::path::Path::new(&ws).join(".env");
                    if env_path.exists() {
                        if let Ok(content) = std::fs::read_to_string(&env_path) {
                            let prefix = format!("{}=", spec);
                            for line in content.lines() {
                                if line.trim().starts_with(&prefix) {
                                    let val = line.trim()[prefix.len()..].trim();
                                    return Ok(!val.is_empty() && !val.starts_with('#'));
                                }
                            }
                        }
                    }
                    false
                }
            }
            _ => return Err(format!("unknown gate kind: {}", kind).into()),
        };
        Ok(result)
    });

    engine.register_fn("get_env", |var: &str| -> String { std::env::var(var).unwrap_or_default() });
    engine.register_fn("read_file", |path: &str| -> Result<String, Box<EvalAltResult>> {
        std::fs::read_to_string(path).map_err(|e| format!("read_file error: {}", e).into())
    });
    engine.register_fn("log_info", |msg: &str| println!("ℹ️  {}", msg));
    engine.register_fn("session_track", |event: &str, detail: &str| {
        let home = std::env::var("HOME").unwrap_or_default();
        let dir = std::path::Path::new(&home).join(".b00t");
        let _ = std::fs::create_dir_all(&dir);
        let path = dir.join("events.jsonl");
        if let Ok(mut file) = std::fs::OpenOptions::new().create(true).append(true).open(&path) {
            use std::io::Write;
            let entry = serde_json::json!({
                "ts": chrono::Utc::now().to_rfc3339(),
                "event": event,
                "detail": detail,
                "pid": std::process::id(),
            });
            let _ = writeln!(file, "{}", entry);
        }
    });

    engine.set_max_expr_depths(128, 64);

    engine
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_hook_result_parsing() {
        assert_eq!(HookResult::from_str("ok"), HookResult::Ok);
        assert_eq!(HookResult::from_str(""), HookResult::Ok);
        assert_eq!(
            HookResult::from_str("warn: use opentofu"),
            HookResult::Warn("use opentofu".into())
        );
        assert_eq!(
            HookResult::from_str("redirect:opentofu"),
            HookResult::Redirect("opentofu".into())
        );
        assert_eq!(
            HookResult::from_str("missing: b00t cli install opentofu"),
            HookResult::Missing("b00t cli install opentofu".into())
        );
        assert_eq!(
            HookResult::from_str("some info"),
            HookResult::Info("some info".into())
        );
    }

    #[test]
    fn test_hook_ok_script() {
        let result = run_hook(r#""ok""#);
        assert_eq!(result, HookResult::Ok);
    }

    #[test]
    fn test_hook_which_returns_string() {
        // which("sh") should find /bin/sh on any POSIX system
        let result = run_hook(
            r#"
            let p = which("sh");
            if p != "" { "ok" } else { "missing: sh not found" }
        "#,
        );
        assert_eq!(result, HookResult::Ok);
    }

    #[test]
    fn test_hook_which_missing_binary() {
        let result = run_hook(
            r#"
            let p = which("this-binary-definitely-does-not-exist-b00t");
            if p == "" { "missing: not found" } else { "ok" }
        "#,
        );
        assert_eq!(result, HookResult::Missing("not found".into()));
    }

    #[test]
    fn test_hook_exec_runs_command() {
        let result = run_hook(
            r#"
            let out = exec("echo hello");
            if out == "hello" { "ok" } else { "warn: unexpected: " + out }
        "#,
        );
        assert_eq!(result, HookResult::Ok);
    }

    #[test]
    fn test_hook_script_error_is_warn() {
        let result = run_hook("this is not valid rhai @@@@");
        assert!(matches!(result, HookResult::Warn(_)));
    }

    #[test]
    fn test_query_capability_registry_skills() {
        // Test querying a skill from capability-registry.toml via hook (which loads registry lazily)
        let result = run_hook(
            r#"
            let exists = which_capability("bash");
            if exists == "yes" { "ok" } else { "warn: bash not found" }
        "#,
        );
        assert_eq!(result, HookResult::Ok);
    }

    #[test]
    fn test_query_capability_registry_roles() {
        // Test querying a role with depends_on via hook
        let result = run_hook(
            r#"
            let exists = which_capability("orchestrator");
            if exists == "yes" { "ok" } else { "warn: orchestrator not found" }
        "#,
        );
        assert_eq!(result, HookResult::Ok);
    }

    #[test]
    fn test_query_capability_registry_not_found() {
        let info = query_capability_registry("nonexistent-capability-xyz");
        assert!(info.is_none());
    }

    #[test]
    fn test_which_capability_hook_function() {
        // Test the which_capability Rhai function returns "yes" for existing capability
        let result = run_hook(
            r#"
            let exists = which_capability("bash");
            if exists == "yes" { "ok" } else { "warn: bash not found" }
        "#,
        );
        assert_eq!(result, HookResult::Ok);
    }

    #[test]
    fn test_which_capability_hook_function_not_found() {
        // Test the which_capability Rhai function returns "" for missing capability
        let result = run_hook(
            r#"
            let exists = which_capability("nonexistent-xyz-capability");
            if exists == "" { "ok" } else { "warn: should be empty" }
        "#,
        );
        assert_eq!(result, HookResult::Ok);
    }

    #[test]
    fn test_capability_depends_hook_function() {
        // Test the capability_depends Rhai function - returns depends for capabilities that have them
        // Note: orchestrator exists in both skills (no deps) and roles (with deps), skills found first
        let result = run_hook(
            r#"
            let deps = capability_depends("bash");
            // bash has no depends_on, so should return empty
            if deps == "" { "ok" } else { "warn: expected empty deps, got: " + deps }
        "#,
        );
        assert_eq!(result, HookResult::Ok);
    }

    #[test]
    fn test_capability_depends_hook_function_not_found() {
        // Test the capability_depends Rhai function returns "" for missing capability
        let result = run_hook(
            r#"
            let deps = capability_depends("nonexistent-xyz-capability");
            if deps == "" { "ok" } else { "warn: expected empty deps, got: " + deps }
        "#,
        );
        assert_eq!(result, HookResult::Ok);
    }
}
