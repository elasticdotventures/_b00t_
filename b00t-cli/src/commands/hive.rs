//! `b00t hive` — hive CMDB commands for local system state management
//!
//! status   — show system resources + active profile
//! plan     — show what activating a profile would do
//! activate — transition to a named profile (stop/start services)
//! run      — run a command through guard evaluation
//! list     — list available hive profiles

use anyhow::{Context, Result, bail};
use clap::Parser;
use std::collections::HashMap;
use std::path::{Path, PathBuf};

use crate::hive::{
    GuardContext, GuardResult, HiveProfile, SystemSnapshot, activate_profile, check_guards,
    discover_profiles, hive_stacks_status, load_profile,
};

#[derive(Parser)]
pub enum HiveCommands {
    #[clap(
        about = "Show system resource state and active hive profile",
        long_about = "Reads RAM, GPU, CPU, running services and active profile.\n\nExamples:\n  b00t hive status\n  b00t hive status --json\n  b00t hive status --guards"
    )]
    Status {
        #[clap(long, help = "Output as JSON")]
        json: bool,
        #[clap(long, help = "Include active guards in output")]
        guards: bool,
    },

    #[clap(
        about = "List available hive profiles",
        long_about = "List all .hive.toml profiles in the datum directory.\n\nExamples:\n  b00t hive list"
    )]
    List {
        #[clap(long, help = "Output as JSON")]
        json: bool,
    },

    #[clap(
        about = "Show what activating a profile would do (dry-run)",
        long_about = "Show resource gate check, services to start/stop, guards.\n\nExamples:\n  b00t hive plan inference-qwen3\n  b00t hive plan download-mode"
    )]
    Plan {
        #[clap(help = "Profile name (e.g. inference-qwen3, download-mode)")]
        profile: String,
        #[clap(long, help = "Output as JSON")]
        json: bool,
    },

    #[clap(
        about = "Activate a hive profile (stop/start systemd services)",
        long_about = "Transitions system to named profile.\nChecks resource gates, stops conflicting services, starts required services.\n\nExamples:\n  b00t hive activate download-mode\n  b00t hive activate inference-qwen3 --force\n  b00t hive activate inference-sm0l --dry-run"
    )]
    Activate {
        #[clap(help = "Profile name")]
        profile: String,
        #[clap(long, help = "Show plan without executing")]
        dry_run: bool,
        #[clap(long, help = "Skip resource gate checks")]
        force: bool,
    },

    #[clap(
        about = "Run a command through guard evaluation",
        long_about = "Checks command against active profile guards + universal guards.\nWarns on misaligned patterns (pip→uv, docker→podman, etc.).\n\nExamples:\n  b00t run pip install requests\n  b00t run docker run --rm ubuntu echo hello\n  b00t run -- rm -rf /tmp/foo"
    )]
    Run {
        #[clap(required = true, help = "Command to evaluate")]
        command: Vec<String>,
        #[clap(long, help = "Strict mode: block on warn (default: warn and proceed)")]
        strict: bool,
        #[clap(long, help = "Dry-run: evaluate guards but don't execute")]
        dry_run: bool,
    },

    #[clap(
        about = "List and manage hive peer nodes across trust zones",
        long_about = "Hive peers are discovered b00t nodes across trust zones (local, LAN, VPN, internet)."
    )]
    Peers {
        #[clap(subcommand)]
        peer_command: PeerCommands,
    },

    #[clap(about = "Manage hive guard patterns (GuardManager authority gate)")]
    Guard {
        #[clap(subcommand)]
        guard_command: GuardManagerCommands,
    },

    #[clap(subcommand)]
    Cyber(HiveCyberCommands),
}

