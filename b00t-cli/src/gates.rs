use anyhow::Result;
use serde::{Deserialize, Serialize};
use ufo_types::{Disposition, IsoAuditable, Satisfies, SatisfiesResult};

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
    /// Justfile path (supports ~) that must parse cleanly (`just --list`
    /// via `JustfileAst::validate()` — reused as-is, no new validation logic)
    pub justfile: Option<String>,
    /// Freeform description shown when gate fails
    pub hint: Option<String>,
}

/// Result of evaluating a single gate precondition.
#[derive(Debug, Clone, PartialEq, Serialize)]
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
        std::path::Path::new(&home).join(spec.strip_prefix("~/").unwrap_or(spec))
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
            if std::env::var(spec).ok().map_or(false, |v| !v.is_empty()) {
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

/// Per-field outcome of a single gate's checks, in field-declaration order
/// (command, file, env, rhai, knowledge_backend). Only non-passing fields
/// produce an outcome. `Violated` is a genuine failed check; `UnknownReason`
/// is reserved for checks that could not be evaluated at all (currently:
/// a rhai expression that failed to *compile/evaluate*, as opposed to one
/// that evaluated cleanly to `false`) — carries its own reason text so the
/// legacy bool-returning `evaluate_gates()` can reproduce today's exact
/// message, even though `ufo_types::Disposition::Unknown` itself carries no
/// payload.
enum FieldOutcome {
    Violated(String),
    UnknownReason(String),
}

impl GateSpec {
    /// Runs the same command/file/env/rhai/knowledge_backend checks
    /// `evaluate_gates()` has always run, in the same order, producing the
    /// same reason text for each failing field — but tagging each failure
    /// as a genuine `Violated` or an `UnknownReason` (rhai eval error)
    /// instead of collapsing both into "not passed".
    fn eval_fields(&self, path: &str) -> Vec<FieldOutcome> {
        let mut outcomes = Vec::new();

        // Command gate: check if command exists on PATH
        if let Some(ref cmd) = self.command {
            if !check_command_available(cmd) {
                outcomes.push(FieldOutcome::Violated(format!(
                    "command '{}' not found on PATH",
                    cmd
                )));
            }
        }

        // File gate: check if file exists (supports ~ expansion and relative paths)
        if let Some(ref file) = self.file {
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
                base.join(&expanded).exists() || std::path::Path::new(&expanded).exists()
            };
            if !exists {
                outcomes.push(FieldOutcome::Violated(format!(
                    "file '{}' does not exist",
                    file
                )));
            }
        }

        // Env gate: check if env var or .env entry is set
        if let Some(ref env_var) = self.env {
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
                    outcomes.push(FieldOutcome::Violated(format!(
                        "env var '{}' not set",
                        env_var
                    )));
                }
            }
        }

        // Rhai gate: evaluate rhai expression via the Disposition-native
        // classifier (#927 fix — an eval ERROR is `Unknown`, not a
        // violation; only `Ok(false)` is a genuine violation).
        if let Some(ref rhai_expr) = self.rhai {
            match rhai_gate_disposition(rhai_expr) {
                Disposition::Satisfied => {}
                Disposition::Violated { reason } => outcomes.push(FieldOutcome::Violated(reason)),
                Disposition::Unknown => {
                    // `Disposition::Unknown` carries no payload, so recover
                    // the original rhai error text (for the legacy
                    // bool-returning `evaluate_gates()`'s reason string) by
                    // re-running the eval. Only reached on eval error, which
                    // is rare, so the extra eval is not a hot-path cost.
                    let (_, err) = evaluate_rhai_gate(rhai_expr);
                    let text = err
                        .map(|e| format!("rhai gate '{rhai_expr}' failed: {e}"))
                        .unwrap_or_else(|| format!("rhai gate '{rhai_expr}' failed"));
                    outcomes.push(FieldOutcome::UnknownReason(text));
                }
            }
        }

        if let Some(ref backend) = self.knowledge_backend {
            if b00t_c0re_lib::compiled_knowledge_backend() != backend {
                outcomes.push(FieldOutcome::Violated(format!(
                    "knowledge backend '{}' does not match compiled backend '{}'",
                    backend,
                    b00t_c0re_lib::compiled_knowledge_backend()
                )));
            }
        }

        // Justfile gate: reuses JustfileAst::validate() (runs `just --list`
        // against the file) rather than reimplementing parse-checking here.
        if let Some(ref justfile_path) = self.justfile {
            let expanded = expand_tilde_path(justfile_path);
            let candidate = if expanded.is_absolute() {
                expanded.clone()
            } else {
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
                base.join(&expanded)
            };
            // Mirror the `file` gate's dual resolution: try datum-dir-relative
            // first, fall back to cwd-relative (a checklist commonly points at
            // the repo-root justfile, not one inside the datum dir itself).
            let resolved = if candidate.exists() {
                candidate
            } else if expanded.exists() {
                expanded.clone()
            } else {
                candidate
            };
            // Absolutize: JustfileAst::load's run_dump sets current_dir to the
            // resolved path's parent *and* re-passes that same relative path to
            // `just --justfile`, so a relative path with a parent component gets
            // joined twice (e.g. "_b00t_/justfile" -> ".../_b00t_/_b00t_/justfile").
            let resolved = std::fs::canonicalize(&resolved).unwrap_or(resolved);
            match crate::just_ast::JustfileAst::load(&resolved) {
                Ok(ast) => {
                    let errors = ast.validate();
                    if !errors.is_empty() {
                        outcomes.push(FieldOutcome::Violated(format!(
                            "justfile '{}' failed validation: {}",
                            justfile_path,
                            errors.join("; ")
                        )));
                    }
                }
                Err(e) => outcomes.push(FieldOutcome::Violated(format!(
                    "justfile '{}' could not be loaded: {}",
                    justfile_path, e
                ))),
            }
        }

        outcomes
    }

    /// Disposition-returning evaluation of this gate's preconditions.
    /// A genuine violation always wins over an undetermined (`Unknown`)
    /// field; only when every field either passed or was undetermined (and
    /// none were genuinely violated) does an undetermined field yield
    /// `Unknown` overall.
    pub fn eval_disposition(&self, path: &str) -> Disposition {
        let outcomes = self.eval_fields(path);

        let violated: Vec<String> = outcomes
            .iter()
            .filter_map(|o| match o {
                FieldOutcome::Violated(reason) => Some(reason.clone()),
                FieldOutcome::UnknownReason(_) => None,
            })
            .collect();
        if !violated.is_empty() {
            let reason = self
                .hint
                .clone()
                .map(|h| format!("{}: {}", h, violated.join("; ")))
                .unwrap_or_else(|| violated.join("; "));
            return Disposition::Violated { reason };
        }

        if outcomes
            .iter()
            .any(|o| matches!(o, FieldOutcome::UnknownReason(_)))
        {
            return Disposition::Unknown;
        }

        Disposition::Satisfied
    }
}

