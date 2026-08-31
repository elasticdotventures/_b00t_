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
//! ## mesh mailbox — reliable delivery for `b00t.hive.mesh.node.*` (#1229)
//!
//! `NatsMeshNode::send()` (`b00t-lib-chat/src/mesh.rs`) publishes a
//! `MeshFrame::Direct` straight to `b00t.hive.mesh.node.<agent_id>` — plain
//! fire-and-forget NATS pub/sub, no ack, no persistence. A message to an
//! agent that isn't subscribed at that exact instant is simply lost. This
//! binary also camps on the whole `b00t.hive.mesh.>` tree (not only
//! `.node.>` — `Presence`/`DiscoveryQuery` frames travel on the separate
//! `discovery.presence`/`discovery.query` subjects under the same tree) and
//! buffers every `Direct` frame it sees into a `mesh_mailbox` soul table
//! (same DataFramerr/`SoulScope` primitive `souls_activity` uses, scoped
//! `system:mesh-mailbox` since this is hive-wide infrastructure state, not
//! tied to one repo/agent/project). Buffered mail for an agent is flushed —
//! republished verbatim to that agent's own node subject — the moment that
//! agent is next seen alive on the mesh (a presence heartbeat or a
//! discovery query), using a per-agent `FrameCursor` (the same durable
//! iterator pointer `b00t soul cursor` already exposes) rather than
//! deleting rows from the intentionally append-only `SoulDataFramerr`.
//! `b00t-historian mailbox-check --agent-id <id>` answers, over NATS
//! request-reply on `mailbox.check` (same shape as `souls.query`), how much
//! mail is currently buffered for an agent — read-only, does not flush.
//!
//! Usage:
//!   b00t-historian run
//!   b00t-historian run --subject 'hive.sm3ll-fung1.>' --nats-url nats://127.0.0.1:4222
//!   b00t-historian replay
//!   b00t-historian replay --month 2026-08 --tail 20
//!   b00t-historian publish --repo _b00t_ --event task_start --detail "reviewing PR #1147"
//!   b00t-historian mailbox-check --agent-id pi

use anyhow::{Context, Result};
use b00t_c0re_lib::soul_dataframerr::{FrameCursor, SoulColumn, SoulDataFramerr, SoulValue};
use b00t_chat::MeshFrame;
use b00t_chat::mesh::node_subject;
use b00t_cli::commands::provider::{get_provider, ComputeProvider};
use b00t_cli::commands::soul::{
    load_registry, load_soul_doc, load_soul_doc_scoped, with_registry, with_registry_scoped,
};
use b00t_cli::soul_scope::{ShardKind, SoulScope};
use b00t_cli::vultr_delegate::{
    self, AllowedRequesters, CapabilityForgeAuthorizer, DeprovisionRequest, ProvisionRequest,
    RequesterAuthorizer, StaticAllowlistAuthorizer, StatusRequest,
};
use capability_forge::identity::AgentKeyPair;
use chrono::{DateTime, Utc};
use clap::{Parser, Subcommand};
use futures::StreamExt;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::collections::BTreeMap;
use std::env;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::Arc;

const SOULS_TABLE: &str = "souls_activity";
const SOULS_QUERY_SUBJECT: &str = "souls.query";
const SOULS_WILDCARD: &str = "souls.>";
const DEFAULT_QUERY_LIMIT: usize = 50;

// 🤓 mesh mailbox (#1229) — see the module doc comment above for the full
// design. Camping on the whole `b00t.hive.mesh.>` tree (not only `.node.>`)
// is what actually lets this handler ever see a `Presence`/`DiscoveryQuery`
// frame — `b00t-lib-chat/src/mesh.rs` publishes those on the separate
// `discovery.presence`/`discovery.query` subjects, not on any `.node.*`
// subject.
const MESH_WILDCARD: &str = "b00t.hive.mesh.>";
// Mirrors `b00t_chat::mesh::NODE_PREFIX`, which is private to that crate —
// duplicated here (rather than widening that crate's public API for one
// caller) purely to strip an inbound subject back down to the agent id it
// was addressed to. `b00t_chat::mesh::node_subject()` (public) covers the
// reverse direction — building the subject to flush *to* — so the two only
// need to agree on this one literal.
const MESH_NODE_PREFIX: &str = "b00t.hive.mesh.node.";
const MAILBOX_TABLE: &str = "mesh_mailbox";
// `FrameCursor` (`b00t soul`'s own durable-iterator-pointer primitive) name
// prefix, one cursor per recipient agent id — reused instead of adding a
// delete/mutate path to the intentionally append-only `SoulDataFramerr`.
const MAILBOX_CURSOR_PREFIX: &str = "mailbox:";
// One-shot request-reply subject for `b00t-historian mailbox-check`. NATS
// request-reply (`souls.query`'s own shape), not a direct soul-table file
// read: the whole point (#1229 acceptance criteria) is letting a
// *different* process/host ask "do I have mail?" without filesystem/SSH
// access to wherever this historian's SOUL.tomllmd happens to live — the
// same reasoning `souls.query` exists for instead of every caller reading
// `souls_activity` off disk directly.
const MAILBOX_CHECK_SUBJECT: &str = "mailbox.check";

