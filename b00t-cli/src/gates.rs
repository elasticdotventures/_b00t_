use anyhow::Result;
use serde::{Deserialize, Serialize};

/// A single gate precondition — late-binding condition evaluated at install time.
/// All fields are optional; any present field must pass for the gate to open.
#[derive(Deserialize, Serialize, Debug, Clone, PartialEq, Default)]
pub struct GateSpec {
    /// Command that must exist on PATH for this gate to pass
    pub command: Option<String>,
    /// File path (supports ~) that must exist
    pub file: Option<String>,
    /// Environment variable (or .env key) that must be set to a non-empty value
    pub env: Option<String>,
    /// Rhai expression to evaluate; must return true for gate to pass
    /// Available vars: name, datum_type, path
    pub rhai: Option<String>,
    /// Knowledge backend that must match the compiled b00t-c0re-lib backend
    pub knowledge_backend: Option<String>,
    /// Freeform description shown when gate fails
    pub hint: Option<String>,
}

/// Result of evaluating a single gate precondition.
#[derive(Debug, Clone, PartialEq)]
pub struct GateResult {
    pub passed: bool,
    pub reason: String,
}

/// A single gate with its origin (explicit or auto-derived)
#[derive(Debug, Clone, Serialize)]
pub struct GateReport {
    pub datum: String,
    pub kind: String,
    pub spec: String,
    pub origin: String, // "explicit" or "auto:requires" or "auto:env"
    pub hint: Option<String>,
    /// "pass" | "fail" | "unknown" — populated at scan time
    pub status: &'static str,
}

pub fn check_command_available(command: &str) -> bool {
    duct::cmd!("which", command).read().is_ok()
}

/// Expand a leading `~/` in a path using the HOME env var.
fn expand_tilde_path(spec: &str) -> std::path::PathBuf {
    if spec.starts_with('~') {
        let home = std::env::var("HOME").unwrap_or_default();
        std::path::Path::new(&home)
            .join(spec.strip_prefix("~/").unwrap_or(spec))
    } else {
        std::path::Path::new(spec).to_path_buf()
    }
}

/// Returns "pass", "fail", or "unknown" for a gate condition checked at scan time.
pub fn eval_gate_status(kind: &str, spec: &str) -> &'static str {
    match kind {
        "command" => {
            if check_command_available(spec) {
                "pass"
            } else {
                "fail"
            }
        }
        "env" => {
            if std::env::var(spec)
                .ok()
                .map_or(false, |v| !v.is_empty())
            {
                return "pass";
            }
            // check .env in workspace root
            let ws = std::env::var("WORKSPACE_ROOT")
                .or_else(|_| std::env::var("HOME"))
                .unwrap_or_default();
            let env_path = std::path::Path::new(&ws).join(".env");
            if env_path.exists() {
                if let Ok(content) = std::fs::read_to_string(env_path) {
                    let prefix = format!("{}=", spec);
                    for line in content.lines() {
                        let trimmed = line.trim();
                        if let Some(rest) = trimmed.strip_prefix(prefix.as_str()) {
                            let val = rest.trim();
                            if !val.is_empty() && !val.starts_with('#') {
                                return "pass";
                            }
                        }
                    }
                }
            }
            "fail"
        }
        "file" => {
            if expand_tilde_path(spec).exists() {
                "pass"
            } else {
                "fail"
            }
        }
        "knowledge_backend" => {
            if b00t_c0re_lib::compiled_knowledge_backend() == spec {
                "pass"
            } else {
                "fail"
            }
        }
        "rhai" => "unknown",
        _ => "unknown",
    }
}

/// Evaluate all gates for a datum. Returns a Vec of GateResults, one per gate.
/// If any gate fails, the datum should be skipped.
pub fn evaluate_gates(gates: &[GateSpec], path: &str) -> Vec<GateResult> {
    let mut results = Vec::new();
    for gate in gates {
        let mut passed = true;
        let mut reasons = Vec::new();

        // Command gate: check if command exists on PATH
        if let Some(ref cmd) = gate.command {
            if !check_command_available(cmd) {
                passed = false;
                reasons.push(format!("command '{}' not found on PATH", cmd));
            }
        }

        // File gate: check if file exists (supports ~ expansion and relative paths)
        if let Some(ref file) = gate.file {
            let expanded = shellexpand::tilde(file).to_string();
            let exists = if std::path::Path::new(&expanded).is_absolute() {
                std::path::Path::new(&expanded).exists()
            } else {
                // Try relative to datum directory (path may be a file; use parent if so).
                // Fall back to current working directory.
                let base = {
                    let p = std::path::Path::new(path);
                    if p.is_dir() {
                        p.to_path_buf()
                    } else {
                        p.parent()
                            .map(|q| q.to_path_buf())
                            .unwrap_or_else(|| std::path::PathBuf::from("."))
                    }
                };
                base.join(&expanded).exists()
                    || std::path::Path::new(&expanded).exists()
            };
            if !exists {
                passed = false;
                reasons.push(format!("file '{}' does not exist", file));
            }
        }

        // Env gate: check if env var or .env entry is set
        if let Some(ref env_var) = gate.env {
            let direct = std::env::var(env_var);
            let env_ok = direct.is_ok() && !direct.unwrap_or_default().is_empty();
            if !env_ok {
                // Fallback: check .env at WORKSPACE_ROOT
                let ws = std::env::var("WORKSPACE_ROOT")
                    .or_else(|_| std::env::var("HOME"))
                    .unwrap_or_default();
                let env_path = std::path::Path::new(&ws).join(".env");
                let env_file_ok = if env_path.exists() {
                    if let Ok(content) = std::fs::read_to_string(&env_path) {
                        let prefix = format!("{}=", env_var);
                        content.lines().any(|line| {
                            let trimmed = line.trim();
                            trimmed.starts_with(&prefix)
                                && !trimmed[prefix.len()..].trim().is_empty()
                                && !trimmed[prefix.len()..].trim().starts_with('#')
                        })
                    } else {
                        false
                    }
                } else {
                    false
                };
                if !env_file_ok {
                    passed = false;
                    reasons.push(format!("env var '{}' not set", env_var));
                }
            }
        }

        // Rhai gate: evaluate rhai expression
        if let Some(ref rhai_expr) = gate.rhai {
            let (rhai_ok, rhai_err) = evaluate_rhai_gate(rhai_expr);
            if !rhai_ok {
                passed = false;
                let reason = if let Some(err) = rhai_err {
                    format!("rhai gate '{rhai_expr}' failed: {err}")
                } else {
                    format!("rhai gate '{rhai_expr}' returned false")
                };
                reasons.push(reason);
            }
        }

        if let Some(ref backend) = gate.knowledge_backend {
            if b00t_c0re_lib::compiled_knowledge_backend() != backend {
                passed = false;
                reasons.push(format!(
                    "knowledge backend '{}' does not match compiled backend '{}'",
                    backend,
                    b00t_c0re_lib::compiled_knowledge_backend()
                ));
            }
        }

        let reason = if reasons.is_empty() {
            gate.hint
                .clone()
                .unwrap_or_else(|| "gate passed".to_string())
        } else {
            gate.hint
                .clone()
                .map(|h| format!("{}: {}", h, reasons.join("; ")))
                .unwrap_or_else(|| reasons.join("; "))
        };

        results.push(GateResult { passed, reason });
    }
    results
}

