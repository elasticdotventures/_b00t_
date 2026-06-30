//! `b00t soul` — agentic soul management
//!
//! The soul is the persistent identity of a b00t agent instance.
//! Lives at ~/._b00t_/SOUL.tomllm — survives sessions, encodes accumulated
//! knowledge, role state, and tribal memory.
//!
//! Inspired by moltis per-agent memory workspaces + session persistence.
//!
//! ## b00t soul serve
//! Exposes soul K/V over HTTP so external consumers (moltis MoltisMemory_🥾)
//! can delegate their K/V caches to b00t without linking the b00t crate.
//!
//! API:
//! - GET    /v1/kv/{key}         → `{"value": "..."}` or 404
//! - PUT    /v1/kv/{key}         → body `{"value": "..."}`, 204
//! - DELETE /v1/kv/{key}         → 204
//! - GET    /v1/kv?prefix=<pfx>  → `{"keys": [...]}`
//! - GET    /healthz              → `{"status": "ok"}`

use anyhow::{Context as _, Result, bail};
use clap::Parser;
use b00t_c0re_lib::soul_dataframerr::{
    AlarmAggregate, FrameCursor, SoulAlarm, SoulColumn, SoulDataFramerr,
    SoulDataFramerrRegistry, SoulValue,
};

use crate::memory_provider::{FileMemory, MemoryProvider, active_soul_path, detect_provider, soul_path};
use crate::soul_writer::{
    SoulMemoryWriter, active_soul_dir, global_soul_dir, local_soul_dir,
};

#[derive(Parser)]
pub enum SoulCommands {
    #[clap(about = "Show current soul state (~/._b00t_/SOUL.tomllm)")]
    Status {
        #[clap(long, help = "Output as JSON")]
        json: bool,
    },

    #[clap(about = "Read a key from soul memory")]
    Get {
        #[clap(help = "Key to read")]
        key: String,
    },

    #[clap(about = "Write a key to soul memory")]
    Set {
        #[clap(help = "Key")]
        key: String,
        #[clap(help = "Value")]
        value: String,
    },

    #[clap(about = "Show soul file path")]
    Path,

    #[clap(about = "Reset soul memory (clears all keys — irreversible)")]
    Reset {
        #[clap(long, help = "Confirm reset without prompt")]
        confirm: bool,
    },

    #[clap(about = "Serve soul K/V over HTTP (port 7700 by default)")]
    Serve {
        #[clap(long, default_value = "7700", help = "TCP port to listen on")]
        port: u16,
        #[clap(long, default_value = "127.0.0.1", help = "Bind address")]
        host: String,
    },

    #[cfg(feature = "dbus")]
    #[clap(about = "Serve b00t hive control over DBus (system bus)")]
    Dbus {
        #[clap(long, help = "Use session bus (dev/test, no root)")]
        session: bool,
    },
    /// Distil session transcript into persistent soul memories.
    ///
    /// Reads session text from stdin, runs a silent LLM turn (sm0l tier),
    /// extracts facts as K/V, writes to SqliteMemoryStore + SOUL.md.
    ///
    /// Example:
    ///   b00t session export | b00t soul distill
    ///   cat session.txt | b00t soul distill --model haiku
    #[clap(about = "Distil session transcript → soul memories (reads stdin)")]
    Distill {
        #[clap(long, default_value = "sm0l", help = "Tier: sm0l|ch0nky|frontier")]
        model: String,
        #[clap(long, help = "OpenAI-compatible base URL (overrides OPENAI_BASE_URL)")]
        base_url: Option<String>,
        #[clap(long, help = "Dry-run — print extracted facts without writing")]
        dry_run: bool,
    },

    /// Initialize a workspace-local soul directory (`._b00t_/`).
    ///
    /// Creates `._b00t_/SOUL.tomllm` and `._b00t_/SOUL.md` in the current directory.
    /// Subsequent soul operations prefer this local store over the global one.
    #[clap(about = "Init local ._b00t_/ soul workspace in current directory")]
    Init {
        #[clap(long, help = "Path to initialize (default: current dir)")]
        path: Option<String>,
    },

    /// Show which soul directories are active (local + global).
    #[clap(about = "Show active soul directories (local + global)")]
    Where,

    /// Append an entry to the ops log (OPS.jsonl in the active soul dir).
    ///
    /// Scope hierarchy mirrors soul: global (~/._b00t_/) > project (._b00t_/) > task.
    /// Agents write here to leave a shared activity trail readable by other agents.
    ///
    /// Examples:
    ///   b00t soul log "submitted HF training job 6a4258f"
    ///   b00t soul log --scope project --result ok "pushed dataset to HF"
    ///   b00t soul log --list
    ///   b00t soul log --list --scope global --tail 20
    #[clap(about = "Append/read ops log (OPS.jsonl) — shared agent activity register")]
    Log {
        #[clap(help = "Message to log (omit to read)")]
        message: Option<String>,
        #[clap(long, default_value = "active", help = "Scope: active|global|project")]
        scope: String,
        #[clap(long, default_value = "info", help = "Result: ok|fail|info|warn")]
        result: String,
        #[clap(long, help = "Agent/actor label (default: $USER)")]
        agent: Option<String>,
        #[clap(long, help = "List log entries instead of appending")]
        list: bool,
        #[clap(long, default_value = "40", help = "Number of entries to show with --list")]
        tail: usize,
        #[clap(long, help = "Filter by scope with --list")]
        filter_scope: Option<String>,
    },

    // ── DataFramerr ───────────────────────────────────────────────────────────

    #[clap(name = "table-create", about = "Create a typed table in soul DataFramerr")]
    TableCreate {
        #[clap(help = "Table name")]
        name: String,
        #[clap(help = "Column definitions: 'name:type' or 'name:type?' (nullable). Types: text int float cake bool timestamp token json")]
        columns: Vec<String>,
    },

