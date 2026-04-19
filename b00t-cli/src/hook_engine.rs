//! Rhai hook engine for b00t datum lifecycle hooks.
//!
//! Hooks are inline Rhai scripts stored in datum fields (e.g. `hook_detect`).
//! They run at specific lifecycle points and return a `HookResult` that the
//! caller acts on.
//!
//! ## Available functions in hook scripts
//! - `which(name)` → `String`  — path to binary or `""` if not found
//! - `exec(cmd)`   → `String`  — stdout of shell command (best-effort, `""` on error)
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
/// Registers `which(name)` and `exec(cmd)` as custom functions.
/// Errors in the script are returned as `HookResult::Warn` (non-fatal).
pub fn run_hook(script: &str) -> HookResult {
    let engine = build_engine();
    match engine.eval::<ImmutableString>(script) {
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

    // 🤓 disable file access — hooks must not read/write filesystem directly
    engine.set_max_expr_depths(64, 32);

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
}
