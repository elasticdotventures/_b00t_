// b00t-ast/src/lib.rs
//
// Rust AST extraction — syn-based parser that walks Rust source files,
// extracts function signatures, structs, enums, traits, impls, doc comments,
// and builds an ontology graph for codebase-memory-mcp integration.
//
// Architecture:
//   1. Walk a directory tree (walkdir)
//   2. Parse each .rs file with syn
//   3. Walk the AST with syn::visit to extract code elements
//   4. Serialize extracted elements as structured data (serde)
//   5. Output as JSON for ontology graph construction (Phase 3)
//      or feed directly to codebase-memory-mcp index_repository (Phase 4)

pub mod extract;
pub mod ontology;
pub mod walker;

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// A single code element extracted from the AST
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CodeElement {
    /// Unique qualified name: crate::module::name
    pub qualified_name: String,
    /// Local name
    pub name: String,
    /// Element kind
    pub kind: CodeElementKind,
    /// Source file path relative to project root
    pub file_path: String,
    /// 1-indexed start line
    pub start_line: usize,
    /// End line
    pub end_line: usize,
    /// Doc comment (/// or //!) extracted from attributes
    pub doc_comment: String,
    /// Visibility: pub, pub(crate), private, etc.
    pub visibility: String,
}

/// Kinds of code elements the extractor recognizes
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum CodeElementKind {
    Function(FunctionInfo),
    Struct(StructInfo),
    Enum(EnumInfo),
    Trait(TraitInfo),
    Impl(ImplInfo),
    Const(ConstInfo),
    Type(TypeInfo),
    Macro(MacroInfo),
    Module(ModuleInfo),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FunctionInfo {
    pub signature: String,
    pub is_async: bool,
    pub is_unsafe: bool,
    pub generics: Vec<String>,
    pub params: Vec<FnParam>,
    pub return_type: String,
    pub attributes: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FnParam {
    pub name: String,
    pub ty: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StructInfo {
    pub fields: Vec<StructField>,
    pub generics: Vec<String>,
    pub attributes: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StructField {
    pub name: String,
    pub ty: String,
    pub visibility: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EnumInfo {
    pub variants: Vec<EnumVariant>,
    pub generics: Vec<String>,
    pub attributes: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EnumVariant {
    pub name: String,
    pub fields: Vec<FnParam>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TraitInfo {
    pub items: Vec<TraitItem>,
    pub generics: Vec<String>,
    pub supertraits: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum TraitItem {
    Function {
        signature: String,
        has_default: bool,
    },
    Type {
        name: String,
    },
    Const {
        name: String,
        ty: String,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImplInfo {
    pub trait_name: Option<String>,
    pub self_ty: String,
    pub items: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConstInfo {
    pub ty: String,
    pub value: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TypeInfo {
    pub aliased_ty: String,
    pub generics: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MacroInfo {
    pub path: String,
    pub body_preview: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModuleInfo {
    pub is_inline: bool,
    pub children: Vec<String>,
}

/// Complete extraction result for one project
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExtractionResult {
    pub project_root: String,
    pub elements: Vec<CodeElement>,
    pub file_count: usize,
    pub counts: HashMap<String, usize>,
    pub errors: Vec<String>,
}

impl ExtractionResult {
    /// Serialize extraction result as JSON string
    pub fn to_json(&self) -> String {
        serde_json::to_string_pretty(self).unwrap_or_else(|_| "{}".to_string())
    }
}

/// Run full pipeline: walk → extract → ontology graph
pub fn run_extraction(project_root: &str) -> Result<ExtractionResult, anyhow::Error> {
    let root = std::path::Path::new(project_root);
    let config = walker::WalkConfig::default();
    // Count source files separately so file_count reflects files scanned, not elements found
    let source_files = walker::collect_source_files(root, &config);
    let file_count = source_files.len();
    let elements = walker::walk_and_extract(root, &config)?;

    let mut counts = HashMap::new();
    for el in &elements {
        let kind = match &el.kind {
            CodeElementKind::Function(_) => "function",
            CodeElementKind::Struct(_) => "struct",
            CodeElementKind::Enum(_) => "enum",
            CodeElementKind::Trait(_) => "trait",
            CodeElementKind::Impl(_) => "impl",
            CodeElementKind::Const(_) => "const",
            CodeElementKind::Type(_) => "type_alias",
            CodeElementKind::Macro(_) => "macro",
            CodeElementKind::Module(_) => "module",
        };
        *counts.entry(kind.to_string()).or_insert(0) += 1;
    }

    Ok(ExtractionResult {
        project_root: project_root.to_string(),
        file_count,
        elements,
        counts,
        errors: vec![],
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_run_extraction_empty_dir() {
        let dir = tempfile::tempdir().unwrap();
        let result = run_extraction(dir.path().to_str().unwrap()).unwrap();
        assert_eq!(result.file_count, 0);
    }

    #[test]
    fn test_run_extraction_creates_json() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(dir.path().join("src")).unwrap();
        std::fs::write(
            dir.path().join("src/lib.rs"),
            "pub fn greet() -> String { \"hi\".into() }",
        )
        .unwrap();

        let result = run_extraction(dir.path().to_str().unwrap()).unwrap();
        assert_eq!(result.file_count, 1);
        let json = result.to_json();
        assert!(json.contains("greet"));
    }
}