    #[clap(name = "table-list", about = "List all DataFramerr tables in active soul")]
    TableList,

    #[clap(name = "table-show", about = "Show schema + row count for a table")]
    TableShow {
        name: String,
    },

    #[clap(name = "table-drop", about = "Drop a table and all its rows (irreversible)")]
    TableDrop {
        name: String,
    },

    #[clap(name = "frame-insert", about = "Append a row to a DataFramerr table")]
    FrameInsert {
        #[clap(help = "Table name")]
        table: String,
        #[clap(help = "Field values as 'key=value' pairs")]
        fields: Vec<String>,
    },

    #[clap(name = "frame-get", about = "Fetch a single row by id")]
    FrameGet {
        table: String,
        id: u64,
    },

    #[clap(name = "frame-dump", about = "Dump rows in tabular format")]
    FrameDump {
        table: String,
        #[clap(long, help = "Show only last N rows")]
        last: Option<usize>,
    },

    #[clap(name = "cursor-create", about = "Create a durable cursor on a table")]
    CursorCreate {
        name: String,
        table: String,
    },

    #[clap(name = "cursor-next", about = "Advance cursor and print next row (exit 1 at EOF)")]
    CursorNext {
        name: String,
    },

    #[clap(name = "cursor-reset", about = "Rewind cursor to frame 0")]
    CursorReset {
        name: String,
    },

    #[clap(name = "cursor-list", about = "List all cursors and positions")]
    CursorList,

    #[clap(name = "alarm-set", about = "Register an alarm on a column aggregate")]
    AlarmSet {
        name: String,
        table: String,
        column: String,
        condition: String,
        #[clap(long, default_value = "sum", help = "Aggregate: sum | avg | count | per_frame")]
        aggregate: String,
        #[clap(long, help = "Event name to emit when alarm fires")]
        emit: String,
    },

    #[clap(name = "alarm-check", about = "Evaluate all alarms on a table; print fired events")]
    AlarmCheck {
        table: String,
    },

    #[clap(name = "alarm-list", about = "List all registered alarms")]
    AlarmList,

    #[clap(name = "alarm-rm", about = "Remove a named alarm")]
    AlarmRm {
        name: String,
    },

    #[clap(name = "token-encode", about = "ObfuscatedStr encode (XOR+base64, agent-identity keyed)")]
    TokenEncode {
        plaintext: String,
        #[clap(long, default_value = "", help = "Context key (e.g. table name)")]
        context: String,
    },

    #[clap(name = "token-decode", about = "ObfuscatedStr decode")]
    TokenDecode {
        token: String,
        #[clap(long, default_value = "", help = "Context key used during encode")]
        context: String,
    },
}

pub fn handle_soul_command(cmd: &SoulCommands) -> Result<()> {
    let path = soul_path();
    let mem = FileMemory::new(path.clone());

    match cmd {
        SoulCommands::Status { json } => {
            if !path.exists() {
                if *json {
                    println!("{{\"soul\": null, \"path\": \"{}\"}}", path.display());
                } else {
                    println!("Soul: uninitialized");
                    println!("  Path: {}", path.display());
                    println!("  Tip:  b00t soul set <key> <value> to initialize");
                }
                return Ok(());
            }

            let raw = std::fs::read_to_string(&path)?;

            if *json {
                // Strip comments, parse, emit JSON
                let stripped: String = raw
                    .lines()
                    .filter(|l| !l.trim_start().starts_with('#'))
                    .collect::<Vec<_>>()
                    .join("\n");
                #[derive(serde::Deserialize, serde::Serialize)]
                struct SoulStore {
                    #[serde(default)]
                    data: std::collections::HashMap<String, String>,
                }
                let store: SoulStore = toml::from_str(&stripped).unwrap_or(SoulStore {
                    data: Default::default(),
                });
                println!("{}", serde_json::to_string_pretty(&store.data)?);
            } else {
                println!("Soul: {}", path.display());
                println!();
                // Print non-comment lines
                for line in raw.lines() {
                    if !line.trim_start().starts_with('#') || line.starts_with("# b00t:map") {
                        println!("  {}", line);
                    }
                }
            }
            Ok(())
        }

        SoulCommands::Get { key } => {
            match mem.read(key)? {
                Some(val) => println!("{}", val),
                None => {
                    eprintln!("soul: key '{}' not found", key);
                    std::process::exit(1);
                }
            }
            Ok(())
        }

        SoulCommands::Set { key, value } => {
            mem.write(key, value)?;
            println!("soul: {} = {}", key, value);
            Ok(())
        }

        SoulCommands::Path => {
            println!("{}", path.display());
            Ok(())
        }

        SoulCommands::Reset { confirm } => {
            if !confirm {
                eprintln!("soul reset clears all memory. Use --confirm to proceed.");
                eprintln!("  b00t soul reset --confirm");
                std::process::exit(1);
            }
            if path.exists() {
                std::fs::remove_file(&path)?;
            }
            println!("soul: reset — {}", path.display());
            Ok(())
        }

        SoulCommands::Serve { port, host } => {
            let rt = tokio::runtime::Runtime::new()?;
            rt.block_on(serve_soul_kv(host, *port))
        }

        #[cfg(feature = "dbus")]
        SoulCommands::Dbus { session } => {
            let datum_dir = crate::get_expanded_path("~/.dotfiles/_b00t_/")?;
            // 🤓 main.rs uses #[tokio::main] so we're already inside a tokio runtime.
            // Runtime::new().block_on() would panic — use block_in_place instead.
            let use_session = *session;
            tokio::task::block_in_place(move || {
                tokio::runtime::Handle::current().block_on(serve_dbus(use_session, datum_dir))
            })
        }
        SoulCommands::Distill {
            model,
            base_url,
            dry_run,
        } => {
            let rt = tokio::runtime::Runtime::new()?;
            rt.block_on(distill_soul(model, base_url.as_deref(), *dry_run))
        }

        SoulCommands::Init { path: init_path } => {
            let target = init_path
                .as_deref()
                .map(std::path::PathBuf::from)
                .unwrap_or_else(|| std::env::current_dir().unwrap_or_default());
            soul_init(&target)
        }

        SoulCommands::Where => soul_where(),

        SoulCommands::Log { message, scope, result, agent, list, tail, filter_scope } => {
            soul_log(message.as_deref(), scope, result, agent.as_deref(), *list, *tail, filter_scope.as_deref())
        }

        // ── DataFramerr ───────────────────────────────────────────────────────
        SoulCommands::TableCreate { name, columns } => df_table_create(name, columns),
        SoulCommands::TableList                     => df_table_list(),
        SoulCommands::TableShow { name }            => df_table_show(name),
        SoulCommands::TableDrop { name }            => df_table_drop(name),

        SoulCommands::FrameInsert { table, fields } => df_frame_insert(table, fields),
        SoulCommands::FrameGet { table, id }        => df_frame_get(table, *id),
        SoulCommands::FrameDump { table, last }     => df_frame_dump(table, *last),

        SoulCommands::CursorCreate { name, table }  => df_cursor_create(name, table),
        SoulCommands::CursorNext { name }           => df_cursor_next(name),
        SoulCommands::CursorReset { name }          => df_cursor_reset(name),
        SoulCommands::CursorList                    => df_cursor_list(),

        SoulCommands::AlarmSet { name, table, column, condition, aggregate, emit } => {
            df_alarm_set(name, table, column, condition, aggregate, emit)
        }
        SoulCommands::AlarmCheck { table }          => df_alarm_check(table),
        SoulCommands::AlarmList                     => df_alarm_list(),
        SoulCommands::AlarmRm { name }              => df_alarm_rm(name),

        SoulCommands::TokenEncode { plaintext, context } => df_token_encode(plaintext, context),
        SoulCommands::TokenDecode { token, context }     => df_token_decode(token, context),
    }
}

