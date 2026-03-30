// blessing/prayer.rs
// Prayer workflow: agents request blessings with policy evaluation
// Path 1: Agent prayer - /negotiate blessing:X with policy checks
// Path 2: Executive grant - direct /negotiate blessing:X override

use crate::blessing::{BlessingGraph, BlessingNode, ExecuteAccess, DataPermissions};
use std::collections::{BTreeMap, HashSet};

/// Result of a blessing prayer (agent request)
#[derive(Debug, Clone, PartialEq)]
pub struct BlessingPrayerResult {
    /// Was the blessing granted?
    pub granted: bool,
    /// The blessing node (if granted)
    pub blessing: Option<BlessingNode>,
    /// If denied, explain why
    pub denial_reason: Option<String>,
    /// Suggestions for getting approval (if denied)
    pub suggestions: Vec<String>,
}

/// Policy evaluation for blessing request
#[derive(Debug, Clone, PartialEq)]
pub enum PolicyCheckResult {
    Approved,
    DeniedRoleNotAllowed(String),     // role doesn't have access
    DeniedBlessingMissing(String),    // blessing doesn't exist
    DeniedBudgetInsufficient(u32, u32),  // available, required
    DeniedPrerequisiteMissing(String),  // required blessing
    DeniedExecutiveVoteRequired,        // needs voting
}

impl PolicyCheckResult {
    pub fn is_approved(&self) -> bool {
        matches!(self, PolicyCheckResult::Approved)
    }

    pub fn denial_reason(&self) -> Option<String> {
        match self {
            PolicyCheckResult::Approved => None,
            PolicyCheckResult::DeniedRoleNotAllowed(role) => {
                Some(format!("Role '{}' not allowed to request this blessing", role))
            }
            PolicyCheckResult::DeniedBlessingMissing(id) => {
                Some(format!("Blessing '{}' does not exist", id))
            }
            PolicyCheckResult::DeniedBudgetInsufficient(avail, required) => {
                Some(format!("Budget insufficient: {} available, {} required", avail, required))
            }
            PolicyCheckResult::DeniedPrerequisiteMissing(prereq) => {
                Some(format!("Missing prerequisite blessing: {}", prereq))
            }
            PolicyCheckResult::DeniedExecutiveVoteRequired => {
                Some("This blessing requires executive approval (voting)".to_string())
            }
        }
    }
}

/// Blessing request context (what agent is asking for)
#[derive(Debug, Clone)]
pub struct BlessingRequest {
    /// Which blessing is being requested
    pub blessing_id: String,
    /// Agent's role
    pub agent_role: String,
    /// Agent's current blessings
    pub agent_blessings: Vec<String>,
    /// Agent's available budget (tokens)
    pub available_budget: u32,
    /// Is this executive override? (bypass policy checks)
    pub executive_override: bool,
}

/// Blessing evaluator: checks policy and grants/denies blessings
pub struct BlessingEvaluator {
    /// The blessing graph (all available blessings)
    graph: BlessingGraph,
    /// Policy rules (role-blessing mappings, budget limits)
    policy: BlessingPolicy,
}

/// Policy rules for blessing granting
#[derive(Debug, Clone, Default)]
pub struct BlessingPolicy {
    /// Which roles can request which blessings
    pub role_blessings: BTreeMap<String, HashSet<String>>,
    /// Global token budget per day
    pub global_daily_budget: u32,
    /// Blessings that require executive vote
    pub requires_voting: HashSet<String>,
}

impl BlessingEvaluator {
    /// Create new evaluator with graph and policy
    pub fn new(graph: BlessingGraph, policy: BlessingPolicy) -> Self {
        BlessingEvaluator { graph, policy }
    }

