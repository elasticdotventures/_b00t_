//! # Cake Economy
//!
//! Minimal cake (token) economy for agent coordination.
//!
//! Provides:
//! - `CakeTransaction`: a record of value transfer between agents
//! - `CakeLedger`: in-memory ledger with mint, spend, balance, and transfer operations
//!
//! Cake balances on individual agents are stored in `roles::Agent.cake_balance`.
//! This module coordinates multi-agent economy operations.
//!
//! # Examples
//!
//! ```
//! use b00t_c0re_hierarchy::cake_economy::{CakeLedger, CakeTransaction};
//! use b00t_c0re_hierarchy::roles::Agent;
//! use b00t_c0re_role::KnownRole;
//!
//! let mut ledger = CakeLedger::new();
//! let mut alice = Agent {
//!     id: "alice".into(), role: KnownRole::executive(), skills: vec![],
//!     cake_balance: 0.0, is_alive: true, manager_id: None, is_player: false,
//! };
//!
//! ledger.mint(&mut alice, 100.0, "Genesis grant").unwrap();
//! assert_eq!(alice.cake_balance, 100.0);
//! assert_eq!(ledger.total_supply(), 100.0);
//! ```

use crate::roles::Agent;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// A single cake token transfer event.
///
/// Immutable after creation — the ledger records these for audit.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct CakeTransaction {
    /// Sender agent id (empty string for mint operations)
    pub from: String,
    /// Receiver agent id (empty string for burn operations)
    pub to: String,
    /// Amount of cake transferred (must be positive, non-zero)
    pub amount: f64,
    /// Human-readable reason for the transfer
    pub reason: String,
    /// Timestamp of the transfer
    pub timestamp: DateTime<Utc>,
}

impl CakeTransaction {
    /// Create a new cake transaction.
    pub fn new(from: String, to: String, amount: f64, reason: String) -> Result<Self, CakeError> {
        if !amount.is_finite() || amount <= 0.0 {
            return Err(CakeError::InvalidAmount(amount));
        }
        Ok(Self {
            from,
            to,
            amount,
            reason,
            timestamp: Utc::now(),
        })
    }

    /// Create a mint transaction (from = empty).
    pub fn mint(to: String, amount: f64, reason: String) -> Result<Self, CakeError> {
        Self::new(String::new(), to, amount, reason)
    }

    /// Create a burn transaction (to = empty).
    pub fn burn(from: String, amount: f64, reason: String) -> Result<Self, CakeError> {
        Self::new(from, String::new(), amount, reason)
    }
}

/// Error type for cake ledger operations.
#[derive(Debug, Clone, PartialEq)]
pub enum CakeError {
    /// Insufficient balance for the requested operation
    InsufficientBalance {
        agent_id: String,
        balance: f64,
        requested: f64,
    },
    /// Invalid amount (zero, negative, NaN, or infinite)
    InvalidAmount(f64),
    /// Agent is not alive
    AgentDead(String),
    /// Cannot send to self
    SelfTransfer(String),
    /// Transaction record is malformed (bad semantics or non-finite amount)
    InvalidTransaction(String),
}

impl std::fmt::Display for CakeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CakeError::InsufficientBalance {
                agent_id,
                balance,
                requested,
            } => {
                write!(
                    f,
                    "agent '{}' has insufficient balance: {:.2} < {:.2}",
                    agent_id, balance, requested
                )
            }
            CakeError::InvalidAmount(amt) => {
                write!(f, "invalid cake amount: {:.2} (must be positive)", amt)
            }
            CakeError::AgentDead(id) => {
                write!(f, "agent '{}' is dead and cannot transact", id)
            }
            CakeError::SelfTransfer(id) => {
                write!(f, "agent '{}' cannot transfer cake to itself", id)
            }
            CakeError::InvalidTransaction(msg) => {
                write!(f, "invalid transaction: {}", msg)
            }
        }
    }
}

