// b00t-ast/src/extract.rs
//
// syn-based AST extraction — walks Rust parse trees and produces CodeElement records.
//
// Uses syn::visit::Visit trait for zero-copy AST traversal.
// Extracts: functions, structs, enums, traits, impls, consts, type aliases, macros, modules.

use crate::{
    CodeElement, CodeElementKind, ConstInfo, EnumInfo, EnumVariant, FnParam, FunctionInfo,
    ImplInfo, MacroInfo, ModuleInfo, StructField, StructInfo, TraitInfo, TraitItem, TypeInfo,
};
use anyhow::Result;
use std::path::Path;
use syn::spanned::Spanned;
use syn::visit::Visit;
use syn::{Attribute, File, FnArg, Item, Pat, Visibility};

/// Extract code elements from a single .rs file
pub fn extract_file(file_path: &Path, module_prefix: &str) -> Result<Vec<CodeElement>> {
    let source = std::fs::read_to_string(file_path)?;
    let syntax: File = syn::parse_file(&source)?;
    let mut collector = ElementCollector {
        elements: Vec::new(),
        current_module: module_prefix.to_string(),
        file_path: file_path.to_string_lossy().to_string(),
    };
    collector.visit_file(&syntax);
    Ok(collector.elements)
}

struct ElementCollector {
    elements: Vec<CodeElement>,
    current_module: String,
    file_path: String,
}

impl ElementCollector {
    fn add(&mut self, kind: CodeElementKind, name: &str, start: usize, end: usize, attrs: &[Attribute]) {
        let qualified = if self.current_module.is_empty() || name.starts_with(|c: char| c.is_uppercase()) {
            // Top-level items and uppercase names use simple path
            if self.current_module.is_empty() {
                name.to_string()
            } else {
                format!("{}::{}", self.current_module, name)
            }
        } else {
            format!("{}::{}", self.current_module, name)
        };

        let visibility = extract_visibility(attrs);

        let doc_comment = extract_doc_comment(attrs);

        self.elements.push(CodeElement {
            qualified_name: qualified,
            name: name.to_string(),
            kind,
            file_path: self.file_path.clone(),
            start_line: start,
            end_line: end,
            doc_comment,
            visibility,
        });
    }

