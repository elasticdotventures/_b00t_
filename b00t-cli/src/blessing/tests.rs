#[cfg(test)]
mod blessing_graph_tests {
    use super::super::*;
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

        let graph = BlessingGraph::from_toml(toml_str).expect("Should parse blessing graph TOML");

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
                BlessingNode {
                    id: "A".to_string(),
                    ..Default::default()
                },
                BlessingNode {
                    id: "B".to_string(),
                    ..Default::default()
                },
                BlessingNode {
                    id: "C".to_string(),
                    ..Default::default()
                },
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
                BlessingNode {
                    id: "A".to_string(),
                    ..Default::default()
                },
                BlessingNode {
                    id: "B".to_string(),
                    ..Default::default()
                },
                BlessingNode {
                    id: "C".to_string(),
                    ..Default::default()
                },
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

    /// Test 10: Blessing trifecta - usage notes
    #[test]
    fn test_blessing_trifecta_usage_notes() {
        let node = BlessingNode {
            id: "blessing:terraform-apply".to_string(),
            type_: "blessing".to_string(),
            usage_notes: Some("Use when you need to apply Terraform configurations. Requires terraform binary and AWS credentials.".to_string()),
            ..Default::default()
        };

        assert!(node.usage_notes.is_some());
        assert!(node.usage_notes.as_ref().unwrap().contains("Terraform"));
    }

    /// Test 11: Blessing trifecta - execute access
    #[test]
    fn test_blessing_trifecta_execute_access() {
        let exec_access = ExecuteAccess {
            binary: "/usr/bin/terraform".to_string(),
            tool_preference: None,
            bash_filters: vec![],
            allowed_args: vec![
                "apply".to_string(),
                "plan".to_string(),
                "destroy".to_string(),
            ],
            denied_args: vec!["login".to_string()],
            timeout_seconds: 600,
            max_cpu_percent: 80,
            max_memory_mb: 2048,
        };

        assert_eq!(exec_access.binary, "/usr/bin/terraform");
        assert!(exec_access.allowed_args.contains(&"apply".to_string()));
        assert!(exec_access.denied_args.contains(&"login".to_string()));
    }

    /// Test 12: Blessing trifecta - data permissions
    #[test]
    fn test_blessing_trifecta_data_permissions() {
        let data_perms = DataPermissions {
            readable_paths: vec![".terraform/".to_string(), "*.tf".to_string()],
            writable_paths: vec!["tfstate".to_string()],
            blocked_paths: vec!["/etc".to_string(), "/root".to_string()],
            requires_blessings: vec!["blessing:aws-credentials".to_string()],
            network_allowed_hosts: vec!["api.terraform.io".to_string()],
            requires_vpn: true,
        };

        assert!(
            data_perms
                .readable_paths
                .contains(&".terraform/".to_string())
        );
        assert!(data_perms.blocked_paths.contains(&"/root".to_string()));
        assert!(
            data_perms
                .requires_blessings
                .contains(&"blessing:aws-credentials".to_string())
        );
    }

    /// Test 13: Blessing node with complete trifecta
    #[test]
    fn test_blessing_node_trifecta_complete() {
        let node = BlessingNode {
            id: "blessing:terraform-apply".to_string(),
            type_: "blessing".to_string(),
            usage_notes: Some("Apply Terraform configs to AWS infrastructure".to_string()),
            execute_access: Some(ExecuteAccess {
                binary: "/usr/bin/terraform".to_string(),
                tool_preference: None,
                bash_filters: vec![],
                allowed_args: vec!["apply".to_string()],
                denied_args: vec![],
                timeout_seconds: 600,
                max_cpu_percent: 80,
                max_memory_mb: 2048,
            }),
            data_permissions: Some(DataPermissions {
                readable_paths: vec![".terraform/".to_string()],
                writable_paths: vec!["tfstate".to_string()],
                blocked_paths: vec!["/etc".to_string()],
                requires_blessings: vec!["blessing:aws-credentials".to_string()],
                network_allowed_hosts: vec![],
                requires_vpn: false,
            }),
            cost_tokens: 500,
            role_access: vec!["executor".to_string()],
            ..Default::default()
        };

        assert!(node.usage_notes.is_some());
        assert!(node.execute_access.is_some());
        assert!(node.data_permissions.is_some());
    }