// ─── soul serve HTTP API ──────────────────────────────────────────────────────

use std::sync::Arc;

use axum::{
    Router,
    extract::{Path as AxumPath, Query, State},
    http::StatusCode,
    response::IntoResponse,
    routing::get,
};
use serde::{Deserialize, Serialize};

#[derive(Clone)]
struct SoulState {
    provider: Arc<dyn MemoryProvider>,
}

#[derive(Serialize, Deserialize)]
struct KvValue {
    value: String,
}

#[derive(Serialize, Deserialize)]
struct KvKeys {
    keys: Vec<String>,
}

#[derive(Deserialize)]
struct PrefixQuery {
    prefix: Option<String>,
}

async fn kv_get(State(s): State<SoulState>, AxumPath(key): AxumPath<String>) -> impl IntoResponse {
    match s.provider.read(&key) {
        Ok(Some(val)) => {
            let body = serde_json::to_string(&KvValue { value: val }).unwrap_or_default();
            (StatusCode::OK, body).into_response()
        }
        Ok(None) => StatusCode::NOT_FOUND.into_response(),
        Err(e) => {
            let msg = format!("{{\"error\":\"{e}\"}}");
            (StatusCode::INTERNAL_SERVER_ERROR, msg).into_response()
        }
    }
}

async fn kv_put(
    State(s): State<SoulState>,
    AxumPath(key): AxumPath<String>,
    body: String,
) -> impl IntoResponse {
    let kv: KvValue = match serde_json::from_str(&body) {
        Ok(v) => v,
        Err(e) => {
            return (
                StatusCode::BAD_REQUEST,
                format!("{{\"error\":\"bad JSON: {e}\"}}"),
            )
                .into_response();
        }
    };
    match s.provider.write(&key, &kv.value) {
        Ok(()) => StatusCode::NO_CONTENT.into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("{{\"error\":\"{e}\"}}"),
        )
            .into_response(),
    }
}

async fn kv_delete(
    State(s): State<SoulState>,
    AxumPath(key): AxumPath<String>,
) -> impl IntoResponse {
    match s.provider.delete(&key) {
        Ok(()) => StatusCode::NO_CONTENT.into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("{{\"error\":\"{e}\"}}"),
        )
            .into_response(),
    }
}

async fn kv_list(State(s): State<SoulState>, Query(q): Query<PrefixQuery>) -> impl IntoResponse {
    let prefix = q.prefix.as_deref().unwrap_or("");
    match s.provider.list_keys(prefix) {
        Ok(keys) => {
            let body = serde_json::to_string(&KvKeys { keys }).unwrap_or_default();
            (StatusCode::OK, body).into_response()
        }
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("{{\"error\":\"{e}\"}}"),
        )
            .into_response(),
    }
}

async fn healthz() -> impl IntoResponse {
    (StatusCode::OK, "{\"status\":\"ok\"}")
}

// ─── soul dbus server ─────────────────────────────────────────────────────────

