// b00t-cli/src/blessing/rag/graph.rs
// GraphRAG: Capability dependency graph for blessing composition
// Provides node/edge management, BFS traversal, and cycle detection

// 🦨 TODO: Integration with blessing composition (Task 9)
// 🦨 TODO: Constraint validation (budget overflow)
// 🦨 TODO: Remediation suggestions for conflicts

use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet, VecDeque};

/// Node in the capability dependency graph
/// Represents a capability (blessing, skill, or agent)
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct GraphRAGNode {
    /// Unique identifier (blessing ID, skill ID, or agent ID)
    pub id: String,

    /// Node type: "blessing", "skill", or "agent"
    pub label: String,

    /// Serialized properties (blessing metadata, skill config, agent capabilities)
    pub properties: serde_json::Value,
}

impl GraphRAGNode {
    /// Create a new graph node
    ///
    /// # Arguments
    /// * `id` - Unique identifier
    /// * `label` - Node type ("blessing", "skill", "agent")
    /// * `properties` - Serialized properties as JSON value
    pub fn new(id: String, label: String, properties: serde_json::Value) -> Self {
        GraphRAGNode {
            id,
            label,
            properties,
        }
    }
}

/// Edge in the capability dependency graph
/// Represents a relationship between two capabilities
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct GraphRAGEdge {
    /// Source node ID
    pub from: String,

    /// Target node ID
    pub to: String,

    /// Relationship type: "requires", "enables", "competes_with"
    pub relationship: String,
}

impl GraphRAGEdge {
    /// Create a new graph edge
    ///
    /// # Arguments
    /// * `from` - Source node ID
    /// * `to` - Target node ID
    /// * `relationship` - Relationship type
    pub fn new(from: String, to: String, relationship: String) -> Self {
        GraphRAGEdge {
            from,
            to,
            relationship,
        }
    }
}

/// GraphRAG: Capability dependency graph
/// Stores nodes and edges for blessing composition and dependency resolution
pub struct GraphRAG {
    /// All capability nodes indexed by ID
    /// Key: node ID, Value: GraphRAGNode
    pub nodes: HashMap<String, GraphRAGNode>,

    /// All dependency edges
    pub edges: Vec<GraphRAGEdge>,
}

impl GraphRAG {
    /// Create an empty capability dependency graph
    ///
    /// # Returns
    /// New GraphRAG with empty nodes and edges
    pub fn new() -> Self {
        GraphRAG {
            nodes: HashMap::new(),
            edges: Vec::new(),
        }
    }

    /// Add a capability node to the graph
    ///
    /// # Arguments
    /// * `id` - Unique identifier (blessing ID, skill ID, agent ID)
    /// * `label` - Node type ("blessing", "skill", "agent")
    /// * `properties` - Serialized properties as JSON
    ///
    /// # Behavior
    /// - Overwrites existing node if ID already exists
    /// - Properties are stored as-is for later retrieval
    pub fn add_node(&mut self, id: String, label: String, properties: serde_json::Value) {
        let node = GraphRAGNode::new(id.clone(), label, properties);
        self.nodes.insert(id, node);
    }

    /// Add a dependency edge to the graph
    ///
    /// # Arguments
    /// * `from` - Source node ID (prerequisite capability)
    /// * `to` - Target node ID (dependent capability)
    /// * `relationship` - Relationship type ("requires", "enables", "competes_with")
    ///
    /// # Behavior
    /// - Both nodes must exist (validate with `node_exists()` before adding)
    /// - Edge represents: `to` requires `from` (dependency)
    /// - No deduplication: duplicate edges are allowed
    pub fn add_edge(&mut self, from: String, to: String, relationship: String) {
        let edge = GraphRAGEdge::new(from, to, relationship);
        self.edges.push(edge);
    }

    /// Check if a node exists in the graph
    fn node_exists(&self, id: &str) -> bool {
        self.nodes.contains_key(id)
    }

    /// Traverse the dependency graph starting from a node (BFS)
    ///
    /// # Arguments
    /// * `node_id` - Starting node ID
    ///
    /// # Returns
    /// Result<Vec<String>> - BFS order of reachable nodes (dependencies first)
    /// Returns Err if node_id doesn't exist
    ///
    /// # Behavior
    /// 1. Start from node_id
    /// 2. BFS traversal following edges where `from` is current node
    /// 3. Return nodes in discovery order (dependencies first)
    /// 4. Each node appears only once
    pub fn traverse_from(&self, node_id: &str) -> Result<Vec<String>, String> {
        if !self.node_exists(node_id) {
            return Err(format!("Node not found: {}", node_id));
        }

        let mut visited = HashSet::new();
        let mut queue = VecDeque::new();
        let mut result = Vec::new();

        queue.push_back(node_id.to_string());
        visited.insert(node_id.to_string());

        while let Some(current) = queue.pop_front() {
            result.push(current.clone());

            // Find all edges where `from` == current
            for edge in &self.edges {
                if edge.from == current && !visited.contains(&edge.to) {
                    visited.insert(edge.to.clone());
                    queue.push_back(edge.to.clone());
                }
            }
        }

        Ok(result)
    }

