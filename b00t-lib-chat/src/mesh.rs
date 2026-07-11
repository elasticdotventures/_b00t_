//! NATS intra-hive mesh for b00t hive coordination.
//!
//! Realizes the `--agent=b00t-comms --skill=nats` capability: every b00t agent
//! process is a first-class node in a subject-routed hive over NATS. All
//! communication is **intra-hive** — the same logical network; there is no
//! cross-network / NAT-traversal path.
//!
//! Four primitives, plus gossip discovery:
//!
//! 1. **Presence / gossip** — nodes advertise their presence by *gossiping* it
//!    on `b00t.hive.mesh.gossip`. Every receiver re-gossips newer advertisements
//!    with a decremented hop budget (epidemic propagation), so the whole hive
//!    learns each node without a central registry or a single query. A plain
//!    `b00t.hive.mesh.discovery.presence` heartbeat is also emitted for backward
//!    compatibility. This is the discovery-advertising mechanism the operator
//!    asked for; the same `GossipTable` drives a future broker-free transport
//!    (Iroh/gwyh) that still runs intra-hive on the same network. See
//!    `_b00t_/NATS-MESH-GOSSIP-DISCOVERY.tomllmd`.
//! 2. **Discovery** — `discover()` additionally publishes a request/reply query
//!    to `b00t.hive.mesh.discovery.query` for instant answers from live nodes.
//! 3. **Direct send** — point-to-point frames to `b00t.hive.mesh.node.{agent_id}`.
//! 4. **Broadcast** — pub/sub frames to `b00t.hive.mesh.channel.{channel}`.
//!
//! The channel-based [`crate::transports::NatsTransport`] cannot express
//! per-agent inboxes or NATS reply-to semantics, so the mesh speaks `async_nats`
//! directly. It is fully compatible with the existing
//! [`crate::ipc_transport::DiscoverableTransport`] trait so it can slot into
//! `b00t agent discover` plumbing.

use crate::error::ChatResult;
use crate::gossip::GossipTable;
use crate::hive_transport::HiveTransport;
#[cfg(not(feature = "nats"))]
use crate::hive_transport::MemoryHiveTransport;
use crate::ipc_transport::{AgentEndpoint, AgentEvent, AgentWatcher, DiscoverableTransport};
use crate::ledgrrr::{FinopsCode, Ledgrrr, UsageReceipt};
use crate::message::ChatMessage;
use ufo_types::{Stereotyped, UfoStereotype};
use async_trait::async_trait;
use futures::stream::BoxStream;
use futures::StreamExt;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, SystemTime};
use tokio::sync::{mpsc, RwLock};
use tokio_stream::wrappers::ReceiverStream;
use tracing::{info, warn};

#[cfg(feature = "nats")]
use crate::hive_transport::NatsHiveTransport;

/// The default transport for a mesh node. With the `nats` feature enabled this
/// is the NATS broker; without it (e.g. `#[cfg]`-disabled broker), it falls back
/// to the in-process broker-free [`MemoryHiveTransport`] so the mesh still runs.
fn default_transport() -> Arc<dyn HiveTransport> {
    #[cfg(feature = "nats")]
    {
        Arc::new(NatsHiveTransport::new())
    }
    #[cfg(not(feature = "nats"))]
    {
        Arc::new(MemoryHiveTransport::new())
    }
}

/// Subject prefix for a node's direct inbox (always intra-hive, same network).
const NODE_PREFIX: &str = "b00t.hive.mesh.node.";
/// Subject prefix for a pub/sub channel.
const CHANNEL_PREFIX: &str = "b00t.hive.mesh.channel.";
/// Subject all nodes listen on to answer discovery queries.
const DISCOVERY_QUERY: &str = "b00t.hive.mesh.discovery.query";
/// Subject all nodes publish presence heartbeats to (backward compat).
const DISCOVERY_PRESENCE: &str = "b00t.hive.mesh.discovery.presence";
/// Subject all nodes gossip presence advertisements on (epidemic discovery).
const DISCOVERY_GOSSIP: &str = "b00t.hive.mesh.gossip";

/// Default time a node waits for discovery replies before returning.
pub const DEFAULT_DISCOVER_TIMEOUT: Duration = Duration::from_millis(1500);
/// Default presence heartbeat interval.
pub const DEFAULT_PRESENCE_INTERVAL: Duration = Duration::from_secs(10);
/// Default time-to-live for a presence record before it is considered stale.
pub const DEFAULT_PRESENCE_TTL: Duration = Duration::from_secs(30);
/// Default gossip hop budget (how many re-gossip hops an advertisement lives).
pub const DEFAULT_GOSSIP_MAX_HOPS: u8 = 5;

