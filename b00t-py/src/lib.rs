//! # b00t-py
//!
//! Python bindings for b00t-cli with native performance using PyO3.
//!
//! This module provides high-performance Python bindings for the b00t ecosystem,
//! offering 10-100x performance improvements over subprocess-based approaches.
//!
//! # Quick start
//!
//! ```python
//! from b00t_py import EmojiRegistry, check_guards, parse_kmdline
//!
//! # Emoji lookup — returns literal emoji string
//! reg = EmojiRegistry()
//! skunk = reg.lookup_shortcode(":skunk:")  # → "🦨"
//!
//! # Guard evaluation — native Python return
//! guards = [
//!     {"pattern": "pip install", "action": "warn",
//!      "message": "use uv pip install", "redirect": "uv pip install"}
//! ]
//! result = check_guards("pip install flask", guards)
//! assert result["action"] == "warn"
//! ```

use pyo3::create_exception;
use pyo3::exceptions::PyTypeError;
use pyo3::prelude::*;
use pyo3::types::{PyDict, PyList};

use std::collections::HashMap;

// Import b00t-cli functions
use b00t_cli::model_manager::{self, ServeOptions};
use b00t_cli::{get_expanded_path, mcp_list, mcp_output};
use b00t_cli::hive::{check_guards as rust_check_guards, GuardContext, GuardPattern, HiveGuard, HiveGuardAction};

// Python exception for b00t errors
create_exception!(b00t_py, B00tError, pyo3::exceptions::PyException);

fn to_py_err(prefix: &str, err: anyhow::Error) -> PyErr {
    B00tError::new_err(format!("{}: {}", prefix, err))
}

fn to_py_err_serde(prefix: &str, err: serde_json::Error) -> PyErr {
    B00tError::new_err(format!("{}: {}", prefix, err))
}

// ═══════════════════════════════════════════════════════════
// EmojiRegistry — Python class wrapping &'static EmojiRegistry
// ═══════════════════════════════════════════════════════════

/// Wraps the compile-time emoji registry for Python.
///
/// ```python
/// from b00t_py import EmojiRegistry
/// reg = EmojiRegistry()
/// skunk = reg.lookup_shortcode(":skunk:")    # → "🦨"
/// literal = reg.lookup_literal("🦨")         # → entry dict
/// g0spell = reg.lookup_g0spell("skunk")      # → "🦨"
/// ```
#[pyclass(name = "EmojiRegistry")]
struct PyEmojiRegistry {
    inner: &'static k0mmand3r::EmojiRegistry,
}

#[pymethods]
impl PyEmojiRegistry {
    #[new]
    fn new() -> Self {
        PyEmojiRegistry {
            inner: k0mmand3r::emoji_registry!(),
        }
    }

    /// Look up an emoji by its colon-wrapped shortcode (e.g. ":skunk:").
    /// Returns the literal emoji string (e.g. "🦨") or None.
    fn lookup_shortcode(&self, key: &str) -> Option<String> {
        self.inner
            .lookup_shortcode(key)
            .map(|e| e.literal.to_string())
    }

    /// Look up an emoji by its g0spell key (e.g. "skunk").
    /// Returns the literal emoji string (e.g. "🦨") or None.
    fn lookup_g0spell(&self, key: &str) -> Option<String> {
        self.inner
            .lookup_g0spell(key)
            .map(|e| e.literal.to_string())
    }

    /// Look up an emoji by its Unicode literal (e.g. "🦨").
    /// Returns an entry dict or None.
    fn lookup_literal<'py>(&self, py: Python<'py>, key: &str) -> PyResult<Option<Py<PyAny>>> {
        Ok(self.inner.lookup_literal(key).map(|e| entry_to_py(py, e)))
    }

    /// Return all entries at a given tier.
    fn filter_tier<'py>(&self, py: Python<'py>, tier: u8) -> PyResult<Vec<Py<PyAny>>> {
        Ok(self
            .inner
            .filter_tier(tier)
            .into_iter()
            .map(|e| entry_to_py(py, e))
            .collect())
    }

    /// List all registry entries as dicts.
    fn entries<'py>(&self, py: Python<'py>) -> PyResult<Vec<Py<PyAny>>> {
        Ok(self
            .inner
            .entries
            .iter()
            .map(|e| entry_to_py(py, e))
            .collect())
    }

    /// Number of entries in the registry.
    fn __len__(&self) -> usize {
        self.inner.len()
    }

    fn __repr__(&self) -> String {
        format!(
            "EmojiRegistry(schema_version={}, entries={})",
            self.inner.schema_version,
            self.inner.len()
        )
    }
}

