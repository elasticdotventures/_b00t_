//! Re-exported pipeline types from the standalone b00t-pipeline-types crate.
//!
//! Extraction: gh #1251 — mirroring the ufo-types precedent.
//! All types and their impls (Satisfies, with_flow_control) now live in
//! `b00t_pipeline_types`. This file preserves backward-compatible imports
//! for all existing `use crate::pipeline_types::*` callers.

pub use b00t_pipeline_types::*;