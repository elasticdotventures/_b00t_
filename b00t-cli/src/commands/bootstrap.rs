//! Bootstrap command for b00t self-configuration
//!
//! Implements Phase 0: Foundation (MVP)
//! - Check prerequisites (required/optional binaries)
//! - Create directory skeleton (~/.b00t/*)
//! - Generate Toon format report

use crate::bootstrap::report::BootstrapReport;
use crate::bootstrap::{
    check_prerequisites, create_skeleton, generate_toon_report, install_missing_required,
    print_toon_report, start_services,
};
use anyhow::{Context, Result};
use chrono::Utc;
use clap::Parser;
use std::path::PathBuf;

#[derive(Debug, Parser, Clone)]
pub enum BootstrapCommands {
    /// Run full bootstrap (check prereqs + auto-install + start services + create skeleton + report)
    #[clap(alias = "init")]
    Run {
        /// Skip directory creation
        #[clap(long)]
        skip_dirs: bool,

        /// Skip auto-installation of missing binaries
        #[clap(long)]
        skip_install: bool,

        /// Skip starting services (Qdrant, etc.)
        #[clap(long)]
        skip_services: bool,

        /// Output path for Toon report (default: ~/.b00t/bootstrap-report.toon)
        #[clap(short, long)]
        output: Option<PathBuf>,

        /// Print report to stdout instead of file
        #[clap(long)]
        print: bool,
    },

    /// Check prerequisites only
    Check,

    /// Create directory skeleton only
    Skeleton,
}

/// Handle bootstrap commands
pub async fn handle_bootstrap_command(cmd: BootstrapCommands) -> Result<()> {
    match cmd {
        BootstrapCommands::Run {
            skip_dirs,
            skip_install,
            skip_services,
            output,
            print,
        } => run_bootstrap(skip_dirs, skip_install, skip_services, output, print).await,
        BootstrapCommands::Check => check_only().await,
        BootstrapCommands::Skeleton => skeleton_only().await,
    }
}

async fn run_bootstrap(
    skip_dirs: bool,
    skip_install: bool,
    skip_services: bool,
    output: Option<PathBuf>,
    print_only: bool,
) -> Result<()> {
    println!("🥾 b00t bootstrap - Phase 0: Foundation (Self-Installing)");
    println!();

    // Locate bootstrap.toml
    let config_path = PathBuf::from("_b00t_/bootstrap.toml");
    if !config_path.exists() {
        anyhow::bail!(
            "Bootstrap config not found: {}\nRun from dotfiles root directory",
            config_path.display()
        );
    }

    // Check prerequisites
    println!("📋 Checking prerequisites...");
    let mut prereq_result =
        check_prerequisites(&config_path).context("Failed to check prerequisites")?;

    // Auto-install missing binaries (unless skipped)
    if !skip_install && !prereq_result.all_required_met {
        println!();
        println!("🔧 Auto-installing missing dependencies...");
        let installed = install_missing_required(&prereq_result)
            .await
            .context("Failed to auto-install dependencies")?;

        if !installed.is_empty() {
            println!("✅ Installed: {}", installed.join(", "));

            // Re-check prerequisites after installation
            prereq_result = check_prerequisites(&config_path)?;
        }
    }

    // Start services (unless skipped)
    if !skip_services {
        println!();
        println!("🚀 Starting services...");
        let started = start_services().await.context("Failed to start services")?;

        if !started.is_empty() {
            println!("✅ Started: {}", started.join(", "));
        } else {
            println!("ℹ️  All services already running");
        }
    }

    // Create skeleton (unless skipped)
    let skeleton_result = if skip_dirs {
        println!("⏭️  Skipping directory creation");
        None
    } else {
        println!("📁 Creating directory skeleton...");
        Some(create_skeleton(&config_path).context("Failed to create directory skeleton")?)
    };

    // Generate report
    let report = BootstrapReport {
        timestamp: Utc::now().to_rfc3339(),
        prereq_result,
        skeleton_result,
    };

    if print_only {
        // Print to stdout
        print_toon_report(&report);
    } else {
        // Write to file
        let output_path = output.unwrap_or_else(|| PathBuf::from("~/.b00t/bootstrap-report.toon"));

        generate_toon_report(&report, &output_path).context("Failed to generate Toon report")?;

        println!();
        print_toon_report(&report);
        println!();
        println!("📄 Report written to: {}", output_path.display());
    }

    if !report.prereq_result.all_required_met {
        anyhow::bail!("⚠️  Bootstrap incomplete - required prerequisites missing");
    }

    println!();
    println!("✅ Bootstrap complete!");

    Ok(())
}

async fn check_only() -> Result<()> {
    let config_path = PathBuf::from("_b00t_/bootstrap.toml");
    if !config_path.exists() {
        anyhow::bail!("Bootstrap config not found: {}", config_path.display());
    }

    println!("📋 Checking prerequisites...");
    let prereq_result = check_prerequisites(&config_path)?;

    let report = BootstrapReport {
        timestamp: Utc::now().to_rfc3339(),
        prereq_result,
        skeleton_result: None,
    };

    print_toon_report(&report);

    if !report.prereq_result.all_required_met {
        std::process::exit(1);
    }

    Ok(())
}

async fn skeleton_only() -> Result<()> {
    let config_path = PathBuf::from("_b00t_/bootstrap.toml");
    if !config_path.exists() {
        anyhow::bail!("Bootstrap config not found: {}", config_path.display());
    }

    println!("📁 Creating directory skeleton...");
    let skeleton_result = create_skeleton(&config_path)?;

    if skeleton_result.is_success() {
        println!("✅ Created {} directories", skeleton_result.created.len());
        println!(
            "ℹ️  {} directories already existed",
            skeleton_result.already_existed.len()
        );

        for dir in &skeleton_result.created {
            println!("  ✨ {}", dir.display());
        }
    } else {
        println!("⚠️  {} errors occurred", skeleton_result.errors.len());
        for (path, error) in &skeleton_result.errors {
            eprintln!("  ❌ {}: {}", path.display(), error);
        }
        std::process::exit(1);
    }

    Ok(())
}
