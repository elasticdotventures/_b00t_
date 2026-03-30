#[cfg(test)]
mod blessing_graph_tests {
    use super::super::*;
    use serde_json::json;

    /// Test 1: Parse blessing graph from TOML
    #[test]
    fn test_parse_blessing_graph_toml() {
        let toml_str = r#"
[b00t]
name = "blessing-graph"
type = "graph"

[[b00t.nodes]]
id = "step:inventory-scan"
type = "step"
cost_tokens = 500
role_access = ["executive", "developer"]

[[b00t.nodes]]
id = "agent:b00t-sandbox"
type = "agent"
constraint = "b00t_commands_only"
budget_tokens = 5000
role_access = ["executive"]

[[b00t.edges]]
from = "step:inventory-scan"
to = "agent:b00t-sandbox"
relationship = "enables"
        "#;

        let graph = BlessingGraph::from_toml(toml_str)
            .expect("Should parse blessing graph TOML");

        assert_eq!(graph.nodes.len(), 2);
        assert_eq!(graph.edges.len(), 1);
        assert_eq!(graph.nodes[0].id, "step:inventory-scan");
    }

    /// Test 2: Filter blessings by role
    #[test]
    fn test_filter_blessings_by_role() {
        let graph = BlessingGraph {
            nodes: vec![
                BlessingNode {
                    id: "skill:hive-cmdb".to_string(),
                    type_: "skill".to_string(),
                    cost_tokens: 0,
                    role_access: vec!["executive".to_string(), "developer".to_string()],
                    ..Default::default()
                },
                BlessingNode {
                    id: "agent:b00t-sandbox".to_string(),
                    type_: "agent".to_string(),
                    cost_tokens: 5000,
                    role_access: vec!["executive".to_string()],
                    ..Default::default()
                },
            ],
            edges: vec![],
        };

        let for_dev = graph.filter_by_role("developer");
        assert_eq!(for_dev.nodes.len(), 1);
        assert_eq!(for_dev.nodes[0].id, "skill:hive-cmdb");

        let for_exec = graph.filter_by_role("executive");
        assert_eq!(for_exec.nodes.len(), 2);
    }

    /// Test 3: Validate graph has no circular dependencies
    #[test]
    fn test_detect_circular_dependencies() {
        let graph = BlessingGraph {
            nodes: vec![
                BlessingNode { id: "A".to_string(), ..Default::default() },
                BlessingNode { id: "B".to_string(), ..Default::default() },
                BlessingNode { id: "C".to_string(), ..Default::default() },
            ],
            edges: vec![
                BlessingEdge {
                    from: "A".to_string(),
                    to: "B".to_string(),
                    relationship: "depends_on".to_string(),
                },
                BlessingEdge {
                    from: "B".to_string(),
                    to: "C".to_string(),
                    relationship: "depends_on".to_string(),
                },
                // Circular: C → A
                BlessingEdge {
                    from: "C".to_string(),
                    to: "A".to_string(),
                    relationship: "depends_on".to_string(),
                },
            ],
        };

        let cycles = graph.find_cycles();
        assert!(!cycles.is_empty(), "Should detect cycle A → B → C → A");
    }

    /// Test 4: Filter available blessings by inventory
    #[test]
    fn test_filter_blessings_by_inventory() {
        let graph = BlessingGraph {
            nodes: vec![
                BlessingNode {
                    id: "mcp:github".to_string(),
                    requires: vec!["github_token".to_string()],
                    ..Default::default()
                },
                BlessingNode {
                    id: "skill:hive-cmdb".to_string(),
                    requires: vec![],
                    ..Default::default()
                },
            ],
            edges: vec![],
        };

        let mut inventory_map = std::collections::BTreeMap::new();
        inventory_map.insert("github_token".to_string(), true);

        let available = graph.filter_by_inventory(&inventory_map);
        assert_eq!(available.nodes.len(), 2); // Both available

        inventory_map.insert("github_token".to_string(), false);
        let available = graph.filter_by_inventory(&inventory_map);
        assert_eq!(available.nodes.len(), 1); // Only skill available
    }

    /// Test 5: Serialize blessing graph back to TOML
    #[test]
    fn test_serialize_blessing_graph() {
        let graph = BlessingGraph {
            nodes: vec![BlessingNode {
                id: "step:test".to_string(),
                type_: "step".to_string(),
                cost_tokens: 100,
                ..Default::default()
            }],
            edges: vec![],
        };

        let toml_str = graph.to_toml().expect("Should serialize to TOML");
        assert!(toml_str.contains("step:test"));
    }

    /// Test 6: Dependency resolution (topological sort)
    #[test]
    fn test_topological_sort_blessings() {
        let graph = BlessingGraph {
            nodes: vec![
                BlessingNode { id: "A".to_string(), ..Default::default() },
                BlessingNode { id: "B".to_string(), ..Default::default() },
                BlessingNode { id: "C".to_string(), ..Default::default() },
            ],
            edges: vec![
                BlessingEdge {
                    from: "A".to_string(),
                    to: "B".to_string(),
                    relationship: "enables".to_string(),
                },
                BlessingEdge {
                    from: "B".to_string(),
                    to: "C".to_string(),
                    relationship: "enables".to_string(),
                },
            ],
        };

        let sorted = graph.topological_sort().expect("Should sort");
        // A must come before B, B before C
        let a_idx = sorted.iter().position(|n| n.id == "A").unwrap();
        let b_idx = sorted.iter().position(|n| n.id == "B").unwrap();
        let c_idx = sorted.iter().position(|n| n.id == "C").unwrap();
        assert!(a_idx < b_idx && b_idx < c_idx);
    }

    /// Test 7: Cost accumulation across blessing chain
    #[test]
    fn test_accumulate_blessing_costs() {
        let graph = BlessingGraph {
            nodes: vec![
                BlessingNode {
                    id: "A".to_string(),
                    cost_tokens: 1000,
                    ..Default::default()
                },
                BlessingNode {
                    id: "B".to_string(),
                    cost_tokens: 2000,
                    ..Default::default()
                },
            ],
            edges: vec![BlessingEdge {
                from: "A".to_string(),
                to: "B".to_string(),
                relationship: "enables".to_string(),
            }],
        };

        let total_cost = graph.total_cost(&["A", "B"]);
        assert_eq!(total_cost, 3000);
    }
}