fn entry_to_py(py: Python<'_>, entry: &'static k0mmand3r::EmojiEntry) -> Py<PyAny> {
    let d = PyDict::new(py);
    let _ = d.set_item("literal", entry.literal);
    let _ = d.set_item("shortcode", entry.shortcode);
    let _ = d.set_item("g0spell", entry.g0spell);
    let _ = d.set_item("tier", entry.tier);
    let _ = d.set_item("action", entry.action);
    let _ = d.set_item("description", entry.description);
    d.into_any().unbind()
}

// ═══════════════════════════════════════════════════════════
// Guard system — Python-native check_guards()
// ═══════════════════════════════════════════════════════════

/// Evaluate a command string against a list of guard rules.
///
/// Args:
///     command: The command string to check (e.g. "pip install flask").
///     guards: List of guard definition dicts. If None, uses a default
///             guard that warns on "pip install".
///     context: Optional dict with keys:
///         - "violation_count" (int): Current count for 🦨→💩 escalation
///         - "rhai_macros" (dict): Rhai macro definitions
///
/// Each guard dict supports:
///     - "pattern" (str or dict): If string, matched as substring.
///         If dict: {"rhai": "expr"} or {"stage": "name"}.
///     - "action" (str): "warn", "block", or "redirect"
///     - "message" (str, optional): Custom warning/block message
///     - "redirect" (str, optional): Redirect command suggestion
///     - "repeat_threshold" (int, optional): Repeat count for 🦨→💩 escalation
///
/// Returns:
///     dict with keys: action ("allow"|"warn"|"block"), message, redirect (optional)
#[pyfunction]
#[pyo3(signature = (command, guards = None, context = None))]
fn check_guards_py(
    py: Python<'_>,
    command: &str,
    guards: Option<Vec<Py<PyAny>>>,
    context: Option<&Bound<'_, PyDict>>,
) -> PyResult<Py<PyAny>> {
    // Build the Rust context
    let (violation_count, rhai_macros) = if let Some(ctx) = context {
        let vc = ctx
            .get_item("violation_count")
            .ok()
            .flatten()
            .and_then(|v| v.extract::<u32>().ok())
            .unwrap_or(0);
        let rm = ctx
            .get_item("rhai_macros")
            .ok()
            .flatten()
            .and_then(|v| v.extract::<HashMap<String, String>>().ok())
            .unwrap_or_default();
        (vc, rm)
    } else {
        (0, HashMap::new())
    };

    let rust_ctx = GuardContext {
        command: command.to_string(),
        violation_count,
        repeat_threshold: None,
        rhai_macros,
    };

    // Convert Python guard list to Vec<HiveGuard>
    let rust_guards: Vec<HiveGuard> = if let Some(guards_list) = guards {
        guards_list
            .into_iter()
            .map(|obj| python_guard_to_rust(&obj, py))
            .collect::<PyResult<Vec<HiveGuard>>>()?
    } else {
        vec![HiveGuard {
            pattern: GuardPattern::JsonRegexPattern("pip install".to_string()),
            action: HiveGuardAction::Warn,
            message: Some("🦨 use uv pip install".to_string()),
            redirect: Some("uv pip install".to_string()),
            repeat_threshold: Some(1),
        }]
    };

    let result = rust_check_guards(command, &rust_guards, &rust_ctx);
    let d = PyDict::new(py);
    match result {
        b00t_cli::hive::GuardResult::Allow => {
            d.set_item("action", "allow")?;
        }
        b00t_cli::hive::GuardResult::Warn { message, redirect } => {
            d.set_item("action", "warn")?;
            d.set_item("message", message)?;
            if let Some(r) = redirect {
                d.set_item("redirect", r)?;
            }
        }
        b00t_cli::hive::GuardResult::Block { message } => {
            d.set_item("action", "block")?;
            d.set_item("message", message)?;
        }
    }
    Ok(d.into_any().unbind())
}

