//! Phase 2: Rust AST → Horn clause triples via `syn`.
//!
//! Walks `syn::File` items, extracts trait impl declarations, and emits
//! SPO triples that feed into the crepe Horn derivation engine:
//!
//!   ("usize",   "b00t:implements",       "Clone")
//!   ("Vec<T>",  "b00t:implements",       "Clone")
//!   ("Vec<T>",  "b00t:requires/Clone",   "T:Clone")   ← bound prerequisite
//!
//! The `b00t:requires/<Trait>` predicate lets callers build a clause database:
//!   `Implements(Vec<T>, Clone) :- Implements(T, Clone)` by checking that all
//!   `requires/Clone` bounds are satisfied in the knowledge base before asserting
//!   the derived `Implements` fact.

use syn::{GenericParam, Item, ItemImpl, Type, TypeParamBound, WherePredicate};
use super::predicates::B00tPredicate;

// ── String representations ────────────────────────────────────────────────

fn ty_str(ty: &Type) -> String {
    quote::quote!(#ty).to_string().replace(' ', "")
}

fn path_str(path: &syn::Path) -> String {
    quote::quote!(#path).to_string().replace(' ', "")
}

// ── Extraction ────────────────────────────────────────────────────────────

fn visit_item(item: &Item, out: &mut Vec<(String, String, String)>) {
    match item {
        Item::Impl(block) => visit_impl(block, out),
        Item::Mod(m) => {
            if let Some((_, items)) = &m.content {
                for item in items { visit_item(item, out); }
            }
        }
        _ => {}
    }
}

fn visit_impl(block: &ItemImpl, out: &mut Vec<(String, String, String)>) {
    let Some((_, trait_path, _)) = &block.trait_ else { return };

    let self_ty = ty_str(&block.self_ty);
    let trait_name = path_str(trait_path);

    // Direct: Type implements Trait
    out.push((self_ty.clone(), B00tPredicate::Implements.as_uri(), trait_name.clone()));

    let req_pred = B00tPredicate::RequiresTrait { trait_name: trait_name.clone() }.as_uri();

    // Generic param bounds: `impl<T: Clone> Clone for Vec<T>`
    for param in &block.generics.params {
        if let GenericParam::Type(tp) = param {
            let param_name = tp.ident.to_string();
            for bound in &tp.bounds {
                if let TypeParamBound::Trait(tb) = bound {
                    let bound_name = path_str(&tb.path);
                    out.push((
                        self_ty.clone(),
                        req_pred.clone(),
                        format!("{param_name}:{bound_name}"),
                    ));
                }
            }
        }
    }

    // Where-clause bounds: `impl<T> Clone for Vec<T> where T: Clone`
    if let Some(wc) = &block.generics.where_clause {
        for pred in &wc.predicates {
            if let WherePredicate::Type(wp) = pred {
                let bounded = ty_str(&wp.bounded_ty);
                for bound in &wp.bounds {
                    if let TypeParamBound::Trait(tb) = bound {
                        let bound_name = path_str(&tb.path);
                        out.push((
                            self_ty.clone(),
                            req_pred.clone(),
                            format!("{bounded}:{bound_name}"),
                        ));
                    }
                }
            }
        }
    }
}

// ── Public API ────────────────────────────────────────────────────────────

/// Extract SPO triples from a Rust source string.
///
/// Returns `(implementing_type, predicate, object)` triples:
/// - `"b00t:implements"` — direct trait implementation
/// - `"b00t:requires/<Trait>"` — prerequisite bound for the impl to hold
pub fn parse_source_triples(source: &str) -> syn::Result<Vec<(String, String, String)>> {
    let file = syn::parse_str::<syn::File>(source)?;
    let mut triples = Vec::new();
    for item in &file.items { visit_item(item, &mut triples); }
    Ok(triples)
}

/// Walk a directory of `.rs` files and accumulate triples from all of them.
pub fn parse_dir_triples(dir: &std::path::Path) -> anyhow::Result<Vec<(String, String, String)>> {
    let mut out = Vec::new();
    for entry in std::fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) != Some("rs") { continue; }
        let src = std::fs::read_to_string(&path)
            .map_err(|e| anyhow::anyhow!("{}: {e}", path.display()))?;
        match parse_source_triples(&src) {
            Ok(mut t) => out.append(&mut t),
            Err(e) => tracing::warn!("skip {}: {e}", path.display()),
        }
    }
    Ok(out)
}

/// Given a set of trait triples, prove whether `subject` implements `trait_name`
/// by checking if the ground fact exists or can be derived from `Implements`
/// entries already in the `horn` results.
///
/// This is a lightweight Horn clause evaluator for the specific `Implements`
/// predicate — full FOHH (generic bounds) requires chalk (Phase 3).
pub fn proves_implements(
    base_triples: &[(String, String, String)],
    subject: &str,
    trait_name: &str,
) -> bool {
    let horn = super::graph_rules::derive(base_triples.to_vec());
    horn.implements.contains(&(subject.to_string(), trait_name.to_string()))
}
