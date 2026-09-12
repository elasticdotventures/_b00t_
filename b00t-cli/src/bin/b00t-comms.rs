//! `b00t-comms`: thin CLI wrapping `b00t_chat::mesh::NatsMeshNode` per the
//! `_b00t_/b00t-comms.agent.toml` datum spec (#1299 / task #210).
//!
//! This binary intentionally invents nothing: no new meeting state, no
//! registration/authz subsystem, no new NATS subjects. It only:
//!
//!   1. reads the already-declared `[b00t.agent.ipc]` / `[b00t.agent.hive]`
//!      fields out of `b00t-comms.agent.toml` (`nats_url`, `subject_prefix`,
//!      `presence_interval_secs`, `presence_ttl_secs`, `gossip_max_hops`),
//!   2. constructs a [`b00t_chat::mesh::MeshNodeConfig`] directly from them,
//!   3. drives the existing, tested [`b00t_chat::mesh::NatsMeshNode`]
//!      (`announce`/`discover`/`join`/`send`/`recv`) — the mesh transport
//!      itself is fully implemented in `b00t-lib-chat/src/mesh.rs`; nothing
//!      here reimplements presence, gossip, or discovery.
//!
//! Subjects live entirely under `b00t.hive.mesh.*` as already defined by
//! `NatsMeshNode` (`node_subject`/`channel_subject`/discovery/gossip
//! constants) — this binary does not construct or publish to any subject
//! string of its own.
//!
//! Captain convention: per the datum's `[b00t.agent.crew]` field and the
//! issue's v1 scope, "captain" is convention-only (first announcer/joiner of
//! a channel) — no new durable state machinery is added here to enforce or
//! persist it.
//!
//! Usage:
//!   b00t-comms announce
//!   b00t-comms discover [--timeout-ms 1500]
//!   b00t-comms join <channel>            # subscribe + print frames forever
//!   b00t-comms send <to-agent> <message>
//!   b00t-comms broadcast <channel> <message>

use anyhow::{Context, Result};
use b00t_chat::ledgrrr::{Ledgrrr, MockLedgrrr};
use b00t_chat::mesh::{MeshFrame, MeshNodeConfig, NatsMeshNode};
use b00t_chat::message::ChatMessage;
use clap::{Parser, Subcommand};
use serde::Deserialize;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

/// Mirrors only the fields this binary actually reads from
/// `b00t-comms.agent.toml` — a thin, local view of the datum, not a new
/// schema. All values already exist in the committed datum file; nothing
/// here is invented.
#[derive(Debug, Deserialize)]
struct CommsDatum {
    b00t: CommsDatumB00t,
}

#[derive(Debug, Deserialize)]
struct CommsDatumB00t {
    agent: CommsDatumAgent,
}

#[derive(Debug, Deserialize)]
struct CommsDatumAgent {
    pid: String,
    role: String,
    ipc: CommsDatumIpc,
    hive: CommsDatumHive,
    finops: Option<CommsDatumFinops>,
}

#[derive(Debug, Deserialize)]
struct CommsDatumIpc {
    nats_url: String,
    subject_prefix: String,
}

#[derive(Debug, Deserialize)]
struct CommsDatumHive {
    // Read for completeness/documentation of the datum's declared fields;
    // `MeshNodeConfig` has no `with_presence_interval`/`with_presence_ttl`
    // builder today (only `DEFAULT_PRESENCE_INTERVAL`/`DEFAULT_PRESENCE_TTL`,
    // which already match these datum values: 10s/30s) — so these two are
    // not yet wired into `MeshNodeConfig`. Not adding new builder methods to
    // `NatsMeshNode` keeps this a thin wrapper; flagged in the coordination
    // comment as an open follow-up rather than invented here.
    #[allow(dead_code)]
    presence_interval_secs: u64,
    #[allow(dead_code)]
    presence_ttl_secs: u64,
    gossip_max_hops: u8,
}

#[derive(Debug, Deserialize)]
struct CommsDatumFinops {
    project: Option<String>,
}

#[derive(Parser)]
#[clap(version, about = "b00t-comms: thin CLI wrapping the NatsMeshNode hive mesh")]
struct Args {
    /// Path to the b00t-comms.agent.toml datum. Defaults to
    /// `$_B00T_Path/b00t-comms.agent.toml` (`_B00T_Path` default:
    /// `~/.dotfiles/_b00t_`), matching the rest of the b00t-cli datum
    /// resolution convention.
    #[clap(long)]
    datum: Option<PathBuf>,

