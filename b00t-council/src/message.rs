//! One generic, serializable envelope for player-to-player traffic.
//!
//! Deliberately generic over the payload type `T` rather than a closed enum —
//! each existing subsystem's own payload shape (`CoordinationMessage` in
//! `b00t-c0re-lib`, `b00t_ipc::Message`, the ad hoc JSON payload behind
//! `b00t-mcp`'s `NotificationMessage`) can be wrapped in [`Envelope<T>`]
//! without every subsystem first agreeing on one shared variant list.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Where an [`Envelope`] is addressed.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum Recipient {
    /// A specific player, by [`crate::Player::player_id`].
    Direct(String),
    /// Every player currently listening.
    Broadcast,
    /// A named logical channel (team, mission, proposal, ...).
    Channel(String),
}

/// A serializable, observable message between players.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Envelope<T> {
    pub id: Uuid,
    pub from: String,
    pub to: Recipient,
    pub sent_at: DateTime<Utc>,
    /// True if `from` is a human player ([`crate::Player::is_human`]).
    pub sender_is_player: bool,
    pub body: T,
}

impl<T> Envelope<T> {
    pub fn new(from: impl Into<String>, to: Recipient, sender_is_player: bool, body: T) -> Self {
        Self {
            id: Uuid::new_v4(),
            from: from.into(),
            to,
            sent_at: Utc::now(),
            sender_is_player,
            body,
        }
    }
}

impl<T: Serialize> Envelope<T> {
    /// Erase the payload type to `serde_json::Value` for storage in a
    /// [`crate::MessageSink`], which is transport/payload-agnostic.
    pub fn to_value_envelope(&self) -> serde_json::Result<Envelope<serde_json::Value>> {
        Ok(Envelope {
            id: self.id,
            from: self.from.clone(),
            to: self.to.clone(),
            sent_at: self.sent_at,
            sender_is_player: self.sender_is_player,
            body: serde_json::to_value(&self.body)?,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trips_through_json_and_erases_payload_type() {
        #[derive(Serialize, Deserialize, PartialEq, Debug)]
        struct Ping {
            n: u32,
        }

        let env = Envelope::new("agentA", Recipient::Direct("agentB".into()), false, Ping { n: 7 });
        let erased = env.to_value_envelope().unwrap();
        assert_eq!(erased.body["n"], 7);

        let json = serde_json::to_string(&env).unwrap();
        let back: Envelope<Ping> = serde_json::from_str(&json).unwrap();
        assert_eq!(back.body, Ping { n: 7 });
        assert_eq!(back.from, "agentA");
    }
}