/// Guard Manager commands — privileged write access to hive-guards.hive.toml.
/// Future: requires ed25519 signature from executive role (PRD-ARCH guard-manager datum).
#[derive(Parser)]
pub enum GuardManagerCommands {
    #[clap(about = "List all active guards (universal + active profile)")]
    List {
        #[clap(long, help = "Output in JSON format")]
        json: bool,
    },
    #[clap(about = "Append a new guard to hive-guards.hive.toml")]
    Add {
        #[clap(help = "Pattern string (exact) or rhai expression")]
        pattern: String,
        #[clap(long, default_value = "warn", value_parser = ["warn", "block", "redirect"])]
        action: String,
        #[clap(long, help = "Warning message shown to agent")]
        message: String,
        #[clap(long, help = "Redirect suggestion (for warn/redirect actions)")]
        redirect: Option<String>,
    },
    #[clap(about = "Show guard violation statistics from guard-violations.jsonl")]
    Audit {
        #[clap(long, help = "Show last N violations", default_value = "20")]
        limit: usize,
        #[clap(long, help = "Output in JSON format")]
        json: bool,
    },
}

#[derive(Parser)]
pub enum PeerCommands {
    #[clap(about = "List all known hive peers with trust zone and health status")]
    List {
        #[clap(long, help = "Output as JSON")]
        json: bool,
        #[clap(
            long,
            help = "Health-check all peers in parallel (5s timeout per peer)"
        )]
        health: bool,
    },
    #[clap(about = "Register a peer in the hive ledger")]
    Add {
        #[clap(help = "Peer identifier")]
        id: String,
        #[clap(help = "Address (host:port or URL)")]
        address: String,
        #[clap(long, help = "Authentication type (ssh, tls, jwt)")]
        auth_type: Option<String>,
    },
    #[clap(about = "Remove a peer from the hive ledger")]
    Remove {
        #[clap(help = "Peer ID to remove")]
        id: String,
    },
    #[clap(about = "Health-check a specific peer")]
    Status {
        #[clap(help = "Peer ID to check")]
        id: String,
    },
    #[clap(about = "Gossip with a random peer to discover new nodes")]
    Gossip,
    #[clap(about = "Remove peers that haven't been seen since a cutoff")]
    Prune {
        #[clap(long, help = "Cutoff age (e.g. 30d, 7d, 24h)", default_value = "30d")]
        older_than: String,
    },
    #[clap(about = "Scan local network for b00t hive nodes")]
    Discover {
        #[clap(long, help = "Subnet to scan (e.g. 192.168.1.0/24)")]
        subnet: Option<String>,
    },
}

#[derive(Parser, Clone)]
pub enum HiveCyberCommands {
    #[clap(about = "Show trust boundary status (OS_GUEST/OS_ROOT/LAN)")]
    RingFence {
        #[clap(long, help = "Emit as JSON")]
        json: bool,
    },
}

