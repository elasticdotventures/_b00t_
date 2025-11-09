//! Bootstrap module for self-configuring b00t installation
//!
//! This module implements Phase 0 of the bootstrap architecture:
//! - Prerequisite checking (required binaries, versions)
//! - Auto-installation of missing dependencies
//! - Service startup (Qdrant, etc.)
//! - Skeleton generation (~/.b00t/ directories)
//! - Toon format reporting

pub mod prereq;
pub mod installer;
pub mod skeleton;
pub mod report;

pub use prereq::{check_prerequisites, PrereqResult};
pub use installer::{install_missing_required, start_services};
pub use skeleton::create_skeleton;
pub use report::{generate_toon_report, print_toon_report};
