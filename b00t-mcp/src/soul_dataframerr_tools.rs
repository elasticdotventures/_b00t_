//! MCP tool bindings for soul DataFramerr (K4).
//! Each tool is a thin dispatcher to `b00t-cli soul <subcmd>`.
//! Tool names mirror PRD-DATAFRAMERR.prd.tomllmd § mcp_tools.

use crate::clap_reflection::{McpExecutor, McpReflection};
use anyhow::Result;
use clap::Parser;
use serde_json::Value;
use std::collections::HashMap;

// ── helpers ───────────────────────────────────────────────────────────────────

fn run_soul(args: &[&str]) -> Result<String> {
    let mut cmd = std::process::Command::new("b00t-cli");
    cmd.arg("soul");
    cmd.args(args);
    let out = cmd
        .output()
        .map_err(|e| anyhow::anyhow!("b00t-cli soul: {e}"))?;
    let stdout = String::from_utf8_lossy(&out.stdout).to_string();
    let stderr = String::from_utf8_lossy(&out.stderr).to_string();
    if !out.status.success() {
        let msg = if stderr.is_empty() {
            stdout.clone()
        } else {
            stderr
        };
        return Err(anyhow::anyhow!(
            "b00t-cli soul {}: {}",
            args.first().unwrap_or(&""),
            msg.trim()
        ));
    }
    Ok(stdout)
}

fn str_param<'a>(params: &'a HashMap<String, Value>, key: &str) -> Result<&'a str> {
    params
        .get(key)
        .and_then(|v| v.as_str())
        .ok_or_else(|| anyhow::anyhow!("required param '{key}' missing or not a string"))
}

// ── soul_table_create ─────────────────────────────────────────────────────────

/// Create a typed DataFramerr table in the active soul (SOUL.tomllmd).
/// Columns are "name:type" or "name:type?" shorthand.
/// Types: text | int | float | cake | bool | timestamp | token | json
#[derive(Parser, Clone)]
pub struct SoulTableCreateCommand {
    #[arg(help = "Table name")]
    pub name: String,
    #[arg(help = "Column defs: 'name:type' or 'name:type?' (nullable)")]
    pub columns: Vec<String>,
    #[arg(long, help = "#1102 shard scope 'kind:id', e.g. 'agent:pi'. Omit for the legacy/default shard.")]
    pub scope: Option<String>,
}

impl McpReflection for SoulTableCreateCommand {
    fn mcp_tool_name() -> String {
        "soul_table_create".to_string()
    }
    fn command_path() -> Vec<String> {
        vec!["soul".into(), "table-create".into()]
    }
}

impl McpExecutor for SoulTableCreateCommand {
    fn execute_mcp_call(params: &HashMap<String, Value>) -> Result<String> {
        let name = str_param(params, "name")?;
        let columns: Vec<String> = params
            .get("columns")
            .and_then(|v| v.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|v| v.as_str().map(String::from))
                    .collect()
            })
            .unwrap_or_default();
        let mut args: Vec<&str> = vec!["table-create", name];
        let col_refs: Vec<&str> = columns.iter().map(|s| s.as_str()).collect();
        args.extend(col_refs.iter());
        let scope_owned;
        let scope: Option<&str> = params.get("scope").and_then(|v| v.as_str());
        if let Some(s) = scope {
            scope_owned = s.to_string();
            args.extend(["--scope", &scope_owned]);
        }
        run_soul(&args)
    }
}

// ── soul_table_list ───────────────────────────────────────────────────────────

#[derive(Parser, Clone)]
pub struct SoulTableListCommand {
    #[arg(long, help = "#1102 shard scope 'kind:id', e.g. 'agent:pi'. Omit for the legacy/default shard.")]
    pub scope: Option<String>,
}

impl McpReflection for SoulTableListCommand {
    fn mcp_tool_name() -> String {
        "soul_table_list".to_string()
    }
    fn command_path() -> Vec<String> {
        vec!["soul".into(), "table-list".into()]
    }
}

impl McpExecutor for SoulTableListCommand {
    fn execute_mcp_call(params: &HashMap<String, Value>) -> Result<String> {
        let mut args: Vec<&str> = vec!["table-list"];
        let scope_owned;
        let scope: Option<&str> = params.get("scope").and_then(|v| v.as_str());
        if let Some(s) = scope {
            scope_owned = s.to_string();
            args.extend(["--scope", &scope_owned]);
        }
        run_soul(&args)
    }
}