pub fn handle_hive_command(cmd: &HiveCommands, path: &str) -> Result<()> {
    let datum_dir = PathBuf::from(shellexpand::tilde(path).to_string());

    match cmd {
        HiveCommands::Status { json, guards } => {
            let (json, guards) = (*json, *guards);
            let snapshot = SystemSnapshot::capture()?;

            if json {
                println!("{}", serde_json::to_string_pretty(&snapshot)?);
                return Ok(());
            }

            println!("HIVE STATUS  {}", snapshot.summary_line());
            println!();
            println!(
                "  RAM:   {:.1}GB avail / {:.1}GB total",
                snapshot.ram_available_gb, snapshot.ram_total_gb
            );
            println!(
                "  Swap:  {:.1}GB free / {:.1}GB total",
                snapshot.swap_free_gb, snapshot.swap_total_gb
            );
            if let (Some(name), Some(free), Some(total)) = (
                &snapshot.gpu_name,
                snapshot.gpu_free_mb,
                snapshot.gpu_total_mb,
            ) {
                println!("  GPU:   {} — {}MB free / {}MB total", name, free, total);
            } else {
                println!("  GPU:   none detected");
            }
            println!("  CPU:   {} cores", snapshot.cpu_cores);
            println!();

            match &snapshot.active_profile {
                Some(p) => println!("  Profile:  {}", p),
                None => println!("  Profile:  none (run: b00t hive activate <profile>)"),
            }

            if !snapshot.active_downloads.is_empty() {
                println!();
                println!("  Downloads active:");
                for d in &snapshot.active_downloads {
                    println!("    {}", d);
                }
            }

            if !snapshot.active_services.is_empty() {
                println!();
                println!("  Systemd user services:");
                for s in &snapshot.active_services {
                    if s.contains(".service") {
                        println!("    {}", s);
                    }
                }
            }

            // Show b00t hive stacks (b00t@*.service and b00t-hive-*.service)
            let stacks = hive_stacks_status();
            if !stacks.is_empty() {
                println!();
                println!("  Hive stacks:");
                for (unit, active, enabled) in &stacks {
                    let status = match (active, enabled) {
                        (true, true) => "active+enabled",
                        (true, false) => "active",
                        (false, true) => "enabled",
                        (false, false) => "inactive",
                    };
                    println!("    [{}] {}", status, unit);
                }
            }

            if guards {
                // Load universal guards + active profile guards
                let all_guards = load_all_guards(&datum_dir, &snapshot);
                println!();
                println!("  Guards ({}):", all_guards.len());
                for g in &all_guards {
                    println!(
                        "    [{:?}] {:?}  →  {}",
                        g.action,
                        g.pattern,
                        g.message.as_deref().unwrap_or("")
                    );
                }
            }

            Ok(())
        }

        HiveCommands::List { json } => {
            let json = *json;
            let profiles = discover_profiles(&datum_dir);

            if json {
                let names: Vec<&str> = profiles.iter().map(|(n, _)| n.as_str()).collect();
                println!("{}", serde_json::to_string_pretty(&names)?);
                return Ok(());
            }

            println!("Hive profiles ({}):", profiles.len());
            for (name, path) in &profiles {
                match HiveProfile::from_file(path) {
                    Ok(p) => {
                        let resources = format!(
                            "RAM: {}GB  GPU: {}MB",
                            p.resources_ram_gb
                                .map(|r| format!("{:.0}", r))
                                .unwrap_or("?".into()),
                            p.resources_gpu_mb
                                .map(|g| g.to_string())
                                .unwrap_or("?".into()),
                        );
                        println!("  {:30}  {:30}  {}", name, resources, p.hint);
                    }
                    Err(e) => println!("  {:30}  [parse error: {}]", name, e),
                }
            }
            Ok(())
        }

        HiveCommands::Plan { profile, json } => {
            let json = *json;
            let snapshot = SystemSnapshot::capture()?;
            let p = load_profile(profile, &datum_dir)?;
            let issues = snapshot.satisfies_gate(&p);

            if json {
                let plan = serde_json::json!({
                    "profile": profile,
                    "snapshot": snapshot,
                    "gate_issues": issues,
                    "services_stop": p.services_stop,
                    "services_start": p.services_start,
                    "guards": p.guards.len(),
                    "service_spec": p.service_spec.is_some(),
                });
                println!("{}", serde_json::to_string_pretty(&plan)?);
                return Ok(());
            }

            println!("Plan: activate '{}'", profile);
            println!("  {}", snapshot.summary_line());
            println!();

            if issues.is_empty() {
                println!("  Gate:  PASS");
            } else {
                println!("  Gate:  FAIL");
                for issue in &issues {
                    println!("    ⚠️  {}", issue);
                }
            }

            if !p.services_stop.is_empty() {
                println!();
                println!("  Stop:");
                for s in &p.services_stop {
                    println!("    systemctl --user stop {}", s);
                }
            }
            if !p.services_start.is_empty() {
                println!();
                println!("  Start:");
                for s in &p.services_start {
                    println!("    systemctl --user start {}", s);
                }
            }
            if !p.guards.is_empty() {
                println!();
                println!("  Guards: {} profile-specific", p.guards.len());
            }

            if p.service_spec.is_some() {
                println!();
                println!("  Service: will generate b00t-hive-{}.service", profile);
            }

            Ok(())
        }

        HiveCommands::Activate {
            profile,
            dry_run,
            force,
        } => {
            let (dry_run, force) = (*dry_run, *force);
            let snapshot = SystemSnapshot::capture()?;
            let p = load_profile(profile, &datum_dir)?;

            println!("Activating profile '{}' ...", profile);
            if dry_run {
                println!("  [dry-run mode]");
            }
            println!("  {}", snapshot.summary_line());
            println!();

            match activate_profile(&p, &snapshot, dry_run, force) {
                Ok(log) => {
                    for line in &log {
                        println!("  {}", line);
                    }
                    Ok(())
                }
                Err(e) => {
                    eprintln!("activation failed: {}", e);
                    eprintln!();
                    eprintln!("Tip: use --force to skip gate, --dry-run to preview");
                    std::process::exit(1);
                }
            }
        }

        HiveCommands::Peers { peer_command } => handle_peer_command(peer_command),
        HiveCommands::Cyber(cyber_cmd) => handle_cyber_command(cyber_cmd),
        HiveCommands::Guard { guard_command } => handle_guard_command(guard_command, path),
        HiveCommands::Run {
            command,
            strict,
            dry_run,
        } => {
            let (strict, dry_run) = (*strict, *dry_run);
            if command.is_empty() {
                bail!("no command specified; usage: b00t run <command> [args...]");
            }

            let cmd_str = command.join(" ");

            // Expand @tier: delegation prefix before guard evaluation
            let (cmd_str, delegation) = {
                let parsed = crate::delegation_macros::parse_delegation_prefix(&cmd_str);
                match parsed {
                    Some(ref m) => {
                        let expanded = crate::delegation_macros::expand(m);
                        if dry_run {
                            println!("[dry-run] @{tier}: expanded → {expanded}", tier = m.tier);
                            if let Some(task) = &m.task_id {
                                println!("  task update: b00t task done {task} on completion");
                            }
                        }
                        (expanded, parsed)
                    }
                    None => (cmd_str.clone(), None),
                }
            };

            let snapshot = SystemSnapshot::capture()?;
            let (all_guards, rhai_macros) = load_all_guard_state(&datum_dir, &snapshot);
            let guard_ctx = GuardContext {
                command: cmd_str.clone(),
                violation_count: 0,
                repeat_threshold: None,
                rhai_macros,
            };

            match check_guards(&cmd_str, &all_guards, &guard_ctx) {
                GuardResult::Allow => {
                    // pass-through
                }
                GuardResult::Warn { message, redirect } => {
                    eprintln!("{}", message);
                    if let Some(alt) = &redirect {
                        eprintln!("  suggested: {}", alt);
                    }
                    if strict {
                        bail!("strict mode: blocked by guard warning");
                    }
                    eprintln!();
                    // proceed with original command (warn-only)
                }
                GuardResult::Block { message } => {
                    eprintln!("🚫 BLOCKED: {}", message);
                    std::process::exit(1);
                }
            }

            if dry_run {
                println!("[dry-run] would execute: {}", cmd_str);
                return Ok(());
            }

            // Execute the command
            let exec_command = if delegation.is_some() {
                shlex::split(&cmd_str)
                    .ok_or_else(|| anyhow::anyhow!("failed to parse expanded command: {cmd_str}"))?
            } else {
                command.clone()
            };
            if exec_command.is_empty() {
                bail!("expanded command is empty");
            }

            let status = std::process::Command::new(&exec_command[0])
                .args(&exec_command[1..])
                .status()
                .map_err(|e| anyhow::anyhow!("failed to execute '{}': {}", exec_command[0], e))?;

            std::process::exit(status.code().unwrap_or(1));
        }
    }
}

