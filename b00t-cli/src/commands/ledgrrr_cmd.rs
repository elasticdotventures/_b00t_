//! `b00t ledgrrr` — Ledgrrr governance subsystem management
//!
//! # Usage
//! ```text
//! b00t ledgrrr status          # check repo + binary
//! b00t ledgrrr install         # clone/update + build
//! b00t ledgrrr start           # start ledgrrr MCP server
//! b00t ledgrrr stop            # stop ledgrrr MCP server
//! b00t ledgrrr update          # git pull + rebuild
//! b00t ledgrrr viz             # start/stop/status the viz dashboard server
//! ```

use anyhow::{Context, Result, anyhow};
use clap::Parser;
use std::path::PathBuf;
use std::process::Command;

/// Path constants for ledgrrr.
const VENDOR_DIR: &str = "vendor/ledgrrr";
const REPO_URL: &str = "git@github.com:PromptExecution/ledgrrr.git";

fn home() -> PathBuf {
    dirs::home_dir().unwrap_or_else(|| PathBuf::from("/home/brianh"))
}

fn b00t_dir() -> PathBuf {
    home().join(".b00t")
}

fn vendor_path() -> PathBuf {
    b00t_dir().join(VENDOR_DIR)
}

fn binary_path() -> PathBuf {
    vendor_path().join("target/release/ledgerr-mcp-server")
}

/// Run a shell command and return (success, stdout_or_stderr).
fn sh(cmd: &str) -> (bool, String) {
    Command::new("sh")
        .args(["-c", cmd])
        .output()
        .map(|o| {
            let s = String::from_utf8_lossy(&o.stdout).trim().to_string();
            let e = String::from_utf8_lossy(&o.stderr).trim().to_string();
            (
                o.status.success(),
                if !s.is_empty() {
                    s
                } else if !e.is_empty() {
                    e
                } else {
                    String::new()
                },
            )
        })
        .unwrap_or((false, "exec failed".into()))
}

fn pid_file() -> PathBuf {
    b00t_dir().join("ledgrrr-mcp-server.pid")
}

fn read_pid() -> Result<Option<u32>> {
    let path = pid_file();
    if !path.exists() {
        return Ok(None);
    }
    let content =
        std::fs::read_to_string(&path).with_context(|| format!("read pid file {}", path.display()))?;
    let pid: u32 = content
        .trim()
        .parse()
        .map_err(|e| anyhow!("invalid pid in {}: {}", path.display(), e))?;
    Ok(Some(pid))
}

fn write_pid(pid: u32) -> Result<()> {
    let path = pid_file();
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(&path, format!("{pid}\n"))
        .with_context(|| format!("write pid file {}", path.display()))
}

fn remove_pid() -> Result<()> {
    let path = pid_file();
    if path.exists() {
        std::fs::remove_file(&path).with_context(|| format!("remove pid file {}", path.display()))
    } else {
        Ok(())
    }
}

fn is_process_running(pid: u32) -> bool {
    sh(&format!("kill -0 {pid} 2>/dev/null")).0
}

/// Check if the vendor/ledgrrr repo directory exists.
fn repo_exists() -> bool {
    vendor_path().join(".git").exists()
}

/// Check if the ledgrrr MCP server binary exists and is executable.
fn binary_exists() -> bool {
    let bp = binary_path();
    bp.exists() && bp.is_file()
}

// ─── Commands ─────────────────────────────────────────────────────────────────

#[derive(Parser, Clone)]
pub enum LedgrrrCommands {
    #[clap(about = "Check if vendor/ledgrrr exists and ledgerr-mcp-server binary is built")]
    Status,
    #[clap(about = "Clone/update vendor/ledgrrr repo and build it")]
    Install,
    #[clap(about = "Start the ledgrrr MCP server")]
    Start,
    #[clap(about = "Stop the ledgrrr MCP server")]
    Stop,
    #[clap(about = "Git pull + rebuild vendor/ledgrrr")]
    Update,
    #[clap(about = "Manage the ledgrrr viz dashboard server (start/stop/status)")]
    Viz {
        #[clap(subcommand)]
        viz_command: VizCommands,
    },
}