// 🤓 vultr delegation (see _b00t_/datums/PROVIDER-VULTR.provider.tomllmd,
//    "MEMOIZED" sections) — b00t-historian is the sole node whose
//    VULTR_API_KEY / allowlisted egress IP is expected to work, so it acts
//    as the single call site for the whole hive rather than every agent
//    needing its own key + allowlist entry. Pure orchestration logic lives
//    in b00t_cli::vultr_delegate; this file is just the NATS wiring, same
//    shape as the existing souls.query request-reply handling below.
const VULTR_PROVISION_SUBJECT: &str = "vultr.provision";
const VULTR_DEPROVISION_SUBJECT: &str = "vultr.deprovision";
const VULTR_STATUS_SUBJECT: &str = "vultr.status";
const VULTR_WILDCARD: &str = "vultr.>";

#[derive(Parser)]
#[clap(version, about = "b00t historian: durable NATS subject scribe")]
struct Args {
    /// Client identifier — shows up in NATS `connz` so a live connection can
    /// be told apart from other subscribers on the same server.
    #[clap(long, default_value = "historian")]
    id: String,

    #[clap(long, env = "NATS_URL", default_value = "nats://127.0.0.1:4222")]
    nats_url: String,

    /// NATS username. Falls back to B00T_HIVE_NATS_USER (the hive-wide
    /// credential convention already used by b00t-lib-chat) if unset.
    #[clap(long, env = "NATS_USER")]
    nats_user: Option<String>,

