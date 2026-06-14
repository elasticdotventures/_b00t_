// b00t-ast/src/ontology.rs
//
// Ontology graph construction — takes extracted CodeElements and builds a
// knowledge graph with typed relationships suitable for codebase-memory-mcp.
//
// Graph stores:
//   - Nodes: CodeElements (functions, structs, enums, traits, impls, etc.)
//   - Edges: typed relationships (CONTAINS, CALLS, IMPLEMENTS, DEFINES, EXTENDS)
//
// Output format matches the codebase-memory-mcp index_repository input schema
// for direct integration in Phase 4.

use crate::{CodeElementKind, ExtractionResult};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// A node in the ontology graph
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OntologyNode {
    /// Unique node ID (same as qualified_name)
    pub id: String,
    /// Label for graph display
    pub label: String,
    /// Node type discriminator
    pub node_type: String,
    /// Source file path
    pub file_path: String,
    /// Start line
    pub start_line: usize,
    /// Doc comment excerpt (first line)
    pub doc_excerpt: String,
}

/// A typed relationship between two nodes
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OntologyEdge {
    /// Source node ID
    pub from: String,
    /// Target node ID
    pub to: String,
    /// Relationship type
    pub rel_type: String,
    /// Optional metadata (e.g., "async": true for calls)
    pub metadata: HashMap<String, String>,
}

/// Complete ontology graph
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OntologyGraph {
    /// Project name
    pub project: String,
    /// All nodes
    pub nodes: Vec<OntologyNode>,
    /// All edges
    pub edges: Vec<OntologyEdge>,
    /// Node count per type
    pub node_counts: HashMap<String, usize>,
}

impl OntologyGraph {
    /// Build an ontology graph from extraction results
    pub fn from_extraction(result: &ExtractionResult) -> Self {
        let mut graph = OntologyGraph {
            project: result.project_root.clone(),
            nodes: Vec::new(),
            edges: Vec::new(),
            node_counts: HashMap::new(),
        };

        // First pass: create nodes
        let mut name_to_id: HashMap<&str, &str> = HashMap::new();
        for element in &result.elements {
            let node_type = element_kind_name(&element.kind);
            let doc_excerpt = element
                .doc_comment
                .lines()
                .next()
                .unwrap_or("")
                .chars()
                .take(120)
                .collect();

            graph.nodes.push(OntologyNode {
                id: element.qualified_name.clone(),
                label: element.name.clone(),
                node_type: node_type.to_string(),
                file_path: element.file_path.clone(),
                start_line: element.start_line,
                doc_excerpt,
            });

            *graph.node_counts.entry(node_type.to_string()).or_insert(0) += 1;
            name_to_id.insert(&element.name, &element.qualified_name);
        }

        // Second pass: infer edges from element relationships
        for element in &result.elements {
            let from_id = &element.qualified_name;

            match &element.kind {
                CodeElementKind::Impl(impl_info) => {
                    // Implements relationship: impl Foo for Bar → IMPLEMENTS
                    if let Some(ref trait_name) = impl_info.trait_name {
                        let trait_qn = find_qualified(trait_name, name_to_id.clone());
                        if let Some(to_id) = trait_qn {
                            graph.edges.push(OntologyEdge {
                                from: from_id.clone(),
                                to: to_id,
                                rel_type: "IMPLEMENTS".to_string(),
                                metadata: HashMap::new(),
                            });
                        }
                    }

                    // Contains methods: IMPL_CONTAINS
                    for method_sig in &impl_info.items {
                        graph.edges.push(OntologyEdge {
                            from: from_id.clone(),
                            to: format!("{}::{}", from_id, method_sig.replace(' ', "_")),
                            rel_type: "IMPL_CONTAINS".to_string(),
                            metadata: HashMap::new(),
                        });
                    }
                }

                CodeElementKind::Struct(struct_info) => {
                    // DEFINES field types: struct field → DEFINED_BY → field type
                    for field in &struct_info.fields {
                        let field_type = field.ty.trim().trim_matches('&').to_string();
                        if let Some(to_id) = find_qualified(&field_type, name_to_id.clone()) {
                            graph.edges.push(OntologyEdge {
                                from: from_id.clone(),
                                to: to_id,
                                rel_type: "HAS_FIELD".to_string(),
                                metadata: {
                                    let mut m = HashMap::new();
                                    m.insert("field_name".to_string(), field.name.clone());
                                    m
                                },
                            });
                        }
                    }
                }

                CodeElementKind::Enum(enum_info) => {
                    // CONTAINS variants
                    for variant in &enum_info.variants {
                        graph.edges.push(OntologyEdge {
                            from: from_id.clone(),
                            to: format!("{}::{}", from_id, variant.name),
                            rel_type: "HAS_VARIANT".to_string(),
                            metadata: HashMap::new(),
                        });
                    }
                }

                CodeElementKind::Trait(trait_info) => {
                    // EXTENDS supertraits
                    for st in &trait_info.supertraits {
                        if let Some(to_id) = find_qualified(st, name_to_id.clone()) {
                            graph.edges.push(OntologyEdge {
                                from: from_id.clone(),
                                to: to_id,
                                rel_type: "EXTENDS".to_string(),
                                metadata: HashMap::new(),
                            });
                        }
                    }

                    // HAS_METHOD trait items
                    for item in &trait_info.items {
                        if let crate::TraitItem::Function { signature, .. } = item {
                            graph.edges.push(OntologyEdge {
                                from: from_id.clone(),
                                to: format!("{}::{}", from_id, signature),
                                rel_type: "HAS_METHOD".to_string(),
                                metadata: HashMap::new(),
                            });
                        }
                    }
                }

                CodeElementKind::Module(module_info) => {
                    // CONTAINS child items
                    for child in &module_info.children {
                        graph.edges.push(OntologyEdge {
                            from: from_id.clone(),
                            to: child.clone(),
                            rel_type: "CONTAINS".to_string(),
                            metadata: HashMap::new(),
                        });
                    }
                }

                _ => {}
            }
        }

        graph
    }

