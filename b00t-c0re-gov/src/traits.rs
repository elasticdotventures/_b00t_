use async_trait::async_trait;

use crate::types::GateResult;

/// A governance gate checks whether an action is allowed.
/// Returns Allow, Deny, or Hook (agent should snapshot and wait).
#[async_trait]
pub trait GovernanceGate: Send + Sync {
    /// Unique name for this gate
    fn name(&self) -> &str;

    /// Check whether an action is allowed
    async fn check(&self, action: &str, context: &GateCheckContext) -> GateResult;

    /// Human-readable description of what this gate checks
    fn description(&self) -> &str {
        "No description provided"
    }
}

#[derive(Debug, Clone)]
pub struct GateCheckContext {
    pub agent_id: String,
    pub task: String,
    pub action: String,
    pub metadata: serde_json::Value,
}
