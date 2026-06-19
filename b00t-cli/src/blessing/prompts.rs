// blessing/prompts.rs
// Role-based prompt generation for blessing discovery and orchestration

use crate::blessing::{BlessingGraph, BlessingNode};
use crate::inventory::Inventory;

/// Prompt generation context: role + current system state
#[derive(Debug, Clone)]
pub struct PromptContext {
    pub agent_role: String,
    pub current_blessings: Vec<String>,
    pub available_blessings: Vec<String>,
    pub missing_blessings: Vec<String>,
    pub total_budget: u32,
    pub available_budget: u32,
}

impl PromptContext {
    /// Build context from role, blessing graph, and inventory
    pub fn build(role: &str, graph: &BlessingGraph, inventory: &Inventory) -> Self {
        // Filter blessings by role
        let filtered = graph.filter_by_role(role);

        // Build a simple availability map from inventory
        // Tools and MCPs that are "present" are considered available
        let mut available_map: std::collections::BTreeMap<String, bool> =
            std::collections::BTreeMap::new();

        for (tool_name, tool) in &inventory.tools {
            available_map.insert(format!("tool:{}", tool_name), tool.present);
        }

        for (mcp_name, mcp) in &inventory.mcp_servers {
            available_map.insert(format!("mcp:{}", mcp_name), mcp.present);
        }

        for (auth_name, auth) in &inventory.auth {
            available_map.insert(format!("auth:{}", auth_name), auth.present);
        }

        let available_by_inventory = graph.filter_by_inventory(&available_map);

        // Find intersection: accessible by role AND available in system
        let current_blessings: Vec<String> = filtered
            .nodes
            .iter()
            .filter(|node| available_by_inventory.nodes.iter().any(|n| n.id == node.id))
            .map(|n| n.id.clone())
            .collect();

        // Missing: accessible by role but NOT available
        let missing_blessings: Vec<String> = filtered
            .nodes
            .iter()
            .filter(|node| !available_by_inventory.nodes.iter().any(|n| n.id == node.id))
            .map(|n| n.id.clone())
            .collect();

        // All accessible
        let available_blessings: Vec<String> =
            filtered.nodes.iter().map(|n| n.id.clone()).collect();

        let total_budget = graph.nodes.iter().map(|n| n.cost_tokens).sum();
        let available_budget = 1000000; // 🤓 TODO: query from b00t hive budget controller

        PromptContext {
            agent_role: role.to_string(),
            current_blessings,
            available_blessings,
            missing_blessings,
            total_budget,
            available_budget,
        }
    }
}

/// System blessing inventory prompt - what can this agent do?
pub fn capability_summary(ctx: &PromptContext) -> String {
    let mut prompt = format!("## Your Blessings as {} Agent\n\n", ctx.agent_role);

    if !ctx.current_blessings.is_empty() {
        prompt.push_str("### ✅ Available Capabilities\n");
        prompt.push_str("You have access to the following blessings:\n");
        for blessing in &ctx.current_blessings {
            prompt.push_str(&format!("- `{}`\n", blessing));
        }
    }

    if !ctx.missing_blessings.is_empty() {
        prompt.push_str("\n### ⏳ Required but Unavailable\n");
        prompt.push_str("These blessings are needed but not currently available:\n");
        for blessing in &ctx.missing_blessings {
            prompt.push_str(&format!("- `{}`\n", blessing));
        }
        prompt.push_str(
            "\nTo request these, use: `/negotiate blessing:<name>` or `/crew blessing:<name>`\n",
        );
    }

    prompt.push_str(&format!(
        "\n### 💰 Budget Status\n{} / {} tokens available\n",
        ctx.available_budget, ctx.total_budget
    ));

    prompt
}

/// Decision support prompt - should I attempt this action?
pub fn decision_guard_prompt(ctx: &PromptContext, required_blessings: &[String]) -> String {
    let mut prompt = String::from("## Can I Execute This Action?\n\n");

    let mut has_all = true;
    let mut missing = Vec::new();

    for req in required_blessings {
        if ctx.current_blessings.contains(req) {
            prompt.push_str(&format!("✅ Have blessing: `{}`\n", req));
        } else {
            has_all = false;
            missing.push(req.clone());
            prompt.push_str(&format!("❌ Missing blessing: `{}`\n", req));
        }
    }

    if has_all {
        prompt.push_str("\n✨ **All requirements met!** You can proceed with this action.\n");
    } else {
        prompt.push_str("\n🚫 **Cannot proceed.** Request missing blessings first:\n");
        for blessing in missing {
            prompt.push_str(&format!("   `/negotiate {}`\n", blessing));
        }
    }

    prompt
}

