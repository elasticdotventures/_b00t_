//! Epidemic (gossip) membership for the b00t mesh.
//!
//! Gossip-based discovery advertising: when a node announces, it *gossips* its
//! presence to the mesh. Every node that receives a newer advertisement
//! re-gossips it (with a decremented hop budget) until the budget hits zero.
//! This is the advertisement primitive the operator asked for — nodes learn
//! each other not by a central registry or a single query/response, but by
//! epidemic propagation of signed presence advertisements.
//!
//! The table is **transport-agnostic**: the mesh wires it to a NATS subject
//! today, but the same logic drives a future P2P transport (Iroh/gossip) where
//! there is no central broker to fan out for you. See
//! `_b00t_/NATS-MESH-GOSSIP-DISCOVERY.tomllmd`.

use crate::ipc_transport::{AgentEndpoint, TransportKind};
use crate::mesh::AgentPresence;
use std::collections::HashMap;
use std::time::{Duration, SystemTime};
use ufo_types::{Stereotyped, UfoStereotype};

/// A single membership entry: the presence plus the monotonically increasing
/// sequence the advertiser assigned it.
#[derive(Debug, Clone)]
pub struct GossipMember {
    pub presence: AgentPresence,
    pub seq: u64,
}

impl Stereotyped for GossipMember {
    fn ufo_stereotype(&self) -> UfoStereotype {
        UfoStereotype::Kind("GossipMember".into())
    }
}

/// Transport-agnostic epidemic membership table.
///
/// `ingest` is the core decision: accept a received advertisement only if it is
/// newer than what we hold, and return the remaining hop budget so the caller
/// can re-advertise (bounded epidemic). `hops == 0` means "accept but do not
/// forward" — this is what stops the gossip from looping forever.
#[derive(Debug, Clone)]
pub struct GossipTable {
    max_hops: u8,
    members: HashMap<String, GossipMember>,
}

impl GossipTable {
    pub fn new(max_hops: u8) -> Self {
        Self {
            max_hops,
            members: HashMap::new(),
        }
    }

    pub fn max_hops(&self) -> u8 {
        self.max_hops
    }

    /// Ingest an incoming advertisement. Returns `Some(remaining_hops)` when the
    /// member was accepted as newer and should be re-gossiped, or `None` when it
    /// was a stale/duplicate (do not forward).
    pub fn ingest(&mut self, presence: AgentPresence, seq: u64, hops: u8) -> Option<u8> {
        match self.members.get(&presence.agent_id) {
            Some(existing) if existing.seq >= seq => return None,
            _ => {}
        }
        self.members.insert(
            presence.agent_id.clone(),
            GossipMember {
                presence: presence.clone(),
                seq,
            },
        );
        if hops == 0 {
            None
        } else {
            Some(hops - 1)
        }
    }

    /// Record a local (self-originated or non-propagating) presence update.
    pub fn insert_local(&mut self, presence: AgentPresence, seq: u64) {
        self.members.insert(
            presence.agent_id.clone(),
            GossipMember {
                presence,
                seq,
            },
        );
    }

    /// Next sequence number for `agent_id` (previous + 1, or 1 if unseen).
    pub fn next_seq(&self, agent_id: &str) -> u64 {
        self.members
            .get(agent_id)
            .map(|m| m.seq + 1)
            .unwrap_or(1)
    }

    /// All live endpoints (excluding self, excluding stale by TTL).
    pub fn live_endpoints(&self, self_id: &str, _ttl: Duration) -> Vec<AgentEndpoint> {
        let now = SystemTime::now();
        self.members
            .values()
            .filter(|m| m.presence.agent_id != self_id && !m.presence.is_stale(now))
            .map(|m| AgentEndpoint {
                agent_id: m.presence.agent_id.clone(),
                endpoint_uri: m.presence.endpoint_uri.clone(),
                transport_kind: TransportKind::Nats,
                last_seen: m.presence.last_seen,
                metadata: Some(serde_json::json!({
                    "role": m.presence.role,
                    "skills": m.presence.skills,
                    "seq": m.seq,
                })),
            })
            .collect()
    }

    pub fn peer_count(&self, self_id: &str, _ttl: Duration) -> usize {
        let now = SystemTime::now();
        self.members
            .values()
            .filter(|m| m.presence.agent_id != self_id && !m.presence.is_stale(now))
            .count()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{Duration, SystemTime};

    fn presence(id: &str, _seq: u64) -> AgentPresence {
        AgentPresence {
            agent_id: id.into(),
            role: "r".into(),
            skills: vec![],
            endpoint_uri: format!("b00t.hive.mesh.node.{id}"),
            last_seen: SystemTime::now(),
            ttl: Duration::from_secs(30),
        }
    }

    #[test]
    fn newer_advertisement_is_accepted_and_forwarded() {
        let mut t = GossipTable::new(5);
        assert_eq!(t.ingest(presence("a", 1), 1, 5), Some(4));
        // same seq -> stale, not forwarded
        assert_eq!(t.ingest(presence("a", 1), 1, 5), None);
        // older seq -> stale
        assert_eq!(t.ingest(presence("a", 1), 1, 5), None);
        // newer seq -> accepted, forwarded with decremented hops
        assert_eq!(t.ingest(presence("a", 2), 2, 3), Some(2));
    }

    #[test]
    fn hop_budget_terminates_propagation() {
        let mut t = GossipTable::new(5);
        // hops==0 means accept but never forward
        assert_eq!(t.ingest(presence("b", 1), 1, 0), None);
        assert_eq!(t.peer_count("self", Duration::from_secs(30)), 1);
    }

    #[test]
    fn self_and_stale_are_excluded_from_endpoints() {
        let mut t = GossipTable::new(5);
        t.insert_local(presence("self", 1), 1);
        t.insert_local(presence("live", 1), 1);
        let stale = presence("ghost", 1);
        t.insert_local(stale, 1);
        // age the ghost
        t.members.get_mut("ghost").unwrap().presence.last_seen =
            SystemTime::now() - Duration::from_secs(120);
        let eps = t.live_endpoints("self", Duration::from_secs(30));
        let ids: Vec<&str> = eps.iter().map(|e| e.agent_id.as_str()).collect();
        assert!(ids.contains(&"live"));
        assert!(!ids.contains(&"self"));
        assert!(!ids.contains(&"ghost"));
    }

    #[test]
    fn gossip_member_is_ufo_grounded() {
        let m = GossipMember {
            presence: presence("a", 1),
            seq: 1,
        };
        assert_eq!(m.ufo_stereotype().to_string(), "Kind:GossipMember");
    }
}