#[cfg(feature = "dbus")]
async fn serve_dbus(session: bool, datum_dir: std::path::PathBuf) -> Result<()> {
    use b00t_ipc::dbus_interface::{B00tService, StackResult, dbus_hive_bridge};

    // Register bridge functions so B00tService methods can call hive logic
    dbus_hive_bridge::register(
        // capture
        || {
            let snapshot = crate::hive::SystemSnapshot::capture()?;
            Ok(serde_json::to_string(&snapshot)?)
        },
        // activate
        |profile: &str, datum_dir: &std::path::Path, force: bool| {
            let p = crate::hive::load_profile(profile, datum_dir)?;
            let snapshot = crate::hive::SystemSnapshot::capture()?;
            match crate::hive::activate_profile(&p, &snapshot, false, force) {
                Ok(log) => Ok(StackResult { success: true, log }),
                Err(e) => Ok(StackResult {
                    success: false,
                    log: vec![e.to_string()],
                }),
            }
        },
        // deactivate
        |profile: &str, _datum_dir: &std::path::Path| {
            let unit = format!("b00t-hive-{profile}.service");
            let _ = std::process::Command::new("systemctl")
                .args(["stop", &unit])
                .status();
            let template_unit = format!("b00t@{profile}.service");
            let _ = std::process::Command::new("systemctl")
                .args(["stop", &template_unit])
                .status();
            Ok(StackResult {
                success: true,
                log: vec![
                    format!("stopped {unit}"),
                    format!("stopped {template_unit}"),
                ],
            })
        },
    );

    let service = B00tService::new(datum_dir);

    let connection = if session {
        println!("soul dbus: connecting to session bus ...");
        zbus::connection::Builder::session()?
    } else {
        println!("soul dbus: connecting to system bus ...");
        zbus::connection::Builder::system()?
    };

    let _conn = connection
        .name("com.promptexecution.b00t1")?
        .serve_at("/com/promptexecution/b00t1", service)?
        .build()
        .await?;

    println!("soul dbus: bus name acquired — com.promptexecution.b00t1");
    println!("soul dbus: serving at /com/promptexecution/b00t1");
    println!("soul dbus: Ctrl+C or SIGTERM to stop");

    // Block until SIGINT (Ctrl+C) or SIGTERM (e.g. systemd stop) on Unix.
    // On non-Unix platforms, fall back to SIGINT only.
    #[cfg(unix)]
    {
        let mut sigterm =
            tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())?;
        tokio::select! {
            _ = tokio::signal::ctrl_c() => {},
            _ = sigterm.recv() => {},
        }
    }

    #[cfg(not(unix))]
    {
        tokio::signal::ctrl_c().await?;
    }

    println!("\nsoul dbus: shutting down");
    Ok(())
}

// ─── soul init ────────────────────────────────────────────────────────────────

/// Create `._b00t_/` workspace soul directory with skeleton files.
fn soul_init(target: &std::path::Path) -> Result<()> {
    let soul_dir = target.join("._b00t_");
    std::fs::create_dir_all(&soul_dir).with_context(|| format!("create {}", soul_dir.display()))?;

    // SOUL.tomllm — K/V identity file
    let tomllm = soul_dir.join("SOUL.tomllm");
    if !tomllm.exists() {
        std::fs::write(
            &tomllm,
            "# b00t SOUL — workspace agentic identity\n\
             # @tribal: edit via `b00t soul set`, not directly\n\n\
             [data]\n\
             # b00t:map v1\n\
             # summary: workspace soul — identity & memory for this repo\n\
             # tags: soul, memory, workspace\n\
             # tier: sm0l\n",
        )?;
        println!("soul init: created {}", tomllm.display());
    } else {
        println!("soul init: exists  {}", tomllm.display());
    }

    // SOUL.md — markdown memory file
    let soul_md = soul_dir.join("SOUL.md");
    if !soul_md.exists() {
        let today = chrono::Utc::now().format("%Y-%m-%d");
        std::fs::write(
            &soul_md,
            format!(
                "# Soul — {}\n\n\
                 Workspace soul initialized {today}.\n\
                 Distilled memories from sessions will be appended here.\n",
                target
                    .file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or("workspace"),
            ),
        )?;
        println!("soul init: created {}", soul_md.display());
    } else {
        println!("soul init: exists  {}", soul_md.display());
    }

    println!("soul: workspace soul active at {}", soul_dir.display());

    // GithubActionsCompatible hint: suggest gen-wrkflw if workflows exist but no local gate
    let workflows_dir = target.join(".github/workflows");
    if workflows_dir.exists() {
        let has_wrkflw = std::fs::read_dir(&workflows_dir)
            .ok()
            .map(|entries| {
                entries
                    .filter_map(|e| e.ok())
                    .any(|e| e.file_name().to_string_lossy().starts_with("wrkflw-"))
            })
            .unwrap_or(false);
        if !has_wrkflw {
            println!(
                "tip: GithubActionsCompatible repo detected — run `b00t datum gen-wrkflw . --write` to add a local CI gate"
            );
        }
    }

    Ok(())
}

// ─── soul where ───────────────────────────────────────────────────────────────

/// Show active soul directories (local + global).
// ─── ops log ─────────────────────────────────────────────────────────────────