    /// Export the graph as JSON structure for codebase-memory-mcp ingestion
    pub fn to_mcp_payload(&self) -> serde_json::Value {
        let nodes: Vec<serde_json::Value> = self
            .nodes
            .iter()
            .map(|n| {
                serde_json::json!({
                    "id": n.id,
                    "label": n.label,
                    "node_type": n.node_type,
                    "file_path": n.file_path,
                    "start_line": n.start_line,
                    "doc_excerpt": n.doc_excerpt,
                })
            })
            .collect();

        let edges: Vec<serde_json::Value> = self
            .edges
            .iter()
            .map(|e| {
                serde_json::json!({
                    "from": e.from,
                    "to": e.to,
                    "rel_type": e.rel_type,
                    "metadata": e.metadata,
                })
            })
            .collect();

        serde_json::json!({
            "project": self.project,
            "nodes": nodes,
            "edges": edges,
        })
    }
}

fn element_kind_name(kind: &CodeElementKind) -> &'static str {
    match kind {
        CodeElementKind::Function(_) => "Function",
        CodeElementKind::Struct(_) => "Struct",
        CodeElementKind::Enum(_) => "Enum",
        CodeElementKind::Trait(_) => "Trait",
        CodeElementKind::Impl(_) => "Impl",
        CodeElementKind::Const(_) => "Const",
        CodeElementKind::Type(_) => "TypeAlias",
        CodeElementKind::Macro(_) => "Macro",
        CodeElementKind::Module(_) => "Module",
    }
}