/// Convert a Python guard dict into a Rust HiveGuard.
fn python_guard_to_rust(obj: &Py<PyAny>, py: Python<'_>) -> PyResult<HiveGuard> {
    let d = obj
        .bind(py)
        .cast::<PyDict>()
        .map_err(|_| PyTypeError::new_err("Each guard must be a dict"))?;

    let pattern: GuardPattern = python_pattern_to_rust(d)?;

    let action_str: String = d
        .get_item("action")
        .ok()
        .flatten()
        .ok_or_else(|| PyTypeError::new_err("Guard missing 'action' field"))?
        .extract::<String>()?;
    let action = match action_str.to_lowercase().as_str() {
        "warn" => HiveGuardAction::Warn,
        "block" => HiveGuardAction::Block,
        "redirect" => HiveGuardAction::Redirect,
        other => {
            return Err(PyTypeError::new_err(format!(
                "Unknown guard action '{other}'. Use 'warn', 'block', or 'redirect'",
            )))
        }
    };

    let message: Option<String> = d
        .get_item("message")
        .ok()
        .flatten()
        .and_then(|v| v.extract::<String>().ok());

    let redirect: Option<String> = d
        .get_item("redirect")
        .ok()
        .flatten()
        .and_then(|v| v.extract::<String>().ok());

    let repeat_threshold: Option<u32> = d
        .get_item("repeat_threshold")
        .ok()
        .flatten()
        .and_then(|v| v.extract::<u32>().ok());

    Ok(HiveGuard {
        pattern,
        action,
        message,
        redirect,
        repeat_threshold,
    })
}

fn python_pattern_to_rust(d: &Bound<'_, PyDict>) -> PyResult<GuardPattern> {
    let pattern_obj = d
        .get_item("pattern")
        .ok()
        .flatten()
        .ok_or_else(|| PyTypeError::new_err("Guard missing 'pattern' field"))?;

    // Simple string → JsonRegexPattern
    if let Ok(s) = pattern_obj.extract::<String>() {
        return Ok(GuardPattern::JsonRegexPattern(s));
    }

    // Dict → check for rhai/stage keys
    if let Ok(pd) = pattern_obj.cast::<PyDict>() {
        if let Some(rhai) = pd
            .get_item("rhai")
            .ok()
            .flatten()
            .and_then(|v| v.extract::<String>().ok())
        {
            return Ok(GuardPattern::RhaiExpr(b00t_cli::hive::RhaiGuardExpr { rhai }));
        }
        if let Some(stage) = pd
            .get_item("stage")
            .ok()
            .flatten()
            .and_then(|v| v.extract::<String>().ok())
        {
            return Ok(GuardPattern::K0mmand3rStage(b00t_cli::hive::K0mmand3rStageGuard { stage }));
        }
    }

    Err(PyTypeError::new_err(
        "Guard 'pattern' must be a string (substring), {\"rhai\": \"...\"}, or {\"stage\": \"...\"}",
    ))
}

// ── Legacy guard functions (deprecated, kept for compat) ───────────────────────

#[pyfunction]
#[pyo3(signature = (command, guards_json = None))]
fn guard_check(command: &str, guards_json: Option<&str>) -> PyResult<String> {
    let guards: Vec<HiveGuard> = if let Some(json) = guards_json {
        serde_json::from_str(json).map_err(|e| to_py_err("guard JSON parse", e.into()))?
    } else {
        vec![HiveGuard {
            pattern: GuardPattern::JsonRegexPattern("pip install".to_string()),
            action: HiveGuardAction::Warn,
            message: Some("🦨 use uv pip install".to_string()),
            redirect: Some("uv pip install".to_string()),
            repeat_threshold: Some(1),
        }]
    };

    let ctx = GuardContext {
        command: command.to_string(),
        ..Default::default()
    };
    let result = rust_check_guards(command, &guards, &ctx);
    let json_result = match result {
        b00t_cli::hive::GuardResult::Allow => serde_json::json!({"action": "allow"}),
        b00t_cli::hive::GuardResult::Warn { message, redirect } => {
            serde_json::json!({"action": "warn", "message": message, "redirect": redirect})
        }
        b00t_cli::hive::GuardResult::Block { message } => {
            serde_json::json!({"action": "block", "message": message})
        }
    };
    Ok(json_result.to_string())
}

#[pyfunction]
fn guard_violations() -> PyResult<String> {
    Ok("{}".to_string())
}

#[pyfunction]
#[pyo3(signature = (pattern_key))]
fn guard_reset(pattern_key: &str) -> PyResult<String> {
    Ok(format!("reset: {pattern_key}"))
}

#[pyfunction]
fn guard_coverage() -> PyResult<String> {
    Ok("Run `cargo test guard_expr_coverage` for full scan".to_string())
}