/// Append-only ops log: shared activity register across agents.
///
/// File: `<soul_dir>/OPS.jsonl` — one JSON object per line, newest at end.
/// Scope hierarchy: global (~/._b00t_/) > project (._b00t_/) > task (in-memory only).
fn soul_log(
    message: Option<&str>,
    scope: &str,
    result: &str,
    agent: Option<&str>,
    list: bool,
    tail: usize,
    filter_scope: Option<&str>,
) -> Result<()> {
    let soul_dir = match scope {
        "global" => global_soul_dir(),
        "project" => local_soul_dir().unwrap_or_else(global_soul_dir),
        _ => active_soul_dir(),
    };

    let log_path = soul_dir.join("OPS.jsonl");

    if list {
        if !log_path.exists() {
            println!("ops log empty: {}", log_path.display());
            return Ok(());
        }
        let content = std::fs::read_to_string(&log_path)?;
        let lines: Vec<&str> = content.lines().collect();
        let start = lines.len().saturating_sub(tail);
        for line in &lines[start..] {
            if let Ok(entry) = serde_json::from_str::<serde_json::Value>(line) {
                if let Some(fs) = filter_scope {
                    if entry.get("scope").and_then(|v| v.as_str()) != Some(fs) {
                        continue;
                    }
                }
                let ts = entry.get("ts").and_then(|v| v.as_str()).unwrap_or("?");
                let sc = entry.get("scope").and_then(|v| v.as_str()).unwrap_or("?");
                let ag = entry.get("agent").and_then(|v| v.as_str()).unwrap_or("?");
                let re = entry.get("result").and_then(|v| v.as_str()).unwrap_or("info");
                let msg = entry.get("message").and_then(|v| v.as_str()).unwrap_or("");
                let icon = match re { "ok" => "✅", "fail" => "❌", "warn" => "⚠️ ", _ => "ℹ️ " };
                println!("{icon} [{ts}] ({sc}/{ag}) {msg}");
            }
        }
        return Ok(());
    }

    let msg = message.ok_or_else(|| anyhow::anyhow!("message required (or use --list to read)"))?;

    let agent_str = agent
        .map(|s| s.to_string())
        .or_else(|| std::env::var("USER").ok())
        .unwrap_or_else(|| "agent".to_string());

    let entry = serde_json::json!({
        "ts": chrono::Utc::now().to_rfc3339(),
        "scope": scope,
        "agent": agent_str,
        "result": result,
        "message": msg,
    });

    std::fs::create_dir_all(&soul_dir)
        .with_context(|| format!("create soul dir {}", soul_dir.display()))?;

    use std::io::Write;
    let mut f = std::fs::OpenOptions::new().create(true).append(true).open(&log_path)?;
    writeln!(f, "{}", serde_json::to_string(&entry)?)?;

    println!("ops: [{scope}] {msg}");
    Ok(())
}

fn soul_where() -> Result<()> {
    let global = global_soul_dir();
    let local = local_soul_dir();

    if let Some(ref l) = local {
        println!("local:  {} ✓", l.display());
        println!("        (active — takes priority over global)");
    } else {
        println!("local:  none (run `b00t soul init` to create ._b00t_/ here)");
    }
    println!("global: {}", global.display());
    if !global.exists() {
        println!("        (uninitialized — run `b00t soul set <key> <val>` to create)");
    }
    println!("active: {}", active_soul_dir().display());
    Ok(())
}

// ─── soul distill ─────────────────────────────────────────────────────────────

/// System prompt for the silent memory-distillation LLM turn.
/// Adapted from moltis `MEMORY_FLUSH_SYSTEM_PROMPT` — outputs TOML for easy K/V parsing.
const DISTILL_SYSTEM_PROMPT: &str = r#"You are a memory distillation agent for b00t soul.

Review the session transcript and extract facts worth persisting across sessions.
Focus on:
- User role, preferences, working style
- Project context, architecture decisions, conventions
- Technical setup: tools, languages, frameworks, env
- Key decisions and their reasoning
- Recurring patterns, tribal knowledge

Output ONLY valid TOML in this exact format (no prose, no code fences):

[facts]
# Each key = concise identifier, value = one-line fact
# key = "value"

[markdown_summary]
text = """
# Session memory — YYYY-MM-DD
(2-5 bullet points of what was done/decided)
"""

If nothing is worth persisting, output an empty [facts] section."#;

/// Tier → model ID map. Uses OPENAI_BASE_URL / ANTHROPIC_API_KEY env routing.
fn tier_to_model(tier: &str) -> &'static str {
    match tier {
        "sm0l" | "small" => "claude-haiku-4-5-20251001",
        "ch0nky" | "chunky" => "claude-sonnet-4-6",
        "frontier" => "claude-opus-4-6",
        other => {
            // Accept raw model IDs passthrough
            // 🤓 leaking lifetime via static ref is ok here — caller uses it immediately
            let _ = other;
            "claude-haiku-4-5-20251001"
        }
    }
}