/// Direct inbox subject for a given agent id.
pub fn node_subject(agent_id: &str) -> String {
    format!("{NODE_PREFIX}{agent_id}")
}

/// Pub/sub subject for a given channel name.
pub fn channel_subject(channel: &str) -> String {
    format!("{CHANNEL_PREFIX}{channel}")
}

/// Wire envelope exchanged between mesh nodes.
///
/// The NATS subject carries routing intent; this frame carries the typed
/// payload so a single fused inbound stream can demultiplex every primitive.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum MeshFrame {
    /// Point-to-point message addressed to this node's inbox.
    Direct(ChatMessage),
    /// Pub/sub broadcast on a channel.
    Broadcast {
        channel: String,
        message: ChatMessage,
    },
    /// A peer asking "who is online?" — answered on the NATS reply inbox.
    DiscoveryQuery {
        from: String,
        role: String,
        skills: Vec<String>,
    },
    /// A node's self-description in response to a query or presence tick.
    DiscoveryReply {
        endpoint: AgentEndpoint,
        role: String,
        skills: Vec<String>,
    },
    /// Periodic heartbeat announcing a node is alive.
    Presence(AgentPresence),
    /// Epidemic gossip advertisement: a presence plus the advertiser-assigned
    /// sequence and the remaining hop budget. Receivers re-gossip newer ads.
    Gossip {
        presence: AgentPresence,
        seq: u64,
        hops: u8,
    },
}

impl Stereotyped for MeshFrame {
    fn ufo_stereotype(&self) -> UfoStereotype {
        // A frame relates a sender to a recipient (and carries a payload) —
        // in UFO terms a Relator binding communication participants.
        UfoStereotype::Relator("hive-mesh-frame".into())
    }
}

/// Live presence record for a mesh node.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentPresence {
    pub agent_id: String,
    pub role: String,
    pub skills: Vec<String>,
    pub endpoint_uri: String,
    pub last_seen: SystemTime,
    pub ttl: Duration,
}

impl AgentPresence {
    /// True when this record has not been refreshed within its TTL.
    pub fn is_stale(&self, now: SystemTime) -> bool {
        match now.duration_since(self.last_seen) {
            Ok(age) => age > self.ttl,
            Err(_) => false,
        }
    }
}

impl Stereotyped for AgentPresence {
    fn ufo_stereotype(&self) -> UfoStereotype {
        // A live snapshot of a hive agent's reachability — a domain Kind.
        UfoStereotype::Kind("AgentPresence".into())
    }
}

/// Builder/configuration for a [`NatsMeshNode`].
#[derive(Clone)]
pub struct MeshNodeConfig {
    pub agent_id: String,
    pub role: String,
    pub skills: Vec<String>,
    pub nats_url: String,
    pub discover_timeout: Duration,
    pub presence_interval: Duration,
    pub presence_ttl: Duration,
    /// Gossip hop budget for discovery advertisements.
    pub gossip_max_hops: u8,
    /// Project attributed to this node's finops receipts (collaborative-autonomy).
    pub project: String,
    /// Optional ledgrrr ledger; when set, every capability execution registers
    /// a usage receipt and mints a finops code.
    pub ledgrrr: Option<Arc<dyn Ledgrrr>>,
    /// Broker-neutral transport. Defaults to NATS (or in-memory when the `nats`
    /// feature is disabled) so the mesh runs broker-free if desired.
    pub transport: Arc<dyn HiveTransport>,
}

impl MeshNodeConfig {
    pub fn new(agent_id: impl Into<String>, nats_url: impl Into<String>) -> Self {
        Self {
            agent_id: agent_id.into(),
            role: "b00t-comms".to_string(),
            skills: vec!["nats".to_string()],
            nats_url: nats_url.into(),
            discover_timeout: DEFAULT_DISCOVER_TIMEOUT,
            presence_interval: DEFAULT_PRESENCE_INTERVAL,
            presence_ttl: DEFAULT_PRESENCE_TTL,
            gossip_max_hops: DEFAULT_GOSSIP_MAX_HOPS,
            project: "b00t-comms".to_string(),
            ledgrrr: None,
            transport: default_transport(),
        }
    }

