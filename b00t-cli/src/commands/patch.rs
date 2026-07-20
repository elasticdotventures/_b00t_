/// `b00t patch` — visible semantic patch workflow.
///
/// Prevents silent doc/code overwrites by showing a unified diff before writing.
use anyhow::Result;
use clap::Subcommand;
use std::path::PathBuf;

use crate::semantic_patch::SemanticPatch;

#[derive(Debug, Subcommand)]
pub enum PatchCommands {
    /// Show diff between current file and proposed content (no write)
    Show {
        /// File to diff
        path: PathBuf,
        /// Proposed content (reads from stdin if "-")
        proposed: String,
    },
    /// Show diff then write if confirmed (or --yes for CI)
    Apply {
        /// File to update
        path: PathBuf,
        /// Proposed content (reads from stdin if "-")
        proposed: String,
        /// Skip confirmation prompt
        #[clap(long)]
        yes: bool,
    },
    /// Exit 0 if no diff, exit 1 if changes exist (CI gate)
    Check {
        /// File to check
        path: PathBuf,
        /// Proposed content
        proposed: String,
    },
}

pub fn handle_patch_command(cmd: &PatchCommands) -> Result<()> {
    match cmd {
        PatchCommands::Show { path, proposed } => {
            let proposed = resolve_proposed(proposed)?;
            let patch = SemanticPatch::from_disk(&path, proposed)?;
            patch.display();
        }
        PatchCommands::Apply {
            path,
            proposed,
            yes,
        } => {
            let proposed = resolve_proposed(proposed)?;
            let patch = SemanticPatch::from_disk(&path, &proposed)?;
            patch.display();

            if !patch.has_changes() {
                println!("Nothing to apply.");
                return Ok(());
            }

            if *yes || confirm_apply(&path)? {
                patch.apply()?;
                println!("Applied to {}", path.display());
            } else {
                println!("Aborted.");
            }
        }
        PatchCommands::Check { path, proposed } => {
            let proposed = resolve_proposed(proposed)?;
            let patch = SemanticPatch::from_disk(&path, proposed)?;
            if patch.has_changes() {
                patch.display();
                std::process::exit(1);
            }
        }
    }
    Ok(())
}

fn resolve_proposed(s: &str) -> Result<String> {
    if s == "-" {
        use std::io::Read as _;
        let mut buf = String::new();
        std::io::stdin().read_to_string(&mut buf)?;
        Ok(buf)
    } else {
        Ok(s.to_owned())
    }
}

fn confirm_apply(path: &std::path::Path) -> Result<bool> {
    use std::io::Write as _;
    print!("Apply changes to {}? [y/N] ", path.display());
    std::io::stdout().flush()?;
    let mut line = String::new();
    std::io::stdin().read_line(&mut line)?;
    Ok(matches!(line.trim().to_lowercase().as_str(), "y" | "yes"))
}