async fn distill_soul(tier: &str, base_url: Option<&str>, dry_run: bool) -> Result<()> {
    use std::io::Read as _;

    // Read transcript from stdin
    let mut transcript = String::new();
    std::io::stdin()
        .read_to_string(&mut transcript)
        .context("read transcript from stdin")?;

    if transcript.trim().is_empty() {
        bail!("soul distill: no transcript on stdin — pipe session text");
    }

    // Truncate to ~32K chars to avoid huge context costs on sm0l tier
    let truncated = if transcript.len() > 32_768 {
        eprintln!(
            "soul distill: transcript truncated to 32K chars ({} total)",
            transcript.len()
        );
        &transcript[..32_768]
    } else {
        &transcript
    };

    let model = tier_to_model(tier);
    let api_key = std::env::var("ANTHROPIC_API_KEY")
        .or_else(|_| std::env::var("OPENAI_API_KEY"))
        .unwrap_or_default();
    let url_base = base_url
        .map(str::to_owned)
        .or_else(|| std::env::var("OPENAI_BASE_URL").ok())
        .unwrap_or_else(|| "https://api.anthropic.com/v1".to_owned());

    eprintln!("soul distill: calling {model} via {url_base} ...");

    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(60))
        .build()?;

    // OpenAI-compatible chat completions payload
    let payload = serde_json::json!({
        "model": model,
        "max_tokens": 1024,
        "system": DISTILL_SYSTEM_PROMPT,
        "messages": [
            { "role": "user", "content": truncated }
        ]
    });

    let resp = client
        .post(format!("{url_base}/messages"))
        .header("x-api-key", &api_key)
        .header("anthropic-version", "2023-06-01")
        .header("content-type", "application/json")
        .json(&payload)
        .send()
        .await
        .context("distill LLM request")?
        .error_for_status()
        .context("distill LLM response")?;

    let body: serde_json::Value = resp.json().await?;
    let text = body["content"][0]["text"].as_str().unwrap_or("").to_owned();

    if text.is_empty() {
        bail!("soul distill: empty response from LLM");
    }

    // Parse TOML output — extract [facts] K/V and [markdown_summary].text
    let parsed: toml::Value =
        toml::from_str(&text).with_context(|| format!("parse distill output as TOML:\n{text}"))?;

    let facts: Vec<(String, String)> = parsed
        .get("facts")
        .and_then(|f| f.as_table())
        .map(|t| {
            t.iter()
                .filter_map(|(k, v)| v.as_str().map(|s| (k.clone(), s.to_owned())))
                .collect()
        })
        .unwrap_or_default();

    let md_summary = parsed
        .get("markdown_summary")
        .and_then(|m| m.get("text"))
        .and_then(|t| t.as_str())
        .unwrap_or("")
        .to_owned();

    if dry_run {
        println!("# soul distill dry-run — {} facts extracted", facts.len());
        for (k, v) in &facts {
            println!("  {k} = {v:?}");
        }
        if !md_summary.is_empty() {
            println!("\n## markdown summary\n{md_summary}");
        }
        return Ok(());
    }

    // Write K/V facts to soul SqliteMemoryStore
    let provider = detect_provider();
    let mut written = 0usize;
    for (k, v) in &facts {
        let key = format!("distill:{k}");
        provider.write(&key, v)?;
        written += 1;
    }

    // Append markdown summary to SOUL.md in active soul dir
    if !md_summary.is_empty() {
        let writer = crate::soul_writer::FileSoulWriter::detect();
        match writer.write_memory("SOUL.md", &md_summary, true) {
            Ok(r) => eprintln!("soul distill: appended summary → {}", r.location),
            Err(e) => eprintln!("soul distill: SOUL.md write skipped: {e}"),
        }
    }

    println!(
        "soul distill: {} facts → soul K/V; SOUL.md updated",
        written
    );
    Ok(())
}

// ─── /v1/memory/write ─────────────────────────────────────────────────────────

#[derive(Deserialize)]
struct MemoryWriteRequest {
    file: String,
    content: String,
    #[serde(default)]
    append: bool,
}

#[derive(Serialize)]
struct MemoryWriteResponse {
    location: String,
    bytes_written: usize,
}

async fn memory_write(State(_s): State<SoulState>, body: String) -> impl IntoResponse {
    let req: MemoryWriteRequest = match serde_json::from_str(&body) {
        Ok(r) => r,
        Err(e) => {
            return (
                StatusCode::BAD_REQUEST,
                format!("{{\"error\":\"bad JSON: {e}\"}}"),
            )
                .into_response();
        }
    };
    let writer = crate::soul_writer::FileSoulWriter::detect();
    match writer.write_memory(&req.file, &req.content, req.append) {
        Ok(r) => {
            let resp = MemoryWriteResponse {
                location: r.location,
                bytes_written: r.bytes_written,
            };
            (
                StatusCode::OK,
                serde_json::to_string(&resp).unwrap_or_default(),
            )
                .into_response()
        }
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("{{\"error\":\"{e}\"}}"),
        )
            .into_response(),
    }
}

async fn serve_soul_kv(host: &str, port: u16) -> Result<()> {
    let provider = detect_provider();
    let state = SoulState {
        provider: Arc::from(provider),
    };

    let app = Router::new()
        .route("/healthz", get(healthz))
        .route("/v1/kv", get(kv_list))
        .route("/v1/kv/:key", get(kv_get).put(kv_put).delete(kv_delete))
        .route("/v1/memory/write", axum::routing::post(memory_write))
        .with_state(state);

    let addr = format!("{host}:{port}");
    let listener = tokio::net::TcpListener::bind(&addr).await?;
    println!("soul serve: listening on http://{addr}");
    axum::serve(listener, app).await?;
    Ok(())
}

// ── DataFramerr registry I/O ──────────────────────────────────────────────────

/// Load the full TOML document from the active soul file.
fn load_soul_doc() -> Result<toml::Table> {
    let path = active_soul_path();
    if !path.exists() {
        return Ok(toml::Table::new());
    }
    let text = std::fs::read_to_string(&path)
        .with_context(|| format!("read {}", path.display()))?;
    text.parse::<toml::Table>()
        .with_context(|| format!("parse TOML {}", path.display()))
}

/// Extract [soul] → SoulDataFramerrRegistry from the doc.
fn load_registry(doc: &toml::Table) -> Result<SoulDataFramerrRegistry> {
    match doc.get("soul") {
        None => Ok(SoulDataFramerrRegistry::default()),
        Some(v) => {
            let s = toml::to_string(v)?;
            toml::from_str(&s).context("deserialize SoulDataFramerrRegistry from [soul]")
        }
    }
}

/// Serialize registry back into doc["soul"] and write the file.
fn save_registry(mut doc: toml::Table, reg: &SoulDataFramerrRegistry) -> Result<()> {
    let path = active_soul_path();
    // Create parent dir if missing
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let reg_str = toml::to_string(reg)?;
    let reg_table: toml::Table = reg_str.parse()?;
    doc.insert("soul".to_string(), toml::Value::Table(reg_table));
    std::fs::write(&path, toml::to_string_pretty(&doc)?)
        .with_context(|| format!("write {}", path.display()))
}

/// Read-modify-write helper.
fn with_registry<F>(f: F) -> Result<()>
where F: FnOnce(&mut SoulDataFramerrRegistry) -> Result<()> {
    let doc = load_soul_doc()?;
    let mut reg = load_registry(&doc)?;
    f(&mut reg)?;
    save_registry(doc, &reg)
}