impl std::error::Error for CakeError {}

/// In-memory cake ledger that coordinates multi-agent economy operations.
///
/// Maintains a running total supply and a full transaction history.
/// Individual agent balances are stored directly on `Agent.cake_balance`.
///
/// This is a minimal implementation — no persistence layer here.
/// Callers may serialize `transactions()` for durable storage.
#[derive(Debug)]
pub struct CakeLedger {
    /// Total cake supply (sum of all mints minus burns)
    total_supply: f64,
    /// Full transaction history (append-only)
    transactions: Vec<CakeTransaction>,
}

impl CakeLedger {
    /// Create a new empty ledger.
    pub fn new() -> Self {
        Self {
            total_supply: 0.0,
            transactions: Vec::new(),
        }
    }

    /// Create a ledger with an initial transaction history (for deserialization).
    ///
    /// Validates that every transaction has a finite positive amount and that
    /// the mint/burn/transfer semantics are consistent (from/to emptiness).
    /// Returns `Err(CakeError::InvalidTransaction)` on the first bad record.
    pub fn with_history(transactions: Vec<CakeTransaction>) -> Result<Self, CakeError> {
        for tx in &transactions {
            if !tx.amount.is_finite() || tx.amount <= 0.0 {
                return Err(CakeError::InvalidTransaction(format!(
                    "non-positive or non-finite amount {:.2} in transaction from '{}' to '{}'",
                    tx.amount, tx.from, tx.to
                )));
            }
            if tx.from.is_empty() && tx.to.is_empty() {
                return Err(CakeError::InvalidTransaction(
                    "transaction has both 'from' and 'to' empty (neither mint, burn, nor transfer)"
                        .into(),
                ));
            }
        }
        let total_supply = transactions
            .iter()
            .map(|t| {
                if t.from.is_empty() {
                    t.amount // mint
                } else if t.to.is_empty() {
                    -t.amount // burn
                } else {
                    0.0 // transfer — no net supply change
                }
            })
            .sum();
        Ok(Self {
            total_supply,
            transactions,
        })
    }

    /// Mint new cake tokens into an agent's balance.
    ///
    /// Returns the transaction record on success.
    pub fn mint(
        &mut self,
        agent: &mut Agent,
        amount: f64,
        reason: &str,
    ) -> Result<CakeTransaction, CakeError> {
        if !amount.is_finite() || amount <= 0.0 {
            return Err(CakeError::InvalidAmount(amount));
        }
        if !agent.is_alive {
            return Err(CakeError::AgentDead(agent.id.clone()));
        }

        agent.cake_balance += amount;
        self.total_supply += amount;

        let tx = CakeTransaction::mint(agent.id.clone(), amount, reason.to_string())?;
        self.transactions.push(tx.clone());
        Ok(tx)
    }

    /// Spend (burn) cake tokens from an agent's balance.
    ///
    /// Returns the transaction record on success.
    pub fn spend(
        &mut self,
        agent: &mut Agent,
        amount: f64,
        reason: &str,
    ) -> Result<CakeTransaction, CakeError> {
        if !amount.is_finite() || amount <= 0.0 {
            return Err(CakeError::InvalidAmount(amount));
        }
        if !agent.is_alive {
            return Err(CakeError::AgentDead(agent.id.clone()));
        }
        if agent.cake_balance < amount {
            return Err(CakeError::InsufficientBalance {
                agent_id: agent.id.clone(),
                balance: agent.cake_balance,
                requested: amount,
            });
        }

        agent.cake_balance -= amount;
        self.total_supply -= amount;

        let tx = CakeTransaction::burn(agent.id.clone(), amount, reason.to_string())?;
        self.transactions.push(tx.clone());
        Ok(tx)
    }

