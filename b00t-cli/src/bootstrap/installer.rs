//! Auto-installer for missing dependencies
//!
//! Uses stack datums to self-install missing binaries and services

use crate::bootstrap::prereq::PrereqResult;
use anyhow::{Context, Result};
use std::process::Command;

/// Install missing required binaries based on OS
pub async fn install_missing_required(prereq: &PrereqResult) -> Result<Vec<String>> {
    let mut installed = Vec::new();

    for binary in prereq.missing_required() {
        // 🤓 Skip docker install if it was satisfied by podman alternative
        if binary.name.contains("podman (docker alternative)") {
            println!("ℹ️  Docker requirement satisfied by podman - skipping docker installation");
            continue;
        }

        println!("🔧 Installing {}...", binary.name);

        match install_binary(&binary.name).await {
            Ok(_) => {
                installed.push(binary.name.clone());
                println!("  ✅ {} installed", binary.name);
            }
            Err(e) => {
                eprintln!("  ❌ Failed to install {}: {}", binary.name, e);
            }
        }
    }

    Ok(installed)
}

/// Install a single binary using appropriate package manager
async fn install_binary(name: &str) -> Result<()> {
    // Detect OS and use appropriate package manager
    if cfg!(target_os = "linux") {
        install_linux(name).await
    } else if cfg!(target_os = "macos") {
        install_macos(name).await
    } else {
        anyhow::bail!("Unsupported OS for auto-install")
    }
}

/// Install on Linux using apt/snap/cargo
async fn install_linux(name: &str) -> Result<()> {
    match name {
        "docker" => {
            // Use snap for Docker on Ubuntu/Debian
            run_command("sudo", &["snap", "install", "docker"])?;
            run_command("sudo", &["addgroup", "--system", "docker"])?;
            run_command("sudo", &["usermod", "-aG", "docker", &whoami::username()])?;
        }
        "git" => {
            run_command("sudo", &["apt-get", "update"])?;
            run_command("sudo", &["apt-get", "install", "-y", "git"])?;
        }
        "just" => {
            // Install just via cargo
            run_command("cargo", &["install", "just"])?;
        }
        "fzf" => {
            run_command("sudo", &["apt-get", "install", "-y", "fzf"])?;
        }
        _ => {
            anyhow::bail!("Unknown binary: {}", name);
        }
    }
    Ok(())
}

/// Install on macOS using brew
async fn install_macos(name: &str) -> Result<()> {
    match name {
        "docker" => {
            run_command("brew", &["install", "--cask", "docker"])?;
        }
        "git" | "just" | "fzf" => {
            run_command("brew", &["install", name])?;
        }
        _ => {
            anyhow::bail!("Unknown binary: {}", name);
        }
    }
    Ok(())
}

/// Start required services using stack datums
pub async fn start_services() -> Result<Vec<String>> {
    let mut started = Vec::new();

    // Check if Qdrant is needed and not running
    if should_start_qdrant().await? {
        println!("🚀 Starting Qdrant service...");
        start_qdrant().await?;
        started.push("qdrant".to_string());
        println!("  ✅ Qdrant started");
    }

    Ok(started)
}

/// Check if Qdrant should be started
async fn should_start_qdrant() -> Result<bool> {
    // 🤓 Check if docker OR podman is available
    let container_runtime = if is_command_available("docker") {
        "docker"
    } else if is_command_available("podman") {
        "podman"
    } else {
        return Ok(false);
    };

    // Check if qdrant container is running
    let output = Command::new(container_runtime)
        .args(&["ps", "--filter", "name=qdrant", "--format", "{{.Names}}"])
        .output()?;

    let running = String::from_utf8_lossy(&output.stdout)
        .lines()
        .any(|line| line.contains("qdrant"));

    Ok(!running) // Start if NOT running
}

/// Start Qdrant using docker or podman
async fn start_qdrant() -> Result<()> {
    // 🤓 Use podman if docker not available
    let container_runtime = if is_command_available("docker") {
        "docker"
    } else if is_command_available("podman") {
        "podman"
    } else {
        anyhow::bail!("Neither docker nor podman available to start Qdrant");
    };

    // Try docker-compose first (using stack datum)
    if std::path::Path::new("_b00t_/qdrant.docker.toml").exists() {
        // 🤓 Future: parse datum and use proper docker/podman command
        // For now, use simple run command
        run_command(
            container_runtime,
            &[
                "run",
                "-d",
                "--name",
                "qdrant",
                "-p",
                "6333:6333",
                "-p",
                "6334:6334",
                "-v",
                "qdrant_storage:/qdrant/storage",
                "qdrant/qdrant:latest",
            ],
        )?;
    }

    Ok(())
}

/// Check if a command is available in PATH
fn is_command_available(cmd: &str) -> bool {
    Command::new("which")
        .arg(cmd)
        .output()
        .map(|output| output.status.success())
        .unwrap_or(false)
}

/// Run a command and check for success
fn run_command(cmd: &str, args: &[&str]) -> Result<()> {
    let status = Command::new(cmd)
        .args(args)
        .status()
        .with_context(|| format!("Failed to execute: {} {}", cmd, args.join(" ")))?;

    if !status.success() {
        anyhow::bail!("Command failed: {} {}", cmd, args.join(" "));
    }

    Ok(())
}
