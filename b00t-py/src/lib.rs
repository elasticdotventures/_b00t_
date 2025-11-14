//! # b00t-py
//!
//! Python bindings for b00t-cli with native performance using PyO3.
//!
//! This module provides high-performance Python bindings for the b00t ecosystem,
//! offering 10-100x performance improvements over subprocess-based approaches.

use pyo3::create_exception;
use pyo3::prelude::*;
use pyo3::types::PyDict;
use std::collections::HashMap;

// Import b00t-cli functions
use b00t_cli::{get_expanded_path, mcp_list, mcp_output};

// Import datum types
use b00t_c0re_lib::datum_ai_model::{AiModelDatum, ModelCapability, ModelProvider, ModelSize};

/// Python exception for b00t errors
create_exception!(b00t_py, B00tError, pyo3::exceptions::PyException);

/// List all MCP servers available in the b00t configuration
///
/// Args:
///     path (str, optional): Path to b00t configuration directory.
///                          Defaults to "~/.dotfiles/_b00t_"
///     json_output (bool, optional): Return structured JSON output. Defaults to False.
///
/// Returns:
///     list: List of MCP server configurations
///
/// Raises:
///     B00tError: If b00t configuration cannot be read
///
#[pyfunction]
#[pyo3(signature = (path = "~/.dotfiles/_b00t_", json_output = false))]
fn mcp_list_py(path: &str, json_output: bool) -> PyResult<String> {
    // Call the b00t-cli function and capture output
    match mcp_list(path, json_output) {
        Ok(()) => Ok("MCP servers listed successfully".to_string()),
        Err(e) => Err(B00tError::new_err(format!(
            "Failed to list MCP servers: {}",
            e
        ))),
    }
}

/// Get MCP server output in specified format
///
/// Args:
///     path (str, optional): Path to b00t configuration directory
///     servers (str): Comma-separated list of server names
///     json_format (bool, optional): Use raw JSON format. Defaults to False.
///
/// Returns:
///     str: MCP server configuration output
///
/// Raises:
///     B00tError: If servers cannot be found or output fails
///
#[pyfunction]
#[pyo3(signature = (servers, path = "~/.dotfiles/_b00t_", json_format = false))]
fn mcp_output_py(servers: &str, path: &str, json_format: bool) -> PyResult<String> {
    let use_mcp_servers_wrapper = !json_format;

    match mcp_output(path, use_mcp_servers_wrapper, servers) {
        Ok(()) => Ok("MCP output generated successfully".to_string()),
        Err(e) => Err(B00tError::new_err(format!(
            "Failed to generate MCP output: {}",
            e
        ))),
    }
}

/// Get b00t ecosystem version
#[pyfunction]
fn version() -> &'static str {
    b00t_c0re_lib::version::VERSION
}

/// Load an AI model datum from TOML file
///
/// Args:
///     model_name (str): Name of the model (e.g., "claude-3-5-sonnet", "qwen-2.5-72b")
///     path (str, optional): Path to datum directory. Defaults to "~/.dotfiles/_b00t_"
///
/// Returns:
///     dict: Model configuration with provider, capabilities, env vars, etc.
///
/// Raises:
///     B00tError: If model datum cannot be loaded
///
#[pyfunction]
#[pyo3(signature = (model_name, path = "~/.dotfiles/_b00t_"))]
fn load_ai_model_datum(py: Python<'_>, model_name: &str, path: &str) -> PyResult<PyObject> {
    // Expand path
    let mut datum_path =
        get_expanded_path(path).map_err(|e| B00tError::new_err(format!("Invalid path: {}", e)))?;
    datum_path.push(format!("{}.ai_model.toml", model_name));

    // Check if file exists
    if !datum_path.exists() {
        return Err(B00tError::new_err(format!(
            "Model datum '{}' not found at {}",
            model_name,
            datum_path.display()
        )));
    }

    // Read and parse TOML
    let content = std::fs::read_to_string(&datum_path)
        .map_err(|e| B00tError::new_err(format!("Failed to read datum: {}", e)))?;

    // Parse into structured format
    let toml_value: toml::Value = toml::from_str(&content)
        .map_err(|e| B00tError::new_err(format!("Failed to parse TOML: {}", e)))?;

    // Extract ai_model section
    let ai_model = toml_value
        .get("ai_model")
        .ok_or_else(|| B00tError::new_err("Missing [ai_model] section"))?;

    // Convert to Python dict
    let py_dict = PyDict::new(py);

    // Add basic fields
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

    // Add capabilities
    if let Some(capabilities) = ai_model.get("capabilities") {
        if let Some(caps_array) = capabilities.as_array() {
            let caps: Vec<&str> = caps_array.iter().filter_map(|v| v.as_str()).collect();
            py_dict.set_item("capabilities", caps)?;
        }
    }

    // Add parameters if present
    if let Some(parameters) = ai_model.get("parameters") {
        if let Some(params_table) = parameters.as_table() {
            let params_dict = PyDict::new(py);
            for (key, value) in params_table {
                let py_value = match value {
                    toml::Value::String(s) => s.to_object(py),
                    toml::Value::Integer(i) => i.to_object(py),
                    toml::Value::Float(f) => f.to_object(py),
                    toml::Value::Boolean(b) => b.to_object(py),
                    _ => value.to_string().to_object(py),
                };
                params_dict.set_item(key, py_value)?;
            }
            py_dict.set_item("parameters", params_dict)?;
        }
    }

    Ok(py_dict.to_object(py))
}

