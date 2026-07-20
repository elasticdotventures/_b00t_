//! BPipelineCommand — expose pipeline lifecycle through b00t-mcp for agent use.
//!
//! Provides a single MCP tool `b00t_pipeline` that dispatches to b00t-cli pipeline
//! subcommands based on the `action` parameter (GH #717 / #736).
//!
//! # Actions
//! - `list`     — List discovered pipeline datums
//! - `run`      — Execute a pipeline by name with optional stage selection
//! - `validate` — Static validation of pipeline DAG topology and resource specs
//! - `inspect`  — Show pipeline metadata, stage count, and topology
//! - `cost`     — Query or forecast pipeline resource costs
//!
//! # Example MCP call
//! ```json
//! { "action": "run", "pipeline": "video-pipeline", "params": "{\"stages\":\"decode,encode\"}" }
//! ```

use crate::clap_reflection::{McpExecutor, McpReflection};
use clap::Parser;
use serde_json::{Map, Value};
use std::collections::HashMap;

/// Manage pipeline lifecycle — create, validate, execute, inspect
///
/// Dispatches to b00t-cli pipeline subcommands based on `action`:
/// - `list` lists all pipeline datums
/// - `run <pipeline>` executes a pipeline with optional stage selection via `params`
/// - `validate <pipeline>` runs static DAG validation
/// - `inspect <pipeline>` shows pipeline metadata and stage topology
/// - `cost <pipeline>` queries cost attribution and forecast
#[derive(Parser, Clone)]
pub struct BPipelineCommand {
    #[arg(help = "Action: list, run, validate, inspect, cost")]
    pub action: String,

    #[arg(help = "Pipeline datum name (required for run, validate, inspect, cost)")]
    pub pipeline: Option<String>,

    #[arg(
        long,
        help = "Additional parameters as JSON object, e.g. {\"stages\":\"ingest,encode\"}"
    )]
    pub params: Option<String>,
}

impl McpReflection for BPipelineCommand {
    fn mcp_tool_name() -> String {
        "b00t_pipeline".to_string()
    }

    fn command_path() -> Vec<String> {
        vec!["pipeline".to_string()]
    }

    /// Custom schema to present action as an enum-like choice with contextual requirements.
    /// Overrides the default derive to provide richer descriptions per action.
    fn generate_json_schema() -> Map<String, Value> {
        let mut schema = Map::new();
        let mut properties = Map::new();

        schema.insert("type".to_string(), Value::String("object".to_string()));

        // action: required string (docs describe valid values)
        let mut action_prop = Map::new();
        action_prop.insert("type".to_string(), Value::String("string".to_string()));
        action_prop.insert(
            "description".to_string(),
            Value::String(
                "Action: list (no pipeline required), run, validate, inspect, cost".to_string(),
            ),
        );
        properties.insert("action".to_string(), Value::Object(action_prop));

        // pipeline: optional string
        let mut pipeline_prop = Map::new();
        pipeline_prop.insert("type".to_string(), Value::String("string".to_string()));
        pipeline_prop.insert(
            "description".to_string(),
            Value::String(
                "Pipeline datum name. Required for: run, validate, inspect, cost".to_string(),
            ),
        );
        properties.insert("pipeline".to_string(), Value::Object(pipeline_prop));

        // params: optional JSON string for extra config
        let mut params_prop = Map::new();
        params_prop.insert("type".to_string(), Value::String("string".to_string()));
        params_prop.insert(
            "description".to_string(),
            Value::String(
                "Additional parameters as JSON object. For run: {\"stages\":\"stage1,stage2\"}"
                    .to_string(),
            ),
        );
        properties.insert("params".to_string(), Value::Object(params_prop));

        schema.insert("properties".to_string(), Value::Object(properties));
        schema.insert(
            "required".to_string(),
            Value::Array(vec![Value::String("action".to_string())]),
        );

        schema
    }
}

