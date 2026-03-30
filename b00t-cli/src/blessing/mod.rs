// b00t-cli/src/blessing/mod.rs
// Blessing graph: directed acyclic graph of capabilities

use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, VecDeque};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct BlessingGraph {
    pub nodes: Vec<BlessingNode>,
    pub edges: Vec<BlessingEdge>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
pub struct BlessingNode {
    pub id: String,
    #[serde(rename = "type")]
    pub type_: String,
    pub datum: Option<String>,
    pub cost_tokens: u32,
    #[serde(default)]
    pub cost_usd: f32,
    #[serde(default)]
    pub role_access: Vec<String>,
    #[serde(default)]
    pub requires: Vec<String>,
    #[serde(default)]
    pub constraint: Option<String>,
    #[serde(default)]
    pub budget_tokens: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct BlessingEdge {
    pub from: String,
    pub to: String,
    pub relationship: String,
}

impl BlessingGraph {
    /// Parse blessing graph from TOML string
    pub fn from_toml(toml_str: &str) -> Result<Self, Box<dyn std::error::Error>> {
        let value: toml::Value = toml::from_str(toml_str)?;

        let nodes: Vec<BlessingNode> = value
            .get("b00t")
            .and_then(|b| b.get("nodes"))
            .and_then(|n| n.as_array())
            .unwrap_or(&vec![])
            .iter()
            .filter_map(|n| {
                let id = n.get("id")?.as_str()?.to_string();
                let type_ = n.get("type")?.as_str()?.to_string();
                let cost_tokens = n
                    .get("cost_tokens")
                    .and_then(|c| c.as_integer())
                    .unwrap_or(0) as u32;
                let role_access = n
                    .get("role_access")
                    .and_then(|r| r.as_array())
                    .unwrap_or(&vec![])
                    .iter()
                    .filter_map(|v| v.as_str().map(|s| s.to_string()))
                    .collect();

                Some(BlessingNode {
                    id,
                    type_,
                    cost_tokens,
                    role_access,
                    ..Default::default()
                })
            })
            .collect();

        let edges: Vec<BlessingEdge> = value
            .get("b00t")
            .and_then(|b| b.get("edges"))
            .and_then(|e| e.as_array())
            .unwrap_or(&vec![])
            .iter()
            .filter_map(|e| {
                Some(BlessingEdge {
                    from: e.get("from")?.as_str()?.to_string(),
                    to: e.get("to")?.as_str()?.to_string(),
                    relationship: e.get("relationship")?.as_str()?.to_string(),
                })
            })
            .collect();

        Ok(BlessingGraph { nodes, edges })
    }

    /// Filter blessings accessible by a specific role
    pub fn filter_by_role(&self, role: &str) -> BlessingGraph {
        let filtered_nodes: Vec<BlessingNode> = self
            .nodes
            .iter()
            .filter(|n| n.role_access.is_empty() || n.role_access.contains(&role.to_string()))
            .cloned()
            .collect();

        let filtered_ids: std::collections::HashSet<String> =
            filtered_nodes.iter().map(|n| n.id.clone()).collect();

        let filtered_edges: Vec<BlessingEdge> = self
            .edges
            .iter()
            .filter(|e| filtered_ids.contains(&e.from) && filtered_ids.contains(&e.to))
            .cloned()
            .collect();

        BlessingGraph {
            nodes: filtered_nodes,
            edges: filtered_edges,
        }
    }

    /// Filter blessings by inventory (what's actually available)
    pub fn filter_by_inventory(&self, inventory: &BTreeMap<String, bool>) -> BlessingGraph {
        let filtered_nodes: Vec<BlessingNode> = self
            .nodes
            .iter()
            .filter(|n| {
                // Node is available if all its requirements are met
                n.requires.iter().all(|req| inventory.get(req).copied().unwrap_or(false))
            })
            .cloned()
            .collect();

        let filtered_ids: std::collections::HashSet<String> =
            filtered_nodes.iter().map(|n| n.id.clone()).collect();

        let filtered_edges: Vec<BlessingEdge> = self
            .edges
            .iter()
            .filter(|e| filtered_ids.contains(&e.from) && filtered_ids.contains(&e.to))
            .cloned()
            .collect();

        BlessingGraph {
            nodes: filtered_nodes,
            edges: filtered_edges,
        }
    }

    /// Find cycles in the graph (should be empty for valid DAG)
    pub fn find_cycles(&self) -> Vec<Vec<String>> {
        let mut cycles = Vec::new();
        let mut visited = std::collections::HashSet::new();
        let mut rec_stack = std::collections::HashSet::new();

        for node in &self.nodes {
            if !visited.contains(&node.id) {
                Self::dfs_cycle(
                    &node.id,
                    self,
                    &mut visited,
                    &mut rec_stack,
                    &mut vec![],
                    &mut cycles,
                );
            }
        }

        cycles
    }

    fn dfs_cycle(
        node: &str,
        graph: &BlessingGraph,
        visited: &mut std::collections::HashSet<String>,
        rec_stack: &mut std::collections::HashSet<String>,
        path: &mut Vec<String>,
        cycles: &mut Vec<Vec<String>>,
    ) {
        visited.insert(node.to_string());
        rec_stack.insert(node.to_string());
        path.push(node.to_string());

        for edge in &graph.edges {
            if edge.from == node {
                if !visited.contains(&edge.to) {
                    Self::dfs_cycle(&edge.to, graph, visited, rec_stack, path, cycles);
                } else if rec_stack.contains(&edge.to) {
                    // Found cycle
                    if let Some(pos) = path.iter().position(|n| n == &edge.to) {
                        cycles.push(path[pos..].to_vec());
                    }
                }
            }
        }

        rec_stack.remove(node);
        path.pop();
    }

    /// Topological sort of blessings (dependencies first)
    pub fn topological_sort(&self) -> Result<Vec<BlessingNode>, String> {
        if !self.find_cycles().is_empty() {
            return Err("Graph contains cycles".to_string());
        }

        let mut in_degree = BTreeMap::new();
        for node in &self.nodes {
            in_degree.insert(&node.id, 0);
        }

        for edge in &self.edges {
            *in_degree.get_mut(&edge.to).unwrap() += 1;
        }

        let mut queue: VecDeque<String> = in_degree
            .iter()
            .filter(|&(_, deg)| *deg == 0)
            .map(|(id, _)| (*id).clone())
            .collect();

        let mut sorted = Vec::new();

        while let Some(node_id) = queue.pop_front() {
            if let Some(node) = self.nodes.iter().find(|n| n.id == node_id) {
                sorted.push(node.clone());
            }

            for edge in self.edges.iter().filter(|e| e.from == node_id) {
                let new_degree = in_degree[&edge.to] - 1;
                *in_degree.get_mut(&edge.to).unwrap() = new_degree;
                if new_degree == 0 {
                    queue.push_back(edge.to.clone());
                }
            }
        }

        if sorted.len() != self.nodes.len() {
            return Err("Not all nodes could be sorted (cycle detected)".to_string());
        }

        Ok(sorted)
    }

    /// Total cost of using blessings
    pub fn total_cost(&self, blessing_ids: &[&str]) -> u32 {
        blessing_ids
            .iter()
            .filter_map(|id| self.nodes.iter().find(|n| n.id == *id))
            .map(|n| n.cost_tokens)
            .sum()
    }

    /// Serialize to TOML
    pub fn to_toml(&self) -> Result<String, Box<dyn std::error::Error>> {
        let mut toml_map = toml::map::Map::new();

        let mut b00t_map = toml::map::Map::new();
        b00t_map.insert("name".to_string(), toml::Value::String("blessing-graph".to_string()));

        let nodes_array: Vec<toml::Value> = self
            .nodes
            .iter()
            .map(|n| {
                let mut node_map = toml::map::Map::new();
                node_map.insert("id".to_string(), toml::Value::String(n.id.clone()));
                node_map.insert("type".to_string(), toml::Value::String(n.type_.clone()));
                node_map.insert("cost_tokens".to_string(), toml::Value::Integer(n.cost_tokens as i64));
                node_map
            })
            .map(toml::Value::Table)
            .collect();

        b00t_map.insert("nodes".to_string(), toml::Value::Array(nodes_array));
        toml_map.insert("b00t".to_string(), toml::Value::Table(b00t_map));

        Ok(toml::to_string_pretty(&toml_map)?)
    }
}

pub mod prompts;
pub mod irontology;

#[cfg(test)]
mod tests;