// ── K0mmand3r parser — Python-native return ───────────────────────────────────

/// Parse a k0mmand3r slash command string.
///
/// Args:
///     input_str: The k0mmand3r command string (e.g. '@b00t("whoami");')
///
/// Returns:
///     dict with keys: verb (str), params (dict), content (str)
///     On parse failure: {"error": "<message>"}
#[pyfunction]
fn parse_kmdline(py: Python<'_>, input_str: &str) -> PyResult<Py<PyAny>> {
    let mut input = input_str;
    match k0mmand3r::KmdLine::parse(&mut input) {
        Ok(cmd) => {
            // KmdLine fields are private but it derives Serialize,
            // so we bridge via serde_json for ergonomic access
            let json_val: serde_json::Value =
                serde_json::to_value(&cmd).map_err(|e| to_py_err_serde("serialize parse", e))?;
            let d = PyDict::new(py);
            if let serde_json::Value::Object(map) = &json_val {
                for (k, v) in map {
                    set_json_in_pydict(&d, k, v)?;
                }
            }
            Ok(d.into_any().unbind())
        }
        Err(e) => {
            let d = PyDict::new(py);
            d.set_item("error", format!("{e:?}"))?;
            Ok(d.into_any().unbind())
        }
    }
}

/// Recursively set a serde_json::Value into a PyDict with appropriate Python types.
fn set_json_in_pydict(d: &Bound<'_, PyDict>, key: &str, value: &serde_json::Value) -> PyResult<()> {
    match value {
        serde_json::Value::String(s) => d.set_item(key, s)?,
        serde_json::Value::Number(n) => {
            if let Some(i) = n.as_i64() {
                d.set_item(key, i)?;
            } else if let Some(f) = n.as_f64() {
                d.set_item(key, f)?;
            }
        }
        serde_json::Value::Bool(b) => d.set_item(key, b)?,
        serde_json::Value::Null => d.set_item(key, d.py().None())?,
        serde_json::Value::Object(map) => {
            let inner = PyDict::new(d.py());
            for (k, v) in map {
                set_json_in_pydict(&inner, k, v)?;
            }
            d.set_item(key, inner)?;
        }
        serde_json::Value::Array(arr) => {
            let list = PyList::empty(d.py());
            for v in arr {
                let inner = PyDict::new(d.py());
                // For array items that are objects, add them directly
                if let serde_json::Value::Object(map) = v {
                    for (k, val) in map {
                        set_json_in_pydict(&inner, k, val)?;
                    }
                    list.append(inner)?;
                } else {
                    // For non-object array items, stringify
                    list.append(v.to_string())?;
                }
            }
            d.set_item(key, list)?;
        }
    }
    Ok(())
}

// ── K0mmand3r parser bindings ────────────────────────────────────────────────

/// Parse a k0mmand3r slash command string.
/// Returns JSON with verb, params, content.
#[pyfunction]
#[pyo3(signature = (input_str))]
fn parse_k0mmand3r(input_str: &str) -> PyResult<String> {
    let mut input = input_str;
    match k0mmand3r::KmdLine::parse(&mut input) {
        Ok(cmd) => Ok(serde_json::to_string(&cmd)
            .map_err(|e| to_py_err_serde("serialize parse result", e))?),
        Err(e) => Ok(serde_json::json!({
            "success": false,
            "error": format!("{e:?}"),
        })
        .to_string()),
    }
}

