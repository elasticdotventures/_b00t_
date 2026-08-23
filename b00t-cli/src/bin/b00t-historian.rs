//! b00t-historian: durable NATS-subject scribe for hive coordination channels.
//!
//! The two-host coordination effort on `hive.sm3ll-fung1.{control,chat}` has
//! no persistence layer of its own — plain NATS pub/sub, so anyone not
//! actively subscribed misses history. The interim fix was a hand-rolled
//! `/dev/tcp` bash subscriber; it died the first time the server sent a
//! keepalive PING it didn't know how to PONG. This binary replaces that with
//! a real NATS client (`async_nats`, the same crate `b00t-chat`'s
//! `NatsTransport`/`NatsHiveTransport` already vendor) so keepalive/PING-PONG
//! and reconnection are handled by a maintained protocol implementation, not
//! hand-rolled socket code.
//!
//! It intentionally does NOT go through `b00t chat` / `NatsTransport`
//! (`b00t-lib-chat/src/transports/nats_transport.rs`): that path forces every
//! message into a `ChatMessage` JSON envelope and remaps the subject to
//! `b00t.agents.<channel>.<sender>`, which does not match the raw subjects
//! (`hive.sm3ll-fung1.control` / `.chat`) the two coordinating agents are
//! actually publishing on. `HiveTransport`/`NatsHiveTransport`
//! (`hive_transport.rs`) is schema-free but drops the NATS subject a message
//! arrived on, which the historian needs to tell `.control` from `.chat`
//! traffic apart when logging a wildcard subscription — so this binary talks
//! to `async_nats` directly, at the same abstraction level those wrappers sit
//! on top of.
//!
//! Durable records land under the project's existing
//! `historian/sessions/YYYY/MM/` convention as newline-delimited JSON,
//! one line per message, opened in append mode and flushed after every write.
//!
//! Usage:
//!   b00t-historian run
//!   b00t-historian run --subject 'hive.sm3ll-fung1.>' --nats-url nats://127.0.0.1:4222
//!   b00t-historian replay
//!   b00t-historian replay --month 2026-08 --tail 20

use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use clap::{Parser, Subcommand};
use futures::StreamExt;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::io::Write;
use std::path::{Path, PathBuf};

#[derive(Parser)]
#[clap(version, about = "b00t historian: durable NATS subject scribe")]
struct Args {
    /// Client identifier — shows up in NATS `connz` so a live connection can
    /// be told apart from other subscribers on the same server.
    #[clap(long, default_value = "historian")]
    id: String,

    #[clap(long, env = "NATS_URL", default_value = "nats://127.0.0.1:4222")]
    nats_url: String,

    /// NATS subject to subscribe (wildcard `>`/`*` supported).
    #[clap(long, default_value = "hive.sm3ll-fung1.>")]
    subject: String,

    /// Root of the `historian/sessions/YYYY/MM/` convention.
    #[clap(long, default_value = "~/.b00t/historian/sessions")]
    log_dir: String,

    /// Log basename — becomes `<basename>.ndjson` inside each YYYY/MM dir.
    #[clap(long, default_value = "hive-sm3ll-fung1-coord")]
    basename: String,

    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Subscribe and durably log forever. Run this under nohup/pm2/systemd
    /// for real persistence — it is a foreground loop, not a self-daemonizing
    /// process.
    Run,
    /// Print a human-readable replay + summary of the durable log, so a
    /// fresh agent can catch up without re-reading raw NATS traffic.
    Replay {
        /// YYYY-MM to replay (default: current UTC month).
        #[clap(long)]
        month: Option<String>,
        /// Only show the last N entries (summary always covers the full file).
        #[clap(long)]
        tail: Option<usize>,
    },
}

#[derive(Debug, Serialize, Deserialize)]
struct LogRecord {
    ts: DateTime<Utc>,
    subject: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    reply: Option<String>,
    /// Lossy UTF-8 decode of the raw payload. Good enough for the plaintext/
    /// JSON traffic this channel carries; binary payloads will show
    /// replacement characters rather than being lost outright.
    payload: String,
}

fn expand_tilde(p: &str) -> PathBuf {
    if let Some(rest) = p.strip_prefix("~/") {
        if let Some(home) = dirs::home_dir() {
            return home.join(rest);
        }
    }
    PathBuf::from(p)
}

fn log_path_for(log_dir: &Path, basename: &str, ts: DateTime<Utc>) -> PathBuf {
    log_dir
        .join(ts.format("%Y").to_string())
        .join(ts.format("%m").to_string())
        .join(format!("{basename}.ndjson"))
}