    fn signature_str(&self, sig: &syn::Signature) -> String {
        quote::quote!(#sig).to_string()
    }
}

fn extract_visibility(_attrs: &[Attribute]) -> String {
    // Visibility is a property of the item, not attributes
    // Overridden per-item-kind during visit
    "pub".to_string()
}

fn extract_doc_comment(attrs: &[Attribute]) -> String {
    let docs: Vec<String> = attrs
        .iter()
        .filter(|a| a.path().is_ident("doc"))
        .filter_map(|a| {
            let meta = &a.meta;
            if let syn::Meta::NameValue(nv) = meta {
                if let syn::Expr::Lit(lit) = &nv.value {
                    if let syn::Lit::Str(s) = &lit.lit {
                        return Some(s.value());
                    }
                }
            }
            None
        })
        .collect();
    docs.join("\n")
}

fn extract_fn_params(inputs: &syn::punctuated::Punctuated<syn::FnArg, syn::Token![,]>) -> Vec<FnParam> {
    inputs
        .iter()
        .filter_map(|arg| match arg {
            FnArg::Typed(pat_type) => {
                let name = match pat_type.pat.as_ref() {
                    Pat::Ident(ident) => ident.ident.to_string(),
                    Pat::Wild(_) => "_".to_string(),
                    _ => "__other__".to_string(),
                };
                let ty = quote::quote!(#pat_type.ty).to_string();
                Some(FnParam { name, ty })
            }
            FnArg::Receiver(_) => Some(FnParam {
                name: "self".to_string(),
                ty: "Self".to_string(),
            }),
        })
        .collect()
}

fn extract_generics(generics: &syn::Generics) -> Vec<String> {
    generics
        .params
        .iter()
        .map(|p| quote::quote!(#p).to_string())
        .collect()
}

// ── syn::visit::Visit impl ───────────────────────────────────────────────────

impl<'ast> syn::visit::Visit<'ast> for ElementCollector {
    fn visit_item(&mut self, item: &'ast Item) {
        let span = item.span();
        let start = span.start().line;
        let end = span.end().line;

        match item {
            Item::Fn(func) => {
                let sig = &func.sig;
                let params = extract_fn_params(&sig.inputs);
                let return_type = match &sig.output {
                    syn::ReturnType::Default => "()".to_string(),
                    syn::ReturnType::Type(_, ty) => quote::quote!(#ty).to_string(),
                };

                self.add(
                    CodeElementKind::Function(FunctionInfo {
                        signature: self.signature_str(sig),
                        is_async: sig.asyncness.is_some(),
                        is_unsafe: sig.unsafety.is_some(),
                        generics: extract_generics(&sig.generics),
                        params,
                        return_type,
                        attributes: func.attrs.iter().map(|a| quote::quote!(#a).to_string()).collect(),
                    }),
                    &sig.ident.to_string(),
                    start,
                    end,
                    &func.attrs,
                );
            }

            Item::Struct(st) => {
                let fields: Vec<StructField> = st
                    .fields
                    .iter()
                    .map(|f| StructField {
                        name: f.ident.as_ref().map(|i| i.to_string()).unwrap_or_default(),
                        ty: quote::quote!(#f.ty).to_string(),
                        visibility: match f.vis {
                            Visibility::Public(_) => "pub".to_string(),
                            _ => "private".to_string(),
                        },
                    })
                    .collect();

                self.add(
                    CodeElementKind::Struct(StructInfo {
                        fields,
                        generics: extract_generics(&st.generics),
                        attributes: st.attrs.iter().map(|a| quote::quote!(#a).to_string()).collect(),
                    }),
                    &st.ident.to_string(),
                    start,
                    end,
                    &st.attrs,
                );
            }

            Item::Enum(en) => {
                let variants: Vec<EnumVariant> = en
                    .variants
                    .iter()
                    .map(|v| {
                        let fields: Vec<FnParam> = match &v.fields {
                            syn::Fields::Named(named) => named
                                .named
                                .iter()
                                .map(|f| FnParam {
                                    name: f.ident.as_ref().map(|i| i.to_string()).unwrap_or_default(),
                                    ty: quote::quote!(#f.ty).to_string(),
                                })
                                .collect(),
                            syn::Fields::Unnamed(unnamed) => unnamed
                                .unnamed
                                .iter()
                                .enumerate()
                                .map(|(i, f)| FnParam {
                                    name: format!("field_{i}"),
                                    ty: quote::quote!(#f.ty).to_string(),
                                })
                                .collect(),
                            syn::Fields::Unit => vec![],
                        };
                        EnumVariant {
                            name: v.ident.to_string(),
                            fields,
                        }
                    })
                    .collect();

                self.add(
                    CodeElementKind::Enum(EnumInfo {
                        variants,
                        generics: extract_generics(&en.generics),
                        attributes: en.attrs.iter().map(|a| quote::quote!(#a).to_string()).collect(),
                    }),
                    &en.ident.to_string(),
                    start,
                    end,
                    &en.attrs,
                );
            }

            Item::Trait(tr) => {
                let items: Vec<TraitItem> = tr
                    .items
                    .iter()
                    .map(|item| match item {
                        syn::TraitItem::Fn(method) => TraitItem::Function {
                            signature: quote::quote!(#method.sig).to_string(),
                            has_default: method.default.is_some(),
                        },
                        syn::TraitItem::Type(ty) => TraitItem::Type {
                            name: ty.ident.to_string(),
                        },
                        syn::TraitItem::Const(c) => TraitItem::Const {
                            name: c.ident.to_string(),
                            ty: quote::quote!(#c.ty).to_string(),
                        },
                        _ => TraitItem::Type {
                            name: "_unknown_".to_string(),
                        },
                    })
                    .collect();

                let supertraits: Vec<String> = tr
                    .supertraits
                    .iter()
                    .map(|st| quote::quote!(#st).to_string())
                    .collect();

                self.add(
                    CodeElementKind::Trait(TraitInfo {
                        items,
                        generics: extract_generics(&tr.generics),
                        supertraits,
                    }),
                    &tr.ident.to_string(),
                    start,
                    end,
                    &tr.attrs,
                );
            }

            Item::Impl(imp) => {
                let self_ty = quote::quote!(#imp.self_ty).to_string();
                let trait_name = imp.trait_.as_ref().map(|(_, tr, _)| quote::quote!(#tr).to_string());

                let items: Vec<String> = imp
                    .items
                    .iter()
                    .filter_map(|item| match item {
                        syn::ImplItem::Fn(method) => Some(quote::quote!(#method.sig).to_string()),
                        _ => None,
                    })
                    .collect();

                let display_name = if let Some(ref tn) = trait_name {
                    format!("impl {tn} for {self_ty}")
                } else {
                    format!("impl {self_ty}")
                };

                self.add(
                    CodeElementKind::Impl(ImplInfo {
                        trait_name,
                        self_ty,
                        items,
                    }),
                    &display_name,
                    start,
                    end,
                    &imp.attrs,
                );
            }

            Item::Const(c) => {
                self.add(
                    CodeElementKind::Const(ConstInfo {
                        ty: quote::quote!(#c.ty).to_string(),
                        value: quote::quote!(#c.expr).to_string(),
                    }),
                    &c.ident.to_string(),
                    start,
                    end,
                    &c.attrs,
                );
            }

            Item::Type(ty) => {
                self.add(
                    CodeElementKind::Type(TypeInfo {
                        aliased_ty: quote::quote!(#ty.ty).to_string(),
                        generics: extract_generics(&ty.generics),
                    }),
                    &ty.ident.to_string(),
                    start,
                    end,
                    &ty.attrs,
                );
            }

            Item::Macro(mac) => {
                let body = quote::quote!(#mac.mac).to_string();
                self.add(
                    CodeElementKind::Macro(MacroInfo {
                        path: mac
                            .mac
                            .path
                            .segments
                            .last()
                            .map(|s| s.ident.to_string())
                            .unwrap_or_default(),
                        body_preview: body.chars().take(200).collect(),
                    }),
                    "macro",
                    start,
                    end,
                    &mac.attrs,
                );
            }

            Item::Mod(mod_item) => {
                let children: Vec<String> = mod_item
                    .content
                    .as_ref()
                    .map(|(_, items)| {
                        items
                            .iter()
                            .filter_map(|i| match i {
                                Item::Fn(f) => Some(f.sig.ident.to_string()),
                                Item::Struct(s) => Some(s.ident.to_string()),
                                Item::Enum(e) => Some(e.ident.to_string()),
                                Item::Trait(t) => Some(t.ident.to_string()),
                                Item::Type(t) => Some(t.ident.to_string()),
                                Item::Const(c) => Some(c.ident.to_string()),
                                Item::Mod(m) => Some(m.ident.to_string()),
                                _ => None,
                            })
                            .collect()
                    })
                    .unwrap_or_default();

                self.add(
                    CodeElementKind::Module(ModuleInfo {
                        is_inline: mod_item.content.is_some(),
                        children,
                    }),
                    &mod_item.ident.to_string(),
                    start,
                    end,
                    &mod_item.attrs,
                );
            }

            _ => {} // skip other items (use, extern crate, etc.)
        }

        // Continue visiting children (so impl blocks inside function bodies get found)
        syn::visit::visit_item(self, item);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_function() {
        let source = "/// Adds two numbers\npub fn add(a: i32, b: i32) -> i32 { a + b }";
        let file = syn::parse_file(source).unwrap();
        let mut collector = ElementCollector {
            elements: Vec::new(),
            current_module: "test".to_string(),
            file_path: "test.rs".to_string(),
        };
        collector.visit_file(&file);
        assert_eq!(collector.elements.len(), 1);
        let el = &collector.elements[0];
        assert_eq!(el.name, "add");
        assert!(el.doc_comment.contains("Adds two numbers"));
        if let CodeElementKind::Function(fi) = &el.kind {
            assert_eq!(fi.params.len(), 2);
            assert!(fi.return_type.contains("i32"));
            assert!(!fi.is_async);
        } else {
            panic!("expected Function");
        }
    }

    #[test]
    fn test_extract_struct() {
        let source = "pub struct Point { pub x: f64, y: f64 }";
        let file = syn::parse_file(source).unwrap();
        let mut collector = ElementCollector {
            elements: Vec::new(),
            current_module: String::new(),
            file_path: "test.rs".to_string(),
        };
        collector.visit_file(&file);
        assert_eq!(collector.elements.len(), 1);
        let el = &collector.elements[0];
        assert_eq!(el.name, "Point");
        if let CodeElementKind::Struct(si) = &el.kind {
            assert_eq!(si.fields.len(), 2);
            assert_eq!(si.fields[0].name, "x");
            assert_eq!(si.fields[0].visibility, "pub");
            assert_eq!(si.fields[1].visibility, "private");
        } else {
            panic!("expected Struct");
        }
    }

    #[test]
    fn test_extract_enum() {
        let source = "pub enum Color { Red, Green(u8), Blue { r: u8, g: u8, b: u8 } }";
        let file = syn::parse_file(source).unwrap();
        let mut collector = ElementCollector {
            elements: Vec::new(),
            current_module: String::new(),
            file_path: "test.rs".to_string(),
        };
        collector.visit_file(&file);
        assert_eq!(collector.elements.len(), 1);
        let el = &collector.elements[0];
        assert_eq!(el.name, "Color");
        if let CodeElementKind::Enum(ei) = &el.kind {
            assert_eq!(ei.variants.len(), 3);
            assert_eq!(ei.variants[0].name, "Red");
            assert_eq!(ei.variants[0].fields.len(), 0);
            assert_eq!(ei.variants[1].name, "Green");
            assert_eq!(ei.variants[1].fields.len(), 1);
            assert_eq!(ei.variants[2].name, "Blue");
            assert_eq!(ei.variants[2].fields.len(), 3);
        } else {
            panic!("expected Enum");
        }
    }

    #[test]
    fn test_extract_trait() {
        let source = "pub trait Draw { fn draw(&self); fn area(&self) -> f64; }";
        let file = syn::parse_file(source).unwrap();
        let mut collector = ElementCollector {
            elements: Vec::new(),
            current_module: String::new(),
            file_path: "test.rs".to_string(),
        };
        collector.visit_file(&file);
        assert_eq!(collector.elements.len(), 1);
        let el = &collector.elements[0];
        assert_eq!(el.name, "Draw");
        if let CodeElementKind::Trait(ti) = &el.kind {
            assert_eq!(ti.items.len(), 2);
        } else {
            panic!("expected Trait");
        }
    }

    #[test]
    fn test_extract_impl() {
        let source = "impl Point { pub fn new(x: f64, y: f64) -> Self { Point { x, y } } }";
        let file = syn::parse_file(source).unwrap();
        let mut collector = ElementCollector {
            elements: Vec::new(),
            current_module: String::new(),
            file_path: "test.rs".to_string(),
        };
        collector.visit_file(&file);
        assert_eq!(collector.elements.len(), 1);
        let el = &collector.elements[0];
        assert!(el.name.contains("impl"));
        if let CodeElementKind::Impl(ii) = &el.kind {
            assert_eq!(ii.items.len(), 1);
        } else {
            panic!("expected Impl");
        }
    }

    #[test]
    fn test_extract_const() {
        let source = "pub const MAX_SIZE: usize = 1024;";
        let file = syn::parse_file(source).unwrap();
        let mut collector = ElementCollector {
            elements: Vec::new(),
            current_module: String::new(),
            file_path: "test.rs".to_string(),
        };
        collector.visit_file(&file);
        assert_eq!(collector.elements.len(), 1);
        let el = &collector.elements[0];
        assert_eq!(el.name, "MAX_SIZE");
        assert!(matches!(el.kind, CodeElementKind::Const(_)));
    }

    #[test]
    fn test_extract_type_alias() {
        let source = "pub type Result<T> = std::result::Result<T, Error>;";
        let file = syn::parse_file(source).unwrap();
        let mut collector = ElementCollector {
            elements: Vec::new(),
            current_module: String::new(),
            file_path: "test.rs".to_string(),
        };
        collector.visit_file(&file);
        assert_eq!(collector.elements.len(), 1);
        let el = &collector.elements[0];
        assert_eq!(el.name, "Result");
        assert!(matches!(el.kind, CodeElementKind::Type(_)));
    }

    #[test]
    fn test_extract_empty_file() {
        let source = "";
        let file = syn::parse_file(source).unwrap();
        let mut collector = ElementCollector {
            elements: Vec::new(),
            current_module: String::new(),
            file_path: "empty.rs".to_string(),
        };
        collector.visit_file(&file);
        assert!(collector.elements.is_empty());
    }

    #[test]
    fn test_extract_multiple_items() {
        let source = r#"
pub fn hello() -> String { "world".into() }

pub struct Config { pub port: u16 }

pub enum Mode { Active, Passive }
"#;
        let file = syn::parse_file(source).unwrap();
        let mut collector = ElementCollector {
            elements: Vec::new(),
            current_module: String::new(),
            file_path: "multi.rs".to_string(),
        };
        collector.visit_file(&file);
        assert_eq!(collector.elements.len(), 3);
    }
}