    /// Test 14: Parse blessing trifecta from TOML
    #[test]
    fn test_parse_blessing_trifecta_from_toml() {
        let toml_str = r#"
[b00t]
name = "blessing-trifecta"

[[b00t.blessings]]
id = "blessing:terraform-apply"
type = "blessing"
usage_notes = "Apply Terraform configurations to AWS"

[b00t.blessings.execute_access]
binary = "/usr/bin/terraform"
allowed_args = ["apply", "plan"]
denied_args = ["login"]
timeout_seconds = 600

[b00t.blessings.data_permissions]
readable_paths = [".terraform/"]
writable_paths = ["tfstate"]
blocked_paths = ["/etc"]
requires_blessings = ["blessing:aws-credentials"]
requires_vpn = false
        "#;

        let graph =
            BlessingGraph::from_toml(toml_str).expect("Should parse blessing trifecta TOML");

        assert!(graph.nodes.len() > 0);
        let node = &graph.nodes[0];
        assert_eq!(node.id, "blessing:terraform-apply");
        assert!(node.usage_notes.is_some());
    }

    /// Test 15: Tool abstraction - prefer tofu over terraform
    #[test]
    fn test_tool_preference_abstraction() {
        let tool_pref = ToolPreference {
            tools: vec!["tofu".to_string(), "terraform".to_string()],
            require_preferred: false,
        };

        // Preference is ordered: tofu first, fallback to terraform
        assert_eq!(tool_pref.tools[0], "tofu");
        assert_eq!(tool_pref.tools[1], "terraform");
    }

    /// Test 16: Role-based bash safety filter - executor role
    #[test]
    fn test_bash_safety_filter_executor_role() {
        let filter = BashSafetyFilter {
            role: "executor".to_string(),
            allowed_commands: vec!["sed".to_string(), "grep".to_string(), "find".to_string()],
            denied_commands: vec!["rm".to_string(), "dd".to_string()],
            deny_by_default: true,
        };

        // sed is allowed
        assert!(filter.allowed_commands.contains(&"sed".to_string()));
        // rm is explicitly denied
        assert!(filter.denied_commands.contains(&"rm".to_string()));
        // deny_by_default means only whitelisted commands can run
        assert!(filter.deny_by_default);
    }

    /// Test 17: Role-based bash safety filter - observer role (read-only)
    #[test]
    fn test_bash_safety_filter_observer_role() {
        let filter = BashSafetyFilter {
            role: "observer".to_string(),
            allowed_commands: vec!["grep".to_string(), "cat".to_string(), "head".to_string()],
            denied_commands: vec!["sed".to_string(), "rm".to_string(), "chmod".to_string()],
            deny_by_default: true,
        };

        // observer can only read
        assert!(filter.allowed_commands.contains(&"grep".to_string()));
        // sed is blocked (modifying)
        assert!(filter.denied_commands.contains(&"sed".to_string()));
    }

    /// Test 18: Execute access with tool preference and bash filters
    #[test]
    fn test_execute_access_with_tool_preference_and_filters() {
        let exec_access = ExecuteAccess {
            binary: "/usr/bin/terraform".to_string(),
            tool_preference: Some(ToolPreference {
                tools: vec!["tofu".to_string(), "terraform".to_string()],
                require_preferred: false,
            }),
            bash_filters: vec![BashSafetyFilter {
                role: "executor".to_string(),
                allowed_commands: vec!["apply".to_string(), "plan".to_string()],
                denied_commands: vec!["destroy".to_string()],
                deny_by_default: true,
            }],
            allowed_args: vec!["apply".to_string()],
            denied_args: vec![],
            timeout_seconds: 600,
            max_cpu_percent: 80,
            max_memory_mb: 2048,
        };

        assert!(exec_access.tool_preference.is_some());
        assert_eq!(exec_access.bash_filters.len(), 1);
        assert_eq!(exec_access.bash_filters[0].role, "executor");
    }
}
