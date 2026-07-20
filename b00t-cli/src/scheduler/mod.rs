//! b00t scheduler — typed Rust schema, claim protocol, and DB conversion
//!
//! This module provides strongly-typed Rust representations of the
//! SCHEDULER-SCHEMA.tomllmd DDL, along with claim protocol logic
//! and rusqlite row conversion utilities.
//!
//! ## Module layout
//!
//! - [`schema`] — Core types: `ScheduleDef`, `RunRecord`, `AgentRegistration`,
//!   plus enums `ScheduleKind`, `RunStatus`, `AgentStatus`.
//! - [`claim`] — Claim protocol: agents request a due schedule via `try_claim`.
//! - [`convert`] — Conversions between DB rows and typed structs.
//!
//! 🔗 See `_b00t_/schema/SCHEDULER-SCHEMA.tomllmd` for the canonical DDL.
//! 🔗 See `crate::commands::scheduler::SchedulerDb` for the DB connection + CRUD.

pub mod claim;
pub mod convert;
pub mod schema;

// Re-export top-level types for convenient `use b00t_cli::scheduler::*;`
pub use claim::{ClaimResult, try_claim};
pub use convert::*;
pub use schema::*;
