//! virtfs — FUSE mount presenting b00t datums as Claude Code skills.
//! ROADMAP-virtfs.md Phase 1 (FUSE skeleton): mount point with three empty
//! top-level directories (skills/, agents/, datums/). Dynamic datum
//! enumeration is a later phase.
//!
//! `tree` (directory-structure logic) is fuser-independent and always
//! available. `fs` (the actual `fuser::Filesystem` impl + mount function)
//! requires the `virtfs` Cargo feature (needs libfuse3-dev/libfuse-dev at
//! build time via fuser's build script).

pub mod tree;

#[cfg(feature = "virtfs")]
pub mod fs;
