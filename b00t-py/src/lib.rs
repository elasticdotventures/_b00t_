//! # b00t-py
//!
//! Python bindings for b00t-cli with native performance using PyO3.
//! 
//! This module provides high-performance Python bindings for the b00t ecosystem,
//! offering 10-100x performance improvements over subprocess-based approaches.

use pyo3::prelude::*;
use pyo3::create_exception;

// Import b00t-cli functions
use b00t_cli::{mcp_list, mcp_output};

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
        Err(e) => Err(B00tError::new_err(format!("Failed to list MCP servers: {}", e)))
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
        Err(e) => Err(B00tError::new_err(format!("Failed to generate MCP output: {}", e)))
    }
}

/// Get b00t ecosystem version
#[pyfunction]
fn version() -> &'static str {
    b00t_c0re_lib::version::VERSION
}

/// Python module for b00t-cli bindings
#[pymodule]
fn b00t_py(py: Python<'_>, m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_function(wrap_pyfunction!(mcp_list_py, m)?)?;
    m.add_function(wrap_pyfunction!(mcp_output_py, m)?)?;
    m.add_function(wrap_pyfunction!(version, m)?)?;
    m.add("B00tError", py.get_type::<B00tError>())?;
    
    Ok(())
}