    /// Transfer cake tokens from one agent to another.
    ///
    /// Returns the transaction record on success.
    pub fn transfer(
        &mut self,
        from: &mut Agent,
        to: &mut Agent,
        amount: f64,
        reason: &str,
    ) -> Result<CakeTransaction, CakeError> {
        if !amount.is_finite() || amount <= 0.0 {
            return Err(CakeError::InvalidAmount(amount));
        }
        if from.id == to.id {
            return Err(CakeError::SelfTransfer(from.id.clone()));
        }
        if !from.is_alive {
            return Err(CakeError::AgentDead(from.id.clone()));
        }
        if !to.is_alive {
            return Err(CakeError::AgentDead(to.id.clone()));
        }
        if from.cake_balance < amount {
            return Err(CakeError::InsufficientBalance {
                agent_id: from.id.clone(),
                balance: from.cake_balance,
                requested: amount,
            });
        }

        from.cake_balance -= amount;
        to.cake_balance += amount;

        let tx = CakeTransaction::new(from.id.clone(), to.id.clone(), amount, reason.to_string())?;
        self.transactions.push(tx.clone());
        Ok(tx)
    }

    /// Get the balance of an agent (reads directly from agent struct).
    pub fn balance(&self, agent: &Agent) -> f64 {
        agent.cake_balance
    }

    /// Get the total supply of cake tokens.
    pub fn total_supply(&self) -> f64 {
        self.total_supply
    }

    /// Get a read-only reference to the transaction history.
    pub fn transactions(&self) -> &[CakeTransaction] {
        &self.transactions
    }

    /// Clear all transactions and reset total supply.
    /// Does NOT modify agent balances — use with care.
    pub fn reset(&mut self) {
        self.total_supply = 0.0;
        self.transactions.clear();
    }
}

impl Default for CakeLedger {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use b00t_c0re_role::KnownRole;

    fn make_agent(id: &str, balance: f64, alive: bool) -> Agent {
        Agent {
            id: id.to_string(),
            role: KnownRole::worker(),
            skills: vec![],
            cake_balance: balance,
            is_alive: alive,
            manager_id: None,
            is_player: false,
        }
    }

    #[test]
    fn test_mint_increases_balance_and_supply() {
        let mut ledger = CakeLedger::new();
        let mut agent = make_agent("alice", 0.0, true);

        let tx = ledger.mint(&mut agent, 100.0, "Genesis").unwrap();
        assert_eq!(agent.cake_balance, 100.0);
        assert_eq!(ledger.total_supply(), 100.0);
        assert_eq!(tx.from, "");
        assert_eq!(tx.to, "alice");
        assert_eq!(tx.amount, 100.0);
    }

    #[test]
    fn test_spend_decreases_balance_and_supply() {
        let mut ledger = CakeLedger::new();
        let mut agent = make_agent("bob", 0.0, true);
        ledger.mint(&mut agent, 100.0, "Setup").unwrap();
        assert_eq!(agent.cake_balance, 100.0);
        assert_eq!(ledger.total_supply(), 100.0);

        let tx = ledger.spend(&mut agent, 30.0, "Coffee").unwrap();
        assert_eq!(agent.cake_balance, 70.0);
        assert_eq!(ledger.total_supply(), 70.0); // 100 minted - 30 burned
        assert_eq!(tx.from, "bob");
        assert_eq!(tx.to, "");
        assert_eq!(tx.amount, 30.0);
    }

    #[test]
    fn test_transfer_moves_cake_between_agents() {
        let mut ledger = CakeLedger::new();
        let mut alice = make_agent("alice", 100.0, true);
        let mut bob = make_agent("bob", 0.0, true);

        let tx = ledger
            .transfer(&mut alice, &mut bob, 40.0, "Payment")
            .unwrap();
        assert_eq!(alice.cake_balance, 60.0);
        assert_eq!(bob.cake_balance, 40.0);
        assert_eq!(ledger.total_supply(), 0.0); // no net change
        assert_eq!(tx.from, "alice");
        assert_eq!(tx.to, "bob");
        assert_eq!(tx.reason, "Payment");
    }

