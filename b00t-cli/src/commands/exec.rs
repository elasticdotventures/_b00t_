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

use anyhow::{bail, Result};
use chrono::Utc;
use clap::Parser;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

use crate::hive::{check_guards, load_profile, GuardContext, GuardResult, SystemSnapshot};
use crate::traits::{ExecPlan, IoMethod, NoSandbox, Sandbox, SandboxKind, SystemdRunSandbox};

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

    #[clap(
        long,
        help = "Justification for a Block-tier command — routes through adversarial-model review instead of the 5-minute resubmit bypass. See PRD-SUDO-OPERATOR-GOVERNANCE."
    )]
    pub justification: Option<String>,

    #[clap(
        long,
        action = clap::ArgAction::Append,
        help = "Commit hash supporting --justification (repeat for multiple); their `git show --stat` grounds the adversarial review"
    )]
    pub cites: Vec<String>,

    #[clap(
        long,
        help = "Treat this invocation as a vetted-script grant request: the single command argument must be a path registered in _b00t_/vetted-scripts.toml whose content matches origin/main. No --justification/--cites needed. See PRD-SUDO-OPERATOR-GOVERNANCE's vetted-script extension."
    )]
    pub vetted: bool,
}

#[derive(Serialize, Deserialize, Default)]
struct AuditLogEntry {
    ts: String,     // ISO8601
    cmd: String,    // full command string
    result: String, // "allow" | "warn" | "block-rejected" | "block-forced" | "background" | "sudo-granted" | "sudo-denied" | "sudo-escalated" | "sudo-vetted-granted" | "sudo-vetted-denied"
    guard_msg: Option<String>,
    pid: Option<u32>, // child PID if executed
    #[serde(default, skip_serializing_if = "Option::is_none")]
    justification: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    cited_commits: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    sudo_disposition: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    checkpoint_ref: Option<String>,
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

/// Fail-closed variant of `append_audit_log` for paths where the caller's
/// safety invariant depends on the write actually happening (currently
/// only the vetted-grant path: "never executes without recorded evidence"
/// must not be satisfied by a write that silently failed).
fn append_audit_log_or_bail(log_path: &PathBuf, entry: &AuditLogEntry) -> Result<()> {
    let mut line = serde_json::to_string(entry)
        .map_err(|e| anyhow::anyhow!("failed to serialize audit log entry: {e}"))?;
    line.push('\n');
    use std::io::Write;
    let mut f = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(log_path)
        .map_err(|e| anyhow::anyhow!("failed to open audit log {}: {e}", log_path.display()))?;
    f.write_all(line.as_bytes())
        .map_err(|e| anyhow::anyhow!("failed to write audit log entry: {e}"))?;
    Ok(())
}

/// Resolve the repo root the same way `sudo` preserves the caller's cwd —
/// via `git rev-parse --show-toplevel`, per the vetted-sudo-mechanism
/// design spec, NOT `std::env::current_dir()` (which only works when
/// invoked from exactly the repo root, not any subdirectory).
fn resolve_repo_toplevel() -> Result<PathBuf> {
    use duct::cmd;
    let top = cmd!("git", "rev-parse", "--show-toplevel")
        .read()
        .map_err(|e| anyhow::anyhow!("git rev-parse --show-toplevel failed: {e}"))?;
    Ok(PathBuf::from(top.trim()))
}

fn now_unix() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

/// Load guards (universal hive-guards + active profile guards)
fn load_all_guard_context(
    datum_dir: &std::path::Path,
    snapshot: &SystemSnapshot,
) -> (Vec<crate::hive::HiveGuard>, HashMap<String, String>) {
    let mut guards = Vec::new();
    let mut rhai_macros = HashMap::new();
    if let Ok(g) = load_profile("hive-guards", datum_dir) {
        rhai_macros.extend(g.rhai_macros);
        guards.extend(g.guards);
    }
    if let Some(active) = &snapshot.active_profile {
        if let Ok(p) = load_profile(active, datum_dir) {
            rhai_macros.extend(p.rhai_macros);
            guards.extend(p.guards);
        }
    }
    // Agent-added session guards (~/.b00t/session-guards.json)
    guards.extend(crate::hive::load_session_guards());
    (guards, rhai_macros)
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
    let (all_guards, rhai_macros) = load_all_guard_context(&datum_dir, &snapshot);
    let guard_ctx = GuardContext {
        command: cmd_str.clone(),
        violation_count: 0,
        repeat_threshold: None,
        rhai_macros,
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

    // --vetted is a distinct, unconditional authorization path: the guard
    // tier (Allow/Warn/Block) is NOT consulted when this flag is set.
    // check_vetted() alone decides — this is what makes it safe to grant
    // this binary NOPASSWD sudo for `exec --vetted *`: the guard system
    // (mostly warn-tier, permissive by design) must never be able to
    // short-circuit this check.
    if args.vetted {
        use b00t_c0re_lib::sudo_operator::{check_vetted, SudoGrantEvidence, VettedResult};

        if args.command.len() != 1 {
            eprintln!("🚫 SUDO-VETTED-DENY: --vetted takes exactly one argument (the script path), no extra args");
            append_audit_log(
                &log_path,
                &AuditLogEntry {
                    ts: Utc::now().to_rfc3339(),
                    cmd: cmd_str.clone(),
                    result: "sudo-vetted-denied".into(),
                    guard_msg: None,
                    pid: None,
                    ..Default::default()
                },
            );
            std::process::exit(1);
        }

        // repo_root resolution — see I6 for why this uses
        // `git rev-parse --show-toplevel` instead of `current_dir()`.
        let project_root = resolve_repo_toplevel()?;

        match check_vetted(&project_root, &args.command[0]) {
            VettedResult::Vetted { blob_hash } => {
                let evidence = SudoGrantEvidence::new_vetted(&args.command[0], &blob_hash);

                // I3: the execution-readiness invariant Task 2 built must
                // actually gate execution, not just exist unused.
                if !evidence.verify() || !evidence.grant_is_execution_ready() {
                    eprintln!("🚫 SUDO-VETTED-DENY: evidence failed execution-readiness check (internal invariant violation)");
                    append_audit_log(
                        &log_path,
                        &AuditLogEntry {
                            ts: Utc::now().to_rfc3339(),
                            cmd: cmd_str.clone(),
                            result: "sudo-vetted-denied".into(),
                            guard_msg: None,
                            pid: None,
                            justification: None,
                            cited_commits: Vec::new(),
                            sudo_disposition: Some(evidence.disposition.clone()),
                            checkpoint_ref: None,
                        },
                    );
                    std::process::exit(1);
                }

                eprintln!(
                    "✅ SUDO-VETTED-GRANT: {} blob={} evidence={}",
                    args.command[0], blob_hash, evidence.content_hash
                );
                // I4: evidence recording must be fail-closed on this path —
                // see append_audit_log_or_bail above.
                append_audit_log_or_bail(
                    &log_path,
                    &AuditLogEntry {
                        ts: Utc::now().to_rfc3339(),
                        cmd: cmd_str.clone(),
                        result: "sudo-vetted-granted".into(),
                        guard_msg: None,
                        pid: None,
                        justification: None,
                        cited_commits: Vec::new(),
                        sudo_disposition: Some(evidence.disposition.clone()),
                        checkpoint_ref: None,
                    },
                )?;
                // fall through to the "Background execution via --sleep" /
                // synchronous execution section below (unchanged) — DO NOT
                // return early here, and DO NOT enter `match &guard_result`.
            }
            VettedResult::NotVetted { reason } => {
                eprintln!("🚫 SUDO-VETTED-DENY: {}", reason);
                append_audit_log(
                    &log_path,
                    &AuditLogEntry {
                        ts: Utc::now().to_rfc3339(),
                        cmd: cmd_str.clone(),
                        result: "sudo-vetted-denied".into(),
                        guard_msg: None,
                        pid: None,
                        justification: None,
                        cited_commits: Vec::new(),
                        sudo_disposition: None,
                        checkpoint_ref: None,
                    },
                );
                std::process::exit(1);
            }
        }
    } else {
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
                if let Some(justification) = &args.justification {
                    // Adversarial-review path (PRD-SUDO-OPERATOR-GOVERNANCE) —
                    // replaces the anonymous TTL-bypass below when a justification
                    // is supplied. Justification-less Block behavior (the `else`
                    // branch) is completely unchanged.
                    use b00t_c0re_lib::sudo_operator::{
                        adversarial_review, checkpoint_system_state, AdversarialVerdict,
                        SudoDisposition, SudoGrantEvidence,
                    };

                    let project_root =
                        std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
                    let (review_event, verdict) =
                        adversarial_review(&project_root, &cmd_str, justification, &args.cites)?;
                    let disposition: SudoDisposition = verdict.clone().into();

                    match &verdict {
                        AdversarialVerdict::Grant { ttl_seconds } => {
                            let checkpoint_id = now_unix().to_string();
                            let checkpoint =
                                checkpoint_system_state(Some(&project_root), &checkpoint_id, None)?;
                            let evidence = SudoGrantEvidence::new(&review_event, &disposition)
                                .with_checkpoint(checkpoint.as_evidence_string());

                            eprintln!(
                                "✅ SUDO-GRANT: {} (ttl={}s) evidence={}",
                                cmd_str, ttl_seconds, evidence.content_hash
                            );
                            append_audit_log(
                                &log_path,
                                &AuditLogEntry {
                                    ts: Utc::now().to_rfc3339(),
                                    cmd: cmd_str.clone(),
                                    result: "sudo-granted".into(),
                                    guard_msg: Some(message.clone()),
                                    pid: None,
                                    justification: Some(justification.clone()),
                                    cited_commits: args.cites.clone(),
                                    sudo_disposition: Some(disposition.to_string()),
                                    checkpoint_ref: Some(checkpoint.as_evidence_string()),
                                },
                            );
                            // fall through to execution below
                        }
                        AdversarialVerdict::Deny { reason } => {
                            eprintln!("🚫 SUDO-DENY: {}", reason);
                            append_audit_log(
                                &log_path,
                                &AuditLogEntry {
                                    ts: Utc::now().to_rfc3339(),
                                    cmd: cmd_str.clone(),
                                    result: "sudo-denied".into(),
                                    guard_msg: Some(message.clone()),
                                    pid: None,
                                    justification: Some(justification.clone()),
                                    cited_commits: args.cites.clone(),
                                    sudo_disposition: Some(disposition.to_string()),
                                    checkpoint_ref: None,
                                },
                            );
                            std::process::exit(1);
                        }
                        AdversarialVerdict::Escalate { reason } => {
                            eprintln!("📡 SUDO-ESCALATE: {}", reason);
                            crate::budget_controller::fire_sudo_escalation(
                                &cmd_str,
                                justification,
                                &args.cites,
                                reason,
                            );
                            notify_send(&format!("b00t sudo escalation: {cmd_str}"), reason);
                            append_audit_log(
                                &log_path,
                                &AuditLogEntry {
                                    ts: Utc::now().to_rfc3339(),
                                    cmd: cmd_str.clone(),
                                    result: "sudo-escalated".into(),
                                    guard_msg: Some(message.clone()),
                                    pid: None,
                                    justification: Some(justification.clone()),
                                    cited_commits: args.cites.clone(),
                                    sudo_disposition: Some(disposition.to_string()),
                                    checkpoint_ref: None,
                                },
                            );
                            eprintln!("   Not executed. Resolve the escalation and re-run.");
                            std::process::exit(1);
                        }
                    }
                } else {
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
                                    ..Default::default()
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
                                    ..Default::default()
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
                ..Default::default()
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
                ..Default::default()
            },
        );
        child.wait()?.code().unwrap_or(1)
    } else {
        // Sandbox provider: run() captures output
        let output = sandbox_provider
            .run(&plan)
            .map_err(|e| anyhow::anyhow!("sandbox exec failed: {}", e))?;

        append_audit_log(
            &log_path,
            &AuditLogEntry {
                ts: Utc::now().to_rfc3339(),
                cmd: cmd_str,
                result: format!("{}:{}", result_label, sandbox_kind_label),
                guard_msg,
                pid: None,
                ..Default::default()
            },
        );

        print!("{}", output.value);
        output.exit_code
    };

    std::process::exit(exit_code);
}

/// Parse simple duration string: "30s", "2m", "1h" → seconds
/// Best-effort local desktop notification (Linux `notify-send`), used as
/// one of the "plurality of channels" for a SudoDisposition::Escalate.
/// Matches the fallback pattern already used for maintenance reminders
/// (_b00t_/bash.🐚/README-bash.md, skills/b00t-maintenance/SKILL.md).
/// Never fails the caller — this is a notification, not a gate.
fn notify_send(summary: &str, body: &str) {
    let _ = std::process::Command::new("notify-send")
        .arg(summary)
        .arg(body)
        .spawn();
}

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