/// Evaluate all gates for a datum. Returns a Vec of GateResults, one per gate.
/// If any gate fails, the datum should be skipped.
///
/// Thin fold over `GateSpec::eval_fields()` — preserves the exact
/// `passed`/`reason` behavior this function has always had (including for
/// the rhai-eval-error case, which today is folded into `passed: false`
/// with the same reason text; #927 exposes that same case as
/// `Disposition::Unknown` via `eval_disposition()` without changing this
/// function's observable output).
pub fn evaluate_gates(gates: &[GateSpec], path: &str) -> Vec<GateResult> {
    gates
        .iter()
        .map(|gate| {
            let outcomes = gate.eval_fields(path);
            let reasons: Vec<String> = outcomes
                .iter()
                .map(|o| match o {
                    FieldOutcome::Violated(reason) => reason.clone(),
                    FieldOutcome::UnknownReason(reason) => reason.clone(),
                })
                .collect();
            let passed = reasons.is_empty();
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
            GateResult { passed, reason }
        })
        .collect()
}

/// Evaluate a simple rhai boolean expression, returning (passed, error_text).
/// `error_text` is `Some` only when the expression failed to evaluate at all
/// (syntax error, unknown symbol, …) — used by `eval_fields()` to reproduce
/// today's exact legacy reason text.
fn evaluate_rhai_gate(expr: &str) -> (bool, Option<String>) {
    use rhai::Engine;
    let engine = Engine::new();
    match engine.eval::<bool>(expr) {
        Ok(true) => (true, None),
        Ok(false) => (false, None),
        Err(e) => (false, Some(e.to_string())),
    }
}