// ── soul_row_insert ───────────────────────────────────────────────────────────

/// Append a row to a DataFramerr table.
/// fields: object of key→value pairs (auto-typed: bool/int/float/text).
#[derive(Parser, Clone)]
pub struct SoulRowInsertCommand {
    #[arg(help = "Table name")]
    pub table: String,
    #[arg(help = "Field key=value pairs")]
    pub fields: Vec<String>,
    #[arg(long, help = "#1102 shard scope 'kind:id', e.g. 'agent:pi'. Omit for the legacy/default shard.")]
    pub scope: Option<String>,
}

impl McpReflection for SoulRowInsertCommand {
    fn mcp_tool_name() -> String {
        "soul_row_insert".to_string()
    }
    fn command_path() -> Vec<String> {
        vec!["soul".into(), "frame-insert".into()]
    }
}

impl McpExecutor for SoulRowInsertCommand {
    fn execute_mcp_call(params: &HashMap<String, Value>) -> Result<String> {
        let table = str_param(params, "table")?;
        let fields_obj = params
            .get("fields")
            .and_then(|v| v.as_object())
            .ok_or_else(|| anyhow::anyhow!("soul_row_insert requires fields: object"))?;
        // Build "key=value" pairs
        let pairs: Vec<String> = fields_obj
            .iter()
            .map(|(k, v)| {
                let val = match v {
                    Value::Bool(b) => b.to_string(),
                    Value::Number(n) => n.to_string(),
                    Value::String(s) => s.clone(),
                    other => other.to_string(),
                };
                format!("{k}={val}")
            })
            .collect();
        let mut args: Vec<&str> = vec!["frame-insert", table];
        let pair_refs: Vec<&str> = pairs.iter().map(|s| s.as_str()).collect();
        args.extend(pair_refs.iter());
        let scope_owned;
        let scope: Option<&str> = params.get("scope").and_then(|v| v.as_str());
        if let Some(s) = scope {
            scope_owned = s.to_string();
            args.extend(["--scope", &scope_owned]);
        }
        run_soul(&args)
    }
}

// ── soul_row_query ────────────────────────────────────────────────────────────

/// Dump rows from a DataFramerr table (tabular text). last: show only last N rows.
#[derive(Parser, Clone)]
pub struct SoulRowQueryCommand {
    #[arg(help = "Table name")]
    pub table: String,
    #[arg(long, help = "Return only last N rows")]
    pub last: Option<usize>,
    #[arg(long, help = "#1102 shard scope 'kind:id', e.g. 'agent:pi'. Omit for the legacy/default shard.")]
    pub scope: Option<String>,
}

impl McpReflection for SoulRowQueryCommand {
    fn mcp_tool_name() -> String {
        "soul_row_query".to_string()
    }
    fn command_path() -> Vec<String> {
        vec!["soul".into(), "frame-dump".into()]
    }
}

impl McpExecutor for SoulRowQueryCommand {
    fn execute_mcp_call(params: &HashMap<String, Value>) -> Result<String> {
        let table = str_param(params, "table")?;
        let mut args = vec!["frame-dump", table];
        let last_str;
        if let Some(n) = params.get("last").and_then(|v| v.as_u64()) {
            last_str = n.to_string();
            args.extend(["--last", &last_str]);
        }
        let scope_owned;
        let scope: Option<&str> = params.get("scope").and_then(|v| v.as_str());
        if let Some(s) = scope {
            scope_owned = s.to_string();
            args.extend(["--scope", &scope_owned]);
        }
        run_soul(&args)
    }
}

// ── soul_cursor_create ────────────────────────────────────────────────────────

#[derive(Parser, Clone)]
pub struct SoulCursorCreateCommand {
    pub name: String,
    pub table: String,
    #[arg(long, help = "#1102 shard scope 'kind:id', e.g. 'agent:pi'. Omit for the legacy/default shard.")]
    pub scope: Option<String>,
}

impl McpReflection for SoulCursorCreateCommand {
    fn mcp_tool_name() -> String {
        "soul_cursor_create".to_string()
    }
    fn command_path() -> Vec<String> {
        vec!["soul".into(), "cursor-create".into()]
    }
}

