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
//! `b00t.chat.<channel>.<sender>`, which does not match the raw subjects
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
//! ## souls — cross-host agent activity coordination
//!
//! Any agent becomes a "soul" simply by publishing significant-event
//! broadcasts to `souls.<repo>.<hostname>.activity` (see `b00t-historian
//! publish`) — no sidecar, no new broker; same NATS+JetStream this binary
//! already durably logs against. This binary is the one process that turns
//! that stream into a queryable record: it persists each broadcast into a
//! local soul table (`souls_activity`, via `b00t_cli::commands::soul`'s
//! DataFramerr — the same typed table/cursor/alarm primitive `b00t soul`
//! already exposes, chosen over a bespoke store per DRY) and answers live
//! queries published on `souls.query` (NATS request-reply: any agent, on any
//! host, can ask "who's been active on repo X" without CLI/SSH access to
//! wherever the table actually lives). Where that table durably lives long
//! term (local file vs. a real backend) is explicitly deferred — for now
//! it's wherever this process's `SOUL.tomllm` resolves to.
//!
//! Usage:
//!   b00t-historian run
//!   b00t-historian run --subject 'hive.sm3ll-fung1.>' --nats-url nats://127.0.0.1:4222
//!   b00t-historian replay
//!   b00t-historian replay --month 2026-08 --tail 20
//!   b00t-historian publish --repo _b00t_ --event task_start --detail "reviewing PR #1147"

use anyhow::{Context, Result};
use b00t_c0re_lib::soul_dataframerr::{SoulColumn, SoulValue};
use b00t_cli::commands::soul::{load_registry, load_soul_doc, with_registry};
use chrono::{DateTime, Utc};
use clap::{Parser, Subcommand};
use futures::StreamExt;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::collections::BTreeMap;
use std::io::Write;
use std::path::{Path, PathBuf};

const SOULS_TABLE: &str = "souls_activity";
const SOULS_QUERY_SUBJECT: &str = "souls.query";
const SOULS_WILDCARD: &str = "souls.>";
const DEFAULT_QUERY_LIMIT: usize = 50;

#[derive(Parser)]
#[clap(version, about = "b00t historian: durable NATS subject scribe")]
struct Args {
    /// Client identifier — shows up in NATS `connz` so a live connection can
    /// be told apart from other subscribers on the same server.
    #[clap(long, default_value = "historian")]
    id: String,

    #[clap(long, env = "NATS_URL", default_value = "nats://127.0.0.1:4222")]
    nats_url: String,