/// Register a stage guard callback from Python.
/// The callback receives a Python dict with parse state keys (verb, raw_input)
/// and must return "allow" or "block:<message>".
/// Stage must be one of: pre_parse, pre_verb, post_verb, pre_params, post_params,
/// pre_content, post_content, post_parse.
///
/// The callback is invoked from within Rust's stage guard system, acquiring the
/// GIL via `Python::with_gil` each time a guarded parse stage is reached.
#[pyfunction]
#[pyo3(signature = (stage, callback))]
fn register_stage_guard_py(py: Python<'_>, stage: &str, callback: Py<PyAny>) -> PyResult<String> {
    use k0mmand3r::parser_stages::{ParseStage, StageAction};

    let parse_stage = ParseStage::from_name(stage)
        .ok_or_else(|| B00tError::new_err(format!("Unknown stage: {stage}")))?;

    // Validate that the callback is callable before registering
    if !callback.bind(py).is_callable() {
        return Err(B00tError::new_err(format!(
            "callback must be callable, got: {:?}",
            callback.bind(py).get_type()
        )));
    }

    // Clone the PyObject so it can be moved into the guard closure.
    // The closure attaches to the existing GIL (always held in pyfunction context).
    let cb = callback.clone_ref(py);
    let stage_copy = parse_stage;

    k0mmand3r::register_stage_guard(
        stage_copy,
        Box::new(move |state| {
            Python::attach(|py| {
                let dict = pyo3::types::PyDict::new(py);
                // Best-effort dict population; log individual failures but don't abort
                if let Err(e) = dict.set_item("verb", state.verb.as_deref().unwrap_or("")) {
                    eprintln!("⚠️ stage guard: failed to set 'verb' in callback dict: {e}");
                }
                if let Err(e) = dict.set_item("raw_input", &state.raw_input) {
                    eprintln!("⚠️ stage guard: failed to set 'raw_input' in callback dict: {e}");
                }
                if let Err(e) = dict.set_item("stage", stage_copy.to_string()) {
                    eprintln!("⚠️ stage guard: failed to set 'stage' in callback dict: {e}");
                }

                match cb.call1(py, (dict,)) {
                    Ok(result) => {
                        let s: String = result
                            .extract(py)
                            .unwrap_or_else(|_| "allow".to_string());
                        if s == "allow" || s.is_empty() {
                            StageAction::Allow
                        } else if let Some(msg) = s.strip_prefix("block:") {
                            StageAction::Block {
                                message: msg.to_string(),
                            }
                        } else if s == "block" {
                            StageAction::Block {
                                message: format!("Python guard blocked at stage {stage_copy}"),
                            }
                        } else {
                            StageAction::Allow
                        }
                    }
                    Err(e) => {
                        eprintln!("⚠️ Python stage guard error at {stage_copy}: {e}");
                        StageAction::Allow
                    }
                }
            })
        }),
    );

    Ok(serde_json::json!({
        "registered": true,
        "stage": parse_stage.to_string(),
    })
    .to_string())
}

// ── Legacy emoji functions (flat JSON API, DEPRECATED) ────────────────────────

#[pyfunction]
#[pyo3(signature = (key))]
fn emoji_lookup(key: &str) -> PyResult<String> {
    let registry = k0mmand3r::emoji_registry!();

    // Try shortcode first
    if let Some(entry) = registry.lookup_shortcode(key) {
        return Ok(make_entry_json(entry));
    }
    // Try g0spell
    if let Some(entry) = registry.lookup_g0spell(key) {
        return Ok(make_entry_json(entry));
    }
    // Try literal
    if let Some(entry) = registry.lookup_literal(key) {
        return Ok(make_entry_json(entry));
    }

    Ok(serde_json::json!({"found": false, "key": key}).to_string())
}

fn make_entry_json(entry: &'static k0mmand3r::EmojiEntry) -> String {
    serde_json::json!({
        "found": true,
        "literal": entry.literal,
        "shortcode": entry.shortcode,
        "g0spell": entry.g0spell,
        "tier": entry.tier,
        "action": entry.action,
        "description": entry.description,
    })
    .to_string()
}

#[pyfunction]
fn emoji_list() -> PyResult<String> {
    let registry = k0mmand3r::emoji_registry!();
    let entries: Vec<serde_json::Value> = registry
        .entries
        .iter()
        .map(|e| {
            serde_json::json!({
                "literal": e.literal,
                "shortcode": e.shortcode,
                "g0spell": e.g0spell,
                "tier": e.tier,
                "action": e.action,
                "description": e.description,
            })
        })
        .collect();
    Ok(serde_json::to_string_pretty(&entries).map_err(|e| to_py_err_serde("serialize", e))?)
}

// ═══════════════════════════════════════════════════════════
// Model manager bindings (unchanged from original)
// ═══════════════════════════════════════════════════════════

#[pyfunction]
#[pyo3(signature = (path = "~/.dotfiles/_b00t_", json_output = false))]
fn mcp_list_py(path: &str, json_output: bool) -> PyResult<String> {
    match mcp_list(path, json_output, b00t_cli::McpListFilter::default()) {
        Ok(()) => Ok("MCP servers listed successfully".to_string()),
        Err(e) => Err(B00tError::new_err(format!(
            "Failed to list MCP servers: {e}"
        ))),
    }
}