impl McpExecutor for SoulCursorCreateCommand {
    fn execute_mcp_call(params: &HashMap<String, Value>) -> Result<String> {
        let name = str_param(params, "name")?;
        let table = str_param(params, "table")?;
        let mut args: Vec<&str> = vec!["cursor-create", name, table];
        let scope_owned;
        let scope: Option<&str> = params.get("scope").and_then(|v| v.as_str());
        if let Some(s) = scope {
            scope_owned = s.to_string();
            args.extend(["--scope", &scope_owned]);
        }
        run_soul(&args)
    }
}

// ── soul_cursor_next ──────────────────────────────────────────────────────────

/// Advance a cursor and return the next unconsumed row.
/// Returns empty string when EOF (cursor is at end of table).
#[derive(Parser, Clone)]
pub struct SoulCursorNextCommand {
    pub name: String,
    #[arg(long, help = "#1102 shard scope 'kind:id', e.g. 'agent:pi'. Omit for the legacy/default shard.")]
    pub scope: Option<String>,
}

impl McpReflection for SoulCursorNextCommand {
    fn mcp_tool_name() -> String {
        "soul_cursor_next".to_string()
    }
    fn command_path() -> Vec<String> {
        vec!["soul".into(), "cursor-next".into()]
    }
}

impl McpExecutor for SoulCursorNextCommand {
    fn execute_mcp_call(params: &HashMap<String, Value>) -> Result<String> {
        let name = str_param(params, "name")?;
        let scope: Option<&str> = params.get("scope").and_then(|v| v.as_str());
        // cursor-next exits 1 at EOF — treat as non-error (return sentinel)
        let mut cmd = std::process::Command::new("b00t-cli");
        cmd.args(["soul", "cursor-next", name]);
        if let Some(s) = scope {
            cmd.args(["--scope", s]);
        }
        let out = cmd.output().map_err(|e| anyhow::anyhow!("b00t-cli: {e}"))?;
        let stdout = String::from_utf8_lossy(&out.stdout).to_string();
        if out.status.code() == Some(1) {
            return Ok("EOF".to_string());
        }
        if !out.status.success() {
            return Err(anyhow::anyhow!("cursor-next: {}", stdout.trim()));
        }
        Ok(stdout)
    }
}

// ── soul_cursor_reset ─────────────────────────────────────────────────────────

#[derive(Parser, Clone)]
pub struct SoulCursorResetCommand {
    pub name: String,
    #[arg(long, help = "#1102 shard scope 'kind:id', e.g. 'agent:pi'. Omit for the legacy/default shard.")]
    pub scope: Option<String>,
}

impl McpReflection for SoulCursorResetCommand {
    fn mcp_tool_name() -> String {
        "soul_cursor_reset".to_string()
    }
    fn command_path() -> Vec<String> {
        vec!["soul".into(), "cursor-reset".into()]
    }
}

impl McpExecutor for SoulCursorResetCommand {
    fn execute_mcp_call(params: &HashMap<String, Value>) -> Result<String> {
        let name = str_param(params, "name")?;
        let mut args: Vec<&str> = vec!["cursor-reset", name];
        let scope_owned;
        let scope: Option<&str> = params.get("scope").and_then(|v| v.as_str());
        if let Some(s) = scope {
            scope_owned = s.to_string();
            args.extend(["--scope", &scope_owned]);
        }
        run_soul(&args)
    }
}

// ── soul_alarm_set ────────────────────────────────────────────────────────────

#[derive(Parser, Clone)]
pub struct SoulAlarmSetCommand {
    pub name: String,
    pub table: String,
    pub column: String,
    pub condition: String,
    #[arg(long, default_value = "sum")]
    pub aggregate: String,
    #[arg(long)]
    pub emit: String,
    #[arg(long, help = "#1102 shard scope 'kind:id', e.g. 'agent:pi'. Omit for the legacy/default shard.")]
    pub scope: Option<String>,
}

impl McpReflection for SoulAlarmSetCommand {
    fn mcp_tool_name() -> String {
        "soul_alarm_set".to_string()
    }
    fn command_path() -> Vec<String> {
        vec!["soul".into(), "alarm-set".into()]
    }
}

