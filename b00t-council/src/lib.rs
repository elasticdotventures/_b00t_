//! Shared abstraction layer for the b00t hive's player-to-player messaging.
//!
//! This crate does not replace any existing transport (Redis `AgentCoordinator`,
//! NATS `b00t-mcp` tools, the in-process `b00t-ipc::MessageBus`, ...). It gives
//! them a common, generic vocabulary — [`player::Player`] identity,
//! [`message::Envelope`] as a serializable wrapper generic over payload type,
//! [`observe::MessageSink`] for durable/observable recording, and
//! [`council::tally`] for pluggable-quorum voting — so each subsystem can adopt
//! pieces of it without a rewrite.

pub mod council;
pub mod message;
pub mod observe;
pub mod player;

pub use council::{Ballot, Outcome, Proposal, Quorum, tally};
pub use message::{Envelope, Recipient};
pub use observe::{JsonlSink, MessageSink, NoopSink, ReplayFilter};
pub use player::Player;