/// Evaluate a simple rhai boolean expression.
fn evaluate_rhai_gate(expr: &str) -> (bool, Option<String>) {
    use rhai::Engine;
    let engine = Engine::new();
    match engine.eval::<bool>(expr) {
        Ok(true) => (true, None),
        Ok(false) => (false, None),
        Err(e) => (false, Some(e.to_string())),
    }
}

/// Scan datum files in `path` (.mcp.toml, .mcp.tomllm, .mcp.tomllmd),
/// extract explicit [[b00t.gate]] declarations and auto-derived gates,
/// and evaluate their current status.
pub fn list_gates(path: &str, search: Option<&str>) -> Result<Vec<GateReport>> {
    let expanded = crate::get_expanded_path(path)?;
    let mut gates = Vec::new();

    for entry in std::fs::read_dir(&expanded)
        .map_err(|e| anyhow::anyhow!("Error reading {}: {}", expanded.display(), e))?
    {
        let entry = entry?;
        let fpath = entry.path();
        let fname = match fpath.file_name().and_then(|s| s.to_str()) {
            Some(n) => n.to_string(),
            None => continue,
        };

        // Accept .mcp.toml, .mcp.tomllm, .mcp.tomllmd
        let is_mcp_datum = fname.ends_with(".mcp.toml")
            || fname.ends_with(".mcp.tomllm")
            || fname.ends_with(".mcp.tomllmd");
        if !is_mcp_datum {
            continue;
        }

        let name = fname
            .trim_end_matches(".tomllmd")
            .trim_end_matches(".tomllm")
            .trim_end_matches(".toml")
            .trim_end_matches(".mcp")
            .to_string();
        if name.is_empty() {
            continue;
        }

        let content = std::fs::read_to_string(&fpath)?;
        let config: Result<crate::UnifiedConfig, _> = toml::from_str(&content);
        let datum = match config {
            Ok(c) => c.b00t,
            Err(_) => continue,
        };

        // apply search filter
        if let Some(q) = search {
            if !name.to_lowercase().contains(&q.to_lowercase())
                && !datum.hint.to_lowercase().contains(&q.to_lowercase())
            {
                continue;
            }
        }

        let mut push_gate = |kind: &str, spec: &str, origin: &str, hint: Option<String>| {
            gates.push(GateReport {
                datum: name.clone(),
                kind: kind.to_string(),
                spec: spec.to_string(),
                origin: origin.to_string(),
                hint,
                status: eval_gate_status(kind, spec),
            });
        };

        // explicit gates from [[b00t.gate]]
        if let Some(explicit) = &datum.gate {
            for g in explicit {
                if let Some(cmd) = &g.command {
                    push_gate("command", cmd, "explicit", g.hint.clone());
                }
                if let Some(f) = &g.file {
                    push_gate("file", f, "explicit", g.hint.clone());
                }
                if let Some(e) = &g.env {
                    push_gate("env", e, "explicit", g.hint.clone());
                }
                if let Some(r) = &g.rhai {
                    push_gate("rhai", r, "explicit", g.hint.clone());
                }
                if let Some(backend) = &g.knowledge_backend {
                    push_gate("knowledge_backend", backend, "explicit", g.hint.clone());
                }
            }
        }

        if let Some(knowledge) = &datum.knowledge {
            if let Some(backend) = &knowledge.backend {
                push_gate(
                    "knowledge_backend",
                    backend,
                    "auto:knowledge",
                    Some(
                        "datum knowledge backend must match compiled b00t-c0re-lib backend"
                            .to_string(),
                    ),
                );
            }
        }

        // auto-derived from requires
        if let Some(req) = &datum.require {
            for r in req {
                if r != "internet" {
                    push_gate("command", r, "auto:requires", None);
                }
            }
        }

        // auto-derived from top-level env
        if let Some(env_map) = &datum.env {
            for (k, _) in env_map {
                if !k.starts_with("LOG_") && !k.starts_with("FAST") {
                    push_gate("env", k, "auto:env", None);
                }
            }
        }
    }

    Ok(gates)
}