    /// Override the hive transport (e.g. plug in [`MemoryHiveTransport`] for a
    /// broker-free node, or an iroh-backed transport once available).
    pub fn with_transport(mut self, transport: Arc<dyn HiveTransport>) -> Self {
        self.transport = transport;
        self
    }

    pub fn with_role(mut self, role: impl Into<String>) -> Self {
        self.role = role.into();
        self
    }

    pub fn with_skills(mut self, skills: Vec<String>) -> Self {
        self.skills = skills;
        self
    }

    pub fn with_project(mut self, project: impl Into<String>) -> Self {
        self.project = project.into();
        self
    }

    pub fn with_gossip_max_hops(mut self, hops: u8) -> Self {
        self.gossip_max_hops = hops;
        self
    }

    pub fn with_ledgrrr(mut self, ledgrrr: Arc<dyn Ledgrrr>) -> Self {
        self.ledgrrr = Some(ledgrrr);
        self
    }
}

impl std::fmt::Debug for MeshNodeConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("MeshNodeConfig")
            .field("agent_id", &self.agent_id)
            .field("role", &self.role)
            .field("project", &self.project)
            .field("nats_url", &self.nats_url)
            .field("gossip_max_hops", &self.gossip_max_hops)
            .field("transport", &self.transport.kind())
            .field("ledgrrr", &self.ledgrrr.is_some())
            .finish()
    }
}

/// A node in the b00t NATS intra-agent mesh.
///
/// Connect, join channels, announce (gossip) presence, then `recv()` frames.
/// Discovery is request/reply over NATS; gossip heartbeats keep the peer table
/// fresh so `discover()` returns instantly for already-known nodes, and even
/// nodes that never send a query learn the mesh via epidemic propagation.
#[derive(Clone)]
pub struct NatsMeshNode {
    config: MeshNodeConfig,
    /// Broker-neutral transport (NATS by default; memory/iroh pluggable).
    transport: Arc<dyn HiveTransport>,
    /// Known peers + gossip membership (presence + sequence + hop budget).
    peers: Arc<RwLock<GossipTable>>,
    /// Application-facing inbound frames.
    inbox_tx: mpsc::Sender<MeshFrame>,
    inbox_rx: Arc<RwLock<Option<mpsc::Receiver<MeshFrame>>>>,
    /// Forwarding task handles, aborted on close.
    forwards: Arc<RwLock<Vec<tokio::task::JoinHandle<()>>>>,
    presence_task: Arc<RwLock<Option<tokio::task::JoinHandle<()>>>>,
    /// Per-node lifecycle flag (separate from the transport's, so multiple nodes
    /// sharing one in-process transport each manage their own inbound tasks).
    live: Arc<AtomicBool>,
    ledgrrr: Option<Arc<dyn Ledgrrr>>,
}

impl NatsMeshNode {
    /// The broker-neutral transport backing this node.
    pub fn transport(&self) -> &Arc<dyn HiveTransport> {
        &self.transport
    }

    /// Whether this node has connected and spawned its inbound tasks.
    pub fn is_connected(&self) -> bool {
        self.live.load(Ordering::SeqCst)
    }
}

impl Stereotyped for NatsMeshNode {
    fn ufo_stereotype(&self) -> UfoStereotype {
        // A hive node plays the intra-hive communication role.
        UfoStereotype::Role("hive-node".into())
    }
}

impl std::fmt::Debug for NatsMeshNode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("NatsMeshNode")
            .field("agent_id", &self.config.agent_id)
            .field("role", &self.config.role)
            .finish()
    }
}

impl NatsMeshNode {
    pub fn new(config: MeshNodeConfig) -> Self {
        let ledgrrr = config.ledgrrr.clone();
        let (inbox_tx, inbox_rx) = mpsc::channel(256);
        Self {
            transport: config.transport.clone(),
            config,
            peers: Arc::new(RwLock::new(GossipTable::new(DEFAULT_GOSSIP_MAX_HOPS))),
            inbox_tx,
            inbox_rx: Arc::new(RwLock::new(Some(inbox_rx))),
            forwards: Arc::new(RwLock::new(Vec::new())),
            presence_task: Arc::new(RwLock::new(None)),
            live: Arc::new(AtomicBool::new(false)),
            ledgrrr,
        }
    }

