// blessing/irontology.rs
// Validation system for blessing graph consistency and constraint satisfaction
// Ensures blessings form a valid DAG with no unresolvable dependencies

use crate::blessing::BlessingGraph;
// 🤓 BlessingNode is available inside mod tests via `use super::*;` (Rust 2024 glob-vis semantics)
use std::collections::{BTreeMap, HashSet};
#[cfg(test)]
use crate::blessing::BlessingNode;

/// Validation result with detailed diagnostics
#[derive(Debug, Clone, PartialEq)]
pub struct ValidationResult {
    pub valid: bool,
    pub errors: Vec<ValidationError>,
    pub warnings: Vec<ValidationWarning>,
}

/// Validation error - blocks blessing execution
#[derive(Debug, Clone, PartialEq)]
pub enum ValidationError {
    CircularDependency(Vec<String>),        // cycle path
    UnresolvableDependency(String, String), // blessing, missing requirement
    InvalidRole(String),                    // role not in any blessing access list
    BudgetExceeded(u32, u32),               // total, limit
    DuplicateBlessingId(String),
}

impl std::fmt::Display for ValidationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ValidationError::CircularDependency(cycle) => {
                write!(f, "Circular dependency: {}", cycle.join(" -> "))
            }
            ValidationError::UnresolvableDependency(blessing, req) => {
                write!(f, "Blessing '{}' requires unresolvable '{}'", blessing, req)
            }
            ValidationError::InvalidRole(role) => {
                write!(f, "Role '{}' not accessible by any blessing", role)
            }
            ValidationError::BudgetExceeded(total, limit) => {
                write!(f, "Total budget {} exceeds limit {}", total, limit)
            }
            ValidationError::DuplicateBlessingId(id) => {
                write!(f, "Duplicate blessing ID: {}", id)
            }
        }
    }
}

/// Validation warning - should be addressed
#[derive(Debug, Clone, PartialEq)]
pub enum ValidationWarning {
    UnusedBlessing(String),             // not required by anything
    InaccessibleToRole(String, String), // blessing, role
    HighCostBlessing(String, u32),      // blessing, cost
}

impl std::fmt::Display for ValidationWarning {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ValidationWarning::UnusedBlessing(id) => {
                write!(f, "Blessing '{}' is not required by any other blessing", id)
            }
            ValidationWarning::InaccessibleToRole(blessing, role) => {
                write!(
                    f,
                    "Blessing '{}' is not accessible to role '{}'",
                    blessing, role
                )
            }
            ValidationWarning::HighCostBlessing(id, cost) => {
                write!(f, "Blessing '{}' has high cost: {} tokens", id, cost)
            }
        }
    }
}

/// Blessing graph validator
pub struct BlessingValidator {
    graph: BlessingGraph,
    budget_limit: u32,
}

impl BlessingValidator {
    /// Create a new validator for a blessing graph
    pub fn new(graph: BlessingGraph, budget_limit: u32) -> Self {
        BlessingValidator {
            graph,
            budget_limit,
        }
    }

    /// Run all validation checks
    pub fn validate(&self) -> ValidationResult {
        let mut errors = Vec::new();
        let mut warnings = Vec::new();

        // Check 1: No duplicate IDs
        if let Some(dup_id) = self.check_duplicate_ids() {
            errors.push(ValidationError::DuplicateBlessingId(dup_id));
        }

        // Check 2: No circular dependencies
        for cycle in self.find_cycles() {
            errors.push(ValidationError::CircularDependency(cycle));
        }

        // Check 3: All requires are resolvable
        for (blessing_id, missing) in self.check_unresolvable_deps() {
            for req in missing {
                errors.push(ValidationError::UnresolvableDependency(
                    blessing_id.clone(),
                    req,
                ));
            }
        }

        // Check 4: Total budget within limits
        let total_cost: u32 = self.graph.nodes.iter().map(|n| n.cost_tokens).sum();
        if total_cost > self.budget_limit {
            errors.push(ValidationError::BudgetExceeded(
                total_cost,
                self.budget_limit,
            ));
        }

        // Warning: Unused blessings
        for unused_id in self.find_unused_blessings() {
            warnings.push(ValidationWarning::UnusedBlessing(unused_id));
        }

        // Warning: High cost blessings
        for (id, cost) in self.find_high_cost_blessings(500) {
            warnings.push(ValidationWarning::HighCostBlessing(id, cost));
        }

        ValidationResult {
            valid: errors.is_empty(),
            errors,
            warnings,
        }
    }

    /// Check for duplicate blessing IDs
    fn check_duplicate_ids(&self) -> Option<String> {
        let mut seen = HashSet::new();
        for node in &self.graph.nodes {
            if !seen.insert(node.id.clone()) {
                return Some(node.id.clone());
            }
        }
        None
    }