    /// Detect cycles in the dependency graph
    ///
    /// # Returns
    /// Result<(), String> - Ok if acyclic (DAG), Err with cycle description if cycle found
    ///
    /// # Algorithm
    /// Uses DFS with color marking:
    /// - White (0): unvisited
    /// - Gray (1): visiting (in current DFS path)
    /// - Black (2): finished
    ///
    /// # Cycle Example
    /// If A→B→C→A exists, returns Err describing the cycle
    pub fn find_cycles(&self) -> Result<(), String> {
        let mut colors: HashMap<String, u8> = HashMap::new();

        // Initialize all nodes as white (0)
        for node_id in self.nodes.keys() {
            colors.insert(node_id.clone(), 0);
        }

        // Run DFS from each unvisited node
        for node_id in self.nodes.keys() {
            if colors[node_id] == 0 {
                if let Err(cycle) = self.dfs_detect_cycle(node_id, &mut colors) {
                    return Err(cycle);
                }
            }
        }

        Ok(())
    }

    /// DFS helper for cycle detection
    /// Colors: 0=white, 1=gray, 2=black
    fn dfs_detect_cycle(
        &self,
        node_id: &str,
        colors: &mut HashMap<String, u8>,
    ) -> Result<(), String> {
        // Mark as visiting (gray)
        colors.insert(node_id.to_string(), 1);

        // Visit all neighbors (edges where `from` == current)
        for edge in &self.edges {
            if edge.from == node_id {
                let neighbor = &edge.to;
                let neighbor_color = *colors.get(neighbor).unwrap_or(&0);

                match neighbor_color {
                    0 => {
                        // Unvisited: recurse
                        self.dfs_detect_cycle(neighbor, colors)?;
                    }
                    1 => {
                        // Visiting: back edge found (cycle)
                        return Err(format!(
                            "Cycle detected: {} -> {} (and back to {})",
                            node_id, neighbor, node_id
                        ));
                    }
                    _ => {
                        // Finished: skip
                    }
                }
            }
        }

        // Mark as finished (black)
        colors.insert(node_id.to_string(), 2);
        Ok(())
    }

    /// Compute topological order for dependency resolution
    ///
    /// # Returns
    /// Result<Vec<String>, String> - Nodes in topological order (dependencies first)
    /// Returns Err if graph contains cycles
    ///
    /// # Algorithm
    /// Kahn's algorithm:
    /// 1. Compute in-degree for each node
    /// 2. Enqueue nodes with in-degree 0
    /// 3. Process queue, decrement in-degrees
    /// 4. If any node still has in-degree > 0, graph has cycle
    pub fn topological_order(&self) -> Result<Vec<String>, String> {
        // Verify no cycles first
        self.find_cycles()?;

        // Compute in-degrees (count incoming edges to each node)
        let mut in_degree: HashMap<String, usize> = HashMap::new();
        for node_id in self.nodes.keys() {
            in_degree.insert(node_id.clone(), 0);
        }

        // Count edges where `to` == node_id
        for edge in &self.edges {
            *in_degree.get_mut(&edge.to).unwrap_or(&mut 0) += 1;
        }

        // Enqueue all nodes with in-degree 0
        let mut queue: VecDeque<String> = VecDeque::new();
        for (node_id, &degree) in &in_degree {
            if degree == 0 {
                queue.push_back(node_id.clone());
            }
        }

        let mut result = Vec::new();

        while let Some(current) = queue.pop_front() {
            result.push(current.clone());

            // Decrement in-degree of neighbors (edges where `from` == current)
            for edge in &self.edges {
                if edge.from == current {
                    let in_deg = in_degree.get_mut(&edge.to).unwrap();
                    *in_deg -= 1;
                    if *in_deg == 0 {
                        queue.push_back(edge.to.clone());
                    }
                }
            }
        }

        // Check if all nodes were processed (would catch cycles)
        if result.len() != self.nodes.len() {
            return Err("Cycle detected during topological sort".to_string());
        }

        Ok(result)
    }
}

impl Default for GraphRAG {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_graph_rag_new() {
        let graph = GraphRAG::new();

        assert_eq!(graph.nodes.len(), 0);
        assert_eq!(graph.edges.len(), 0);
    }

    #[test]
    fn test_add_node() {
        let mut graph = GraphRAG::new();

        graph.add_node(
            "blessing:auth".to_string(),
            "blessing".to_string(),
            serde_json::json!({"quality": 0.95}),
        );

        assert_eq!(graph.nodes.len(), 1);
        assert!(graph.nodes.contains_key("blessing:auth"));

        let node = graph.nodes.get("blessing:auth").unwrap();
        assert_eq!(node.label, "blessing");
        assert_eq!(node.properties["quality"], 0.95);
    }