    /// Register a finops usage receipt for a capability execution, if a ledger
    /// is configured. Returns the minted code (or `None` when no ledger is set).
    ///
    /// This is the collaborative-autonomy accounting seam: every project/
    /// capability an agent executes is attributable via a ledgrrr finops code.
    pub async fn record_usage(
        &self,
        project: &str,
        capability: &str,
        units: u64,
    ) -> ChatResult<Option<FinopsCode>> {
        let Some(ledger) = &self.ledgrrr else {
            return Ok(None);
        };
        let receipt = UsageReceipt::new(&self.config.agent_id, project, capability, units);
        let code = ledger.register(&receipt)?;
        Ok(Some(code))
    }

    /// Connect the transport and start the inbound forwarding tasks.
    pub async fn connect(&self) -> ChatResult<()> {
        if self.live.swap(true, Ordering::SeqCst) {
            return Ok(());
        }
        self.transport.connect(&self.config.nats_url).await?;
        self.spawn_inbound().await?;
        info!(
            "hive mesh node connected ({:?}): {}",
            self.transport.kind(),
            self.config.agent_id
        );
        Ok(())
    }

    /// Subscribe to the inbox, presence, gossip, discovery-query, and any
    /// joined channels; forward every frame into the application inbox.
    async fn spawn_inbound(&self) -> ChatResult<()> {
        let mut handles = self.forwards.write().await;

        let inbox_stream = self
            .transport
            .subscribe(&node_subject(&self.config.agent_id))
            .await?;
        handles.push(self.forward_stream(inbox_stream));

        let presence_stream = self.transport.subscribe(DISCOVERY_PRESENCE).await?;
        handles.push(self.forward_stream(presence_stream));

        let gossip_stream = self.transport.subscribe(DISCOVERY_GOSSIP).await?;
        handles.push(self.forward_stream(gossip_stream));

        let query_stream = self.transport.subscribe(DISCOVERY_QUERY).await?;
        handles.push(self.forward_stream(query_stream));

        Ok(())
    }

    /// Consume a transport frame stream, demultiplex/discover/gossip, and forward
    /// application frames into the inbox. Re-gossip and discovery replies are
    /// published back through the same broker-neutral transport — no NATS reply
    /// semantics are assumed.
    fn forward_stream(&self, mut stream: BoxStream<'static, Vec<u8>>) -> tokio::task::JoinHandle<()> {
        let inbox_tx = self.inbox_tx.clone();
        let transport = self.transport.clone();
        let agent_id = self.config.agent_id.clone();
        let peers = self.peers.clone();
        let this_role = self.config.role.clone();
        let this_skills = self.config.skills.clone();
        let endpoint_uri = node_subject(&agent_id);
        let ttl = self.config.presence_ttl;
        tokio::spawn(async move {
            while let Some(payload) = stream.next().await {
                let frame: MeshFrame = match serde_json::from_slice(&payload) {
                    Ok(f) => f,
                    Err(e) => {
                        warn!("mesh: dropping undecodable frame: {}", e);
                        continue;
                    }
                };
                match &frame {
                    MeshFrame::Presence(p) => {
                        let mut g = peers.write().await;
                        let seq = g.next_seq(&p.agent_id);
                        g.ingest(p.clone(), seq, 0);
                    }
                    MeshFrame::Gossip { presence, seq, hops } => {
                        // Accept if newer; re-gossip with decremented budget.
                        let forward = {
                            let mut g = peers.write().await;
                            g.ingest(presence.clone(), *seq, *hops)
                        };
                        if let Some(remaining) = forward {
                            let re = MeshFrame::Gossip {
                                presence: presence.clone(),
                                seq: *seq,
                                hops: remaining,
                            };
                            if let Ok(payload) = serde_json::to_vec(&re) {
                                let _ = transport.publish(DISCOVERY_GOSSIP, &payload).await;
                            }
                        }
                    }
                    MeshFrame::DiscoveryReply { endpoint, role, skills } => {
                        if endpoint.agent_id != agent_id {
                            let mut g = peers.write().await;
                            let seq = g.next_seq(&endpoint.agent_id);
                            g.ingest(
                                AgentPresence {
                                    agent_id: endpoint.agent_id.clone(),
                                    role: role.clone(),
                                    skills: skills.clone(),
                                    endpoint_uri: endpoint.endpoint_uri.clone(),
                                    last_seen: endpoint.last_seen,
                                    ttl,
                                },
                                seq,
                                0,
                            );
                        }
                    }
                    MeshFrame::DiscoveryQuery { from, role, skills } => {
                        if from != &agent_id {
                            {
                                let mut g = peers.write().await;
                                let seq = g.next_seq(from);
                                g.ingest(
                                    AgentPresence {
                                        agent_id: from.clone(),
                                        role: role.clone(),
                                        skills: skills.clone(),
                                        endpoint_uri: node_subject(from),
                                        last_seen: SystemTime::now(),
                                        ttl,
                                    },
                                    seq,
                                    0,
                                );
                            }
                            // Answer the query on the querier's own inbox.
                            let reply_frame = MeshFrame::DiscoveryReply {
                                endpoint: AgentEndpoint {
                                    agent_id: agent_id.clone(),
                                    endpoint_uri: endpoint_uri.clone(),
                                    transport_kind: transport.kind(),
                                    last_seen: SystemTime::now(),
                                    metadata: None,
                                },
                                role: this_role.clone(),
                                skills: this_skills.clone(),
                            };
                            if let Ok(payload) = serde_json::to_vec(&reply_frame) {
                                let _ = transport.publish(&node_subject(from), &payload).await;
                            }
                        }
                    }
                    _ => {}
                }
                if inbox_tx.send(frame).await.is_err() {
                    break; // application dropped the node
                }
            }
        })
    }

