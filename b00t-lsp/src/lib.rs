#![recursion_limit = "256"]
//! b00t-lsp — language server for b00t datum dialects.
//!
//! The `analysis` module is pure (no LSP transport types) so it is directly
//! unit-testable and reusable from the `--check` CLI mode.

pub mod analysis;
pub mod schema;
