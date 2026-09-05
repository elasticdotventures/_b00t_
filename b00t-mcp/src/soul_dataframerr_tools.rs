//! MCP tool bindings for soul DataFramerr (K4).
//! Each tool is a thin dispatcher to `b00t-cli soul <subcmd>`.
//! Tool names mirror PRD-DATAFRAMERR.prd.tomllmd § mcp_tools.

use crate::clap_reflection::{McpExecutor, McpReflection};
use anyhow::Result;
use clap::Parser;
use serde_json::{Map, Value, json};
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

impl SoulTableCreateCommand {
    /// Extract `columns` from MCP params as a `Vec<String>` of "name:type"
    /// shorthand entries.
    ///
    /// Bug fix: the previous implementation used
    /// `.and_then(|v| v.as_array()).unwrap_or_default()`, which SILENTLY
    /// turned any non-array `columns` value (e.g. a plain string — exactly
    /// what a caller following the tool's old, incorrectly-declared
    /// `"type": "string"` schema would send) into an empty `Vec`. That made
    /// `soul_table_create` report success while creating a table with zero
    /// columns, which then made every subsequent `frame-insert`'s fields
    /// invisible in `frame-dump` (its column list is empty). Now a
    /// non-array, non-null `columns` value is a hard error instead of a
    /// silent no-op; omitting `columns` entirely still yields an empty
    /// (schemaless) table, unchanged from before.
    fn columns_from_params(params: &HashMap<String, Value>) -> Result<Vec<String>> {
        match params.get("columns") {
            None | Some(Value::Null) => Ok(Vec::new()),
            Some(Value::Array(arr)) => arr
                .iter()
                .map(|v| {
                    v.as_str().map(String::from).ok_or_else(|| {
                        anyhow::anyhow!(
                            "soul_table_create: columns[] entries must be strings \
                             ('name:type' or 'name:type?'), got: {v}"
                        )
                    })
                })
                .collect(),
            Some(other) => Err(anyhow::anyhow!(
                "soul_table_create requires columns: array of \"name:type\" strings, got: {other}"
            )),
        }
    }
}