/// Disposition-returning evaluation of a rhai boolean expression. An eval
/// ERROR (bad syntax, unknown symbol, …) means the expression could not be
/// determined true or false — that is `Unknown`, not a violation. Fixes the
/// #927 bug where `evaluate_rhai_gate`'s bool-collapsing treated a rhai eval
/// error as an outright `false` (i.e. a violation).
fn rhai_gate_disposition(expr: &str) -> Disposition {
    use rhai::Engine;
    let engine = Engine::new();
    match engine.eval::<bool>(expr) {
        Ok(true) => Disposition::Satisfied,
        Ok(false) => Disposition::Violated {
            reason: format!("rhai gate '{expr}' returned false"),
        },
        Err(_) => Disposition::Unknown,
    }
}

/// Constraint: `path`'s datum must satisfy every gate in `gates`.
/// Bundles the gate list with the evaluation path since `Satisfies<C>`
/// takes the constraint by reference alone — `GateSpec::eval_disposition`
/// needs both.
pub struct GatePreconditions<'a> {
    pub gates: &'a [GateSpec],
    pub path: &'a str,
}

impl IsoAuditable for GatePreconditions<'_> {
    fn iso_standard_ids(&self) -> Vec<String> {
        vec!["b00t:GatePrecondition".into()]
    }
}

