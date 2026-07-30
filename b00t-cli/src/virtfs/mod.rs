//! virtfs — FUSE mount presenting b00t datums as Claude Code skills.
//! ROADMAP-virtfs.md Phase 1 (FUSE skeleton): mount point with three empty
//! top-level directories (skills/, agents/, datums/). Dynamic datum
//! enumeration is a later phase.
//!
//! `tree` (this module's directory-structure logic) is fuser-independent
//! and always available. The actual `fuser::Filesystem` impl + `b00t mount`
//! CLI command are NOT yet implemented: this sandbox has no `libfuse3-dev`
//! (pkg-config can't find fuse3.pc/fuse.pc) and no sudo to install it, so
//! `fuser`'s build script fails whenever `--features virtfs` is activated —
//! nothing built against `fuser` here could be verified to compile.

pub mod tree;