/// Find a qualified name for a short type name, falling back to using the short name
fn find_qualified(name: &str, name_to_id: HashMap<&str, &str>) -> Option<String> {
    // Try exact match on short name
    if let Some(&qn) = name_to_id.get(name) {
        return Some(qn.to_string());
    }
    // Try stripping generic params: Vec<T> → Vec
    let base = name.split('<').next().unwrap_or(name).trim();
    if let Some(&qn) = name_to_id.get(base) {
        return Some(qn.to_string());
    }
    // No match — could be std/prelude type, return as-is
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::*;

    fn make_fn(name: &str, mod_path: &str) -> CodeElement {
        CodeElement {
            qualified_name: format!("{mod_path}::{name}"),
            name: name.to_string(),
            kind: CodeElementKind::Function(FunctionInfo {
                signature: format!("fn {name}()"),
                is_async: false,
                is_unsafe: false,
                generics: vec![],
                params: vec![],
                return_type: "()".to_string(),
                attributes: vec![],
            }),
            file_path: "src/lib.rs".to_string(),
            start_line: 1,
            end_line: 3,
            doc_comment: String::new(),
            visibility: "pub".to_string(),
        }
    }

    fn make_struct(name: &str, mod_path: &str) -> CodeElement {
        CodeElement {
            qualified_name: format!("{mod_path}::{name}"),
            name: name.to_string(),
            kind: CodeElementKind::Struct(StructInfo {
                fields: vec![],
                generics: vec![],
                attributes: vec![],
            }),
            file_path: "src/lib.rs".to_string(),
            start_line: 1,
            end_line: 1,
            doc_comment: String::new(),
            visibility: "pub".to_string(),
        }
    }

    #[test]
    fn test_ontology_empty_result() {
        let result = ExtractionResult {
            project_root: "test".to_string(),
            elements: vec![],
            file_count: 0,
            counts: HashMap::new(),
            errors: vec![],
        };
        let graph = OntologyGraph::from_extraction(&result);
        assert!(graph.nodes.is_empty());
        assert!(graph.edges.is_empty());
    }

    #[test]
    fn test_ontology_single_function() {
        let elements = vec![make_fn("hello", "crate")];
        let result = ExtractionResult {
            project_root: "test".to_string(),
            elements,
            file_count: 1,
            counts: HashMap::new(),
            errors: vec![],
        };
        let graph = OntologyGraph::from_extraction(&result);
        assert_eq!(graph.nodes.len(), 1);
        assert_eq!(graph.nodes[0].node_type, "Function");
    }

    #[test]
    fn test_ontology_struct_with_impl() {
        let elements = vec![
            make_struct("Server", "http"),
            CodeElement {
                qualified_name: "http::impl_Server".to_string(),
                name: "impl Server".to_string(),
                kind: CodeElementKind::Impl(ImplInfo {
                    trait_name: None,
                    self_ty: "Server".to_string(),
                    items: vec!["fn start()".to_string()],
                }),
                file_path: "src/http.rs".to_string(),
                start_line: 10,
                end_line: 20,
                doc_comment: String::new(),
                visibility: "pub".to_string(),
            },
        ];
        let result = ExtractionResult {
            project_root: "test".to_string(),
            elements,
            file_count: 1,
            counts: HashMap::new(),
            errors: vec![],
        };
        let graph = OntologyGraph::from_extraction(&result);
        assert!(graph.nodes.len() >= 2);
        // Should have at least one IMPL_CONTAINS edge
        let impl_edges: Vec<_> = graph
            .edges
            .iter()
            .filter(|e| e.rel_type == "IMPL_CONTAINS")
            .collect();
        assert!(impl_edges.len() >= 1);
    }

    #[test]
    fn test_mcp_payload_output() {
        let elements = vec![make_fn("main", "app")];
        let result = ExtractionResult {
            project_root: "test".to_string(),
            elements,
            file_count: 1,
            counts: HashMap::new(),
            errors: vec![],
        };
        let graph = OntologyGraph::from_extraction(&result);
        let payload = graph.to_mcp_payload();
        assert!(payload.get("nodes").and_then(|n| n.as_array()).is_some());
        assert_eq!(payload["nodes"][0]["id"].as_str().unwrap(), "app::main");
    }
}
