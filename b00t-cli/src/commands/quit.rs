//! `b00t quit` — killswitch: terminate the upper agent instance, return CLI to prompt
//!
//! Resolution order:
//!   1. B00T_AGENT_PID env var (explicit)
//!   2. Walk /proc/<ppid>/status upward looking for known agent process names
//!   3. Fallback: SIGTERM to direct parent (PPID)
//!
//! Known agent process names: claude, opencode, aider, ralph, cursor, copilot

use anyhow::{Result, bail};
use clap::Parser;

const AGENT_NAMES: &[&str] = &["claude", "opencode", "aider", "ralph", "cursor"];

#[derive(Parser)]
#[clap(
    about = "Killswitch: terminate upper agent instance and return CLI to prompt",
    long_about = "Sends SIGTERM to the agent process managing this session.\n\n\
Resolution order:\n\
  1. B00T_AGENT_PID env var (explicit PID)\n\
  2. Walk process tree upward for known agent names (claude, opencode, etc.)\n\
  3. Fallback: SIGTERM to direct parent (PPID)\n\n\
Examples:\n\
  b00t quit\n\
  B00T_AGENT_PID=12345 b00t quit"
)]
pub struct QuitArgs {
    #[clap(long, help = "Signal to send (default: SIGTERM=15)", default_value = "15")]
    pub signal: i32,
    #[clap(long, help = "Dry-run: print target PID without sending signal")]
    pub dry_run: bool,
}

pub fn handle_quit(args: &QuitArgs) -> Result<()> {
    let target_pid = resolve_agent_pid()?;

    if args.dry_run {
        println!("[dry-run] would send signal {} to pid {}", args.signal, target_pid);
        return Ok(());
    }

    println!("🔴 b00t quit: sending signal {} to agent pid={}", args.signal, target_pid);

    // Safety: can only target our own process tree (non-root)
    let ret = libc_kill(target_pid as i32, args.signal);
    if ret != 0 {
        let err = std::io::Error::last_os_error();
        bail!("kill({}, {}) failed: {}", target_pid, args.signal, err);
    }

    Ok(())
}

/// Resolve the agent PID via env var or process tree walk
fn resolve_agent_pid() -> Result<u32> {
    // 1. Explicit env var
    if let Ok(s) = std::env::var("B00T_AGENT_PID") {
        if let Ok(pid) = s.trim().parse::<u32>() {
            eprintln!("🎯 target from B00T_AGENT_PID={}", pid);
            return Ok(pid);
        }
    }

    // 2. Walk process tree
    let my_pid = std::process::id();
    if let Some(agent_pid) = walk_to_agent(my_pid) {
        eprintln!("🎯 target via process tree: pid={}", agent_pid);
        return Ok(agent_pid);
    }

    // 3. Fallback: PPID
    let ppid = get_ppid(my_pid)?;
    eprintln!("⚠️  no agent process found; falling back to PPID={}", ppid);
    Ok(ppid)
}

/// Walk process tree upward until we find a known agent process name
fn walk_to_agent(start_pid: u32) -> Option<u32> {
    let mut current = start_pid;
    for _ in 0..16 {
        // max 16 levels
        let ppid = match get_ppid(current) {
            Ok(p) if p != 0 && p != current => p,
            _ => return None,
        };
        let name = proc_name(ppid).unwrap_or_default();
        let name_lower = name.to_lowercase();
        if AGENT_NAMES.iter().any(|&a| name_lower.contains(a)) {
            return Some(ppid);
        }
        current = ppid;
    }
    None
}

/// Read PPID from /proc/<pid>/status
fn get_ppid(pid: u32) -> Result<u32> {
    let path = format!("/proc/{}/status", pid);
    let content = std::fs::read_to_string(&path)
        .map_err(|e| anyhow::anyhow!("read {}: {}", path, e))?;
    for line in content.lines() {
        if let Some(rest) = line.strip_prefix("PPid:") {
            return Ok(rest.trim().parse::<u32>()
                .map_err(|_| anyhow::anyhow!("parse PPid from {}", path))?);
        }
    }
    bail!("PPid not found in {}", path)
}

/// Read process name from /proc/<pid>/comm
fn proc_name(pid: u32) -> Result<String> {
    let path = format!("/proc/{}/comm", pid);
    Ok(std::fs::read_to_string(&path)?.trim().to_string())
}

/// Thin libc kill wrapper — avoids pulling libc crate as dependency
#[cfg(target_os = "linux")]
fn libc_kill(pid: i32, sig: i32) -> i32 {
    use std::os::raw::c_int;
    unsafe extern "C" {
        fn kill(pid: c_int, sig: c_int) -> c_int;
    }
    unsafe { kill(pid, sig) }
}

#[cfg(not(target_os = "linux"))]
fn libc_kill(_pid: i32, _sig: i32) -> i32 {
    eprintln!("b00t quit: unsupported platform (linux only)");
    -1
}