/// Parse "key=value" field args into a BTreeMap<String, SoulValue>.
fn parse_fields(fields: &[String]) -> Result<std::collections::BTreeMap<String, SoulValue>> {
    let mut map = std::collections::BTreeMap::new();
    for f in fields {
        let (k, v) = f.split_once('=')
            .ok_or_else(|| anyhow::anyhow!("field must be 'key=value', got: {f}"))?;
        // Infer type: bool → int → float → text
        let val = if v == "true" {
            SoulValue::Bool(true)
        } else if v == "false" {
            SoulValue::Bool(false)
        } else if let Ok(i) = v.parse::<i64>() {
            SoulValue::Int(i)
        } else if let Ok(f) = v.parse::<f64>() {
            SoulValue::Float(f)
        } else {
            SoulValue::Text(v.to_string())
        };
        map.insert(k.to_string(), val);
    }
    Ok(map)
}

/// Derive agent_id from environment or fallback to username.
fn agent_id() -> String {
    std::env::var("B00T_AGENT_ID")
        .or_else(|_| std::env::var("USER"))
        .unwrap_or_else(|_| "b00t-agent".to_string())
}

// ── table commands ────────────────────────────────────────────────────────────

fn df_table_create(name: &str, columns: &[String]) -> Result<()> {
    let cols: Vec<SoulColumn> = columns.iter()
        .map(|s| SoulColumn::parse(s))
        .collect::<Result<_>>()?;
    with_registry(|reg| {
        if reg.tables.contains_key(name) {
            bail!("table '{name}' already exists; use frame-insert to add rows");
        }
        reg.tables.insert(name.to_string(), SoulDataFramerr::new(name, cols.clone()));
        println!("table '{name}' created ({} columns)", cols.len());
        Ok(())
    })
}

fn df_table_list() -> Result<()> {
    let doc = load_soul_doc()?;
    let reg = load_registry(&doc)?;
    if reg.tables.is_empty() {
        println!("(no tables)");
        return Ok(());
    }
    println!("{:<24} {:>6}  columns", "table", "rows");
    println!("{}", "-".repeat(42));
    for (name, df) in &reg.tables {
        println!("{:<24} {:>6}  {}", name, df.rows.len(),
            df.columns.iter().map(|c| format!("{}:{}", c.name,
                format!("{:?}", c.col_type).to_lowercase())).collect::<Vec<_>>().join(", "));
    }
    Ok(())
}

fn df_table_show(name: &str) -> Result<()> {
    let doc = load_soul_doc()?;
    let reg = load_registry(&doc)?;
    let df = reg.tables.get(name)
        .ok_or_else(|| anyhow::anyhow!("no table '{name}'"))?;
    println!("table: {name}");
    println!("rows:  {}", df.rows.len());
    println!("columns:");
    for c in &df.columns {
        let nullable = if c.nullable { "?" } else { "" };
        println!("  {}{}: {:?}", c.name, nullable, c.col_type);
    }
    let alarms: Vec<_> = reg.alarms.iter().filter(|a| a.table == name).collect();
    if !alarms.is_empty() {
        println!("alarms:");
        for a in alarms {
            println!("  {} — {} {} {} ({:?}) → {}", a.name, a.column, a.condition,
                format!("{:?}", a.aggregate).to_lowercase(), a.aggregate, a.emit);
        }
    }
    Ok(())
}

fn df_table_drop(name: &str) -> Result<()> {
    with_registry(|reg| {
        if reg.tables.remove(name).is_none() {
            bail!("no table '{name}'");
        }
        reg.alarms.retain(|a| a.table != name);
        reg.cursors.retain(|_, c| c.table != name);
        println!("table '{name}' dropped");
        Ok(())
    })
}

// ── frame commands ────────────────────────────────────────────────────────────

fn df_frame_insert(table: &str, fields: &[String]) -> Result<()> {
    let field_map = parse_fields(fields)?;
    with_registry(|reg| {
        let df = reg.tables.get_mut(table)
            .ok_or_else(|| anyhow::anyhow!("no table '{table}' — run: b00t soul table-create {table}"))?;
        let id = df.insert(field_map)?;
        println!("inserted frame {id} into '{table}'");
        Ok(())
    })
}

fn df_frame_get(table: &str, id: u64) -> Result<()> {
    let doc = load_soul_doc()?;
    let reg = load_registry(&doc)?;
    let df = reg.tables.get(table)
        .ok_or_else(|| anyhow::anyhow!("no table '{table}'"))?;
    let row = df.get(id).ok_or_else(|| anyhow::anyhow!("no frame {id} in '{table}'"))?;
    println!("id: {}  created_at: {}", row.id, row.created_at.format("%Y-%m-%dT%H:%M:%SZ"));
    for (k, v) in &row.fields {
        println!("  {} = {:?}", k, v);
    }
    Ok(())
}

fn df_frame_dump(table: &str, last: Option<usize>) -> Result<()> {
    let doc = load_soul_doc()?;
    let reg = load_registry(&doc)?;
    let df = reg.tables.get(table)
        .ok_or_else(|| anyhow::anyhow!("no table '{table}'"))?;
    let rows: Vec<_> = match last {
        Some(n) => df.rows.iter().rev().take(n).rev().collect(),
        None    => df.rows.iter().collect(),
    };
    if rows.is_empty() {
        println!("(no rows in '{table}')");
        return Ok(());
    }
    // collect all column keys in order
    let cols: Vec<&str> = df.columns.iter().map(|c| c.name.as_str()).collect();
    print!("{:>4}  {:19}", "id", "created_at");
    for c in &cols { print!("  {:<16}", c); }
    println!();
    println!("{}", "-".repeat(4 + 2 + 19 + cols.len() * 18));
    for row in rows {
        print!("{:>4}  {}", row.id, row.created_at.format("%Y-%m-%dT%H:%M:%S"));
        for c in &cols {
            match row.fields.get(*c) {
                Some(SoulValue::Bool(b))  => print!("  {:<16}", b),
                Some(SoulValue::Int(i))   => print!("  {:<16}", i),
                Some(SoulValue::Float(f)) => print!("  {:<16.4}", f),
                Some(SoulValue::Text(s))  => print!("  {:<16}", &s[..s.len().min(16)]),
                None                      => print!("  {:<16}", "-"),
            }
        }
        println!();
    }
    Ok(())
}

