//! `b00t exec` — audited command execution with guard enforcement and broad authority
//!
//! Broad authority rules:
//!   Allow  → execute immediately
//!   Warn   → print warning + execute immediately (no user gate)
//!   Block  → first submission: record hash, reject; re-submission within TTL: execute with warning
//!
//! Audit log: ~/.b00t/exec-log.jsonl  (append-only JSONL)
//! Audit cache: ~/.b00t/exec-audit.json  (BlockedCmd hash → unix timestamp)
//!
//! `--sleep=<duration>` → spawn background detached process; returns immediately

use anyhow::{Result, bail};
use chrono::Utc;
use clap::Parser;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

use crate::hive::{GuardContext, GuardResult, SystemSnapshot, check_guards, load_profile};
use crate::traits::{ExecPlan, IoMethod, NoSandbox, Sandbox, SandboxKind, SandboxRequirements, SystemdRunSandbox};

const AUDIT_CACHE_FILE: &str = "~/.b00t/exec-audit.json";
const AUDIT_LOG_FILE: &str = "~/.b00t/exec-log.jsonl";
const BLOCK_TTL_SECS: u64 = 300; // 5 min re-submission window

#[derive(Parser)]
#[clap(
    about = "Execute command with guard enforcement and audit log",
    long_about = "Broad-authority audited execution:\n\
  Allow  → run immediately\n\
  Warn   → print warning, run immediately (no gate)\n\
  Block  → first time: record + reject; re-submit within 5 min: run with audit warning\n\n\
All executions appended to ~/.b00t/exec-log.jsonl\n\n\
Examples:\n\
  b00t exec pip install requests        # blocked (first time); re-run to force\n\
  b00t exec cargo build                 # allowed directly\n\
  b00t exec --sleep=30s cargo test      # background; returns immediately"
)]
pub struct ExecArgs {
    #[clap(
        help = "Command and arguments",
        trailing_var_arg = true,
        allow_hyphen_values = true,
        num_args = 1..,
    )]
    pub command: Vec<String>,

    #[clap(
        long,
        help = "Background execution delay (e.g. 30s, 2m). Returns immediately; result summarized asynchronously."
    )]
    pub sleep: Option<String>,

    #[clap(long, help = "Dry-run: evaluate guards but don't execute")]
    pub dry_run: bool,

    #[clap(
        long,
        default_value = "direct",
        help = "Sandbox provider: direct | systemd-run | podman"
    )]
    pub sandbox: String,
}

#[derive(Serialize, Deserialize)]
struct AuditLogEntry {
    ts: String,     // ISO8601
    cmd: String,    // full command string
    result: String, // "allow" | "warn" | "block-rejected" | "block-forced" | "background"
    guard_msg: Option<String>,
    pid: Option<u32>, // child PID if executed
}

/// Load block-cache: cmd_key → unix timestamp of first rejection
fn load_block_cache(path: &PathBuf) -> HashMap<String, u64> {
    match std::fs::read_to_string(path) {
        Ok(s) => serde_json::from_str(&s).unwrap_or_default(),
        Err(_) => HashMap::new(),
    }
}

fn save_block_cache(path: &PathBuf, cache: &HashMap<String, u64>) {
    if let Ok(s) = serde_json::to_string(cache) {
        let _ = std::fs::write(path, s);
    }
}

/// Append one entry to the audit log (JSONL, best-effort)
fn append_audit_log(log_path: &PathBuf, entry: &AuditLogEntry) {
    if let Ok(mut line) = serde_json::to_string(entry) {
        line.push('\n');
        use std::io::Write;
        if let Ok(mut f) = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(log_path)
        {
            let _ = f.write_all(line.as_bytes());
        }
    }
}

fn now_unix() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

/// Load guards (universal hive-guards + active profile guards)
fn load_all_guards(
    datum_dir: &std::path::Path,
    snapshot: &SystemSnapshot,
) -> Vec<crate::hive::HiveGuard> {
    let mut guards = Vec::new();
    if let Ok(g) = load_profile("hive-guards", datum_dir) {
        guards.extend(g.guards);
    }
    if let Some(active) = &snapshot.active_profile {
        if let Ok(p) = load_profile(active, datum_dir) {
            guards.extend(p.guards);
        }
    }
    guards
}