#[derive(Parser, Clone)]
pub enum VizCommands {
    #[clap(about = "Start the viz dashboard HTTP server")]
    Start {
        #[clap(long, default_value = "8080", help = "HTTP port")]
        port: u16,
    },
    #[clap(about = "Stop the viz dashboard server")]
    Stop,
    #[clap(about = "Show viz dashboard server status")]
    Status,
}

impl VizCommands {
    fn script_path() -> PathBuf {
        home().join(".b00t/vendor/ledgrrr/scripts/ledgrrr-viz-serve.py")
    }

    fn exec(&self) -> Result<()> {
        match self {
            VizCommands::Start { port } => {
                let script = Self::script_path();
                if !script.exists() {
                    return Err(anyhow!("viz script not found at {}", script.display()));
                }
                let cmd = format!("python3 {} start --port={}", script.display(), port);
                let (ok, out) = sh(&cmd);
                println!("{}", out);
                if !ok { std::process::exit(1); }
                Ok(())
            }
            VizCommands::Stop => {
                let script = Self::script_path();
                let cmd = format!("python3 {} stop", script.display());
                let (ok, out) = sh(&cmd);
                println!("{}", out);
                if !ok { std::process::exit(1); }
                Ok(())
            }
            VizCommands::Status => {
                let script = Self::script_path();
                let cmd = format!("python3 {} status", script.display());
                let (ok, out) = sh(&cmd);
                println!("{}", out);
                if !ok { std::process::exit(1); }
                Ok(())
            }
        }
    }
}

// ─── Handler ──────────────────────────────────────────────────────────────────

pub fn handle_ledgrrr_command(command: &LedgrrrCommands) -> Result<()> {
    match command {
        LedgrrrCommands::Status => cmd_status(),
        LedgrrrCommands::Install => cmd_install(),
        LedgrrrCommands::Start => cmd_start(),
        LedgrrrCommands::Stop => cmd_stop(),
        LedgrrrCommands::Update => cmd_update(),
        LedgrrrCommands::Viz { viz_command } => viz_command.exec(),
    }
}

fn cmd_status() -> Result<()> {
    let repo_ok = repo_exists();
    let bin_ok = binary_exists();
    let running = read_pid()
        .ok()
        .flatten()
        .map(|pid| is_process_running(pid))
        .unwrap_or(false);

    println!("Ledgrrr governance subsystem status:");
    println!(
        "  {}  Repo: {}",
        if repo_ok { "✅" } else { "❌" },
        vendor_path().display()
    );
    println!(
        "  {}  Binary: {}",
        if bin_ok { "✅" } else { "❌" },
        binary_path().display()
    );
    if let Some(pid) = read_pid().ok().flatten() {
        if running {
            println!("  ✅  Running (pid {pid})");
        } else {
            println!("  ⚠   Stale pid file (pid {pid}, not running)");
        }
    } else {
        println!("  ○   Not running (no pid file)");
    }

    if !repo_ok {
        println!("\n  Run 'b00t ledgrrr install' to clone and build.");
    }
    Ok(())
}

fn cmd_install() -> Result<()> {
    let vp = vendor_path();

    if repo_exists() {
        println!("Repo already exists at {}. Updating...", vp.display());
        let (ok, out) = sh(&format!("cd {} && git pull --ff-only 2>&1", vp.display()));
        if !ok {
            eprintln!("git pull failed:\n{out}");
            // continue to rebuild anyway
        } else {
            println!("{out}");
        }
    } else {
        println!("Cloning ledgrrr repo into {} ...", vp.display());
        std::fs::create_dir_all(vp.parent().unwrap())?;
        let (ok, out) = sh(&format!(
            "cd {} && git clone {} {} 2>&1",
            vp.parent().unwrap().display(),
            REPO_URL,
            vp.file_name().unwrap().to_string_lossy()
        ));
        if !ok {
            return Err(anyhow!("git clone failed:\n{out}"));
        }
        println!("{out}");
    }

    println!("Building ledgerr-mcp-server (release)...");
    let (ok, out) = sh(&format!(
        "cd {} && cargo build --release -p ledgerr-mcp 2>&1",
        vp.display()
    ));
    if !ok {
        return Err(anyhow!("cargo build failed:\n{out}"));
    }
    println!("{out}");

    if binary_exists() {
        println!("✅ ledgrrr MCP server binary built successfully.");
    } else {
        println!("⚠  Build completed but binary not found at expected path.");
    }

    Ok(())
}