    /// Join a pub/sub channel (start receiving broadcasts on it).
    pub async fn join(&self, channel: &str) -> ChatResult<()> {
        let sub = self.transport.subscribe(&channel_subject(channel)).await?;
        self.forwards.write().await.push(self.forward_stream(sub));
        Ok(())
    }

    /// Announce presence: gossip an advertisement (bounded hop budget) and emit
    /// a plain presence heartbeat for backward compatibility.
    pub async fn announce(&self) -> ChatResult<()> {
        let presence = AgentPresence {
            agent_id: self.config.agent_id.clone(),
            role: self.config.role.clone(),
            skills: self.config.skills.clone(),
            endpoint_uri: node_subject(&self.config.agent_id),
            last_seen: SystemTime::now(),
            ttl: self.config.presence_ttl,
        };
        let seq = {
            let mut g = self.peers.write().await;
            let s = g.next_seq(&self.config.agent_id);
            g.insert_local(presence.clone(), s);
            s
        };
        let gossip = MeshFrame::Gossip {
            presence: presence.clone(),
            seq,
            hops: self.config.gossip_max_hops,
        };
        self.transport
            .publish(DISCOVERY_GOSSIP, &serde_json::to_vec(&gossip)?)
            .await?;
        self.transport
            .publish(
                DISCOVERY_PRESENCE,
                &serde_json::to_vec(&MeshFrame::Presence(presence))?,
            )
            .await?;
        self.record_usage(&self.config.project, "nats.presence", 1)
            .await
            .ok();
        Ok(())
    }

    /// Start a background heartbeat that gossips presence every interval.
    pub async fn start_presence(&self) {
        let node = self.clone();
        let interval = self.config.presence_interval;
        let task = tokio::spawn(async move {
            loop {
                if let Err(e) = node.announce().await {
                    warn!("mesh: presence announce failed: {}", e);
                }
                tokio::time::sleep(interval).await;
            }
        });
        *self.presence_task.write().await = Some(task);
    }

    /// Discover peers: publish a query with the local inbox as reply target,
    /// then collect replies for the configured timeout. Gossip advertisements
    /// received meanwhile also populate the peer table.
    pub async fn discover(&self) -> ChatResult<Vec<AgentEndpoint>> {
        self.discover_with_timeout(self.config.discover_timeout).await
    }