    /// Find circular dependencies (cycles in DAG)
    fn find_cycles(&self) -> Vec<Vec<String>> {
        let mut cycles = Vec::new();
        let mut visited = HashSet::new();
        let mut rec_stack = HashSet::new();

        for node in &self.graph.nodes {
            if !visited.contains(&node.id) {
                let mut path = Vec::new();
                self.dfs_detect_cycle(
                    &node.id,
                    &mut visited,
                    &mut rec_stack,
                    &mut path,
                    &mut cycles,
                );
            }
        }

        cycles
    }

    /// DFS helper for cycle detection
    fn dfs_detect_cycle(
        &self,
        node_id: &str,
        visited: &mut HashSet<String>,
        rec_stack: &mut HashSet<String>,
        path: &mut Vec<String>,
        cycles: &mut Vec<Vec<String>>,
    ) {
        visited.insert(node_id.to_string());
        rec_stack.insert(node_id.to_string());
        path.push(node_id.to_string());

        // Get dependencies
        if let Some(node) = self.graph.nodes.iter().find(|n| n.id == node_id) {
            for req in &node.requires {
                if !visited.contains(req) {
                    self.dfs_detect_cycle(req, visited, rec_stack, path, cycles);
                } else if rec_stack.contains(req) {
                    // Found cycle: from req back to req through current path
                    if let Some(start_idx) = path.iter().position(|x| x == req) {
                        let cycle: Vec<String> = path[start_idx..].to_vec();
                        cycles.push(cycle);
                    }
                }
            }
        }

        rec_stack.remove(node_id);
        path.pop();
    }

    /// Check for unresolvable dependencies
    fn check_unresolvable_deps(&self) -> BTreeMap<String, Vec<String>> {
        let available: HashSet<String> = self.graph.nodes.iter().map(|n| n.id.clone()).collect();
        let mut unresolvable = BTreeMap::new();

        for node in &self.graph.nodes {
            let missing: Vec<String> = node
                .requires
                .iter()
                .filter(|req| !available.contains(*req))
                .cloned()
                .collect();

            if !missing.is_empty() {
                unresolvable.insert(node.id.clone(), missing);
            }
        }

        unresolvable
    }

    /// Find blessings not required by any other blessing
    fn find_unused_blessings(&self) -> Vec<String> {
        let required: HashSet<String> = self
            .graph
            .nodes
            .iter()
            .flat_map(|n| n.requires.clone())
            .collect();

        self.graph
            .nodes
            .iter()
            .filter(|n| !required.contains(&n.id))
            .map(|n| n.id.clone())
            .collect()
    }