    /// Override the agent id this node presents as (defaults to the
    /// datum's `[b00t.agent].pid`).
    #[clap(long)]
    agent_id: Option<String>,

    /// Override nats_url from the datum (falls back to NATS_URL env, then
    /// the datum's `[b00t.agent.ipc].nats_url`).
    #[clap(long, env = "NATS_URL")]
    nats_url: Option<String>,

    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Gossip + heartbeat a presence announcement, then exit.
    Announce,
    /// Publish a discovery query and print live peers found within the timeout.
    Discover {
        #[clap(long, default_value_t = 1500)]
        timeout_ms: u64,
    },
    /// Join a pub/sub channel: announce presence, subscribe, and print every
    /// inbound mesh frame forever (ctrl-c to stop). By convention (v1, no new
    /// registration subsystem), the first agent to `join` a channel is its
    /// captain-by-convention.
    Join { channel: String },
    /// Send a direct point-to-point message to another agent's inbox.
    Send { to_agent: String, message: String },
    /// Broadcast a message on a pub/sub channel.
    Broadcast { channel: String, message: String },
}

fn resolve_datum_path(explicit: &Option<PathBuf>) -> PathBuf {
    if let Some(p) = explicit {
        return p.clone();
    }
    let base = std::env::var("_B00T_Path").unwrap_or_else(|_| "~/.dotfiles/_b00t_".to_string());
    let expanded = shellexpand::tilde(&base).to_string();
    PathBuf::from(expanded).join("b00t-comms.agent.toml")
}

fn load_datum(path: &PathBuf) -> Result<CommsDatum> {
    let content = std::fs::read_to_string(path)
        .with_context(|| format!("reading b00t-comms datum at {}", path.display()))?;
    let datum: CommsDatum = toml::from_str(&content)
        .with_context(|| format!("parsing b00t-comms datum at {}", path.display()))?;
    Ok(datum)
}

#[tokio::main]
async fn main() -> Result<()> {
    let args = Args::parse();
    let datum_path = resolve_datum_path(&args.datum);
    let datum = load_datum(&datum_path)?;

    let subject_prefix = datum.b00t.agent.ipc.subject_prefix.clone();
    if subject_prefix != "b00t.hive.mesh" {
        eprintln!(
            "⚠️  datum subject_prefix is '{subject_prefix}', but NatsMeshNode's \
             node/channel/discovery/gossip subject constants are hardcoded to \
             'b00t.hive.mesh' — this binary does not override them (no new \
             subjects invented); the two must agree for this wrapper to be \
             correct."
        );
    }

    let agent_id = args
        .agent_id
        .clone()
        .unwrap_or_else(|| datum.b00t.agent.pid.clone());
    let nats_url = args
        .nats_url
        .clone()
        .unwrap_or_else(|| datum.b00t.agent.ipc.nats_url.clone());
    let project = datum
        .b00t
        .agent
        .finops
        .as_ref()
        .and_then(|f| f.project.clone())
        .unwrap_or_else(|| "b00t-comms".to_string());

    let ledger: Arc<dyn Ledgrrr> =
        match std::env::var("B00T_LEDGER").ok().filter(|s| !s.is_empty()) {
            Some(path) => Arc::new(MockLedgrrr::file(&path).unwrap_or_else(|_| MockLedgrrr::mock())),
            None => Arc::new(MockLedgrrr::mock()),
        };

    let config = MeshNodeConfig::new(agent_id.clone(), nats_url)
        .with_role(datum.b00t.agent.role.clone())
        .with_skills(vec!["nats".to_string()])
        .with_project(project.clone())
        .with_gossip_max_hops(datum.b00t.agent.hive.gossip_max_hops)
        .with_ledgrrr(ledger.clone());

    let node = NatsMeshNode::new(config);
    node.connect().await.context("connecting mesh node")?;

    match &args.command {
        Command::Announce => {
            node.announce().await.context("announce")?;
            println!("🥾 {agent_id} announced presence on b00t.hive.mesh");
        }
        Command::Discover { timeout_ms } => {
            let peers = node
                .discover_with_timeout(Duration::from_millis(*timeout_ms))
                .await
                .context("discover")?;
            println!("discovered {} peer(s):", peers.len());
            for p in &peers {
                println!("  - {} ({})", p.agent_id, p.endpoint_uri);
            }
        }
        Command::Join { channel } => {
            node.join(channel).await.context("join channel")?;
            node.start_presence().await;
            println!(
                "🥾 {agent_id} joined '{channel}' (captain-by-convention if first here; \
                 ctrl-c to stop)"
            );
            while let Some(frame) = node.recv().await.context("recv")? {
                print_frame(&frame);
            }
        }
        Command::Send { to_agent, message } => {
            let msg = ChatMessage::new(channel_for_direct(to_agent), &agent_id, message.clone());
            node.send(to_agent, &msg).await.context("send")?;
            println!("🥾 sent direct message to {to_agent}");
        }
        Command::Broadcast { channel, message } => {
            let msg = ChatMessage::new(channel.clone(), &agent_id, message.clone());
            node.publish(channel, &msg).await.context("broadcast")?;
            println!("🥾 broadcast on channel '{channel}'");
        }
    }

    let codes = ledger.codes_for(&project);
    if !codes.is_empty() {
        println!("finops: {project} -> {} receipt(s) minted", codes.len());
    }

    node.close().await.ok();
    Ok(())
}