impl McpExecutor for SoulAlarmSetCommand {
    fn execute_mcp_call(params: &HashMap<String, Value>) -> Result<String> {
        let name = str_param(params, "name")?;
        let table = str_param(params, "table")?;
        let column = str_param(params, "column")?;
        let condition = str_param(params, "condition")?;
        let aggregate = params
            .get("aggregate")
            .and_then(|v| v.as_str())
            .unwrap_or("sum");
        let emit = str_param(params, "emit")?;
        let mut args: Vec<&str> = vec![
            "alarm-set",
            name,
            table,
            column,
            condition,
            "--aggregate",
            aggregate,
            "--emit",
            emit,
        ];
        let scope_owned;
        let scope: Option<&str> = params.get("scope").and_then(|v| v.as_str());
        if let Some(s) = scope {
            scope_owned = s.to_string();
            args.extend(["--scope", &scope_owned]);
        }
        run_soul(&args)
    }
}

// ── soul_alarm_check ──────────────────────────────────────────────────────────

#[derive(Parser, Clone)]
pub struct SoulAlarmCheckCommand {
    pub table: String,
    #[arg(long, help = "#1102 shard scope 'kind:id', e.g. 'agent:pi'. Omit for the legacy/default shard.")]
    pub scope: Option<String>,
}

impl McpReflection for SoulAlarmCheckCommand {
    fn mcp_tool_name() -> String {
        "soul_alarm_check".to_string()
    }
    fn command_path() -> Vec<String> {
        vec!["soul".into(), "alarm-check".into()]
    }
}

impl McpExecutor for SoulAlarmCheckCommand {
    fn execute_mcp_call(params: &HashMap<String, Value>) -> Result<String> {
        let table = str_param(params, "table")?;
        let mut args: Vec<&str> = vec!["alarm-check", table];
        let scope_owned;
        let scope: Option<&str> = params.get("scope").and_then(|v| v.as_str());
        if let Some(s) = scope {
            scope_owned = s.to_string();
            args.extend(["--scope", &scope_owned]);
        }
        run_soul(&args)
    }
}

// ── soul_token_encode / decode ────────────────────────────────────────────────

#[derive(Parser, Clone)]
pub struct SoulTokenEncodeCommand {
    pub plaintext: String,
    #[arg(long, default_value = "")]
    pub context: String,
}

impl McpReflection for SoulTokenEncodeCommand {
    fn mcp_tool_name() -> String {
        "soul_token_encode".to_string()
    }
    fn command_path() -> Vec<String> {
        vec!["soul".into(), "token-encode".into()]
    }
}

impl McpExecutor for SoulTokenEncodeCommand {
    fn execute_mcp_call(params: &HashMap<String, Value>) -> Result<String> {
        let pt = str_param(params, "plaintext")?;
        let ctx = params.get("context").and_then(|v| v.as_str()).unwrap_or("");
        run_soul(&["token-encode", pt, "--context", ctx])
    }
}

#[derive(Parser, Clone)]
pub struct SoulTokenDecodeCommand {
    pub token: String,
    #[arg(long, default_value = "")]
    pub context: String,
}

impl McpReflection for SoulTokenDecodeCommand {
    fn mcp_tool_name() -> String {
        "soul_token_decode".to_string()
    }
    fn command_path() -> Vec<String> {
        vec!["soul".into(), "token-decode".into()]
    }
}

impl McpExecutor for SoulTokenDecodeCommand {
    fn execute_mcp_call(params: &HashMap<String, Value>) -> Result<String> {
        let token = str_param(params, "token")?;
        let ctx = params.get("context").and_then(|v| v.as_str()).unwrap_or("");
        run_soul(&["token-decode", token, "--context", ctx])
    }
}

// ── registration helper ───────────────────────────────────────────────────────

/// Register all DataFramerr MCP tools into a registry builder.
pub fn register_dataframerr_tools(builder: &mut crate::clap_reflection::McpCommandRegistryBuilder) {
    builder
        .register::<SoulTableCreateCommand>()
        .register::<SoulTableListCommand>()
        .register::<SoulRowInsertCommand>()
        .register::<SoulRowQueryCommand>()
        .register::<SoulCursorCreateCommand>()
        .register::<SoulCursorNextCommand>()
        .register::<SoulCursorResetCommand>()
        .register::<SoulAlarmSetCommand>()
        .register::<SoulAlarmCheckCommand>()
        .register::<SoulTokenEncodeCommand>()
        .register::<SoulTokenDecodeCommand>();
}