    /// Evaluate a blessing prayer (agent request)
    pub fn evaluate_prayer(&self, req: &BlessingRequest) -> BlessingPrayerResult {
        // Step 1: Check if blessing exists
        let blessing = self.graph.nodes.iter().find(|n| n.id == req.blessing_id);
        if blessing.is_none() {
            return BlessingPrayerResult {
                granted: false,
                blessing: None,
                denial_reason: PolicyCheckResult::DeniedBlessingMissing(req.blessing_id.clone()).denial_reason(),
                suggestions: vec![
                    "Check available blessings with: /negotiate list".to_string(),
                    "Request executive to create this blessing".to_string(),
                ],
            };
        }

        let blessing = blessing.unwrap();

        // Step 2: Check if role can request this blessing
        let policy_check = self.check_role_access(&req.agent_role, &req.blessing_id);
        if !policy_check.is_approved() {
            return BlessingPrayerResult {
                granted: false,
                blessing: None,
                denial_reason: policy_check.denial_reason(),
                suggestions: self.suggest_alternatives(&req.agent_role),
            };
        }

        // Step 3: Check if budget is available
        if let Some(cost) = self.get_blessing_cost(&req.blessing_id) {
            if req.available_budget < cost {
                return BlessingPrayerResult {
                    granted: false,
                    blessing: None,
                    denial_reason: PolicyCheckResult::DeniedBudgetInsufficient(req.available_budget, cost)
                        .denial_reason(),
                    suggestions: vec!["Wait for daily budget refresh".to_string()],
                };
            }
        }

        // Step 4: Check if prerequisites are met
        for prereq in &blessing.requires {
            if !req.agent_blessings.contains(prereq) {
                return BlessingPrayerResult {
                    granted: false,
                    blessing: None,
                    denial_reason: PolicyCheckResult::DeniedPrerequisiteMissing(prereq.clone()).denial_reason(),
                    suggestions: vec![format!("Request blessing first: /negotiate {}", prereq)],
                };
            }
        }

        // Step 5: Check if executive vote required
        if self.policy.requires_voting.contains(&req.blessing_id) && !req.executive_override {
            return BlessingPrayerResult {
                granted: false,
                blessing: None,
                denial_reason: PolicyCheckResult::DeniedExecutiveVoteRequired.denial_reason(),
                suggestions: vec!["This blessing requires /vote approval from executive".to_string()],
            };
        }

        // All checks passed - grant blessing
        BlessingPrayerResult {
            granted: true,
            blessing: Some(blessing.clone()),
            denial_reason: None,
            suggestions: vec![],
        }
    }

    /// Check if role is allowed to request this blessing
    fn check_role_access(&self, role: &str, blessing_id: &str) -> PolicyCheckResult {
        // First check: blessing must exist
        if self.graph.nodes.iter().find(|n| n.id == blessing_id).is_none() {
            return PolicyCheckResult::DeniedBlessingMissing(blessing_id.to_string());
        }

        // Check policy: does role have blessing?
        if let Some(allowed) = self.policy.role_blessings.get(role) {
            if allowed.contains(blessing_id) {
                return PolicyCheckResult::Approved;
            } else {
                return PolicyCheckResult::DeniedRoleNotAllowed(format!("{} cannot request {}", role, blessing_id));
            }
        }

        // No policy rule = deny
        PolicyCheckResult::DeniedRoleNotAllowed(format!("Role '{}' has no policies defined", role))
    }

    /// Get token cost for blessing
    fn get_blessing_cost(&self, blessing_id: &str) -> Option<u32> {
        self.graph
            .nodes
            .iter()
            .find(|n| n.id == blessing_id)
            .map(|n| n.cost_tokens)
    }

    /// Suggest alternative blessings for this role
    fn suggest_alternatives(&self, role: &str) -> Vec<String> {
        if let Some(allowed) = self.policy.role_blessings.get(role) {
            let available: Vec<String> = allowed.iter().take(3).cloned().collect();
            if available.is_empty() {
                vec!["Request executive to define blessings for this role".to_string()]
            } else {
                vec![
                    format!("Available blessings: {}", available.join(", ")),
                    "Request with one of these instead".to_string(),
                ]
            }
        } else {
            vec!["Role not configured in policy".to_string()]
        }
    }
}

#[cfg(test)]
mod tests;
