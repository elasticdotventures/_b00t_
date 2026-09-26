//! `b00t rep0` — the literal filesystem anchor for a b00t project.
//!
//! Fills a real gap identified while designing `ProjectProvider` (spec:
//! `docs/superpowers/specs/2026-09-25-project-scope-and-ontology-pipeline-design.md`,
//! sub-project A): `-p/--path` (`_B00T_Path`) had no directory walk-up
//! discovery — always either an explicit flag or the hardcoded global
//! default (`~/.dotfiles/_b00t_`). `rep0` gives ergonomic discovery of the
//! nearest project-local `_b00t_/` (plain, unhidden — the datum-scoping
//! anchor, distinct from `._b00t_/`, the *soul* workspace directory created
//! by `b00t soul init`).
//!
//! `r00t` (`src/commands/r00t.rs`) reuses `find_b00t_ancestors` below to walk
//! one tier further out.

use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use clap::Parser;

#[derive(Parser)]
pub enum Rep0Commands {
    #[clap(about = "Create ./_b00t_/ in the current directory (idempotent)")]
    Init,
    #[clap(
        about = "Walk up from cwd for the nearest _b00t_/ (git-.git-style ancestor search)"
    )]
    Where,
}

pub fn handle_rep0_command(cmd: &Rep0Commands, global_fallback: &str) -> Result<()> {
    match cmd {
        Rep0Commands::Init => rep0_init(&std::env::current_dir()?),
        Rep0Commands::Where => rep0_where(global_fallback),
    }
}

/// Idempotently create `<dir>/_b00t_/` — the literal filesystem anchor.
pub fn rep0_init(dir: &Path) -> Result<()> {
    let target = dir.join("_b00t_");
    if target.is_dir() {
        println!("rep0: exists  {}", target.display());
    } else if target.exists() {
        anyhow::bail!(
            "rep0 init: {} already exists and is not a directory",
            target.display()
        );
    } else {
        std::fs::create_dir_all(&target)
            .with_context(|| format!("create {}", target.display()))?;
        println!("rep0: created {}", target.display());
    }
    Ok(())
}

/// Walk up from `start` (inclusive) collecting every ancestor directory that
/// contains a `_b00t_/` subdirectory, nearest first. Unbounded — walks all
/// the way to the filesystem root the same way git's own `.git` ancestor
/// search does, rather than stopping at some other repo-boundary marker.
pub fn find_b00t_ancestors(start: &Path) -> Vec<PathBuf> {
    let mut hits = Vec::new();
    let mut current: Option<PathBuf> = Some(start.to_path_buf());
    while let Some(dir) = current {
        let candidate = dir.join("_b00t_");
        if candidate.is_dir() {
            hits.push(candidate);
        }
        current = dir.parent().map(Path::to_path_buf);
    }
    hits
}

fn rep0_where(global_fallback: &str) -> Result<()> {
    let cwd = std::env::current_dir()?;
    let hits = find_b00t_ancestors(&cwd);
    match hits.first() {
        Some(p) => println!("rep0: {} (nearest _b00t_/ above cwd)", p.display()),
        None => println!("rep0: none — run `b00t rep0 init`"),
    }
    println!(
        "global fallback: {}",
        shellexpand::tilde(global_fallback)
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rep0_init_is_idempotent() {
        let tmp = tempfile::tempdir().unwrap();
        rep0_init(tmp.path()).unwrap();
        assert!(tmp.path().join("_b00t_").is_dir());
        // second call must not error
        rep0_init(tmp.path()).unwrap();
        assert!(tmp.path().join("_b00t_").is_dir());
    }

    #[test]
    fn find_b00t_ancestors_finds_nearest_first_then_further_ones() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path();
        let mid = root.join("mid");
        let leaf = mid.join("leaf");
        std::fs::create_dir_all(&leaf).unwrap();
        std::fs::create_dir_all(root.join("_b00t_")).unwrap();
        std::fs::create_dir_all(mid.join("_b00t_")).unwrap();

        let hits = find_b00t_ancestors(&leaf);
        assert_eq!(hits.len(), 2);
        assert_eq!(hits[0], mid.join("_b00t_"));
        assert_eq!(hits[1], root.join("_b00t_"));
    }

    #[test]
    fn find_b00t_ancestors_empty_when_none_exist() {
        let tmp = tempfile::tempdir().unwrap();
        let leaf = tmp.path().join("a").join("b");
        std::fs::create_dir_all(&leaf).unwrap();
        assert!(find_b00t_ancestors(&leaf).is_empty());
    }
}