    /// Discover peers, waiting up to `timeout` for fresh replies.
    pub async fn discover_with_timeout(&self, timeout: Duration) -> ChatResult<Vec<AgentEndpoint>> {
        let query = MeshFrame::DiscoveryQuery {
            from: self.config.agent_id.clone(),
            role: self.config.role.clone(),
            skills: self.config.skills.clone(),
        };
        let payload = serde_json::to_vec(&query)?;
        // Reply target is the querier's own inbox (encoded in `from`); no broker
        // reply semantics required, so this works on any HiveTransport.
        self.transport.publish(DISCOVERY_QUERY, &payload).await?;
        // Also announce so peers learn us (and gossip propagates).
        self.announce().await.ok();
        self.record_usage(&self.config.project, "nats.discover", 1)
            .await
            .ok();

        let deadline = tokio::time::Instant::now() + timeout;
        let mut rx_guard = self.inbox_rx.write().await;
        let mut rx = match rx_guard.take() {
            Some(rx) => rx,
            None => return Ok(self.endpoints_locked().await),
        };
        drop(rx_guard);
        while tokio::time::Instant::now() < deadline {
            let remaining = (deadline - tokio::time::Instant::now()).as_millis() as u64;
            match tokio::time::timeout(Duration::from_millis(remaining.max(1)), rx.recv()).await {
                Ok(Some(MeshFrame::DiscoveryReply {
                    endpoint,
                    role,
                    skills,
                })) => {
                    if endpoint.agent_id != self.config.agent_id {
                        let mut g = self.peers.write().await;
                        let seq = g.next_seq(&endpoint.agent_id);
                        g.ingest(
                            AgentPresence {
                                agent_id: endpoint.agent_id.clone(),
                                role,
                                skills,
                                endpoint_uri: endpoint.endpoint_uri.clone(),
                                last_seen: endpoint.last_seen,
                                ttl: self.config.presence_ttl,
                            },
                            seq,
                            0,
                        );
                    }
                }
                Ok(Some(_)) => { /* non-reply frame; ignore during discovery */ }
                Ok(None) => break,
                Err(_) => break,
            }
        }
        *self.inbox_rx.write().await = Some(rx);
        Ok(self.endpoints_locked().await)
    }

    async fn endpoints_locked(&self) -> Vec<AgentEndpoint> {
        self.peers
            .read()
            .await
            .live_endpoints(&self.config.agent_id, self.config.presence_ttl)
    }

    /// Send a direct (point-to-point) message to a specific agent id.
    pub async fn send(&self, to_agent: &str, message: &ChatMessage) -> ChatResult<()> {
        let frame = MeshFrame::Direct(message.clone());
        let payload = serde_json::to_vec(&frame)?;
        self.transport.publish(&node_subject(to_agent), &payload).await?;
        self.record_usage(&self.config.project, "nats.send", 1).await?;
        Ok(())
    }

    /// Broadcast a message to every subscriber of `channel`.
    pub async fn publish(&self, channel: &str, message: &ChatMessage) -> ChatResult<()> {
        let frame = MeshFrame::Broadcast {
            channel: channel.to_string(),
            message: message.clone(),
        };
        let payload = serde_json::to_vec(&frame)?;
        self.transport
            .publish(&channel_subject(channel), &payload)
            .await?;
        self.record_usage(&self.config.project, "nats.broadcast", 1)
            .await?;
        Ok(())
    }

    /// Receive the next inbound mesh frame (direct, broadcast, discovery,
    /// presence, or gossip). Returns `None` if the node has been closed.
    pub async fn recv(&self) -> ChatResult<Option<MeshFrame>> {
        let mut rx_guard = self.inbox_rx.write().await;
        let mut rx = match rx_guard.take() {
            Some(rx) => rx,
            None => return Ok(None),
        };
        drop(rx_guard);
        let frame = rx.recv().await;
        *self.inbox_rx.write().await = Some(rx);
        Ok(frame)
    }

    /// Number of currently-known live peers (excluding self).
    pub async fn peer_count(&self) -> usize {
        self.peers
            .read()
            .await
            .peer_count(&self.config.agent_id, self.config.presence_ttl)
    }

    /// Gracefully close: abort forwarding + presence tasks, drop transport.
    pub async fn close(&self) -> ChatResult<()> {
        if let Some(task) = self.presence_task.write().await.take() {
            task.abort();
        }
        for handle in self.forwards.write().await.drain(..) {
            handle.abort();
        }
        self.live.store(false, Ordering::SeqCst);
        self.transport.close().await.ok();
        info!("hive mesh node closed: {}", self.config.agent_id);
        Ok(())
    }
}

#[async_trait]
impl DiscoverableTransport for NatsMeshNode {
    async fn discover_agents(&self) -> ChatResult<Vec<AgentEndpoint>> {
        self.discover().await
    }