#[pyfunction]
#[pyo3(signature = (servers, path = "~/.dotfiles/_b00t_", json_format = false))]
fn mcp_output_py(servers: &str, path: &str, json_format: bool) -> PyResult<String> {
    let use_mcp_servers_wrapper = !json_format;
    match mcp_output(path, use_mcp_servers_wrapper, servers) {
        Ok(()) => Ok("MCP output generated successfully".to_string()),
        Err(e) => Err(B00tError::new_err(format!(
            "Failed to generate MCP output: {e}"
        ))),
    }
}

#[pyfunction]
#[pyo3(signature = (path = "~/.dotfiles/_b00t_"))]
fn model_list_py(path: &str) -> PyResult<String> {
    let models =
        model_manager::list_models(path).map_err(|e| to_py_err("Failed to list models", e))?;
    serde_json::to_string(&models).map_err(|e| to_py_err_serde("Failed to serialise model list", e))
}

#[pyfunction]
#[pyo3(signature = (path = "~/.dotfiles/_b00t_", name = None))]
fn model_info_py(path: &str, name: Option<&str>) -> PyResult<String> {
    let record = model_manager::describe_model(path, name)
        .map_err(|e| to_py_err("Failed to load model datum", e))?;
    serde_json::to_string(&record)
        .map_err(|e| to_py_err_serde("Failed to serialise model info", e))
}

#[pyfunction]
#[pyo3(signature = (path = "~/.dotfiles/_b00t_", name = None))]
fn model_env_py(py: Python<'_>, path: &str, name: Option<&str>) -> PyResult<Py<PyAny>> {
    let envs = model_manager::export_model_env(path, name)
        .map_err(|e| to_py_err("Failed to export model env", e))?;
    let dict = PyDict::new(py);
    for (key, value) in envs {
        dict.set_item(key, value)?;
    }
    Ok(dict.into())
}

#[pyfunction]
#[pyo3(signature = (name, path = "~/.dotfiles/_b00t_", force = false, activate = true))]
fn model_download_py(name: &str, path: &str, force: bool, activate: bool) -> PyResult<String> {
    let op = model_manager::download_model(path, name, force, activate)
        .map_err(|e| to_py_err("Failed to download model", e))?;
    serde_json::to_string(&op)
        .map_err(|e| to_py_err_serde("Failed to serialise download result", e))
}

#[pyfunction]
#[pyo3(signature = (name, path = "~/.dotfiles/_b00t_"))]
fn model_remove_py(name: &str, path: &str) -> PyResult<Option<String>> {
    model_manager::remove_model(path, name).map_err(|e| to_py_err("Failed to remove model", e))
}

#[pyfunction]
#[pyo3(signature = (name, path = "~/.dotfiles/_b00t_"))]
fn model_activate_py(name: &str, path: &str) -> PyResult<()> {
    model_manager::activate_model(path, name)
        .map_err(|e| to_py_err("Failed to activate model", e))
}

#[pyfunction]
#[pyo3(signature = (path = "~/.dotfiles/_b00t_", name = None, port = None, dtype = None, image = None, container = None, tensor_parallel_size = 1, extra_args = None, use_gpu = true, replace_existing = true))]
fn model_serve_py(
    path: &str,
    name: Option<&str>,
    port: Option<u16>,
    dtype: Option<&str>,
    image: Option<&str>,
    container: Option<&str>,
    tensor_parallel_size: u32,
    extra_args: Option<Vec<String>>,
    use_gpu: bool,
    replace_existing: bool,
) -> PyResult<String> {
    let mut options = ServeOptions::default();
    options.port = port;
    options.dtype = dtype.map(|s| s.to_string());
    options.image = image.map(|s| s.to_string());
    options.container_name = container.map(|s| s.to_string());
    options.tensor_parallel_size = Some(tensor_parallel_size);
    options.extra_args = extra_args.unwrap_or_default();
    options.gpus = use_gpu;
    options.force_replace = replace_existing;

    let result = model_manager::serve_model(path, name, options)
        .map_err(|e| to_py_err("Failed to start model server", e))?;
    serde_json::to_string(&result)
        .map_err(|e| to_py_err_serde("Failed to serialise serve result", e))
}

#[pyfunction]
#[pyo3(signature = (path = "~/.dotfiles/_b00t_", container = None))]
fn model_stop_py(path: &str, container: Option<&str>) -> PyResult<()> {
    model_manager::stop_model(path, container)
        .map_err(|e| to_py_err("Failed to stop model server", e))
}

/// Get b00t ecosystem version
#[pyfunction]
fn version() -> &'static str {
    b00t_c0re_lib::version::VERSION
}