/// Discovery prompt - what can I learn about?
pub fn discovery_prompt(ctx: &PromptContext, graph: &BlessingGraph) -> String {
    let mut prompt = String::from("## Blessing Discovery\n\n");
    prompt.push_str("As a ");
    prompt.push_str(&ctx.agent_role);
    prompt.push_str(" agent, you can work toward these capabilities:\n\n");

    for node in &graph.nodes {
        if ctx.available_blessings.iter().any(|b| b == &node.id) {
            let status = if ctx.current_blessings.contains(&node.id) {
                "✅"
            } else {
                "⏳"
            };

            prompt.push_str(&format!(
                "{} **{}** (costs {} tokens)\n",
                status, node.id, node.cost_tokens
            ));

            // Show requires
            if !node.requires.is_empty() {
                prompt.push_str("  Depends on:\n");
                for req in &node.requires {
                    prompt.push_str(&format!("    - {}\n", req));
                }
            }
        }
    }

    prompt
}

/// Prayer/request prompt - how to ask for help
pub fn request_prompt(ctx: &PromptContext) -> String {
    let mut prompt = String::from("## How to Request Blessings\n\n");
    prompt.push_str("Use k0mmand3r protocol to negotiate blessings:\n\n");

    prompt.push_str("### Single Blessing Request\n");
    prompt.push_str("```\n/negotiate blessing:name\n```\n\n");

    prompt.push_str("### Voting on Sensitive Actions\n");
    prompt.push_str("```\n/vote on blessing:execute-transition-safely\n```\n\n");

    prompt.push_str("### Delegating to Sandbox\n");
    prompt.push_str("```\n/delegate step:apply-config to agent:b00t-sandbox\n```\n\n");

    prompt.push_str("### Check Agent Status\n");
    prompt.push_str("```\n/status from agent:executive\n```\n\n");

    prompt.push_str(&format!(
        "Current role: `{}`\nYour accessible blessings: {}\n",
        ctx.agent_role,
        ctx.current_blessings.len()
    ));

    prompt
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_capability_summary_generation() {
        let ctx = PromptContext {
            agent_role: "executive".to_string(),
            current_blessings: vec!["blessing:observe-infrastructure".to_string()],
            available_blessings: vec![
                "blessing:observe-infrastructure".to_string(),
                "blessing:execute-transition".to_string(),
            ],
            missing_blessings: vec!["blessing:execute-transition".to_string()],
            total_budget: 10000,
            available_budget: 5000,
        };

        let summary = capability_summary(&ctx);
        assert!(summary.contains("executive"));
        assert!(summary.contains("observe-infrastructure"));
        assert!(summary.contains("execute-transition"));
        assert!(summary.contains("5000 / 10000"));
    }

    #[test]
    fn test_decision_guard_prompt_with_all_blessings() {
        let ctx = PromptContext {
            agent_role: "executor".to_string(),
            current_blessings: vec![
                "blessing:observe-infrastructure".to_string(),
                "blessing:execute-transition".to_string(),
            ],
            available_blessings: vec![],
            missing_blessings: vec![],
            total_budget: 1000,
            available_budget: 500,
        };

        let prompt = decision_guard_prompt(
            &ctx,
            &vec![
                "blessing:observe-infrastructure".to_string(),
                "blessing:execute-transition".to_string(),
            ],
        );

        assert!(prompt.contains("✅"));
        assert!(prompt.contains("All requirements met"));
    }

    #[test]
    fn test_decision_guard_prompt_with_missing_blessings() {
        let ctx = PromptContext {
            agent_role: "executor".to_string(),
            current_blessings: vec!["blessing:observe-infrastructure".to_string()],
            available_blessings: vec![],
            missing_blessings: vec!["blessing:execute-transition".to_string()],
            total_budget: 1000,
            available_budget: 500,
        };

        let prompt = decision_guard_prompt(&ctx, &vec!["blessing:execute-transition".to_string()]);

        assert!(prompt.contains("❌"));
        assert!(prompt.contains("Cannot proceed"));
        assert!(prompt.contains("blessing:execute-transition"));
    }

    #[test]
    fn test_request_prompt_format() {
        let ctx = PromptContext {
            agent_role: "observer".to_string(),
            current_blessings: vec![],
            available_blessings: vec![],
            missing_blessings: vec![],
            total_budget: 1000,
            available_budget: 500,
        };

        let prompt = request_prompt(&ctx);
        assert!(prompt.contains("/negotiate blessing:"));
        assert!(prompt.contains("/vote on blessing:"));
        assert!(prompt.contains("/delegate step:"));
        assert!(prompt.contains("observer"));
    }

    #[test]
    fn test_discovery_prompt_lists_accessible_blessings() {
        let ctx = PromptContext {
            agent_role: "observer".to_string(),
            current_blessings: vec!["blessing:observe".to_string()],
            available_blessings: vec!["blessing:observe".to_string(), "blessing:audit".to_string()],
            missing_blessings: vec!["blessing:audit".to_string()],
            total_budget: 1000,
            available_budget: 500,
        };

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
                    id: "blessing:audit".to_string(),
                    type_: "capability".to_string(),
                    datum: None,
                    cost_tokens: 200,
                    cost_usd: 0.0,
                    role_access: vec!["observer".to_string()],
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

        let prompt = discovery_prompt(&ctx, &graph);
        assert!(prompt.contains("observer"));
        assert!(prompt.contains("blessing:observe"));
        assert!(prompt.contains("blessing:audit"));
    }
}