/// `ChatMessage::new` requires a channel; for a direct send there is no
/// pub/sub channel, so we label it with the recipient's inbox name purely
/// for the message's own `channel` field (NatsMeshNode does not use this
/// value for routing — routing is by NATS subject, computed internally by
/// `node_subject`/`channel_subject`, which this binary never constructs
/// itself).
fn channel_for_direct(to_agent: &str) -> String {
    format!("direct:{to_agent}")
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Parses the real, committed datum file — proves this binary's local
    /// `CommsDatum` view matches `_b00t_/b00t-comms.agent.toml` as it exists
    /// today, without needing a NATS server.
    #[test]
    fn parses_real_comms_datum() {
        let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../_b00t_/b00t-comms.agent.toml");
        let datum = load_datum(&path).expect("b00t-comms.agent.toml should parse");
        assert_eq!(datum.b00t.agent.pid, "b00t-comms-001");
        assert_eq!(datum.b00t.agent.role, "b00t-comms");
        assert_eq!(datum.b00t.agent.ipc.nats_url, "nats://localhost:4222");
        assert_eq!(datum.b00t.agent.ipc.subject_prefix, "b00t.hive.mesh");
        assert_eq!(datum.b00t.agent.hive.presence_interval_secs, 10);
        assert_eq!(datum.b00t.agent.hive.presence_ttl_secs, 30);
        assert_eq!(datum.b00t.agent.hive.gossip_max_hops, 5);
        assert_eq!(
            datum.b00t.agent.finops.and_then(|f| f.project),
            Some("b00t-comms".to_string())
        );
    }

    #[test]
    fn datum_path_defaults_to_dotfiles_b00t_path() {
        let saved = std::env::var("_B00T_Path").ok();
        std::env::remove_var("_B00T_Path");
        let resolved = resolve_datum_path(&None);
        assert!(resolved.ends_with("_b00t_/b00t-comms.agent.toml"));
        if let Some(v) = saved {
            std::env::set_var("_B00T_Path", v);
        }
    }

    #[test]
    fn explicit_datum_path_overrides_default() {
        let explicit = PathBuf::from("/tmp/custom.agent.toml");
        let resolved = resolve_datum_path(&Some(explicit.clone()));
        assert_eq!(resolved, explicit);
    }

    #[test]
    fn direct_send_channel_label_is_namespaced() {
        assert_eq!(channel_for_direct("peer-1"), "direct:peer-1");
    }
}

fn print_frame(frame: &MeshFrame) {
    match frame {
        MeshFrame::Direct(m) => println!("[direct] {}: {}", m.sender, m.body),
        MeshFrame::Broadcast { channel, message } => {
            println!("[broadcast:{channel}] {}: {}", message.sender, message.body)
        }
        MeshFrame::Presence(p) => println!("[presence] {} role={}", p.agent_id, p.role),
        MeshFrame::DiscoveryReply { endpoint, .. } => {
            println!("[discovery-reply] {}", endpoint.agent_id)
        }
        MeshFrame::DiscoveryQuery { from, .. } => println!("[discovery-query] from {from}"),
        // Gossip is internal mesh plumbing, not surfaced to the app channel
        // (matches examples/mesh_cli.rs's own treatment).
        MeshFrame::Gossip { .. } => {}
    }
}