fn append_record(log_dir: &Path, basename: &str, rec: &LogRecord) -> Result<PathBuf> {
    let path = log_path_for(log_dir, basename, rec.ts);
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .with_context(|| format!("creating {}", parent.display()))?;
    }
    let mut f = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&path)
        .with_context(|| format!("opening {}", path.display()))?;
    let line = serde_json::to_string(rec)?;
    writeln!(f, "{line}")?;
    f.flush()?;
    Ok(path)
}

#[tokio::main]
async fn main() -> Result<()> {
    let args = Args::parse();
    let log_dir = expand_tilde(&args.log_dir);

    match &args.command {
        Command::Run => run(&args, &log_dir).await,
        Command::Replay { month, tail } => replay(&args, &log_dir, month.clone(), *tail),
    }
}

async fn run(args: &Args, log_dir: &Path) -> Result<()> {
    eprintln!(
        "🥾 b00t-historian[{}]: connecting to {}",
        args.id, args.nats_url
    );

    // async_nats owns the wire protocol (PING/PONG keepalive, transparent
    // reconnect on drop) — this is exactly the failure mode that killed the
    // hand-rolled `/dev/tcp` subscriber, and it's solved for free by using a
    // real client instead of re-deriving the protocol in bash.
    let client = async_nats::ConnectOptions::new()
        .name(args.id.clone())
        .connect(&args.nats_url)
        .await
        .with_context(|| format!("connecting to NATS at {}", args.nats_url))?;
    eprintln!("connected. subscribing to '{}'", args.subject);

    let mut sub = client
        .subscribe(args.subject.clone())
        .await
        .with_context(|| format!("subscribing to {}", args.subject))?;

    eprintln!(
        "camping on '{}' — logging to {}/<YYYY>/<MM>/{}.ndjson (Ctrl-C to stop)",
        args.subject,
        log_dir.display(),
        args.basename
    );

    let mut count: u64 = 0;
    while let Some(msg) = sub.next().await {
        let ts = Utc::now();
        let payload = String::from_utf8_lossy(&msg.payload).into_owned();
        let rec = LogRecord {
            ts,
            subject: msg.subject.to_string(),
            reply: msg.reply.map(|r| r.to_string()),
            payload,
        };
        match append_record(log_dir, &args.basename, &rec) {
            Ok(path) => {
                count += 1;
                eprintln!(
                    "[{}] #{count} {} -> {}",
                    ts.to_rfc3339(),
                    rec.subject,
                    path.display()
                );
            }
            Err(e) => eprintln!("failed to persist message on {}: {e}", rec.subject),
        }
    }

    eprintln!(
        "subscription stream ended after {count} messages (server closed the sub or process is shutting down)"
    );
    Ok(())
}

fn replay(args: &Args, log_dir: &Path, month: Option<String>, tail: Option<usize>) -> Result<()> {
    let month = month.unwrap_or_else(|| Utc::now().format("%Y-%m").to_string());
    let (year, mon) = month
        .split_once('-')
        .context("--month must be YYYY-MM")?;
    let path = log_dir
        .join(year)
        .join(mon)
        .join(format!("{}.ndjson", args.basename));

    if !path.exists() {
        println!(
            "no durable log yet at {} (run `b00t-historian run` first)",
            path.display()
        );
        return Ok(());
    }

    let content = std::fs::read_to_string(&path)
        .with_context(|| format!("reading {}", path.display()))?;
    let mut records: Vec<LogRecord> = Vec::new();
    for line in content.lines() {
        if line.trim().is_empty() {
            continue;
        }
        match serde_json::from_str::<LogRecord>(line) {
            Ok(r) => records.push(r),
            Err(e) => eprintln!("skipping malformed line: {e}"),
        }
    }

    let total = records.len();
    let shown: &[LogRecord] = match tail {
        Some(n) if n < total => &records[total - n..],
        _ => &records[..],
    };

    println!(
        "🥾 b00t-historian replay — {} ({total} total message(s), showing {})",
        path.display(),
        shown.len()
    );
    println!();
    for r in shown {
        println!("[{}] {}", r.ts.to_rfc3339(), r.subject);
        println!("    {}", r.payload);
    }

    if let (Some(first), Some(last)) = (records.first(), records.last()) {
        let mut by_subject: BTreeMap<String, usize> = BTreeMap::new();
        for r in &records {
            *by_subject.entry(r.subject.clone()).or_default() += 1;
        }
        println!();
        println!("— summary —");
        println!("span: {} .. {}", first.ts.to_rfc3339(), last.ts.to_rfc3339());
        for (subj, n) in by_subject {
            println!("  {subj}: {n}");
        }
    }

    Ok(())
}