    #[test]
    fn test_insufficient_balance_returns_error() {
        let mut ledger = CakeLedger::new();
        let mut agent = make_agent("alice", 10.0, true);

        let err = ledger.spend(&mut agent, 20.0, "Overdraft").unwrap_err();
        assert_eq!(
            err,
            CakeError::InsufficientBalance {
                agent_id: "alice".into(),
                balance: 10.0,
                requested: 20.0
            }
        );
    }

    #[test]
    fn test_transfer_insufficient_balance() {
        let mut ledger = CakeLedger::new();
        let mut alice = make_agent("alice", 5.0, true);
        let mut bob = make_agent("bob", 0.0, true);

        let err = ledger
            .transfer(&mut alice, &mut bob, 10.0, "Fail")
            .unwrap_err();
        assert!(matches!(err, CakeError::InsufficientBalance { .. }));
    }

    #[test]
    fn test_invalid_amount_rejected() {
        let mut ledger = CakeLedger::new();
        let mut agent = make_agent("alice", 100.0, true);

        assert!(matches!(
            ledger.mint(&mut agent, 0.0, "Zero").unwrap_err(),
            CakeError::InvalidAmount(_)
        ));
        assert!(matches!(
            ledger.mint(&mut agent, -5.0, "Neg").unwrap_err(),
            CakeError::InvalidAmount(_)
        ));
        assert!(matches!(
            ledger.mint(&mut agent, f64::NAN, "NaN").unwrap_err(),
            CakeError::InvalidAmount(_)
        ));
        assert!(matches!(
            ledger.mint(&mut agent, f64::INFINITY, "Inf").unwrap_err(),
            CakeError::InvalidAmount(_)
        ));
        assert!(matches!(
            ledger.spend(&mut agent, f64::NAN, "NaN spend").unwrap_err(),
            CakeError::InvalidAmount(_)
        ));
        assert!(matches!(
            ledger.spend(&mut agent, 0.0, "Zero").unwrap_err(),
            CakeError::InvalidAmount(_)
        ));
        // Self-transfer: use two agents with same id to check id-based detection
        let mut dup = make_agent("alice", 100.0, true);
        assert!(matches!(
            ledger
                .transfer(&mut agent, &mut dup, 10.0, "Self")
                .unwrap_err(),
            CakeError::SelfTransfer(_)
        ));
        // NaN in transfer
        let mut bob = make_agent("bob", 0.0, true);
        assert!(matches!(
            ledger
                .transfer(&mut agent, &mut bob, f64::NAN, "NaN transfer")
                .unwrap_err(),
            CakeError::InvalidAmount(_)
        ));
    }

    #[test]
    fn test_dead_agent_cannot_transact() {
        let mut ledger = CakeLedger::new();
        let mut alive = make_agent("alive", 100.0, true);
        let mut dead = make_agent("dead", 100.0, false);

        assert!(matches!(
            ledger.mint(&mut dead, 50.0, "Dead mint").unwrap_err(),
            CakeError::AgentDead(_)
        ));
        assert!(matches!(
            ledger.spend(&mut dead, 10.0, "Dead spend").unwrap_err(),
            CakeError::AgentDead(_)
        ));
        assert!(matches!(
            ledger
                .transfer(&mut dead, &mut alive, 10.0, "Dead send")
                .unwrap_err(),
            CakeError::AgentDead(_)
        ));
        assert!(matches!(
            ledger
                .transfer(&mut alive, &mut dead, 10.0, "Send to dead")
                .unwrap_err(),
            CakeError::AgentDead(_)
        ));
    }

