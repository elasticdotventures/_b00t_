//! # tomllm
//!
//! TOML + LLM comment conventions.
//!
//! `.tomllm` files are **valid TOML** with enriched `#` comment semantics:
//! - Comments associate with the next key-value pair or section header
//! - Special prefixes encode tribal knowledge: `# 🤓`, `# @tribal:`, `# @example:`, `# @requires:`
//! - Tail-map block (last ≤10 lines): fast executive agent scanning without full context load
//!
//! ## Design principle
//! Comments are FOR agents reading the **source file** as documentation.
//! When passing data VALUES downstream (to pipelines or further LLMs), strip comments
//! to minimize token usage. The recipient gets clean TOML; the source stays rich.
//!
//! ## Cognitive tier
//! Each `.tomllm` file MAY declare its required cognitive tier in the tail-map:
//! - `sm0l` — any small model can process this (classify, route, format)
//! - `ch0nky` — requires code-generation capable model (implement, refactor)
//! - `frontier` — requires frontier reasoning (architecture, security, compliance)
//!
//! ## Example
//! ```toml
//! # @tribal: always use uv pip, never pip install directly
//! # @example: uv pip install requests
//! package_manager = "uv"
//!
//! # b00t:map v1
//! # summary: Python toolchain config
//! # tags: python, uv
//! # tier: sm0l
//! ```

use std::collections::BTreeMap;
use thiserror::Error;

#[cfg(feature = "wasm")]
use wasm_bindgen::prelude::*;

pub mod loader;
pub mod map_block;
pub mod parser;
pub mod registry;
pub mod stripper;

pub use loader::{load_any_typed, load_first, load_typed, resolve_path};
pub use map_block::{CognitiveTier, MapBlock};
pub use parser::TomllmDoc;
pub use registry::TomllmRegistry;
// 🤓 define_typed_registry! is #[macro_export] — already at crate root; no pub use needed

#[derive(Debug, Error)]
pub enum TomllmError {
    #[error("TOML parse error: {0}")]
    TomlParse(#[from] toml::de::Error),
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    #[error("Map block parse error: {0}")]
    MapBlock(String),
}

pub type Result<T> = std::result::Result<T, TomllmError>;