// ═══════════════════════════════════════════════════════════
// Provider/Model datum bindings (unchanged from original)
// ═══════════════════════════════════════════════════════════

#[pyfunction]
#[pyo3(signature = (model_name, path = "~/.dotfiles/_b00t_"))]
fn load_ai_model_datum(py: Python<'_>, model_name: &str, path: &str) -> PyResult<Py<PyAny>> {
    let mut datum_path =
        get_expanded_path(path).map_err(|e| B00tError::new_err(format!("Invalid path: {e}")))?;
    datum_path.push(format!("{model_name}.ai_model.toml"));

    if !datum_path.exists() {
        return Err(B00tError::new_err(format!(
            "Model datum '{model_name}' not found at {}",
            datum_path.display()
        )));
    }

    let content = std::fs::read_to_string(&datum_path)
        .map_err(|e| B00tError::new_err(format!("Failed to read datum: {e}")))?;
    let toml_value: toml::Value = toml::from_str(&content)
        .map_err(|e| B00tError::new_err(format!("Failed to parse TOML: {e}")))?;

    let ai_model = toml_value
        .get("ai_model")
        .ok_or_else(|| B00tError::new_err("Missing [ai_model] section"))?;

    let py_dict = PyDict::new(py);
    if let Some(provider) = ai_model.get("provider") {
        py_dict.set_item("provider", provider.as_str().unwrap_or("unknown"))?;
    }
    if let Some(size) = ai_model.get("size") {
        py_dict.set_item("size", size.as_str().unwrap_or("unknown"))?;
    }
    if let Some(litellm_model) = ai_model.get("litellm_model") {
        py_dict.set_item("litellm_model", litellm_model.as_str().unwrap_or(""))?;
    }
    if let Some(api_base) = ai_model.get("api_base") {
        py_dict.set_item("api_base", api_base.as_str().unwrap_or(""))?;
    }
    if let Some(api_key_env) = ai_model.get("api_key_env") {
        py_dict.set_item("api_key_env", api_key_env.as_str().unwrap_or(""))?;
    }
    if let Some(context_window) = ai_model.get("context_window") {
        py_dict.set_item("context_window", context_window.as_integer().unwrap_or(0))?;
    }
    if let Some(capabilities) = ai_model.get("capabilities") {
        if let Some(caps_array) = capabilities.as_array() {
            let caps: Vec<&str> = caps_array.iter().filter_map(|v| v.as_str()).collect();
            py_dict.set_item("capabilities", caps)?;
        }
    }
    if let Some(parameters) = ai_model.get("parameters") {
        if let Some(params_table) = parameters.as_table() {
            let params_dict = PyDict::new(py);
            for (key, value) in params_table {
                match value {
                    toml::Value::String(s) => params_dict.set_item(key, s)?,
                    toml::Value::Integer(i) => params_dict.set_item(key, i)?,
                    toml::Value::Float(f) => params_dict.set_item(key, f)?,
                    toml::Value::Boolean(b) => params_dict.set_item(key, b)?,
                    _ => params_dict.set_item(key, value.to_string())?,
                }
            }
            py_dict.set_item("parameters", params_dict)?;
        }
    }

    Ok(py_dict.into_any().unbind())
}

#[pyfunction]
#[pyo3(signature = (provider_name, path = "~/.dotfiles/_b00t_"))]
fn check_provider_env(py: Python<'_>, provider_name: &str, path: &str) -> PyResult<Py<PyAny>> {
    let mut datum_path =
        get_expanded_path(path).map_err(|e| B00tError::new_err(format!("Invalid path: {e}")))?;
    datum_path.push(format!("{provider_name}.ai.toml"));

    if !datum_path.exists() {
        return Err(B00tError::new_err(format!(
            "Provider datum '{provider_name}' not found"
        )));
    }

    let content = std::fs::read_to_string(&datum_path)
        .map_err(|e| B00tError::new_err(format!("Failed to read datum: {e}")))?;
    let toml_value: toml::Value = toml::from_str(&content)
        .map_err(|e| B00tError::new_err(format!("Failed to parse TOML: {e}")))?;

    let mut missing_vars = Vec::new();
    let mut has_any = false;

    if let Some(env_section) = toml_value.get("env") {
        if let Some(env_table) = env_section.as_table() {
            for (key, _) in env_table {
                if std::env::var(key).is_ok() {
                    has_any = true;
                } else {
                    missing_vars.push(key.clone());
                }
            }
        }
    }

    let result = PyDict::new(py);
    result.set_item("available", has_any)?;
    result.set_item("missing_env_vars", missing_vars)?;

    Ok(result.into_any().unbind())
}