    /// NATS password. Falls back to B00T_HIVE_NATS_PASSWORD.
    #[clap(long, env = "NATS_PASSWORD")]
    nats_password: Option<String>,

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
    /// One-shot: ask the historian running `run` to provision a Vultr VPS
    /// on your behalf (NATS request-reply on `vultr.provision`). Requires
    /// `requested_by` to be on that historian's VULTR_DELEGATE_ALLOWLIST.
    Provision {
        /// Identifier checked against the historian's allowlist (e.g. your
        /// hostname — `fung1`, `sm3lly`).
        #[clap(long)]
        requested_by: String,
        /// Why — recorded on the instance's labels and in the durable log.
        #[clap(long)]
        purpose: String,
        /// Required: how long until auto-deprovision. Bounded by
        /// vultr_delegate::MAX_TTL_HOURS (one week).
        #[clap(long)]
        ttl_hours: u32,
        /// Vultr plan id (e.g. `vc2-4c-8gb`); defaults to the historian's
        /// process default (VULTR_PLAN env / vc2-1c-1gb) if omitted.
        #[clap(long)]
        plan: Option<String>,
        /// Vultr region code (e.g. `syd`); same default-fallback as `plan`.
        #[clap(long)]
        region: Option<String>,
    },
    /// One-shot: ask the historian to tear down a Vultr instance it
    /// provisioned (NATS request-reply on `vultr.deprovision`).
    Deprovision {
        #[clap(long)]
        requested_by: String,
        #[clap(long)]
        instance_id: String,
    },
    /// One-shot: ask the historian for the status of b00t-managed Vultr
    /// instance(s) (NATS request-reply on `vultr.status`). Not
    /// allowlist-gated — read-only.
    InstanceStatus {
        /// Omit to list all b00t-managed instances.
        #[clap(long)]
        instance_id: Option<String>,
    },
    /// One-shot: ask the historian how much mail (#1229 mesh mailbox) is
    /// buffered for an agent id that hasn't been seen on the mesh recently
    /// (NATS request-reply on `mailbox.check`). Read-only — does not
    /// flush; flushing happens automatically once that agent is seen alive
    /// again (a presence heartbeat or a discovery query).
    MailboxCheck {
        /// Agent id to check (same id `b00t.hive.mesh.node.<id>` addresses).
        #[clap(long)]
        agent_id: String,
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

/// Wire shape for a `mailbox.check` request.
#[derive(Debug, Serialize, Deserialize)]
struct MailboxCheckRequest {
    agent_id: String,
}

/// Wire shape for a `mailbox.check` reply.
#[derive(Debug, Serialize, Deserialize)]
struct MailboxCheckResponse {
    agent_id: String,
    pending: usize,
}

/// One buffered mailbox entry ready to redeliver to its recipient — the
/// original sender, plus the exact original wire bytes (UTF-8 lossy
/// decoded, same convention `LogRecord::payload` uses) so a flush
/// republishes byte-for-byte what was sent rather than re-encoding it.
#[derive(Debug, Clone, PartialEq)]
struct MailboxEntry {
    sender: String,
    payload: String,
}

fn connect_options(args: &Args, name: &str) -> async_nats::ConnectOptions {
    let user = args
        .nats_user
        .clone()
        .or_else(|| std::env::var("B00T_HIVE_NATS_USER").ok());
    let password = args
        .nats_password
        .clone()
        .or_else(|| std::env::var("B00T_HIVE_NATS_PASSWORD").ok());
    let opts = async_nats::ConnectOptions::new().name(name);
    match (user, password) {
        (Some(u), Some(p)) => opts.user_and_password(u, p),
        _ => opts,
    }
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

/// #1229 mesh mailbox: hive-wide infrastructure state, not tied to any one
/// repo/agent/project, so it lives in the `system` shard (#1102) rather
/// than the legacy/default unscoped shard `souls_activity` still uses.
fn mesh_scope() -> SoulScope {
    SoulScope::new(ShardKind::System, "mesh-mailbox")
}

fn mailbox_columns() -> Vec<SoulColumn> {
    vec![
        SoulColumn::parse("recipient:text").unwrap(),
        SoulColumn::parse("sender:text").unwrap(),
        SoulColumn::parse("payload:text").unwrap(),
    ]
}

fn mailbox_cursor_name(agent_id: &str) -> String {
    format!("{MAILBOX_CURSOR_PREFIX}{agent_id}")
}

/// Extract the recipient agent id from a subject the `b00t.hive.mesh.>`
/// wildcard delivered a message on, e.g. `b00t.hive.mesh.node.pi` ->
/// `Some("pi")`. `None` for anything not on a per-node inbox subject
/// (channel/discovery/gossip traffic under the same tree, which the
/// mailbox does not buffer).
fn agent_id_from_node_subject(subject: &str) -> Option<&str> {
    subject
        .strip_prefix(MESH_NODE_PREFIX)
        .filter(|id| !id.is_empty())
}

/// Pure: scan `df` forward from `cursor`'s watermark, collecting every row
/// addressed to `agent_id`, and advance `cursor` past everything scanned
/// (matched or not) — the same durable-iterator-pointer primitive `b00t
/// soul cursor` already exposes (`FrameCursor::next`), reused instead of
/// adding a delete/mutate path to the intentionally append-only
/// `SoulDataFramerr`. A no-op (empty result, cursor untouched) when there
/// is nothing new past the watermark — safe to call opportunistically.
fn flush_pending_for_agent(
    df: &SoulDataFramerr,
    cursor: &mut FrameCursor,
    agent_id: &str,
) -> Vec<MailboxEntry> {
    let mut out = Vec::new();
    while let Some(row) = cursor.next(df) {
        if row.fields.get("recipient").and_then(SoulValue::as_str) == Some(agent_id) {
            out.push(MailboxEntry {
                sender: row
                    .fields
                    .get("sender")
                    .and_then(SoulValue::as_str)
                    .unwrap_or_default()
                    .to_string(),
                payload: row
                    .fields
                    .get("payload")
                    .and_then(SoulValue::as_str)
                    .unwrap_or_default()
                    .to_string(),
            });
        }
    }
    out
}

/// Pure, read-only: how many rows are buffered for `agent_id` past
/// `cursor`'s watermark, without consuming/advancing anything — what
/// `mailbox-check` reports. Deliberately distinct from
/// `flush_pending_for_agent`, which both reports AND consumes; a status
/// check must never itself trigger a flush.
fn pending_count_for_agent(df: &SoulDataFramerr, cursor: &FrameCursor, agent_id: &str) -> usize {
    df.rows_after(cursor.frame_id)
        .filter(|row| row.fields.get("recipient").and_then(SoulValue::as_str) == Some(agent_id))
        .count()
}

/// Persist one `MeshFrame::Direct` frame into the mesh mailbox table, keyed
/// by the recipient agent id extracted from the subject it arrived on.
fn record_mailbox_message(recipient: &str, sender: &str, raw_payload: &str) -> Result<()> {
    with_registry_scoped(Some(&mesh_scope()), |reg| {
        let df = reg.get_or_create(MAILBOX_TABLE, mailbox_columns());
        let mut fields = BTreeMap::new();
        fields.insert(
            "recipient".to_string(),
            SoulValue::Text(recipient.to_string()),
        );
        fields.insert("sender".to_string(), SoulValue::Text(sender.to_string()));
        fields.insert(
            "payload".to_string(),
            SoulValue::Text(raw_payload.to_string()),
        );
        df.insert(fields)?;
        Ok(())
    })
}

/// Flush (report-and-consume) every mailbox entry buffered for `agent_id`,
/// advancing its durable cursor so nothing is redelivered on the next
/// flush. Called whenever that agent makes any noise on the mesh (a
/// presence heartbeat or a discovery query) — the concrete, simplest
/// answer to "when does historian flush" that #1229 left open:
/// opportunistically on next-seen-alive, not on a schedule.
fn flush_mailbox_for_agent(agent_id: &str) -> Result<Vec<MailboxEntry>> {
    let mut pending = Vec::new();
    with_registry_scoped(Some(&mesh_scope()), |reg| {
        // `SoulDataFramerrRegistry::get_or_create` takes `&mut self`, so its
        // returned `&mut SoulDataFramerr` can't stay alive alongside a
        // second `&mut reg.cursors` borrow (E0499) — do the two mutable
        // touches sequentially instead: ensure the table exists, clone the
        // cursor out (or default it) via an immutable borrow, run the pure
        // scan against an immutable `&SoulDataFramerr`, then write the
        // advanced cursor back.
        reg.get_or_create(MAILBOX_TABLE, mailbox_columns());
        let cursor_name = mailbox_cursor_name(agent_id);
        let mut cursor = reg
            .cursors
            .get(&cursor_name)
            .cloned()
            .unwrap_or_else(|| FrameCursor::new(MAILBOX_TABLE));
        {
            let df = reg
                .tables
                .get(MAILBOX_TABLE)
                .expect("mesh_mailbox table just created above");
            pending = flush_pending_for_agent(df, &mut cursor, agent_id);
        }
        reg.cursors.insert(cursor_name, cursor);
        Ok(())
    })?;
    Ok(pending)
}

/// Answer `mailbox.check` from the local mailbox table — read-only, does
/// not go through `with_registry_scoped` (which always saves back), same
/// reasoning as `query_soul_activity`.
fn count_pending_mailbox(agent_id: &str) -> Result<usize> {
    let doc = load_soul_doc_scoped(Some(&mesh_scope()))?;
    let reg = load_registry(&doc)?;
    let count = match reg.tables.get(MAILBOX_TABLE) {
        None => 0,
        Some(df) => {
            let default_cursor;
            let cursor = match reg.cursors.get(&mailbox_cursor_name(agent_id)) {
                Some(c) => c,
                None => {
                    default_cursor = FrameCursor::new(MAILBOX_TABLE);
                    &default_cursor
                }
            };
            pending_count_for_agent(df, cursor, agent_id)
        }
    };
    Ok(count)
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
        Command::Provision {
            requested_by,
            purpose,
            ttl_hours,
            plan,
            region,
        } => {
            provision_cmd(
                &args,
                requested_by.clone(),
                purpose.clone(),
                *ttl_hours,
                plan.clone(),
                region.clone(),
            )
            .await
        }
        Command::Deprovision {
            requested_by,
            instance_id,
        } => deprovision_cmd(&args, requested_by.clone(), instance_id.clone()).await,
        Command::InstanceStatus { instance_id } => {
            instance_status_cmd(&args, instance_id.clone()).await
        }
        Command::MailboxCheck { agent_id } => mailbox_check_cmd(&args, agent_id.clone()).await,
    }
}

/// Shared request-reply helper for the three vultr.* one-shot commands —
/// same connect/timeout/pretty-print shape as `query_cmd`.
async fn vultr_request_reply<Req: Serialize, Resp: for<'de> Deserialize<'de>>(
    args: &Args,
    subject: &str,
    request: &Req,
) -> Result<Resp> {
    let client = async_nats::ConnectOptions::new()
        .name(args.id.clone())
        .connect(&args.nats_url)
        .await
        .with_context(|| format!("connecting to NATS at {}", args.nats_url))?;

    let payload = serde_json::to_vec(request)?;
    let response = tokio::time::timeout(
        std::time::Duration::from_secs(30),
        client.request(subject.to_string(), payload.into()),
    )
    .await
    .with_context(|| {
        format!("{subject} timed out after 30s (is `b00t-historian run` active on this NATS server?)")
    })?
    .with_context(|| format!("requesting {subject}"))?;

    let body: serde_json::Value = serde_json::from_slice(&response.payload)
        .with_context(|| format!("parsing {subject} reply"))?;
    if let Some(err) = body.get("error").and_then(|e| e.as_str()) {
        anyhow::bail!("{subject} failed: {err}");
    }
    println!("{}", serde_json::to_string_pretty(&body)?);
    serde_json::from_value(body).with_context(|| format!("deserializing {subject} reply"))
}

async fn provision_cmd(
    args: &Args,
    requested_by: String,
    purpose: String,
    ttl_hours: u32,
    plan: Option<String>,
    region: Option<String>,
) -> Result<()> {
    let req = ProvisionRequest {
        requested_by,
        purpose,
        ttl_hours,
        plan,
        region,
    };
    let _: vultr_delegate::ProvisionResponse =
        vultr_request_reply(args, VULTR_PROVISION_SUBJECT, &req).await?;
    Ok(())
}

async fn deprovision_cmd(args: &Args, requested_by: String, instance_id: String) -> Result<()> {
    let req = DeprovisionRequest {
        requested_by,
        instance_id,
    };
    let _: vultr_delegate::DeprovisionResponse =
        vultr_request_reply(args, VULTR_DEPROVISION_SUBJECT, &req).await?;
    Ok(())
}

async fn instance_status_cmd(args: &Args, instance_id: Option<String>) -> Result<()> {
    let req = StatusRequest { instance_id };
    let _: vultr_delegate::StatusResponse =
        vultr_request_reply(args, VULTR_STATUS_SUBJECT, &req).await?;
    Ok(())
}

/// One-shot: ask `mailbox.check` over NATS request-reply and print the
/// answer — same zero-dependency shape as `query_cmd`/`souls.query`, and
/// reuses the generic `vultr_request_reply` helper (nothing vultr-specific
/// about it despite the name: connect+timeout(30s)+request+parse+error-check).
async fn mailbox_check_cmd(args: &Args, agent_id: String) -> Result<()> {
    let req = MailboxCheckRequest { agent_id };
    let _: MailboxCheckResponse = vultr_request_reply(args, MAILBOX_CHECK_SUBJECT, &req).await?;
    Ok(())
}

async fn query_cmd(
    args: &Args,
    repo: &str,
    hostname: Option<String>,
    since: Option<DateTime<Utc>>,
    limit: Option<usize>,
) -> Result<()> {
    let client = connect_options(args, &args.id)
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

    let client = connect_options(args, &args.id)
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

/// Sends a NATS request-reply response for one of the vultr.* subjects —
/// success serializes `T` directly; failure sends `{"error": "..."}"` so
/// `vultr_request_reply` (the CLI-side one-shot helper) can tell the two
/// apart without a separate envelope type.
async fn vultr_reply<T: Serialize>(
    client: &async_nats::Client,
    reply_to: Option<async_nats::Subject>,
    label: &str,
    result: Result<T>,
) {
    let Some(reply_subject) = reply_to else {
        eprintln!("{label} received with no reply-to inbox, ignoring");
        return;
    };
    let body = match result {
        Ok(v) => serde_json::to_value(v)
            .unwrap_or_else(|e| json!({"error": format!("serializing {label} reply: {e}")})),
        Err(e) => json!({"error": e.to_string()}),
    };
    let payload = match serde_json::to_vec(&body) {
        Ok(p) => p,
        Err(e) => {
            eprintln!("failed to encode {label} reply: {e}");
            return;
        }
    };
    if let Err(e) = client.publish(reply_subject.clone(), payload.into()).await {
        eprintln!("failed to reply to {label} on {reply_subject}: {e}");
    } else {
        eprintln!("[{}] answered {label} -> {reply_subject}", Utc::now().to_rfc3339());
    }
}

/// Flush an agent's buffered mailbox and republish each entry verbatim to
/// its own node inbox — called the moment that agent is seen alive again
/// (a presence heartbeat or a discovery query), per #1229's mailbox design.
async fn flush_mailbox_and_publish(client: &async_nats::Client, agent_id: &str) {
    let entries = match flush_mailbox_for_agent(agent_id) {
        Ok(entries) => entries,
        Err(e) => {
            eprintln!("mailbox flush failed for {agent_id}: {e}");
            return;
        }
    };
    if entries.is_empty() {
        return;
    }
    let subject = node_subject(agent_id);
    for entry in entries {
        match client
            .publish(subject.clone(), entry.payload.clone().into_bytes().into())
            .await
        {
            Ok(()) => eprintln!(
                "[{}] flushed mailbox: {} -> {subject}",
                Utc::now().to_rfc3339(),
                entry.sender
            ),
            Err(e) => eprintln!(
                "failed to flush mailbox message from {} to {subject}: {e}",
                entry.sender
            ),
        }
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
    let client = connect_options(args, &args.id)
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
    let mut vultr_sub = client
        .subscribe(VULTR_WILDCARD)
        .await
        .with_context(|| format!("subscribing to {VULTR_WILDCARD}"))?;
    let mut mesh_sub = client
        .subscribe(MESH_WILDCARD)
        .await
        .with_context(|| format!("subscribing to {MESH_WILDCARD}"))?;
    let mut mailbox_check_sub = client
        .subscribe(MAILBOX_CHECK_SUBJECT)
        .await
        .with_context(|| format!("subscribing to {MAILBOX_CHECK_SUBJECT}"))?;

    // 🤓 VULTR_API_KEY is expected to be UNSET on most hosts running this
    // binary (only b00t-node itself is meant to hold it — see
    // vultr_delegate.rs's module doc). Missing it must not crash the whole
    // souls/archive scribe, which has nothing to do with vultr delegation —
    // log once and reply with a clear error to every vultr.* request instead.
    let vultr_provider: Option<Arc<dyn ComputeProvider>> = match get_provider("vultr") {
        Ok(p) => Some(Arc::from(p)),
        Err(e) => {
            eprintln!(
                "vultr delegation disabled on this historian: {e} (vultr.* requests will get an error reply)"
            );
            None
        }
    };
    // 🤓 Authorization backend selection: capability-forge (if
    // VULTR_DELEGATE_CAPFORGE_AGENT_SEED is set — see
    // vultr_delegate::CapabilityForgeAuthorizer's doc comment for exactly
    // what this does and doesn't verify) upgrades from the original static
    // env-var allowlist to a live, tiered, judged policy. Falls back to the
    // static allowlist if the seed is missing OR malformed — a bad seed
    // must not silently disable authorization entirely.
    let vultr_authorizer: Box<dyn RequesterAuthorizer> = match env::var("VULTR_DELEGATE_CAPFORGE_AGENT_SEED")
    {
        Ok(seed) => {
            let agent_id = env::var("VULTR_DELEGATE_CAPFORGE_AGENT_ID")
                .unwrap_or_else(|_| "b00t-historian".to_string());
            match AgentKeyPair::from_seed(&seed) {
                Ok(keypair) => {
                    eprintln!("vultr delegate authorization: capability-forge (agent_id={agent_id})");
                    Box::new(CapabilityForgeAuthorizer::new(client.clone(), agent_id, keypair))
                }
                Err(e) => {
                    eprintln!(
                        "VULTR_DELEGATE_CAPFORGE_AGENT_SEED is set but invalid ({e}) —                          falling back to the static allowlist"
                    );
                    Box::new(StaticAllowlistAuthorizer(AllowedRequesters::from_env()))
                }
            }
        }
        Err(_) => Box::new(StaticAllowlistAuthorizer(AllowedRequesters::from_env())),
    };
    let vultr_max_instances = vultr_delegate::max_instances_from_env();

    eprintln!(
        "camping on '{}' (archive) + '{}' (souls) + '{}' (vultr, {}) + '{}' (mesh mailbox) + '{}' (mailbox check) — logging to {}/<YYYY>/<MM>/{}.ndjson, souls table '{}', mailbox table '{}' (Ctrl-C to stop)",
        args.subject,
        SOULS_WILDCARD,
        VULTR_WILDCARD,
        if vultr_provider.is_some() { "enabled" } else { "disabled: no VULTR_API_KEY" },
        MESH_WILDCARD,
        MAILBOX_CHECK_SUBJECT,
        log_dir.display(),
        args.basename,
        SOULS_TABLE,
        MAILBOX_TABLE,
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
            maybe_msg = vultr_sub.next() => {
                let Some(msg) = maybe_msg else { continue; };
                let subject = msg.subject.to_string();
                let reply_to = msg.reply.clone();
                match subject.as_str() {
                    VULTR_PROVISION_SUBJECT => {
                        let result: Result<vultr_delegate::ProvisionResponse> = async {
                            let req: ProvisionRequest = serde_json::from_slice(&msg.payload)
                                .context("parsing vultr.provision request")?;
                            let provider = vultr_provider
                                .as_deref()
                                .context("vultr provider not configured on this historian (VULTR_API_KEY unset)")?;
                            let resp = vultr_delegate::handle_provision(
                                &req,
                                provider,
                                vultr_authorizer.as_ref(),
                                vultr_max_instances,
                            )
                            .await?;
                            if let Some(p) = vultr_provider.clone() {
                                vultr_delegate::spawn_ttl_teardown(
                                    resp.instance_id.clone(),
                                    std::time::Duration::from_secs(u64::from(req.ttl_hours) * 3600),
                                    p,
                                );
                            }
                            Ok(resp)
                        }
                        .await;
                        vultr_reply(&client, reply_to, VULTR_PROVISION_SUBJECT, result).await;
                    }
                    VULTR_DEPROVISION_SUBJECT => {
                        let result: Result<vultr_delegate::DeprovisionResponse> = async {
                            let req: DeprovisionRequest = serde_json::from_slice(&msg.payload)
                                .context("parsing vultr.deprovision request")?;
                            let provider = vultr_provider
                                .as_deref()
                                .context("vultr provider not configured on this historian (VULTR_API_KEY unset)")?;
                            vultr_delegate::handle_deprovision(&req, provider, vultr_authorizer.as_ref()).await
                        }
                        .await;
                        vultr_reply(&client, reply_to, VULTR_DEPROVISION_SUBJECT, result).await;
                    }
                    VULTR_STATUS_SUBJECT => {
                        let result: Result<vultr_delegate::StatusResponse> = async {
                            let req: StatusRequest = serde_json::from_slice(&msg.payload)
                                .context("parsing vultr.status request")?;
                            let provider = vultr_provider
                                .as_deref()
                                .context("vultr provider not configured on this historian (VULTR_API_KEY unset)")?;
                            vultr_delegate::handle_status(&req, provider).await
                        }
                        .await;
                        vultr_reply(&client, reply_to, VULTR_STATUS_SUBJECT, result).await;
                    }
                    other => eprintln!("unrecognized vultr.* subject: {other}"),
                }
            }
            maybe_msg = mesh_sub.next() => {
                let Some(msg) = maybe_msg else { continue; };
                let subject = msg.subject.to_string();
                let raw_payload = String::from_utf8_lossy(&msg.payload).into_owned();
                match serde_json::from_slice::<MeshFrame>(&msg.payload) {
                    Ok(MeshFrame::Direct(chat_msg)) => match agent_id_from_node_subject(&subject) {
                        Some(agent_id) => match record_mailbox_message(agent_id, &chat_msg.sender, &raw_payload) {
                            Ok(()) => eprintln!(
                                "[{}] buffered mesh mail {} -> {agent_id} (offline, will flush on next presence/discovery)",
                                Utc::now().to_rfc3339(),
                                chat_msg.sender
                            ),
                            Err(e) => eprintln!("failed to buffer mesh mail on {subject}: {e}"),
                        },
                        None => eprintln!(
                            "mesh Direct frame on non-node subject {subject}, ignoring for mailbox"
                        ),
                    },
                    Ok(MeshFrame::Presence(presence)) => {
                        flush_mailbox_and_publish(&client, &presence.agent_id).await;
                    }
                    Ok(MeshFrame::DiscoveryQuery { from, .. }) => {
                        flush_mailbox_and_publish(&client, &from).await;
                    }
                    Ok(_) => { /* Broadcast/DiscoveryReply/Gossip: not mailbox-relevant */ }
                    Err(e) => eprintln!("malformed mesh frame on {subject}: {e}"),
                }
            }
            maybe_msg = mailbox_check_sub.next() => {
                let Some(msg) = maybe_msg else { continue; };
                let reply_to = msg.reply.clone();
                let result: Result<MailboxCheckResponse> =
                    serde_json::from_slice::<MailboxCheckRequest>(&msg.payload)
                        .context("parsing mailbox.check request")
                        .and_then(|req| {
                            let pending = count_pending_mailbox(&req.agent_id)?;
                            Ok(MailboxCheckResponse {
                                agent_id: req.agent_id,
                                pending,
                            })
                        });
                vultr_reply(&client, reply_to, MAILBOX_CHECK_SUBJECT, result).await;
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


#[cfg(test)]
mod mailbox_tests {
    use super::*;

    fn sample_row_fields(
        recipient: &str,
        sender: &str,
        payload: &str,
    ) -> BTreeMap<String, SoulValue> {
        let mut fields = BTreeMap::new();
        fields.insert(
            "recipient".to_string(),
            SoulValue::Text(recipient.to_string()),
        );
        fields.insert("sender".to_string(), SoulValue::Text(sender.to_string()));
        fields.insert("payload".to_string(), SoulValue::Text(payload.to_string()));
        fields
    }

    #[test]
    fn agent_id_from_node_subject_strips_prefix() {
        assert_eq!(
            agent_id_from_node_subject("b00t.hive.mesh.node.pi"),
            Some("pi")
        );
        assert_eq!(agent_id_from_node_subject("b00t.hive.mesh.node."), None);
        assert_eq!(
            agent_id_from_node_subject("b00t.hive.mesh.channel.general"),
            None
        );
        assert_eq!(
            agent_id_from_node_subject("b00t.hive.mesh.discovery.query"),
            None
        );
    }

    #[test]
    fn flush_pending_for_agent_only_matches_recipient_and_advances_cursor() {
        let mut df = SoulDataFramerr::new(MAILBOX_TABLE, mailbox_columns());
        df.insert(sample_row_fields("pi", "sm3lly", "frame-1"))
            .unwrap();
        df.insert(sample_row_fields("fung1", "sm3lly", "frame-2"))
            .unwrap();
        df.insert(sample_row_fields("pi", "vultr1", "frame-3"))
            .unwrap();

        let mut cursor = FrameCursor::new(MAILBOX_TABLE);
        let delivered = flush_pending_for_agent(&df, &mut cursor, "pi");

        assert_eq!(delivered.len(), 2);
        assert_eq!(delivered[0].sender, "sm3lly");
        assert_eq!(delivered[0].payload, "frame-1");
        assert_eq!(delivered[1].sender, "vultr1");
        assert_eq!(delivered[1].payload, "frame-3");
        // cursor advances past every row scanned, not just matches
        assert_eq!(cursor.frame_id, 3);
        assert!(cursor.at_eof(&df));
    }

    #[test]
    fn flush_pending_for_agent_is_idempotent_with_nothing_new() {
        let mut df = SoulDataFramerr::new(MAILBOX_TABLE, mailbox_columns());
        df.insert(sample_row_fields("pi", "sm3lly", "frame-1"))
            .unwrap();
        let mut cursor = FrameCursor::new(MAILBOX_TABLE);

        let first = flush_pending_for_agent(&df, &mut cursor, "pi");
        assert_eq!(first.len(), 1);

        let second = flush_pending_for_agent(&df, &mut cursor, "pi");
        assert!(
            second.is_empty(),
            "flushing again with nothing new must not re-deliver"
        );
    }

    #[test]
    fn flush_pending_for_agent_does_not_redeliver_after_unrelated_row() {
        let mut df = SoulDataFramerr::new(MAILBOX_TABLE, mailbox_columns());
        df.insert(sample_row_fields("pi", "sm3lly", "frame-1"))
            .unwrap();
        let mut cursor = FrameCursor::new(MAILBOX_TABLE);
        assert_eq!(flush_pending_for_agent(&df, &mut cursor, "pi").len(), 1);

        // a message arrives for someone else while "pi" stays quiet
        df.insert(sample_row_fields("fung1", "sm3lly", "frame-2"))
            .unwrap();
        assert!(flush_pending_for_agent(&df, &mut cursor, "pi").is_empty());

        // a new message for "pi" arrives — only the new one comes back
        df.insert(sample_row_fields("pi", "sm3lly", "frame-3"))
            .unwrap();
        let third = flush_pending_for_agent(&df, &mut cursor, "pi");
        assert_eq!(third.len(), 1);
        assert_eq!(third[0].payload, "frame-3");
    }

    #[test]
    fn pending_count_for_agent_does_not_consume() {
        let mut df = SoulDataFramerr::new(MAILBOX_TABLE, mailbox_columns());
        df.insert(sample_row_fields("pi", "sm3lly", "frame-1"))
            .unwrap();
        df.insert(sample_row_fields("pi", "sm3lly", "frame-2"))
            .unwrap();
        df.insert(sample_row_fields("fung1", "sm3lly", "frame-3"))
            .unwrap();
        let cursor = FrameCursor::new(MAILBOX_TABLE);

        assert_eq!(pending_count_for_agent(&df, &cursor, "pi"), 2);
        assert_eq!(pending_count_for_agent(&df, &cursor, "fung1"), 1);
        assert_eq!(pending_count_for_agent(&df, &cursor, "nobody"), 0);
        // a second, independent look must see the exact same count — no
        // hidden mutation through the shared &cursor
        assert_eq!(pending_count_for_agent(&df, &cursor, "pi"), 2);
    }

    #[test]
    fn pending_count_matches_what_flush_would_deliver() {
        let mut df = SoulDataFramerr::new(MAILBOX_TABLE, mailbox_columns());
        df.insert(sample_row_fields("pi", "a", "1")).unwrap();
        df.insert(sample_row_fields("pi", "b", "2")).unwrap();
        let cursor_for_count = FrameCursor::new(MAILBOX_TABLE);
        let mut cursor_for_flush = FrameCursor::new(MAILBOX_TABLE);

        let count = pending_count_for_agent(&df, &cursor_for_count, "pi");
        let delivered = flush_pending_for_agent(&df, &mut cursor_for_flush, "pi");
        assert_eq!(count, delivered.len());
    }
}
