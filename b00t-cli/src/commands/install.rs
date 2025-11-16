use anyhow::{Context, Result};
use clap::Parser;
use duct::cmd;

#[derive(Parser)]
pub enum InstallCommands {
    #[clap(
        about = "Run 'just install' to install b00t components",
        long_about = "Executes the justfile install recipe which:\n  - Installs b00t-mcp via cargo\n  - Installs b00t-cli via cargo\n  - Installs cocogitto\n  - Sets up git commit hooks\n\nExamples:\n  b00t install\n  b00t install --dry-run"
    )]
    Run {
        #[clap(long, help = "Show what would be installed without installing")]
        dry_run: bool,
    },
}

impl InstallCommands {
    pub fn execute(&self, _path: &str) -> Result<()> {
        match self {
            InstallCommands::Run { dry_run } => run_just_install(*dry_run),
        }
    }
}

fn run_just_install(dry_run: bool) -> Result<()> {
    let workspace_root = crate::utils::get_workspace_root();

    if dry_run {
        println!(
            "🔍 Dry run: Would execute 'just install' from {}",
            workspace_root
        );
        println!("\nThe justfile install recipe would:");
        println!("  1. cargo install --path b00t-mcp --force");
        println!("  2. cargo install --path b00t-cli --force");
        println!("  3. cargo install cocogitto --locked --force");
        println!("  4. just install-commit-hook");
        return Ok(());
    }

    println!("🥾 Running 'just install' from {}", workspace_root);

    let output = cmd!("just", "install")
        .dir(&workspace_root)
        .stdout_capture()
        .stderr_capture()
        .unchecked()
        .run()
        .context("Failed to execute 'just install'")?;

    // Print stdout
    if !output.stdout.is_empty() {
        println!("{}", String::from_utf8_lossy(&output.stdout));
    }

    // Print stderr
    if !output.stderr.is_empty() {
        eprintln!("{}", String::from_utf8_lossy(&output.stderr));
    }

    if !output.status.success() {
        anyhow::bail!(
            "just install failed with exit code: {}",
            output.status.code().unwrap_or(-1)
        );
    }

    println!("✅ Installation complete!");

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_install_dry_run() {
        let result = run_just_install(true);
        assert!(result.is_ok());
    }
}
