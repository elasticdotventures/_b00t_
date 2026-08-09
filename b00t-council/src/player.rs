//! Identity abstraction over whatever struct a subsystem already uses to
//! represent an agent or human ("player"). Several existing types already
//! carry an `is_player: bool` field that nothing reads — implementing this
//! trait for them is what activates it.

/// A participant in hive messaging/voting — software agent or human.
pub trait Player {
    /// Stable identifier used as the `from`/`to`/voter key in messages and ballots.
    fn player_id(&self) -> &str;
    /// True if this player represents a human, not a software agent.
    fn is_human(&self) -> bool;
}