fn handle_peer_command(cmd: &PeerCommands) -> Result<()> {
    match cmd {
        PeerCommands::List { json, health: _ } => {
            if *json {
                println!("[]");
            } else {
                println!("No hive peers registered");
            }
            Ok(())
        }
        PeerCommands::Add { .. }
        | PeerCommands::Remove { .. }
        | PeerCommands::Status { .. }
        | PeerCommands::Gossip
        | PeerCommands::Prune { .. }
        | PeerCommands::Discover { .. } => {
            bail!("hive peer registry commands are declared but not implemented yet")
        }
    }
}

fn handle_cyber_command(cmd: &HiveCyberCommands) -> Result<()> {
    match cmd {
        HiveCyberCommands::RingFence { json } => {
            let is_root = unsafe { libc::geteuid() == 0 };
            let mode = if is_root { "OS_ROOT" } else { "OS_GUEST" };

            if *json {
                println!(r#"{{"mode":"{mode}","is_root":{is_root}}}"#);
            } else {
                println!("🔒 Trust boundary: {mode}");
                println!("   is_root: {is_root}");
            }
            Ok(())
        }
    }
}

/// Load universal guards from hive-guards.hive.toml + active profile guards
fn load_all_guards(datum_dir: &Path, snapshot: &SystemSnapshot) -> Vec<crate::hive::HiveGuard> {
    load_all_guard_state(datum_dir, snapshot).0
}

/// Load universal guards, active profile guards, and their Rhai macro table.
fn load_all_guard_state(
    datum_dir: &Path,
    snapshot: &SystemSnapshot,
) -> (Vec<crate::hive::HiveGuard>, HashMap<String, String>) {
    let mut guards = Vec::new();
    let mut rhai_macros = HashMap::new();

    // 1. Universal guards
    if let Ok(g) = load_profile("hive-guards", datum_dir) {
        rhai_macros.extend(g.rhai_macros);
        guards.extend(g.guards);
    }

    // 2. Active profile guards
    if let Some(active) = &snapshot.active_profile {
        if let Ok(p) = load_profile(active, datum_dir) {
            rhai_macros.extend(p.rhai_macros);
            guards.extend(p.guards);
        }
    }

    (guards, rhai_macros)
}

// ── Guard Manager ─────────────────────────────────────────────────────────────

fn handle_guard_command(cmd: &GuardManagerCommands, path: &str) -> Result<()> {
    match cmd {
        GuardManagerCommands::List { json } => guard_list(path, *json),
        GuardManagerCommands::Add {
            pattern,
            action,
            message,
            redirect,
        } => guard_add(path, pattern, action, message, redirect.as_deref()),
        GuardManagerCommands::Audit { limit, json } => guard_audit(path, *limit, *json),
    }
}

fn guard_list(path: &str, as_json: bool) -> Result<()> {
    use crate::hive::SystemSnapshot;

    let datum_dir = std::path::PathBuf::from(shellexpand::tilde(path).to_string());
    let snapshot = SystemSnapshot::capture()?;
    let (guards, _macros) = load_all_guard_state(&datum_dir, &snapshot);

    if as_json {
        println!("{}", serde_json::to_string_pretty(&guards)?);
        return Ok(());
    }

    println!("{:<5} {:<12} {}", "IDX", "ACTION", "PATTERN");
    println!("{}", "-".repeat(70));
    for (i, g) in guards.iter().enumerate() {
        let pat_str = match &g.pattern {
            crate::hive::GuardPattern::JsonRegexPattern(s) => s.clone(),
            crate::hive::GuardPattern::RhaiExpr(e) => format!("rhai:{}", e.rhai),
            crate::hive::GuardPattern::K0mmand3rStage(s) => format!("stage:{}", s.stage),
        };
        println!("{:<5} {:<12} {}", i, format!("{:?}", g.action), pat_str);
    }
    println!("\n{} guards active", guards.len());
    Ok(())
}

fn guard_add(
    path: &str,
    pattern: &str,
    action: &str,
    message: &str,
    redirect: Option<&str>,
) -> Result<()> {
    let datum_dir = shellexpand::tilde(path).to_string();
    let guards_file = format!("{datum_dir}/_b00t_/hive-guards.hive.toml");

    let append_block = format!(
        "\n[[b00t.hive.guards]]\npattern = {pattern_toml}\naction = \"{action}\"\nmessage = \"{message}\"\n{redirect_line}",
        pattern_toml = format_pattern_toml(pattern),
        redirect_line = redirect
            .map(|r| format!("redirect = \"{r}\"\n"))
            .unwrap_or_default(),
    );

    // Validate the pattern is non-empty
    if pattern.trim().is_empty() {
        anyhow::bail!("guard pattern must not be empty");
    }

    // Append to file (preserves existing content + comments)
    use std::io::Write;
    let mut f = std::fs::OpenOptions::new()
        .append(true)
        .open(&guards_file)
        .with_context(|| format!("opening {guards_file}"))?;
    f.write_all(append_block.as_bytes())?;

    // Append audit log entry
    append_guard_audit_log("add", pattern, action, message);

    eprintln!("✅ guard added to {guards_file}");
    eprintln!("   pattern: {pattern}  action: {action}");
    eprintln!("   validate: b00t hive guard list");
    Ok(())
}

fn format_pattern_toml(pattern: &str) -> String {
    // If pattern looks like a Rhai expression, wrap as inline table
    if pattern.starts_with('{') {
        pattern.to_owned()
    } else {
        format!("\"{}\"", pattern.replace('"', "\\\""))
    }
}

fn guard_audit(path: &str, limit: usize, as_json: bool) -> Result<()> {
    let datum_dir = shellexpand::tilde(path).to_string();
    let violations_file = format!("{datum_dir}/guard-violations.jsonl");
    let audit_file = format!("{datum_dir}/guard-manager-audit.jsonl");

    // Parse guard-violations.jsonl
    let mut violations: Vec<serde_json::Value> = Vec::new();
    if let Ok(content) = std::fs::read_to_string(&violations_file) {
        for line in content.lines().rev().take(limit) {
            if let Ok(v) = serde_json::from_str::<serde_json::Value>(line) {
                violations.push(v);
            }
        }
    }

    // Parse guard manager audit log
    let mut audit_entries: Vec<serde_json::Value> = Vec::new();
    if let Ok(content) = std::fs::read_to_string(&audit_file) {
        for line in content.lines().rev().take(10) {
            if let Ok(v) = serde_json::from_str::<serde_json::Value>(line) {
                audit_entries.push(v);
            }
        }
    }

    if as_json {
        println!(
            "{}",
            serde_json::to_string_pretty(&serde_json::json!({
                "violations": violations,
                "guard_manager_audit": audit_entries,
            }))?
        );
        return Ok(());
    }

    println!("Guard violations (last {limit}):");
    if violations.is_empty() {
        println!("  (none)");
    } else {
        for v in &violations {
            let cmd = v.get("command").and_then(|c| c.as_str()).unwrap_or("?");
            let guard = v.get("guard").and_then(|g| g.as_str()).unwrap_or("?");
            let ts = v.get("timestamp").and_then(|t| t.as_str()).unwrap_or("?");
            println!("  [{ts}] {guard}: {cmd}");
        }
    }

    if !audit_entries.is_empty() {
        println!("\nGuard Manager audit (last 10 changes):");
        for e in &audit_entries {
            let op = e.get("op").and_then(|o| o.as_str()).unwrap_or("?");
            let pat = e.get("pattern").and_then(|p| p.as_str()).unwrap_or("?");
            let ts = e.get("timestamp").and_then(|t| t.as_str()).unwrap_or("?");
            println!("  [{ts}] {op}: {pat}");
        }
    }

    println!(
        "\n{} violations | guard-manager-audit: {} entries",
        violations.len(),
        audit_entries.len()
    );
    Ok(())
}

fn append_guard_audit_log(op: &str, pattern: &str, action: &str, message: &str) {
    let datum_dir = shellexpand::tilde("~/.b00t").to_string();
    let audit_file = format!("{datum_dir}/guard-manager-audit.jsonl");
    let entry = serde_json::json!({
        "timestamp": chrono::Utc::now().to_rfc3339(),
        "op": op,
        "pattern": pattern,
        "action": action,
        "message": message,
    });
    if let Ok(mut f) = std::fs::OpenOptions::new()
        .append(true)
        .create(true)
        .open(&audit_file)
    {
        use std::io::Write;
        let _ = writeln!(f, "{}", entry);
    }
}