impl McpExecutor for SoulTableCreateCommand {
    fn execute_mcp_call(params: &HashMap<String, Value>) -> Result<String> {
        let name = str_param(params, "name")?;
        let columns = Self::columns_from_params(params)?;
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

    /// `fields` is a `Vec<String>` of "key=value" tokens at the CLAP layer,
    /// which the generic reflection in `McpReflection::generate_json_schema`
    /// would (after the generic array-inference fix) correctly describe as `"type":
    /// "array"` of strings — but PRD-DATAFRAMERR §4 deliberately specifies
    /// `soul_row_insert(table: str, fields: dict, ...)`: a JSON object is a
    /// much nicer MCP-client ergonomic than an array of pre-formatted
    /// "key=value" strings, and `execute_mcp_call` below has always parsed
    /// it as one. Before this fix the schema still said `"type": "string"`
    /// (the pre-fix generic bug), which matched neither the array the
    /// generic rule would now infer nor the object the executor actually
    /// requires — this override makes the declared schema match the real
    /// contract exactly.
    fn schema_override(arg_name: &str) -> Option<Map<String, Value>> {
        if arg_name == "fields" {
            let mut m = Map::new();
            m.insert("type".to_string(), json!("object"));
            m.insert(
                "description".to_string(),
                json!(
                    "Field key=value pairs, e.g. {\"task\": \"rust_codegen\", \"rank\": 0}"
                ),
            );
            m.insert("additionalProperties".to_string(), json!(true));
            Some(m)
        } else {
            None
        }
    }
}

impl SoulRowInsertCommand {
    /// Extract `fields` from MCP params (a JSON object) as "key=value"
    /// CLAP argv tokens for `frame-insert`.
    fn fields_from_params(params: &HashMap<String, Value>) -> Result<Vec<String>> {
        let fields_obj = params
            .get("fields")
            .and_then(|v| v.as_object())
            .ok_or_else(|| anyhow::anyhow!("soul_row_insert requires fields: object"))?;
        Ok(fields_obj
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
            .collect())
    }
}

impl McpExecutor for SoulRowInsertCommand {
    fn execute_mcp_call(params: &HashMap<String, Value>) -> Result<String> {
        let table = str_param(params, "table")?;
        let pairs = Self::fields_from_params(params)?;
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

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    // ── schema shape ────────────────────────────────────────────────────────

    #[test]
    fn table_create_columns_schema_is_array_of_strings() {
        // Regression test: this was declared as "type": "string" before the
        // generic clap_reflection fix, which is what led a compliant MCP
        // client to send a plain string for `columns` — silently producing
        // a zero-column table (see columns_from_params tests below).
        let tool = SoulTableCreateCommand::to_mcp_tool();
        let props = tool.input_schema["properties"].as_object().unwrap();
        assert_eq!(props["columns"]["type"], json!("array"));
        assert_eq!(props["columns"]["items"]["type"], json!("string"));
    }

    #[test]
    fn row_insert_fields_schema_is_object() {
        // Regression test for the reported MCP-layer mismatch: schema said
        // "string", runtime demanded "object". PRD-DATAFRAMERR §4 specifies
        // `fields: dict`, so the corrected schema must say "object" — not
        // "array" (what the generic Vec<String> rule alone would infer) or
        // "string" (the old, wrong value).
        let tool = SoulRowInsertCommand::to_mcp_tool();
        let props = tool.input_schema["properties"].as_object().unwrap();
        assert_eq!(props["fields"]["type"], json!("object"));
    }

    // ── SoulTableCreateCommand::columns_from_params ─────────────────────────

    #[test]
    fn columns_from_params_accepts_proper_array() {
        let mut params = HashMap::new();
        params.insert(
            "columns".to_string(),
            json!(["task:text", "provider:text", "rank:int?"]),
        );
        let cols = SoulTableCreateCommand::columns_from_params(&params).unwrap();
        assert_eq!(cols, vec!["task:text", "provider:text", "rank:int?"]);
    }

    #[test]
    fn columns_from_params_missing_is_empty_table() {
        let params = HashMap::new();
        let cols = SoulTableCreateCommand::columns_from_params(&params).unwrap();
        assert!(cols.is_empty());
    }

    #[test]
    fn columns_from_params_rejects_plain_string_instead_of_silently_dropping() {
        // Repro: a caller following the (pre-fix) "type": "string"
        // schema would send `columns` as one comma-separated string. The old
        // code turned that into an EMPTY Vec via `.unwrap_or_default()`,
        // silently creating a zero-column table and reporting success. It
        // must now be a clear error instead.
        let mut params = HashMap::new();
        params.insert(
            "columns".to_string(),
            json!("task:text, provider:text, model:text"),
        );
        let err = SoulTableCreateCommand::columns_from_params(&params).unwrap_err();
        assert!(
            err.to_string().contains("array"),
            "error should explain columns must be an array, got: {err}"
        );
    }

    #[test]
    fn columns_from_params_rejects_non_string_array_entries() {
        let mut params = HashMap::new();
        params.insert("columns".to_string(), json!(["task:text", 42]));
        let err = SoulTableCreateCommand::columns_from_params(&params).unwrap_err();
        assert!(err.to_string().contains("must be strings"));
    }

    // ── SoulRowInsertCommand::fields_from_params ────────────────────────────

    #[test]
    fn fields_from_params_converts_object_to_key_value_pairs() {
        let mut params = HashMap::new();
        params.insert(
            "fields".to_string(),
            json!({"task": "rust_codegen", "rank": 0, "flopped": true}),
        );
        let mut pairs = SoulRowInsertCommand::fields_from_params(&params).unwrap();
        pairs.sort();
        assert_eq!(
            pairs,
            vec![
                "flopped=true".to_string(),
                "rank=0".to_string(),
                "task=rust_codegen".to_string(),
            ]
        );
    }

    #[test]
    fn fields_from_params_rejects_non_object() {
        let mut params = HashMap::new();
        params.insert("fields".to_string(), json!("task=rust_codegen"));
        let err = SoulRowInsertCommand::fields_from_params(&params).unwrap_err();
        assert!(err.to_string().contains("object"));
    }
}