    /// NATS subject to durably archive as raw NDJSON (wildcard `>`/`*` supported).
    /// `souls.>` is always additionally subscribed for the souls coordination
    /// feature, independent of this flag.
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
    /// One-shot: publish a souls activity broadcast and exit. This is how an
    /// agent becomes a "soul" — no daemon, no persistent connection needed.
    Publish {
        /// Repo this activity concerns (e.g. `_b00t_`).
        #[clap(long)]
        repo: String,
        /// Hostname this activity is happening on (default: this machine's).
        #[clap(long)]
        hostname: Option<String>,
        /// Groups multiple events from one agent run together (default: a
        /// generated pid+timestamp id).
        #[clap(long)]
        session_id: Option<String>,
        /// session_start | session_end | task_start | task_complete | claim
        #[clap(long)]
        event: String,
        /// Free text — task description, or the claimed file/area for `claim`.
        #[clap(long)]
        detail: Option<String>,
    },
    /// One-shot: ask `souls.query` over NATS request-reply and print the
    /// answer. Zero-dependency way for any agent/operator to ask "who's
    /// been active on repo X" without writing their own NATS client.
    Query {
        /// Repo to ask about (e.g. `_b00t_`).
        #[clap(long)]
        repo: String,
        /// Only events from this hostname.
        #[clap(long)]
        hostname: Option<String>,
        /// Only events at or after this RFC3339 timestamp.
        #[clap(long)]
        since: Option<DateTime<Utc>>,
        /// Max events to return (default: 50).
        #[clap(long)]
        limit: Option<usize>,
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

/// Wire shape for a `souls.<repo>.<hostname>.activity` broadcast.
#[derive(Debug, Clone, Serialize, Deserialize)]
struct SoulActivity {
    repo: String,
    hostname: String,
    session_id: String,
    event: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    detail: Option<String>,
}

/// Wire shape for a `souls.query` request.
#[derive(Debug, Serialize, Deserialize)]
struct SoulQuery {
    repo: String,
    #[serde(default)]
    hostname: Option<String>,
    #[serde(default)]
    since: Option<DateTime<Utc>>,
    #[serde(default)]
    limit: Option<usize>,
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

fn souls_columns() -> Vec<SoulColumn> {
    vec![
        SoulColumn::parse("repo:text").unwrap(),
        SoulColumn::parse("hostname:text").unwrap(),
        SoulColumn::parse("session_id:text").unwrap(),
        SoulColumn::parse("event:text").unwrap(),
        SoulColumn::parse("detail:text?").unwrap(),
    ]
}

/// Persist one souls activity broadcast into the local soul table.
fn record_soul_activity(activity: &SoulActivity) -> Result<()> {
    with_registry(|reg| {
        let df = reg.get_or_create(SOULS_TABLE, souls_columns());
        let mut fields = BTreeMap::new();
        fields.insert("repo".to_string(), SoulValue::Text(activity.repo.clone()));
        fields.insert(
            "hostname".to_string(),
            SoulValue::Text(activity.hostname.clone()),
        );
        fields.insert(
            "session_id".to_string(),
            SoulValue::Text(activity.session_id.clone()),
        );
        fields.insert("event".to_string(), SoulValue::Text(activity.event.clone()));
        if let Some(detail) = &activity.detail {
            fields.insert("detail".to_string(), SoulValue::Text(detail.clone()));
        }
        df.insert(fields)?;
        Ok(())
    })
}

/// Answer a `souls.query` request from the local soul table. Read-only — does
/// not go through `with_registry` (which always saves back), since a query
/// has nothing to persist.
fn query_soul_activity(q: &SoulQuery) -> Result<serde_json::Value> {
    let doc = load_soul_doc()?;
    let reg = load_registry(&doc)?;
    let limit = q.limit.unwrap_or(DEFAULT_QUERY_LIMIT);

    let events: Vec<serde_json::Value> = match reg.tables.get(SOULS_TABLE) {
        None => Vec::new(),
        Some(df) => df
            .rows
            .iter()
            .filter(|row| {
                row.fields.get("repo").and_then(SoulValue::as_str) == Some(q.repo.as_str())
            })
            .filter(|row| match &q.hostname {
                None => true,
                Some(h) => row.fields.get("hostname").and_then(SoulValue::as_str) == Some(h.as_str()),
            })
            .filter(|row| match q.since {
                None => true,
                Some(since) => row.created_at >= since,
            })
            .rev()
            .take(limit)
            .map(|row| {
                json!({
                    "ts": row.created_at.to_rfc3339(),
                    "repo": row.fields.get("repo").and_then(SoulValue::as_str),
                    "hostname": row.fields.get("hostname").and_then(SoulValue::as_str),
                    "session_id": row.fields.get("session_id").and_then(SoulValue::as_str),
                    "event": row.fields.get("event").and_then(SoulValue::as_str),
                    "detail": row.fields.get("detail").and_then(SoulValue::as_str),
                })
            })
            .collect(),
    };

    Ok(json!({ "events": events }))
}

fn expand_souls_subject(subject: &str) -> Option<(&str, &str)> {
    // souls.<repo>.<hostname>.activity
    let parts: Vec<&str> = subject.split('.').collect();
    if parts.len() == 4 && parts[0] == "souls" && parts[3] == "activity" {
        Some((parts[1], parts[2]))
    } else {
        None
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    let args = Args::parse();
    let log_dir = expand_tilde(&args.log_dir);

    match &args.command {
        Command::Run => run(&args, &log_dir).await,
        Command::Replay { month, tail } => replay(&args, &log_dir, month.clone(), *tail),
        Command::Publish {
            repo,
            hostname,
            session_id,
            event,
            detail,
        } => publish(&args, repo, hostname.clone(), session_id.clone(), event, detail.clone()).await,
        Command::Query {
            repo,
            hostname,
            since,
            limit,
        } => query_cmd(&args, repo, hostname.clone(), *since, *limit).await,
    }
}

async fn query_cmd(
    args: &Args,
    repo: &str,
    hostname: Option<String>,
    since: Option<DateTime<Utc>>,
    limit: Option<usize>,
) -> Result<()> {
    let client = async_nats::ConnectOptions::new()
        .name(args.id.clone())
        .connect(&args.nats_url)
        .await
        .with_context(|| format!("connecting to NATS at {}", args.nats_url))?;

    let request = SoulQuery {
        repo: repo.to_string(),
        hostname,
        since,
        limit,
    };
    let payload = serde_json::to_vec(&request)?;
    let response = tokio::time::timeout(
        std::time::Duration::from_secs(3),
        client.request(SOULS_QUERY_SUBJECT, payload.into()),
    )
    .await
    .context("souls.query timed out after 3s (is `b00t-historian run` active on this NATS server?)")?
    .with_context(|| format!("requesting {SOULS_QUERY_SUBJECT}"))?;

    let body: serde_json::Value = serde_json::from_slice(&response.payload)
        .context("parsing souls.query reply")?;
    println!("{}", serde_json::to_string_pretty(&body)?);
    Ok(())
}

async fn publish(
    args: &Args,
    repo: &str,
    hostname: Option<String>,
    session_id: Option<String>,
    event: &str,
    detail: Option<String>,
) -> Result<()> {
    let hostname = hostname.unwrap_or_else(|| {
        hostname::get()
            .map(|h| h.to_string_lossy().into_owned())
            .unwrap_or_else(|_| "unknown-host".to_string())
    });
    let session_id = session_id.unwrap_or_else(|| {
        format!("{}-{}", std::process::id(), Utc::now().timestamp())
    });

    let activity = SoulActivity {
        repo: repo.to_string(),
        hostname: hostname.clone(),
        session_id,
        event: event.to_string(),
        detail,
    };

    let client = async_nats::ConnectOptions::new()
        .name(args.id.clone())
        .connect(&args.nats_url)
        .await
        .with_context(|| format!("connecting to NATS at {}", args.nats_url))?;

    let subject = format!("souls.{}.{}.activity", activity.repo, hostname);
    let payload = serde_json::to_vec(&activity)?;
    client
        .publish(subject.clone(), payload.into())
        .await
        .with_context(|| format!("publishing to {subject}"))?;
    tokio::time::timeout(std::time::Duration::from_secs(1), client.flush())
        .await
        .context("NATS flush timed out after 1s")?
        .context("NATS flush failed")?;

    eprintln!("🥾 published soul activity to {subject}: {}", activity.event);
    Ok(())
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
    eprintln!("connected. subscribing to '{}' and '{}'", args.subject, SOULS_WILDCARD);

    let mut archive_sub = client
        .subscribe(args.subject.clone())
        .await
        .with_context(|| format!("subscribing to {}", args.subject))?;
    let mut souls_sub = client
        .subscribe(SOULS_WILDCARD)
        .await
        .with_context(|| format!("subscribing to {SOULS_WILDCARD}"))?;

    eprintln!(
        "camping on '{}' (archive) + '{}' (souls) — logging to {}/<YYYY>/<MM>/{}.ndjson, souls table '{}' (Ctrl-C to stop)",
        args.subject,
        SOULS_WILDCARD,
        log_dir.display(),
        args.basename,
        SOULS_TABLE,
    );

    let mut count: u64 = 0;
    loop {
        tokio::select! {
            maybe_msg = archive_sub.next() => {
                let Some(msg) = maybe_msg else { continue; };
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
            maybe_msg = souls_sub.next() => {
                let Some(msg) = maybe_msg else { continue; };
                let subject = msg.subject.to_string();
                if subject == SOULS_QUERY_SUBJECT {
                    let reply_to = msg.reply.clone();
                    let result = serde_json::from_slice::<SoulQuery>(&msg.payload)
                        .context("parsing souls.query request")
                        .and_then(|q| query_soul_activity(&q));
                    match (result, reply_to) {
                        (Ok(reply_body), Some(reply_subject)) => {
                            if let Err(e) = client
                                .publish(reply_subject.clone(), serde_json::to_vec(&reply_body)?.into())
                                .await
                            {
                                eprintln!("failed to reply to souls.query on {reply_subject}: {e}");
                            } else {
                                eprintln!("[{}] answered souls.query -> {reply_subject}", Utc::now().to_rfc3339());
                            }
                        }
                        (Ok(_), None) => eprintln!("souls.query received with no reply-to inbox, ignoring"),
                        (Err(e), _) => eprintln!("malformed souls.query: {e}"),
                    }
                } else if let Some((repo, hostname)) = expand_souls_subject(&subject) {
                    match serde_json::from_slice::<SoulActivity>(&msg.payload) {
                        Ok(activity) => match record_soul_activity(&activity) {
                            Ok(()) => eprintln!(
                                "[{}] soul activity {repo}/{hostname}: {} ({})",
                                Utc::now().to_rfc3339(),
                                activity.event,
                                activity.session_id
                            ),
                            Err(e) => eprintln!("failed to record soul activity on {subject}: {e}"),
                        },
                        Err(e) => eprintln!("malformed soul activity on {subject}: {e}"),
                    }
                } else {
                    eprintln!("unrecognized souls.* subject: {subject}");
                }
            }
            else => break,
        }
    }

    eprintln!(
        "subscription streams ended after {count} archived message(s) (server closed the sub or process is shutting down)"
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