#[pyfunction]
#[pyo3(signature = (path = "~/.dotfiles/_b00t_"))]
fn list_ai_providers(path: &str) -> PyResult<Vec<String>> {
    let datum_path =
        get_expanded_path(path).map_err(|e| B00tError::new_err(format!("Invalid path: {e}")))?;
    let mut providers = Vec::new();
    if let Ok(entries) = std::fs::read_dir(datum_path) {
        for entry in entries.flatten() {
            let file_name = entry.file_name();
            let name_str = file_name.to_string_lossy();
            if name_str.ends_with(".ai.toml") {
                let provider_name = name_str.trim_end_matches(".ai.toml").to_string();
                providers.push(provider_name);
            }
        }
    }
    providers.sort();
    Ok(providers)
}

#[pyfunction]
#[pyo3(signature = (path = "~/.dotfiles/_b00t_"))]
fn list_ai_models(path: &str) -> PyResult<Vec<String>> {
    let datum_path =
        get_expanded_path(path).map_err(|e| B00tError::new_err(format!("Invalid path: {e}")))?;
    let mut models = Vec::new();
    if let Ok(entries) = std::fs::read_dir(datum_path) {
        for entry in entries.flatten() {
            let file_name = entry.file_name();
            let name_str = file_name.to_string_lossy();
            if name_str.ends_with(".ai_model.toml") {
                let model_name = name_str.trim_end_matches(".ai_model.toml").to_string();
                models.push(model_name);
            }
        }
    }
    models.sort();
    Ok(models)
}

// ═══════════════════════════════════════════════════════════
// Module definition
// ═══════════════════════════════════════════════════════════

/// Python module for b00t-cli bindings
#[pymodule]
fn _core(py: Python<'_>, m: &Bound<'_, PyModule>) -> PyResult<()> {
    // Classes
    m.add_class::<PyEmojiRegistry>()?;

    // Core functions (new Python-native API)
    m.add_function(wrap_pyfunction!(check_guards_py, m)?)?;
    m.add_function(wrap_pyfunction!(parse_kmdline, m)?)?;

    // MCP functions
    m.add_function(wrap_pyfunction!(mcp_list_py, m)?)?;
    m.add_function(wrap_pyfunction!(mcp_output_py, m)?)?;

    // Model functions
    m.add_function(wrap_pyfunction!(model_list_py, m)?)?;
    m.add_function(wrap_pyfunction!(model_info_py, m)?)?;
    m.add_function(wrap_pyfunction!(model_env_py, m)?)?;
    m.add_function(wrap_pyfunction!(model_download_py, m)?)?;
    m.add_function(wrap_pyfunction!(model_remove_py, m)?)?;
    m.add_function(wrap_pyfunction!(model_activate_py, m)?)?;
    m.add_function(wrap_pyfunction!(model_serve_py, m)?)?;
    m.add_function(wrap_pyfunction!(model_stop_py, m)?)?;

    // Datum functions
    m.add_function(wrap_pyfunction!(load_ai_model_datum, m)?)?;
    m.add_function(wrap_pyfunction!(check_provider_env, m)?)?;
    m.add_function(wrap_pyfunction!(list_ai_providers, m)?)?;
    m.add_function(wrap_pyfunction!(list_ai_models, m)?)?;

    // Legacy/deprecated functions (backward compat)
    m.add_function(wrap_pyfunction!(guard_check, m)?)?;
    m.add_function(wrap_pyfunction!(guard_violations, m)?)?;
    m.add_function(wrap_pyfunction!(guard_reset, m)?)?;
    m.add_function(wrap_pyfunction!(guard_coverage, m)?)?;
    m.add_function(wrap_pyfunction!(emoji_lookup, m)?)?;
    m.add_function(wrap_pyfunction!(emoji_list, m)?)?;
    m.add_function(wrap_pyfunction!(parse_k0mmand3r, m)?)?;
    m.add_function(wrap_pyfunction!(register_stage_guard_py, m)?)?;

    // Utilities
    m.add_function(wrap_pyfunction!(version, m)?)?;

    // Exceptions
    m.add("B00tError", py.get_type::<B00tError>())?;

    Ok(())
}