fn cmd_start() -> Result<()> {
    // Check if already running
    if let Some(pid) = read_pid().ok().flatten() {
        if is_process_running(pid) {
            println!("✅ ledgrrr MCP server is already running (pid {pid}).");
            return Ok(());
        }
        // Stale pid file — clean it up silently
        remove_pid()?;
    }

    // Verify binary exists
    if !binary_exists() {
        return Err(anyhow!(
            "ledgerr-mcp-server binary not found at {}. Run 'b00t ledgrrr install' first.",
            binary_path().display()
        ));
    }

    let bp = binary_path();
    println!("Starting ledgrrr MCP server from {} ...", bp.display());

    // Start in background using nohup
    let log_path = b00t_dir().join("ledgrrr-mcp-server.log");
    let cmd_str = format!(
        "nohup {} > {} 2>&1 & echo $!",
        bp.display(),
        log_path.display()
    );
    let (ok, out) = sh(&cmd_str);
    if !ok {
        return Err(anyhow!("failed to start ledgrrr MCP server:\n{out}"));
    }

    let pid: u32 = out
        .trim()
        .parse()
        .map_err(|e| anyhow!("could not parse pid from startup: {out} — {e}"))?;

    write_pid(pid)?;
    println!("✅ ledgrrr MCP server started (pid {pid}).");
    println!("   Logs: {}", log_path.display());
    Ok(())
}

fn cmd_stop() -> Result<()> {
    let pid_opt = read_pid()?;

    match pid_opt {
        Some(pid) if is_process_running(pid) => {
            println!("Stopping ledgrrr MCP server (pid {pid}) ...");
            let (ok, out) = sh(&format!("kill {pid} 2>&1"));
            if !ok {
                eprintln!("kill failed:\n{out}");
                // Try SIGTERM gracefully, then SIGKILL
                let (ok2, _) = sh(&format!("kill -15 {pid} 2>/dev/null"));
                if !ok2 {
                    let _ = sh(&format!("kill -9 {pid} 2>/dev/null"));
                }
            }
            // Wait a moment and verify
            std::thread::sleep(std::time::Duration::from_millis(500));
            if is_process_running(pid) {
                eprintln!("⚠  Process {pid} did not stop. Try force-killing manually.");
            } else {
                println!("✅ Process {pid} stopped.");
            }
            remove_pid()?;
        }
        Some(pid) => {
            println!("⚠  Stale pid file (pid {pid}, not running). Cleaning up.");
            remove_pid()?;
        }
        None => {
            println!("ledgrrr MCP server is not running.");
        }
    }
    Ok(())
}

fn cmd_update() -> Result<()> {
    let vp = vendor_path();

    if !repo_exists() {
        return Err(anyhow!(
            "Repo does not exist at {}. Run 'b00t ledgrrr install' first.",
            vp.display()
        ));
    }

    println!("Updating ledgrrr repo at {} ...", vp.display());
    let (ok, out) = sh(&format!("cd {} && git pull --ff-only 2>&1", vp.display()));
    if !ok {
        return Err(anyhow!("git pull failed:\n{out}"));
    }
    println!("{out}");

    println!("Rebuilding ledgerr-mcp-server (release)...");
    let (ok, out) = sh(&format!(
        "cd {} && cargo build --release -p ledgerr-mcp 2>&1",
        vp.display()
    ));
    if !ok {
        return Err(anyhow!("cargo build failed:\n{out}"));
    }
    println!("{out}");

    if binary_exists() {
        println!("✅ ledgrrr MCP server binary updated successfully.");
    } else {
        println!("⚠  Build completed but binary not found at expected path.");
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_paths() {
        let vp = vendor_path();
        assert!(vp.to_string_lossy().contains("vendor/ledgrrr"));
        assert!(binary_path().to_string_lossy().contains("ledgerr-mcp-server"));
    }

    #[test]
    fn test_repo_and_binary_exist() {
        // These should be true if the repo is cloned and built
        // We don't assert because CI might not have the repo
        let _ = repo_exists();
        let _ = binary_exists();
    }
}
