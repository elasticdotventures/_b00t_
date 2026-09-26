//! `b00t r00t` — the same `_b00t_/` anchor primitive as `rep0`, one tier
//! further out.
//!
//! Real example on disk: `/home/brianh/promptexecution/_b00t_`, shared by
//! sibling project repos underneath it (read-only reference for this repo —
//! never written to by b00t-cli itself).
//!
//! Deliberately its own subcommand rather than folded into the silent
//! `--path` default chain: `rep0`'s nearest-hit IS wired into `--path`'s
//! implicit resolution (see `resolve_datum_dir` in `main.rs`), but silently
//! reaching two directories up by default would be more surprising than
//! useful. A caller who wants r00t-scoped resolution asks for it explicitly
//! via this subcommand (or `--path "$(b00t r00t where)"`-style composition).

use std::path::Path;

use anyhow::{Context, Result};
use clap::Parser;

use super::rep0::{find_b00t_ancestors, rep0_init};

#[derive(Parser)]
pub enum R00tCommands {
    #[clap(
        about = "Create _b00t_/ one directory level above cwd (idempotent)"
    )]
    Init,
    #[clap(
        about = "Walk up from cwd, skip the nearest _b00t_/ hit, report the next one up"
    )]
    Where,
}

pub fn handle_r00t_command(cmd: &R00tCommands) -> Result<()> {
    match cmd {
        R00tCommands::Init => r00t_init(&std::env::current_dir()?),
        R00tCommands::Where => r00t_where(),
    }
}

/// Same `_b00t_/` create-if-absent primitive as `rep0_init`, applied to the
/// parent of `dir` rather than `dir` itself.
pub fn r00t_init(dir: &Path) -> Result<()> {
    let parent = dir.parent().with_context(|| {
        format!(
            "r00t init: '{}' has no parent directory (already at the filesystem root)",
            dir.display()
        )
    })?;
    rep0_init(parent)
}

fn r00t_where() -> Result<()> {
    let cwd = std::env::current_dir()?;
    let hits = find_b00t_ancestors(&cwd);
    match hits.get(1) {
        Some(p) => println!(
            "r00t: {} (next _b00t_/ up the ancestor chain, past the nearest one)",
            p.display()
        ),
        None if hits.len() == 1 => println!(
            "r00t: none — only one _b00t_/ found (at {}); r00t reports the NEXT one up. Run `b00t r00t init` to create it.",
            hits[0].display()
        ),
        None => println!(
            "r00t: none — no _b00t_/ found walking up from cwd at all. Run `b00t rep0 init` first, then `b00t r00t init`."
        ),
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn r00t_init_creates_parent_b00t_dir() {
        let tmp = tempfile::tempdir().unwrap();
        let child = tmp.path().join("child");
        std::fs::create_dir_all(&child).unwrap();

        r00t_init(&child).unwrap();

        assert!(tmp.path().join("_b00t_").is_dir());
        assert!(!child.join("_b00t_").exists());
    }

    #[test]
    fn r00t_init_is_idempotent() {
        let tmp = tempfile::tempdir().unwrap();
        let child = tmp.path().join("child");
        std::fs::create_dir_all(&child).unwrap();

        r00t_init(&child).unwrap();
        r00t_init(&child).unwrap();

        assert!(tmp.path().join("_b00t_").is_dir());
    }
}
