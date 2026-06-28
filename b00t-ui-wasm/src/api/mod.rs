//! HTTP client for b00t-admin REST API.
//!
//! Base URL defaults to `http://localhost:31337`.
//! All functions return `serde_json::Value` for flexibility — typed
//! deserialisation can be added later via b00t-c0re-lib types.

use serde_json::Value;

const API_BASE: &str = "http://localhost:31337";

// ---------------------------------------------------------------------------
// Internal helpers
// ---------------------------------------------------------------------------

async fn fetch(path: &str) -> Result<Value, String> {
    let url = format!("{API_BASE}{path}");
    let resp = reqwest::get(&url)
        .await
        .map_err(|e| format!("HTTP request failed: {e}"))?;
    resp.json::<Value>()
        .await
        .map_err(|e| format!("JSON deserialisation failed: {e}"))
}

// ---------------------------------------------------------------------------
// Public API surface
// ---------------------------------------------------------------------------

/// GET /api/admin/pipeline — pipeline execution stats
pub async fn get_pipeline() -> Result<Value, String> {
    fetch("/api/admin/pipeline").await
}

/// GET /api/admin/processes — running process list
#[allow(dead_code)]
pub async fn get_processes() -> Result<Value, String> {
    fetch("/api/admin/processes").await
}

/// GET /api/admin/viz/{viz_type} — graph data for visualisations
pub async fn get_viz(viz_type: &str) -> Result<Value, String> {
    fetch(&format!("/api/admin/viz/{viz_type}")).await
}

/// GET /api/admin/types — list of available type names
pub async fn get_types() -> Result<Value, String> {
    fetch("/api/admin/types").await
}

/// GET /api/admin/types/{name} — detailed type definition
pub async fn get_type_detail(name: &str) -> Result<Value, String> {
    fetch(&format!("/api/admin/types/{name}")).await
}

/// GET /api/admin/simulate/tick — advance simulation one tick
pub async fn sim_tick() -> Result<Value, String> {
    fetch("/api/admin/simulate/tick").await
}

/// GET /api/admin/simulate/state — current simulation state
pub async fn sim_state() -> Result<Value, String> {
    fetch("/api/admin/simulate/state").await
}

/// GET /api/admin/simulate/rollback — roll back simulation state
pub async fn sim_rollback() -> Result<Value, String> {
    fetch("/api/admin/simulate/rollback").await
}

/// GET /api/admin/health — server health check
#[allow(dead_code)]
pub async fn get_health() -> Result<Value, String> {
    fetch("/api/admin/health").await
}