    #[test]
    fn test_add_edge() {
        let mut graph = GraphRAG::new();

        // Add nodes first
        graph.add_node(
            "blessing:auth".to_string(),
            "blessing".to_string(),
            serde_json::json!({}),
        );
        graph.add_node(
            "blessing:compute".to_string(),
            "blessing".to_string(),
            serde_json::json!({}),
        );

        // Add edge
        graph.add_edge(
            "blessing:auth".to_string(),
            "blessing:compute".to_string(),
            "requires".to_string(),
        );

        assert_eq!(graph.edges.len(), 1);

        let edge = &graph.edges[0];
        assert_eq!(edge.from, "blessing:auth");
        assert_eq!(edge.to, "blessing:compute");
        assert_eq!(edge.relationship, "requires");
    }

    #[test]
    fn test_traverse_from() {
        let mut graph = GraphRAG::new();

        // Create nodes: A -> B -> C
        graph.add_node("A".to_string(), "blessing".to_string(), serde_json::json!({}));
        graph.add_node("B".to_string(), "blessing".to_string(), serde_json::json!({}));
        graph.add_node("C".to_string(), "blessing".to_string(), serde_json::json!({}));

        // Create edges: A requires B, B requires C
        graph.add_edge("B".to_string(), "A".to_string(), "requires".to_string());
        graph.add_edge("C".to_string(), "B".to_string(), "requires".to_string());

        // Traverse from A (should get A only, no dependencies via edges from A)
        let result = graph.traverse_from("A").unwrap();
        assert_eq!(result, vec!["A"]);

        // Traverse from B (should get B and A)
        let result = graph.traverse_from("B").unwrap();
        assert_eq!(result.len(), 2);
        assert!(result.contains(&"B".to_string()));
        assert!(result.contains(&"A".to_string()));

        // Traverse from C (should get C, B, A in some order)
        let result = graph.traverse_from("C").unwrap();
        assert_eq!(result.len(), 3);
        assert!(result.contains(&"C".to_string()));
        assert!(result.contains(&"B".to_string()));
        assert!(result.contains(&"A".to_string()));
    }

    #[test]
    fn test_traverse_from_nonexistent_node() {
        let graph = GraphRAG::new();

        let result = graph.traverse_from("nonexistent");

        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Node not found"));
    }

    #[test]
    fn test_find_cycles_acyclic() {
        let mut graph = GraphRAG::new();

        // Create acyclic graph: A -> B -> C
        graph.add_node("A".to_string(), "blessing".to_string(), serde_json::json!({}));
        graph.add_node("B".to_string(), "blessing".to_string(), serde_json::json!({}));
        graph.add_node("C".to_string(), "blessing".to_string(), serde_json::json!({}));

        graph.add_edge("A".to_string(), "B".to_string(), "requires".to_string());
        graph.add_edge("B".to_string(), "C".to_string(), "requires".to_string());

        let result = graph.find_cycles();

        assert!(result.is_ok());
    }

    #[test]
    fn test_find_cycles_cyclic() {
        let mut graph = GraphRAG::new();

        // Create cyclic graph: A -> B -> C -> A
        graph.add_node("A".to_string(), "blessing".to_string(), serde_json::json!({}));
        graph.add_node("B".to_string(), "blessing".to_string(), serde_json::json!({}));
        graph.add_node("C".to_string(), "blessing".to_string(), serde_json::json!({}));

        graph.add_edge("A".to_string(), "B".to_string(), "requires".to_string());
        graph.add_edge("B".to_string(), "C".to_string(), "requires".to_string());
        graph.add_edge("C".to_string(), "A".to_string(), "requires".to_string());

        let result = graph.find_cycles();

        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Cycle detected"));
    }

    #[test]
    fn test_topological_order_dependency_resolution() {
        let mut graph = GraphRAG::new();

        // Create DAG:
        //   C
        //   |
        //   B
        //   |
        //   A
        graph.add_node("A".to_string(), "blessing".to_string(), serde_json::json!({}));
        graph.add_node("B".to_string(), "blessing".to_string(), serde_json::json!({}));
        graph.add_node("C".to_string(), "blessing".to_string(), serde_json::json!({}));

        // C depends on B, B depends on A
        graph.add_edge("B".to_string(), "C".to_string(), "requires".to_string());
        graph.add_edge("A".to_string(), "B".to_string(), "requires".to_string());

        let result = graph.topological_order().unwrap();

        // Should be: A, B, C (dependencies first)
        assert_eq!(result.len(), 3);
        let a_idx = result.iter().position(|x| x == "A").unwrap();
        let b_idx = result.iter().position(|x| x == "B").unwrap();
        let c_idx = result.iter().position(|x| x == "C").unwrap();

        assert!(a_idx < b_idx);
        assert!(b_idx < c_idx);
    }

    #[test]
    fn test_topological_order_with_cycle_fails() {
        let mut graph = GraphRAG::new();

        // Create cyclic graph: A -> B -> A
        graph.add_node("A".to_string(), "blessing".to_string(), serde_json::json!({}));
        graph.add_node("B".to_string(), "blessing".to_string(), serde_json::json!({}));

        graph.add_edge("A".to_string(), "B".to_string(), "requires".to_string());
        graph.add_edge("B".to_string(), "A".to_string(), "requires".to_string());

        let result = graph.topological_order();

        assert!(result.is_err());
    }
}