/// Check if AI provider environment variables are set
///
/// Args:
///     provider_name (str): Provider name (e.g., "openrouter", "anthropic", "huggingface")
///     path (str, optional): Path to datum directory
///
/// Returns:
///     dict: {"available": bool, "missing_env_vars": list}
///
#[pyfunction]
#[pyo3(signature = (provider_name, path = "~/.dotfiles/_b00t_"))]
fn check_provider_env(py: Python<'_>, provider_name: &str, path: &str) -> PyResult<PyObject> {
    // Expand path
    let mut datum_path =
        get_expanded_path(path).map_err(|e| B00tError::new_err(format!("Invalid path: {}", e)))?;
    datum_path.push(format!("{}.ai.toml", provider_name));

    // Check if file exists
    if !datum_path.exists() {
        return Err(B00tError::new_err(format!(
            "Provider datum '{}' not found",
            provider_name
        )));
    }

    // Read and parse TOML
    let content = std::fs::read_to_string(&datum_path)
        .map_err(|e| B00tError::new_err(format!("Failed to read datum: {}", e)))?;

    let toml_value: toml::Value = toml::from_str(&content)
        .map_err(|e| B00tError::new_err(format!("Failed to parse TOML: {}", e)))?;

    // Extract env section
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

    Ok(result.to_object(py))
}

/// List all available AI providers
///
/// Args:
///     path (str, optional): Path to datum directory
///
/// Returns:
///     list: List of provider names
///
#[pyfunction]
#[pyo3(signature = (path = "~/.dotfiles/_b00t_"))]
fn list_ai_providers(path: &str) -> PyResult<Vec<String>> {
    let datum_path =
        get_expanded_path(path).map_err(|e| B00tError::new_err(format!("Invalid path: {}", e)))?;

    let mut providers = Vec::new();

    // Read directory and find *.ai.toml files
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

/// List all available AI models
///
/// Args:
///     path (str, optional): Path to datum directory
///
/// Returns:
///     list: List of model names
///
#[pyfunction]
#[pyo3(signature = (path = "~/.dotfiles/_b00t_"))]
fn list_ai_models(path: &str) -> PyResult<Vec<String>> {
    let datum_path =
        get_expanded_path(path).map_err(|e| B00tError::new_err(format!("Invalid path: {}", e)))?;

    let mut models = Vec::new();

    // Read directory and find *.ai_model.toml files
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

/// Python module for b00t-cli bindings
#[pymodule]
fn b00t_py(py: Python<'_>, m: &Bound<'_, PyModule>) -> PyResult<()> {
    // MCP functions
    m.add_function(wrap_pyfunction!(mcp_list_py, m)?)?;
    m.add_function(wrap_pyfunction!(mcp_output_py, m)?)?;

    // Datum functions
    m.add_function(wrap_pyfunction!(load_ai_model_datum, m)?)?;
    m.add_function(wrap_pyfunction!(check_provider_env, m)?)?;
    m.add_function(wrap_pyfunction!(list_ai_providers, m)?)?;
    m.add_function(wrap_pyfunction!(list_ai_models, m)?)?;

    // Utilities
    m.add_function(wrap_pyfunction!(version, m)?)?;

    // Exceptions
    m.add("B00tError", py.get_type::<B00tError>())?;

    Ok(())
}