    #[test]
    fn test_transaction_history() {
        let mut ledger = CakeLedger::new();
        let mut alice = make_agent("alice", 0.0, true);
        let mut bob = make_agent("bob", 0.0, true);

        ledger.mint(&mut alice, 100.0, "Mint").unwrap();
        ledger.transfer(&mut alice, &mut bob, 30.0, "Gift").unwrap();
        ledger.spend(&mut bob, 10.0, "Burn").unwrap();

        assert_eq!(ledger.transactions().len(), 3);
        assert_eq!(ledger.transactions()[0].reason, "Mint");
        assert_eq!(ledger.transactions()[1].reason, "Gift");
        assert_eq!(ledger.transactions()[2].reason, "Burn");
        assert_eq!(ledger.total_supply(), 90.0); // 100 minted - 10 burned
    }

    #[test]
    fn test_with_history_reconstructs_supply() {
        let txs = vec![
            CakeTransaction::mint("a".into(), 100.0, "M1".into()).unwrap(),
            CakeTransaction::mint("b".into(), 50.0, "M2".into()).unwrap(),
            CakeTransaction::burn("a".into(), 20.0, "B1".into()).unwrap(),
        ];
        let ledger = CakeLedger::with_history(txs).unwrap();
        assert_eq!(ledger.total_supply(), 130.0);
    }

    #[test]
    fn test_with_history_rejects_nan_amount() {
        let txs = vec![CakeTransaction {
            from: String::new(),
            to: "a".into(),
            amount: f64::NAN,
            reason: "bad".into(),
            timestamp: Utc::now(),
        }];
        assert!(matches!(
            CakeLedger::with_history(txs).unwrap_err(),
            CakeError::InvalidTransaction(_)
        ));
    }

    #[test]
    fn test_with_history_rejects_infinite_amount() {
        let txs = vec![CakeTransaction {
            from: String::new(),
            to: "a".into(),
            amount: f64::INFINITY,
            reason: "bad".into(),
            timestamp: Utc::now(),
        }];
        assert!(matches!(
            CakeLedger::with_history(txs).unwrap_err(),
            CakeError::InvalidTransaction(_)
        ));
    }

    #[test]
    fn test_with_history_rejects_zero_amount() {
        let txs = vec![CakeTransaction {
            from: String::new(),
            to: "a".into(),
            amount: 0.0,
            reason: "bad".into(),
            timestamp: Utc::now(),
        }];
        assert!(matches!(
            CakeLedger::with_history(txs).unwrap_err(),
            CakeError::InvalidTransaction(_)
        ));
    }

    #[test]
    fn test_with_history_rejects_empty_from_and_to() {
        let txs = vec![CakeTransaction {
            from: String::new(),
            to: String::new(),
            amount: 10.0,
            reason: "bad".into(),
            timestamp: Utc::now(),
        }];
        assert!(matches!(
            CakeLedger::with_history(txs).unwrap_err(),
            CakeError::InvalidTransaction(_)
        ));
    }

    #[test]
    fn test_transaction_constructors_reject_invalid_amounts() {
        assert!(matches!(
            CakeTransaction::new("a".into(), "b".into(), 0.0, "bad".into()).unwrap_err(),
            CakeError::InvalidAmount(_)
        ));
        assert!(matches!(
            CakeTransaction::mint("a".into(), f64::NAN, "bad".into()).unwrap_err(),
            CakeError::InvalidAmount(_)
        ));
        assert!(matches!(
            CakeTransaction::burn("a".into(), f64::NEG_INFINITY, "bad".into()).unwrap_err(),
            CakeError::InvalidAmount(_)
        ));
    }

    #[test]
    fn test_reset_clears_everything() {
        let mut ledger = CakeLedger::new();
        let mut agent = make_agent("alice", 0.0, true);
        ledger.mint(&mut agent, 100.0, "Mint").unwrap();
        assert_eq!(ledger.total_supply(), 100.0);
        assert_eq!(ledger.transactions().len(), 1);

        ledger.reset();
        assert_eq!(ledger.total_supply(), 0.0);
        assert!(ledger.transactions().is_empty());
        // agent balance is NOT reset by ledger
        assert_eq!(agent.cake_balance, 100.0);
    }
}