impl McpExecutor for BPipelineCommand {
    fn execute_mcp_call(params: &HashMap<String, Value>) -> anyhow::Result<String> {
        let action = params
            .get("action")
            .and_then(|v| v.as_str())
            .ok_or_else(|| {
                anyhow::anyhow!(
                    "b00t_pipeline requires 'action' field (list|run|validate|inspect|cost)"
                )
            })?;

        match action {
            "list" => exec_pipeline_list(),
            "run" => {
                let pipeline =
                    params
                        .get("pipeline")
                        .and_then(|v| v.as_str())
                        .ok_or_else(|| {
                            anyhow::anyhow!("b00t_pipeline run requires 'pipeline' field")
                        })?;
                let stages = params
                    .get("params")
                    .and_then(|v| v.as_str())
                    .and_then(|s| serde_json::from_str::<HashMap<String, String>>(s).ok())
                    .and_then(|m| m.get("stages").cloned())
                    .unwrap_or_default();
                exec_pipeline_run(pipeline, &stages)
            }
            "validate" => {
                let pipeline =
                    params
                        .get("pipeline")
                        .and_then(|v| v.as_str())
                        .ok_or_else(|| {
                            anyhow::anyhow!("b00t_pipeline validate requires 'pipeline' field")
                        })?;
                exec_pipeline_validate(pipeline)
            }
            "inspect" => {
                let pipeline = params.get("pipeline").and_then(|v| v.as_str());
                exec_pipeline_inspect(pipeline)
            }
            "cost" => {
                let pipeline =
                    params
                        .get("pipeline")
                        .and_then(|v| v.as_str())
                        .ok_or_else(|| {
                            anyhow::anyhow!("b00t_pipeline cost requires 'pipeline' field")
                        })?;
                exec_pipeline_cost(pipeline)
            }
            other => anyhow::bail!(
                "Unknown pipeline action: '{other}'. Valid actions: list, run, validate, inspect, cost"
            ),
        }
    }
}

// ── Dispatch helpers ──────────────────────────────────────────────────────────

/// Run `b00t pipeline list` and return the output.
fn exec_pipeline_list() -> anyhow::Result<String> {
    let output = std::process::Command::new("b00t-cli")
        .args(["pipeline", "list"])
        .output()
        .map_err(|e| anyhow::anyhow!("Failed to execute b00t-cli: {e}"))?;
    if output.status.success() {
        Ok(String::from_utf8_lossy(&output.stdout).to_string())
    } else {
        anyhow::bail!("{}", String::from_utf8_lossy(&output.stderr))
    }
}

/// Run `b00t pipeline run <name> [--stage <s> ...]` and return the output.
fn exec_pipeline_run(name: &str, stages: &str) -> anyhow::Result<String> {
    let mut args: Vec<&str> = vec!["pipeline", "run", name];
    if !stages.is_empty() {
        for stage in stages.split(',') {
            let trimmed = stage.trim();
            if !trimmed.is_empty() {
                args.push("--stage");
                args.push(trimmed);
            }
        }
    }
    let output = std::process::Command::new("b00t-cli")
        .args(&args)
        .output()
        .map_err(|e| anyhow::anyhow!("Failed to execute b00t-cli: {e}"))?;
    if output.status.success() {
        Ok(String::from_utf8_lossy(&output.stdout).to_string())
    } else {
        anyhow::bail!("{}", String::from_utf8_lossy(&output.stderr))
    }
}

/// Run `b00t pipeline validate <name>` and return the validation report.
fn exec_pipeline_validate(name: &str) -> anyhow::Result<String> {
    let output = std::process::Command::new("b00t-cli")
        .args(["pipeline", "validate", name])
        .output()
        .map_err(|e| anyhow::anyhow!("Failed to execute b00t-cli: {e}"))?;
    if output.status.success() {
        Ok(String::from_utf8_lossy(&output.stdout).to_string())
    } else {
        // Validation failures exit 1 but carry the structured report on stderr
        anyhow::bail!("{}", String::from_utf8_lossy(&output.stderr))
    }
}

/// Show pipeline metadata. Calls `b00t pipeline list` and filters for the named
/// pipeline if `name` is Some.
fn exec_pipeline_inspect(name: Option<&str>) -> anyhow::Result<String> {
    let output = std::process::Command::new("b00t-cli")
        .args(["pipeline", "list"])
        .output()
        .map_err(|e| anyhow::anyhow!("Failed to execute b00t-cli: {e}"))?;
    if !output.status.success() {
        anyhow::bail!("{}", String::from_utf8_lossy(&output.stderr));
    }
    let listing = String::from_utf8_lossy(&output.stdout).to_string();
    match name {
        Some(pname) => {
            let filtered: Vec<&str> = listing
                .lines()
                .filter(|line| line.contains(pname))
                .collect();
            if filtered.is_empty() {
                Ok(format!(
                    "Pipeline '{pname}' not found.\nDiscovered pipelines:\n{listing}"
                ))
            } else {
                Ok(filtered.join("\n"))
            }
        }
        None => Ok(listing),
    }
}

/// Run `b00t pipeline cost <name>` and return the cost report.
fn exec_pipeline_cost(name: &str) -> anyhow::Result<String> {
    let output = std::process::Command::new("b00t-cli")
        .args(["pipeline", "cost", name])
        .output()
        .map_err(|e| anyhow::anyhow!("Failed to execute b00t-cli: {e}"))?;
    if output.status.success() {
        Ok(String::from_utf8_lossy(&output.stdout).to_string())
    } else {
        anyhow::bail!("{}", String::from_utf8_lossy(&output.stderr))
    }
}