pub fn handle_exec(args: &ExecArgs, path: &str) -> Result<()> {
    if args.command.is_empty() {
        bail!("no command; usage: b00t exec <command> [args...]");
    }

    let datum_dir = PathBuf::from(shellexpand::tilde(path).to_string());
    let cache_path = PathBuf::from(shellexpand::tilde(AUDIT_CACHE_FILE).to_string());
    let log_path = PathBuf::from(shellexpand::tilde(AUDIT_LOG_FILE).to_string());

    // ensure ~/.b00t/ exists
    if let Some(parent) = cache_path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }

    let cmd_str = args.command.join(" ");

    // Guard evaluation
    let snapshot = SystemSnapshot::capture()?;
    let all_guards = load_all_guards(&datum_dir, &snapshot);
    let guard_ctx = GuardContext {
        command: cmd_str.clone(),
        violation_count: 0,
        repeat_threshold: None,
        rhai_macros: HashMap::new(),
    };
    let guard_result = check_guards(&cmd_str, &all_guards, &guard_ctx);

    // In dry-run mode, evaluate guards but avoid any cache/log side effects.
    if args.dry_run {
        match &guard_result {
            GuardResult::Allow => {
                println!("[dry-run] would execute: {}", cmd_str);
            }
            GuardResult::Warn { message, redirect } => {
                eprintln!("⚠️  {}", message);
                if let Some(alt) = redirect {
                    eprintln!("   suggested: {}", alt);
                }
                println!("[dry-run] would execute: {}", cmd_str);
            }
            GuardResult::Block { message } => {
                eprintln!("❌ BLOCK: {}", message);
                println!("[dry-run] blocked command; would NOT execute: {}", cmd_str);
            }
        }
        return Ok(());
    }

    match &guard_result {
        GuardResult::Allow => {
            // no-op
        }
        GuardResult::Warn { message, redirect } => {
            eprintln!("⚠️  {}", message);
            if let Some(alt) = redirect {
                eprintln!("   suggested: {}", alt);
            }
            // broad authority: proceed without user gate
        }
        GuardResult::Block { message } => {
            let mut cache = load_block_cache(&cache_path);
            let now = now_unix();

            match cache.get(&cmd_str).copied() {
                Some(first_ts) if now.saturating_sub(first_ts) < BLOCK_TTL_SECS => {
                    // Re-submission within TTL → force-execute with audit warning
                    eprintln!(
                        "🔶 BLOCK-OVERRIDE: {} (re-submission within {}s TTL)",
                        message, BLOCK_TTL_SECS
                    );
                    eprintln!("   Executing with audit trail.");
                    cache.remove(&cmd_str); // consume the bypass
                    save_block_cache(&cache_path, &cache);

                    append_audit_log(
                        &log_path,
                        &AuditLogEntry {
                            ts: Utc::now().to_rfc3339(),
                            cmd: cmd_str.clone(),
                            result: "block-forced".into(),
                            guard_msg: Some(message.clone()),
                            pid: None,
                        },
                    );
                    // fall through to execution below
                }
                _ => {
                    // First rejection (or expired TTL): record + reject
                    cache.insert(cmd_str.clone(), now);
                    save_block_cache(&cache_path, &cache);

                    append_audit_log(
                        &log_path,
                        &AuditLogEntry {
                            ts: Utc::now().to_rfc3339(),
                            cmd: cmd_str.clone(),
                            result: "block-rejected".into(),
                            guard_msg: Some(message.clone()),
                            pid: None,
                        },
                    );

                    eprintln!("🚫 BLOCKED: {}", message);
                    eprintln!(
                        "   Re-submit within {}s to force execution.",
                        BLOCK_TTL_SECS
                    );
                    std::process::exit(1);
                }
            }
        }
    }

    // Background execution via --sleep
    if let Some(sleep_dur) = &args.sleep {
        let sleep_secs = parse_duration(sleep_dur)?;
        let command_clone = args.command.clone();

        // Apply the requested delay before spawning the background process.
        std::thread::sleep(std::time::Duration::from_secs(sleep_secs));

        append_audit_log(
            &log_path,
            &AuditLogEntry {
                ts: Utc::now().to_rfc3339(),
                cmd: cmd_str.clone(),
                result: "background".into(),
                guard_msg: None,
                pid: None,
            },
        );

        // Spawn detached — double-fork idiom via std::process::Command
        let child = std::process::Command::new(&command_clone[0])
            .args(&command_clone[1..])
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .spawn()
            .map_err(|e| anyhow::anyhow!("spawn failed '{}': {}", command_clone[0], e))?;

        let pid = child.id();
        println!(
            "🔄 background pid={} delay={}s cmd={}",
            pid, sleep_secs, cmd_str
        );

        // Return immediately; OS manages child lifetime
        std::mem::forget(child); // don't wait
        return Ok(());
    }

    // Synchronous execution
    let guard_msg = match &guard_result {
        GuardResult::Warn { message, .. } => Some(message.clone()),
        GuardResult::Block { message } => Some(message.clone()),
        GuardResult::Allow => None,
    };

    let result_label = match &guard_result {
        GuardResult::Warn { .. } => "warn",
        GuardResult::Block { .. } => "block-forced",
        GuardResult::Allow => "allow",
    };

    // Select sandbox provider from --sandbox flag
    let sandbox_provider: Box<dyn Sandbox> = match args.sandbox.as_str() {
        "systemd-run" => Box::new(SystemdRunSandbox::default()),
        "direct" | _ => Box::new(NoSandbox),
    };

    let sandbox_kind_label = match args.sandbox.as_str() {
        "systemd-run" => "systemd-run",
        _ => "direct",
    };

    // Build ExecPlan and run through sandbox provider
    let plan = ExecPlan {
        command_line: cmd_str.clone(),
        working_dir: std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from(".")),
        env: vec![],
        declared_effects: vec![],
        sandbox_kind: if sandbox_kind_label == "systemd-run" {
            SandboxKind::SystemdRun
        } else {
            SandboxKind::None
        },
        io_method: IoMethod::Stdio,
    };

    // For direct provider: use raw spawn so stdin/stdout/stderr are inherited
    let exit_code = if sandbox_kind_label == "direct" {
        let mut child = std::process::Command::new(&args.command[0])
            .args(&args.command[1..])
            .spawn()
            .map_err(|e| anyhow::anyhow!("exec failed '{}': {}", args.command[0], e))?;

        let pid = child.id();
        append_audit_log(
            &log_path,
            &AuditLogEntry {
                ts: Utc::now().to_rfc3339(),
                cmd: cmd_str.clone(),
                result: format!("{}:{}", result_label, sandbox_kind_label),
                guard_msg: guard_msg.clone(),
                pid: Some(pid),
            },
        );
        child.wait()?.code().unwrap_or(1)
    } else {
        // Sandbox provider: run() captures output
        let output = sandbox_provider.run(&plan)
            .map_err(|e| anyhow::anyhow!("sandbox exec failed: {}", e))?;

        append_audit_log(
            &log_path,
            &AuditLogEntry {
                ts: Utc::now().to_rfc3339(),
                cmd: cmd_str,
                result: format!("{}:{}", result_label, sandbox_kind_label),
                guard_msg,
                pid: None,
            },
        );

        print!("{}", output.value);
        output.exit_code
    };

    std::process::exit(exit_code);
}

/// Parse simple duration string: "30s", "2m", "1h" → seconds
fn parse_duration(s: &str) -> Result<u64> {
    if let Some(n) = s.strip_suffix('s') {
        return Ok(n
            .parse::<u64>()
            .map_err(|_| anyhow::anyhow!("invalid duration: {}", s))?);
    }
    if let Some(n) = s.strip_suffix('m') {
        return Ok(n
            .parse::<u64>()
            .map_err(|_| anyhow::anyhow!("invalid duration: {}", s))?
            * 60);
    }
    if let Some(n) = s.strip_suffix('h') {
        return Ok(n
            .parse::<u64>()
            .map_err(|_| anyhow::anyhow!("invalid duration: {}", s))?
            * 3600);
    }
    bail!("invalid duration '{}'; use 30s, 2m, 1h", s);
}