// ── cursor commands ───────────────────────────────────────────────────────────

fn df_cursor_create(name: &str, table: &str) -> Result<()> {
    with_registry(|reg| {
        if !reg.tables.contains_key(table) {
            bail!("no table '{table}'");
        }
        reg.cursors.insert(name.to_string(), FrameCursor::new(table));
        println!("cursor '{name}' created on table '{table}' at frame 0");
        Ok(())
    })
}

fn df_cursor_next(name: &str) -> Result<()> {
    let doc = load_soul_doc()?;
    let mut reg = load_registry(&doc)?;
    let cursor = reg.cursors.get_mut(name)
        .ok_or_else(|| anyhow::anyhow!("no cursor '{name}' — run: b00t soul cursor-create {name} <table>"))?;
    let table_name = cursor.table.clone();
    let df = reg.tables.get_mut(&table_name)
        .ok_or_else(|| anyhow::anyhow!("cursor '{name}' points to missing table '{table_name}'"))?;
    match cursor.next(df) {
        Some(row) => {
            println!("frame {} ({})", row.id, row.created_at.format("%Y-%m-%dT%H:%M:%SZ"));
            for (k, v) in &row.fields {
                println!("  {} = {:?}", k, v);
            }
            save_registry(doc, &reg)
        }
        None => {
            println!("EOF: cursor '{name}' at end of '{table_name}'");
            std::process::exit(1);
        }
    }
}

fn df_cursor_reset(name: &str) -> Result<()> {
    with_registry(|reg| {
        let cursor = reg.cursors.get_mut(name)
            .ok_or_else(|| anyhow::anyhow!("no cursor '{name}'"))?;
        cursor.reset();
        println!("cursor '{name}' reset to frame 0");
        Ok(())
    })
}

fn df_cursor_list() -> Result<()> {
    let doc = load_soul_doc()?;
    let reg = load_registry(&doc)?;
    if reg.cursors.is_empty() {
        println!("(no cursors)");
        return Ok(());
    }
    println!("{:<20} {:<20} {:>8}", "cursor", "table", "frame_id");
    println!("{}", "-".repeat(52));
    for (name, c) in &reg.cursors {
        println!("{:<20} {:<20} {:>8}", name, c.table, c.frame_id);
    }
    Ok(())
}

// ── alarm commands ────────────────────────────────────────────────────────────

fn df_alarm_set(name: &str, table: &str, column: &str, condition: &str, aggregate: &str, emit: &str) -> Result<()> {
    let agg = match aggregate {
        "sum"       => AlarmAggregate::Sum,
        "avg"       => AlarmAggregate::Avg,
        "count"     => AlarmAggregate::Count,
        "per_frame" => AlarmAggregate::PerFrame,
        other       => bail!("unknown aggregate '{other}'; valid: sum avg count per_frame"),
    };
    with_registry(|reg| {
        reg.alarms.retain(|a| a.name != name);
        reg.alarms.push(SoulAlarm {
            name: name.to_string(), table: table.to_string(), column: column.to_string(),
            condition: condition.to_string(), aggregate: agg, emit: emit.to_string(),
        });
        println!("alarm '{name}' set on {table}.{column} {condition} → {emit}");
        Ok(())
    })
}

fn df_alarm_check(table: &str) -> Result<()> {
    let doc = load_soul_doc()?;
    let reg = load_registry(&doc)?;
    let fired: Vec<_> = reg.alarms.iter()
        .filter(|a| a.table == table)
        .filter(|a| reg.tables.get(table).map(|df| a.check(df)).unwrap_or(false))
        .collect();
    if fired.is_empty() {
        println!("no alarms fired for '{table}'");
    } else {
        for a in &fired {
            println!("ALARM: {} → {}", a.name, a.emit);
        }
    }
    Ok(())
}

fn df_alarm_list() -> Result<()> {
    let doc = load_soul_doc()?;
    let reg = load_registry(&doc)?;
    if reg.alarms.is_empty() {
        println!("(no alarms)");
        return Ok(());
    }
    for a in &reg.alarms {
        println!("{}: {}.{} {} ({:?}) → {}", a.name, a.table, a.column,
            a.condition, a.aggregate, a.emit);
    }
    Ok(())
}

fn df_alarm_rm(name: &str) -> Result<()> {
    with_registry(|reg| {
        let before = reg.alarms.len();
        reg.alarms.retain(|a| a.name != name);
        if reg.alarms.len() == before {
            bail!("no alarm '{name}'");
        }
        println!("alarm '{name}' removed");
        Ok(())
    })
}

// ── token commands ────────────────────────────────────────────────────────────

fn df_token_encode(plaintext: &str, context: &str) -> Result<()> {
    use b00t_c0re_lib::soul_dataframerr::ObfuscatedStr;
    let id = agent_id();
    let enc = ObfuscatedStr::encode(plaintext, &id, context);
    println!("{}", enc.as_str());
    Ok(())
}

fn df_token_decode(token: &str, context: &str) -> Result<()> {
    use b00t_c0re_lib::soul_dataframerr::ObfuscatedStr;
    let id = agent_id();
    let enc = ObfuscatedStr::from_raw(token)?;
    let plain = enc.decode(&id, context)?;
    println!("{}", plain);
    Ok(())
}
