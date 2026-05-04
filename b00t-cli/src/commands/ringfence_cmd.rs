//! `b00t ring-fence` — trust boundary inspection.
//!
//! Checks:
//! 1. OS_GUEST mode: running as non-root, no privileged capabilities
//! 2. OS_ROOT mode: running as root, or has CAP_SYS_ADMIN
//! 3. LAN status: local network interfaces, listening ports

use anyhow::Result;
use clap::Parser;
use serde_json::json;

#[derive(Parser, Clone)]
pub enum RingFenceCommands {
    #[clap(about = "Show current trust boundary status")]
    Status {
        #[clap(long, help = "Emit as JSON")]
        json: bool,
    },
}

pub fn handle_ringfence_command(cmd: &RingFenceCommands) -> Result<()> {
    match cmd {
        RingFenceCommands::Status { json } => handle_status(*json),
    }
}

/// Check if we are running as root (UID 0).
fn is_root() -> bool {
    // SAFETY: geteuid(2) is a trivial syscall with no side effects.
    unsafe { libc::geteuid() == 0 }
}

/// Check if the process has CAP_SYS_ADMIN by reading /proc/self/status.
fn has_cap_sys_admin() -> bool {
    let status = std::fs::read_to_string("/proc/self/status").unwrap_or_default();
    // CapEff (effective capability set) is a bitmask printed in hex.
    // CAP_SYS_ADMIN = 21 → bit 21.
    for line in status.lines() {
        if let Some(hex) = line.strip_prefix("CapEff:\t") {
            if let Ok(mask) = u64::from_str_radix(hex.trim(), 16) {
                return (mask & (1u64 << 21)) != 0;
            }
        }
    }
    false
}

/// Collect listening TCP ports by trying to bind to port 0 on 127.0.0.1
/// to see what's in use — lightweight, no external deps.
fn listening_ports() -> Vec<u16> {
    // We can't enumerate without raw socket access or /proc/net/tcp parsing.
    // Simple approach: check a few well-known ranges by attempting connect.
    // For portability, just check /proc/net/tcp if available.
    let mut ports = Vec::new();
    let content = std::fs::read_to_string("/proc/net/tcp").unwrap_or_default();
    // /proc/net/tcp lines: sl local_address rem_address st tx_queue ...
    // local_address format: 0100007F:0CEA → 127.0.0.1:3306  (hex host:hex port)
    for line in content.lines().skip(1) {
        let fields: Vec<&str> = line.split_whitespace().collect();
        if fields.len() < 2 {
            continue;
        }
        let local = fields[1]; // e.g. 0100007F:0CEA
        if let Some(hex_port) = local.split(':').nth(1) {
            if let Ok(port) = u16::from_str_radix(hex_port, 16) {
                // state 0A = TCP_LISTEN
                if fields.len() > 3 && fields[3] == "0A" {
                    ports.push(port);
                }
            }
        }
    }
    ports.sort_unstable();
    ports.dedup();
    ports
}

/// Collect local network interface IPs (simplified: read /proc/net/fib_trie or
/// use nix — here we use the `std::net::lookup_host` approach or parse /proc/net/route + /proc/net/fib_trie).
/// Most portable: try binding to determine addresses, or just note what's available.
fn local_interface_ips() -> Vec<String> {
    let mut ips = Vec::new();
    // Parse /proc/net/fib_trie for local addresses
    if let Ok(content) = std::fs::read_to_string("/proc/net/fib_trie") {
        for line in content.lines() {
            // Lines like `  +-- 127.0.0.1/32`
            if let Some(stripped) = line.trim().strip_prefix("+-- ") {
                if let Some(ip) = stripped.split('/').next() {
                    if !ip.is_empty() && ip != "0.0.0.0" && ip != "::" {
                        ips.push(ip.to_string());
                    }
                }
            }
        }
    }
    ips.sort_unstable();
    ips.dedup();
    ips
}

fn handle_status(json_output: bool) -> Result<()> {
    let root = is_root();
    let cap_sys_admin = has_cap_sys_admin();
    let ports = listening_ports();
    let iface_ips = local_interface_ips();

    let os_mode = if root {
        "OS_ROOT"
    } else if cap_sys_admin {
        "OS_ROOT (CAP_SYS_ADMIN)"
    } else {
        "OS_GUEST"
    };

    if json_output {
        let report = json!({
            "os_mode": os_mode,
            "is_root": root,
            "has_cap_sys_admin": cap_sys_admin,
            "listening_ports": ports,
            "local_ips": iface_ips,
            "trust_boundary": if root || cap_sys_admin { "elevated" } else { "restricted" },
        });
        println!("{}", serde_json::to_string_pretty(&report)?);
    } else {
        println!("🔒 Ring-Fence — Trust Boundary Status");
        println!("{:-^48}", "");
        println!("  OS mode:        {}", os_mode);
        println!("  is_root:        {}", root);
        println!("  CAP_SYS_ADMIN:  {}", cap_sys_admin);
        println!(
            "  Trust boundary: {}",
            if root || cap_sys_admin {
                "elevated (privileged access)"
            } else {
                "restricted (guest)"
            }
        );
        if !iface_ips.is_empty() {
            println!("  Local IPs:      {}", iface_ips.join(", "));
        }
        if !ports.is_empty() {
            println!(
                "  Listening:      {} port(s)",
                ports.len()
            );
            for chunk in ports.chunks(16) {
                let vals: Vec<String> = chunk.iter().map(|p| p.to_string()).collect();
                println!("    ports:        {}", vals.join(", "));
            }
        }
        println!("{:-^48}", "");
    }

    Ok(())
}
