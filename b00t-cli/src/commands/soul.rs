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

use anyhow::Result;
use clap::Parser;

use crate::memory_provider::{FileMemory, MemoryProvider, detect_provider, soul_path};

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
            let rt = tokio::runtime::Runtime::new()?;
            rt.block_on(serve_dbus(*session, datum_dir))
        }
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

async fn kv_get(
    State(s): State<SoulState>,
    AxumPath(key): AxumPath<String>,
) -> impl IntoResponse {
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
                .into_response()
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

async fn kv_list(
    State(s): State<SoulState>,
    Query(q): Query<PrefixQuery>,
) -> impl IntoResponse {
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
                Ok(log) => Ok(StackResult {
                    success: true,
                    log,
                }),
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
                log: vec![format!("stopped {unit}"), format!("stopped {template_unit}")],
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
    println!("soul dbus: Ctrl+C to stop");

    // Block until SIGINT/SIGTERM
    tokio::signal::ctrl_c().await?;
    println!("\nsoul dbus: shutting down");
    Ok(())
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
        .with_state(state);

    let addr = format!("{host}:{port}");
    let listener = tokio::net::TcpListener::bind(&addr).await?;
    println!("soul serve: listening on http://{addr}");
    axum::serve(listener, app).await?;
    Ok(())
}