    /// Find blessings with cost exceeding threshold
    fn find_high_cost_blessings(&self, threshold: u32) -> Vec<(String, u32)> {
        self.graph
            .nodes
            .iter()
            .filter(|n| n.cost_tokens > threshold)
            .map(|n| (n.id.clone(), n.cost_tokens))
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_valid_acyclic_blessing_graph() {
        let graph = BlessingGraph {
            nodes: vec![
                BlessingNode {
                    id: "blessing:observe".to_string(),
                    type_: "capability".to_string(),
                    datum: None,
                    cost_tokens: 100,
                    cost_usd: 0.0,
                    role_access: vec!["observer".to_string()],
                    requires: vec![],
                    constraint: None,
                    budget_tokens: None,
                    usage_notes: None,
                    execute_access: None,
                    data_permissions: None,
                },
                BlessingNode {
                    id: "blessing:analyze".to_string(),
                    type_: "capability".to_string(),
                    datum: None,
                    cost_tokens: 200,
                    cost_usd: 0.0,
                    role_access: vec!["analyst".to_string()],
                    requires: vec!["blessing:observe".to_string()],
                    constraint: None,
                    budget_tokens: None,
                    usage_notes: None,
                    execute_access: None,
                    data_permissions: None,
                },
            ],
            edges: vec![],
        };

        let validator = BlessingValidator::new(graph, 10000);
        let result = validator.validate();

        assert!(result.valid);
        assert!(result.errors.is_empty());
    }

    #[test]
    fn test_circular_dependency_detection() {
        let graph = BlessingGraph {
            nodes: vec![
                BlessingNode {
                    id: "blessing:a".to_string(),
                    type_: "capability".to_string(),
                    datum: None,
                    cost_tokens: 100,
                    cost_usd: 0.0,
                    role_access: vec![],
                    requires: vec!["blessing:b".to_string()],
                    constraint: None,
                    budget_tokens: None,
                    usage_notes: None,
                    execute_access: None,
                    data_permissions: None,
                },
                BlessingNode {
                    id: "blessing:b".to_string(),
                    type_: "capability".to_string(),
                    datum: None,
                    cost_tokens: 100,
                    cost_usd: 0.0,
                    role_access: vec![],
                    requires: vec!["blessing:a".to_string()],
                    constraint: None,
                    budget_tokens: None,
                    usage_notes: None,
                    execute_access: None,
                    data_permissions: None,
                },
            ],
            edges: vec![],
        };

        let validator = BlessingValidator::new(graph, 10000);
        let result = validator.validate();

        assert!(!result.valid);
        assert!(!result.errors.is_empty());
        assert!(matches!(
            result.errors[0],
            ValidationError::CircularDependency(_)
        ));
    }

    #[test]
    fn test_unresolvable_dependency() {
        let graph = BlessingGraph {
            nodes: vec![BlessingNode {
                id: "blessing:analyze".to_string(),
                type_: "capability".to_string(),
                datum: None,
                cost_tokens: 200,
                cost_usd: 0.0,
                role_access: vec![],
                requires: vec!["blessing:missing".to_string()],
                constraint: None,
                budget_tokens: None,
                usage_notes: None,
                execute_access: None,
                data_permissions: None,
            }],
            edges: vec![],
        };

        let validator = BlessingValidator::new(graph, 10000);
        let result = validator.validate();

        assert!(!result.valid);
        assert!(
            result
                .errors
                .iter()
                .any(|e| { matches!(e, ValidationError::UnresolvableDependency(_, _)) })
        );
    }

    #[test]
    fn test_budget_limit_exceeded() {
        let graph = BlessingGraph {
            nodes: vec![
                BlessingNode {
                    id: "blessing:a".to_string(),
                    type_: "capability".to_string(),
                    datum: None,
                    cost_tokens: 6000,
                    cost_usd: 0.0,
                    role_access: vec![],
                    requires: vec![],
                    constraint: None,
                    budget_tokens: None,
                    usage_notes: None,
                    execute_access: None,
                    data_permissions: None,
                },
                BlessingNode {
                    id: "blessing:b".to_string(),
                    type_: "capability".to_string(),
                    datum: None,
                    cost_tokens: 5000,
                    cost_usd: 0.0,
                    role_access: vec![],
                    requires: vec![],
                    constraint: None,
                    budget_tokens: None,
                    usage_notes: None,
                    execute_access: None,
                    data_permissions: None,
                },
            ],
            edges: vec![],
        };

        let validator = BlessingValidator::new(graph, 10000);
        let result = validator.validate();

        assert!(!result.valid);
        assert!(
            result
                .errors
                .iter()
                .any(|e| { matches!(e, ValidationError::BudgetExceeded(_, _)) })
        );
    }

    #[test]
    fn test_duplicate_blessing_ids() {
        let graph = BlessingGraph {
            nodes: vec![
                BlessingNode {
                    id: "blessing:dup".to_string(),
                    type_: "capability".to_string(),
                    datum: None,
                    cost_tokens: 100,
                    cost_usd: 0.0,
                    role_access: vec![],
                    requires: vec![],
                    constraint: None,
                    budget_tokens: None,
                    usage_notes: None,
                    execute_access: None,
                    data_permissions: None,
                },
                BlessingNode {
                    id: "blessing:dup".to_string(),
                    type_: "capability".to_string(),
                    datum: None,
                    cost_tokens: 100,
                    cost_usd: 0.0,
                    role_access: vec![],
                    requires: vec![],
                    constraint: None,
                    budget_tokens: None,
                    usage_notes: None,
                    execute_access: None,
                    data_permissions: None,
                },
            ],
            edges: vec![],
        };

        let validator = BlessingValidator::new(graph, 10000);
        let result = validator.validate();

        assert!(!result.valid);
        assert!(
            result
                .errors
                .iter()
                .any(|e| { matches!(e, ValidationError::DuplicateBlessingId(_)) })
        );
    }

    #[test]
    fn test_warnings_for_unused_blessings() {
        let graph = BlessingGraph {
            nodes: vec![
                BlessingNode {
                    id: "blessing:root".to_string(),
                    type_: "capability".to_string(),
                    datum: None,
                    cost_tokens: 100,
                    cost_usd: 0.0,
                    role_access: vec![],
                    requires: vec![],
                    constraint: None,
                    budget_tokens: None,
                    usage_notes: None,
                    execute_access: None,
                    data_permissions: None,
                },
                BlessingNode {
                    id: "blessing:unused".to_string(),
                    type_: "capability".to_string(),
                    datum: None,
                    cost_tokens: 100,
                    cost_usd: 0.0,
                    role_access: vec![],
                    requires: vec![],
                    constraint: None,
                    budget_tokens: None,
                    usage_notes: None,
                    execute_access: None,
                    data_permissions: None,
                },
            ],
            edges: vec![],
        };

        let validator = BlessingValidator::new(graph, 10000);
        let result = validator.validate();

        assert!(result.valid);
        assert!(
            result
                .warnings
                .iter()
                .any(|w| { matches!(w, ValidationWarning::UnusedBlessing(_)) })
        );
    }
}