impl Satisfies<GatePreconditions<'_>> for crate::BootDatum {
    fn satisfies(&self, c: &GatePreconditions<'_>) -> SatisfiesResult {
        if c.gates.is_empty() {
            return SatisfiesResult::satisfied(1.0, Vec::new());
        }

        let dispositions: Vec<Disposition> = c
            .gates
            .iter()
            .map(|g| g.eval_disposition(c.path))
            .collect();

        let violated: Vec<String> = dispositions
            .iter()
            .filter_map(|d| match d {
                Disposition::Violated { reason } => Some(reason.clone()),
                _ => None,
            })
            .collect();
        if !violated.is_empty() {
            return SatisfiesResult::violated(violated.join("; "));
        }

        if dispositions
            .iter()
            .any(|d| matches!(d, Disposition::Unknown))
        {
            return SatisfiesResult::unknown();
        }

        SatisfiesResult::satisfied(1.0, Vec::new())
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

#[cfg(test)]
mod satisfies_tests {
    use super::*;
    use ufo_types::satisfies::Disposition as UfoDisposition;

    #[test]
    fn eval_disposition_bad_rhai_syntax_is_unknown_not_violated() {
        // Regression lock for the #927 bug: a rhai eval ERROR (bad syntax)
        // must be Unknown, never Violated — we could not determine
        // pass/fail, we did not determine it to be false.
        let gate = GateSpec {
            rhai: Some("this is not valid rhai (((".to_string()),
            ..Default::default()
        };
        let disposition = gate.eval_disposition("/tmp");
        assert!(
            matches!(disposition, UfoDisposition::Unknown),
            "expected Unknown for a rhai syntax error, got {:?}",
            disposition
        );
    }

    #[test]
    fn eval_disposition_rhai_false_is_violated() {
        // A rhai expression that evaluates cleanly to `false` IS a genuine
        // violation — distinct from the syntax-error/Unknown case above,
        // proving the 3-way split (Satisfied/Violated/Unknown) is real.
        let gate = GateSpec {
            rhai: Some("false".to_string()),
            ..Default::default()
        };
        let disposition = gate.eval_disposition("/tmp");
        match disposition {
            UfoDisposition::Violated { reason } => {
                assert!(reason.contains("returned false"), "reason: {reason}");
            }
            other => panic!("expected Violated, got {:?}", other),
        }
    }

    #[test]
    fn eval_disposition_rhai_true_is_satisfied() {
        let gate = GateSpec {
            rhai: Some("true".to_string()),
            ..Default::default()
        };
        assert!(matches!(
            gate.eval_disposition("/tmp"),
            UfoDisposition::Satisfied
        ));
    }

    #[test]
    fn evaluate_gates_regression_lock_bad_rhai_syntax_matches_legacy_output() {
        // Regression-lock: legacy bool-returning evaluate_gates() must
        // produce byte-identical passed/reason output for a rhai syntax
        // error after the #927 refactor — this test would have caught a
        // behavior change in that path.
        let gates = vec![GateSpec {
            rhai: Some("this is not valid rhai (((".to_string()),
            ..Default::default()
        }];
        let results = evaluate_gates(&gates, "/tmp");
        assert_eq!(results.len(), 1);
        assert!(!results[0].passed, "rhai syntax error should fail the gate");
        assert!(
            results[0].reason.contains("rhai gate")
                && results[0].reason.contains("failed:"),
            "reason should preserve the legacy 'rhai gate ... failed: <err>' text, got: {}",
            results[0].reason
        );
    }

    #[test]
    fn evaluate_gates_regression_lock_rhai_false_matches_legacy_output() {
        let gates = vec![GateSpec {
            rhai: Some("false".to_string()),
            ..Default::default()
        }];
        let results = evaluate_gates(&gates, "/tmp");
        assert_eq!(results.len(), 1);
        assert!(!results[0].passed);
        assert!(results[0].reason.contains("returned false"));
    }

    #[test]
    fn evaluate_gates_regression_lock_rhai_true_passes() {
        let gates = vec![GateSpec {
            rhai: Some("true".to_string()),
            ..Default::default()
        }];
        let results = evaluate_gates(&gates, "/tmp");
        assert_eq!(results.len(), 1);
        assert!(results[0].passed);
    }

    #[test]
    fn gate_preconditions_empty_gates_is_satisfied() {
        let datum = crate::BootDatum {
            name: "empty-gates".into(),
            ..Default::default()
        };
        let gates: Vec<GateSpec> = vec![];
        let constraint = GatePreconditions {
            gates: &gates,
            path: "/tmp",
        };
        let result = datum.satisfies(&constraint);
        assert!(result.is_satisfied());
    }

    #[test]
    fn gate_preconditions_violated_gate_wins() {
        let datum = crate::BootDatum {
            name: "violated-gate".into(),
            ..Default::default()
        };
        let gates = vec![GateSpec {
            command: Some("definitely-not-a-real-command-927".to_string()),
            ..Default::default()
        }];
        let constraint = GatePreconditions {
            gates: &gates,
            path: "/tmp",
        };
        let result = datum.satisfies(&constraint);
        assert!(result.is_violated());
    }

    #[test]
    fn gate_preconditions_unknown_gate_yields_unknown_disposition() {
        let datum = crate::BootDatum {
            name: "unknown-gate".into(),
            ..Default::default()
        };
        let gates = vec![GateSpec {
            rhai: Some("this is not valid rhai (((".to_string()),
            ..Default::default()
        }];
        let constraint = GatePreconditions {
            gates: &gates,
            path: "/tmp",
        };
        let result = datum.satisfies(&constraint);
        assert!(
            matches!(result.disposition, UfoDisposition::Unknown),
            "expected Unknown, got {:?}",
            result.disposition
        );
    }

    #[test]
    fn gate_preconditions_violated_wins_over_unknown() {
        // One violated gate + one undecidable (rhai syntax error) gate:
        // Violated must win per the documented precedence.
        let datum = crate::BootDatum {
            name: "mixed-gates".into(),
            ..Default::default()
        };
        let gates = vec![
            GateSpec {
                command: Some("definitely-not-a-real-command-927".to_string()),
                ..Default::default()
            },
            GateSpec {
                rhai: Some("this is not valid rhai (((".to_string()),
                ..Default::default()
            },
        ];
        let constraint = GatePreconditions {
            gates: &gates,
            path: "/tmp",
        };
        let result = datum.satisfies(&constraint);
        assert!(result.is_violated());
    }
}