    async fn watch_agents(&self) -> ChatResult<AgentWatcher> {
        let (tx, rx) = mpsc::channel(64);
        let peers = self.peers.clone();
        let self_id = self.config.agent_id.clone();
        let ttl = self.config.presence_ttl;
        tokio::spawn(async move {
            loop {
                {
                    let eps = peers.read().await.live_endpoints(&self_id, ttl);
                    for e in eps {
                        let _ = tx.send(AgentEvent::Discovered(e)).await;
                    }
                }
                tokio::time::sleep(ttl / 2).await;
            }
        });
        let stream = ReceiverStream::new(rx);
        Ok(AgentWatcher::new(stream))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn subjects_are_well_formed() {
        assert_eq!(node_subject("alpha"), "b00t.hive.mesh.node.alpha");
        assert_eq!(channel_subject("mission.x"), "b00t.hive.mesh.channel.mission.x");
    }

    #[test]
    fn hive_types_are_ufo_grounded() {
        use ufo_types::Stereotyped;
        let presence = AgentPresence {
            agent_id: "a".into(),
            role: "r".into(),
            skills: vec![],
            endpoint_uri: "b00t.hive.mesh.node.a".into(),
            last_seen: SystemTime::now(),
            ttl: Duration::from_secs(30),
        };
        assert_eq!(presence.ufo_stereotype().to_string(), "Kind:AgentPresence");
        assert_eq!(
            MeshFrame::Presence(presence.clone()).ufo_stereotype().to_string(),
            "Relator:hive-mesh-frame"
        );
        let node = NatsMeshNode::new(
            MeshNodeConfig::new("a", "nats://localhost:4222").with_project("p"),
        );
        assert_eq!(node.ufo_stereotype().to_string(), "Role:hive-node");
    }

    #[test]
    fn presence_expiry_tracks_ttl() {
        let now = SystemTime::now();
        let fresh = AgentPresence {
            agent_id: "a".into(),
            role: "r".into(),
            skills: vec![],
            endpoint_uri: "u".into(),
            last_seen: now,
            ttl: Duration::from_secs(30),
        };
        assert!(!fresh.is_stale(now));
        let stale = AgentPresence {
            agent_id: "a".into(),
            role: "r".into(),
            skills: vec![],
            endpoint_uri: "u".into(),
            last_seen: now - Duration::from_secs(60),
            ttl: Duration::from_secs(30),
        };
        assert!(stale.is_stale(now));
    }

    #[test]
    fn frame_round_trips_through_json() {
        let msg = ChatMessage::new("chan", "sender", "hi");
        let frame = MeshFrame::Direct(msg);
        let bytes = serde_json::to_vec(&frame).unwrap();
        let back: MeshFrame = serde_json::from_slice(&bytes).unwrap();
        match back {
            MeshFrame::Direct(m) => assert_eq!(m.body, "hi"),
            _ => panic!("wrong frame variant"),
        }

        let g = MeshFrame::Gossip {
            presence: AgentPresence {
                agent_id: "a".into(),
                role: "r".into(),
                skills: vec![],
                endpoint_uri: "u".into(),
                last_seen: SystemTime::now(),
                ttl: Duration::from_secs(30),
            },
            seq: 3,
            hops: 2,
        };
        let bytes = serde_json::to_vec(&g).unwrap();
        assert!(serde_json::from_slice::<MeshFrame>(&bytes).is_ok());
    }

    #[tokio::test]
    async fn peer_table_excludes_self_and_stale() {
        let node = NatsMeshNode::new(MeshNodeConfig::new("self", "nats://x"));
        {
            let mut g = node.peers.write().await;
            g.insert_local(
                AgentPresence {
                    agent_id: "self".into(),
                    role: "r".into(),
                    skills: vec![],
                    endpoint_uri: "u".into(),
                    last_seen: SystemTime::now(),
                    ttl: node.config.presence_ttl,
                },
                1,
            );
            g.insert_local(
                AgentPresence {
                    agent_id: "peer".into(),
                    role: "r".into(),
                    skills: vec!["nats".to_string()],
                    endpoint_uri: "u".into(),
                    last_seen: SystemTime::now(),
                    ttl: node.config.presence_ttl,
                },
                1,
            );
            g.insert_local(
                AgentPresence {
                    agent_id: "ghost".into(),
                    role: "r".into(),
                    skills: vec![],
                    endpoint_uri: "u".into(),
                    last_seen: SystemTime::now() - Duration::from_secs(120),
                    ttl: node.config.presence_ttl,
                },
                1,
            );
        }
        assert_eq!(node.peer_count().await, 1);
    }
}
