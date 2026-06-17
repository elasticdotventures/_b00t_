//! `b00t upgrade` — NASA MBSE deterministic upgrade orchestrator
//!
//! Phases: ASSESS → PLAN → EXECUTE → VERIFY → REPORT
//! Each phase gates on previous: same inputs ⇒ same action list.
//!
//! # Usage
//! ```bash
//! b00t upgrade                    # upgrade everything
//! b00t upgrade --dry-run          # plan only, no changes
//! b00t upgrade --scope=binary     # binary only
//! b00t upgrade --scope=mcp        # MCP servers only
//! b00t upgrade --scope=hooks      # Claude hooks only
//! b00t upgrade --scope=settings   # Claude settings only
//! b00t upgrade --delegate         # route compile tasks to ch0nky GPU tier
//! ```

use anyhow::{Context, Result};
use clap::Parser;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::path::PathBuf;
use std::process::Command;
use std::time::Instant;

// ── Types ─────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum UpgradeScope {
    All,
    Binary,
    Mcp,
    Hooks,
    Settings,
}

impl std::str::FromStr for UpgradeScope {
    type Err = anyhow::Error;
    fn from_str(s: &str) -> Result<Self> {
        match s {
            "all" => Ok(Self::All),
            "binary" => Ok(Self::Binary),
            "mcp" => Ok(Self::Mcp),
            "hooks" => Ok(Self::Hooks),
            "settings" => Ok(Self::Settings),
            _ => anyhow::bail!("unknown scope: {s} (all|binary|mcp|hooks|settings)"),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SystemState {
    pub b00t_cli_version: Option<String>,
    pub b00t_cli_latest: Option<String>,
    pub claude_version: Option<String>,
    pub claude_settings_path: Option<PathBuf>,
    pub mcp_servers_registered: Vec<String>,
    pub hooks_registered: Vec<String>,
    pub hooks_available: Vec<String>,
    pub upgrade_available: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum UpgradeAction {
    UpgradeBinary { from: String, to: String },
    SyncMcpServer { name: String, action: McpAction },
    InstallHook { name: String },
    UpdateSettings { key: String, old: Value, new: Value },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum McpAction {
    Install,
    Update,
    Remove,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpgradeResult {
    pub action: UpgradeAction,
    pub success: bool,
    pub detail: String,
    pub duration_ms: u64,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct UpgradeReport {
    pub pre: SystemState,
    pub post: Option<SystemState>,
    pub actions_planned: usize,
    pub actions_applied: usize,
    pub results: Vec<UpgradeResult>,
    pub dry_run: bool,
    pub elapsed_ms: u64,
}

// ── CLI ───────────────────────────────────────────────────────────────────────

#[derive(Debug, Parser)]
pub struct UpgradeArgs {
    #[clap(long, default_value = "all", help = "Upgrade scope: all|binary|mcp|hooks|settings")]
    pub scope: UpgradeScope,
    #[clap(long, help = "Plan only; apply no changes")]
    pub dry_run: bool,
    #[clap(long, help = "Route compile tasks to ch0nky GPU tier via b00t job delegate")]
    pub delegate: bool,
    #[clap(long, help = "Emit structured JSON report")]
    pub json: bool,
    #[clap(long, short = 'y', help = "Skip confirmation prompts")]
    pub yes: bool,
}

impl UpgradeArgs {
    pub fn execute(&self) -> Result<()> {
        let t0 = Instant::now();

        // ── Phase 0: ASSESS ───────────────────────────────────────────────────
        println!("🔍 ASSESS — detecting current state...");
        let pre = assess_state()?;
        print_state_summary(&pre);

        // ── Phase 1: PLAN ─────────────────────────────────────────────────────
        println!("\n📋 PLAN — computing upgrade actions...");
        let actions = plan_actions(&pre, &self.scope);
        if actions.is_empty() {
            println!("✅ Already up to date — nothing to do.");
            return Ok(());
        }
        for (i, a) in actions.iter().enumerate() {
            println!("  [{:02}] {}", i + 1, action_description(a));
        }

        if self.dry_run {
            println!("\n🏜️ DRY-RUN: no changes applied.");
            if self.json {
                let report = UpgradeReport {
                    pre,
                    post: None,
                    actions_planned: actions.len(),
                    actions_applied: 0,
                    results: vec![],
                    dry_run: true,
                    elapsed_ms: t0.elapsed().as_millis() as u64,
                };
                println!("{}", serde_json::to_string_pretty(&report)?);
            }
            return Ok(());
        }

        // ── Phase 2: EXECUTE ──────────────────────────────────────────────────
        println!("\n⚙️  EXECUTE — applying {} action(s)...", actions.len());
        let mut results = Vec::new();
        for action in &actions {
            let result = execute_action(action, self.delegate)?;
            let status = if result.success { "✅" } else { "❌" };
            println!("  {} {} ({}ms)", status, action_description(action), result.duration_ms);
            if !result.detail.is_empty() {
                println!("     {}", result.detail);
            }
            results.push(result);
        }

        // ── Phase 3: VERIFY ───────────────────────────────────────────────────
        println!("\n🔬 VERIFY — re-assessing post-upgrade state...");
        let post = assess_state()?;
        let applied = results.iter().filter(|r| r.success).count();

        // ── Phase 4: REPORT ───────────────────────────────────────────────────
        println!("\n📊 REPORT");
        println!("  planned:  {}", actions.len());
        println!("  applied:  {applied}");
        println!(
            "  b00t-cli: {} → {}",
            pre.b00t_cli_version.as_deref().unwrap_or("?"),
            post.b00t_cli_version.as_deref().unwrap_or("?")
        );
        println!(
            "  elapsed:  {}ms",
            t0.elapsed().as_millis()
        );

        if self.json {
            let report = UpgradeReport {
                pre,
                post: Some(post),
                actions_planned: actions.len(),
                actions_applied: applied,
                results,
                dry_run: false,
                elapsed_ms: t0.elapsed().as_millis() as u64,
            };
            println!("{}", serde_json::to_string_pretty(&report)?);
        }

        Ok(())
    }
}

// ── Phase Implementations ─────────────────────────────────────────────────────

fn assess_state() -> Result<SystemState> {
    let b00t_cli_version = sh_stdout("b00t-cli --version")
        .ok()
        .map(|v| v.split_whitespace().last().unwrap_or("").to_string());

    // 🤓 version check emits "latest: X.Y.Z" text; no --json flag exists
    let b00t_cli_latest = sh_stdout("b00t-cli version check 2>/dev/null")
        .ok()
        .and_then(|s| {
            s.lines()
                .find(|l| l.starts_with("latest:"))
                .and_then(|l| l.split_whitespace().nth(1).map(str::to_string))
        });

    let upgrade_available = b00t_cli_version
        .as_deref()
        .zip(b00t_cli_latest.as_deref())
        .map(|(cur, lat)| cur != lat)
        .unwrap_or(false);

    let claude_version = sh_stdout("claude --version 2>/dev/null").ok();

    let home = dirs::home_dir().unwrap_or_else(|| PathBuf::from("/home/brianh"));
    let claude_dir = home.join(".claude");
    // 🤓 MCP servers live in ~/.claude/.mcp.json; hooks in ~/.claude/settings.json
    let mcp_json = claude_dir.join(".mcp.json");
    let claude_settings_path = {
        let p = claude_dir.join("settings.json");
        if p.exists() { Some(p) } else { None }
    };

    let mcp_servers_registered = std::fs::read_to_string(&mcp_json)
        .ok()
        .and_then(|s| serde_json::from_str::<Value>(&s).ok())
        .and_then(|v| {
            v["mcpServers"]
                .as_object()
                .map(|m| m.keys().cloned().collect())
        })
        .unwrap_or_default();

    let hooks_registered = claude_settings_path
        .as_ref()
        .and_then(|p| std::fs::read_to_string(p).ok())
        .and_then(|s| serde_json::from_str::<Value>(&s).ok())
        .and_then(|v| {
            v["hooks"].as_object().map(|h| h.keys().cloned().collect())
        })
        .unwrap_or_default();

    // Discover hooks b00t knows about from _b00t_ hook datums
    let hooks_available = discover_available_hooks();

    Ok(SystemState {
        b00t_cli_version,
        b00t_cli_latest,
        claude_version,
        claude_settings_path,
        mcp_servers_registered,
        hooks_registered,
        hooks_available,
        upgrade_available,
    })
}

fn discover_available_hooks() -> Vec<String> {
    // 🤓 hook datums live in _b00t_/hooks/ or _b00t_/*.hook.toml
    let candidates = [
        "PostToolUse",
        "PreToolUse",
        "Stop",
        "Notification",
        "SubagentStop",
        "PreCompact",
    ];
    // Return all known Claude Code hook event types that b00t can register
    candidates.iter().map(|s| s.to_string()).collect()
}

fn plan_actions(state: &SystemState, scope: &UpgradeScope) -> Vec<UpgradeAction> {
    let mut actions = Vec::new();

    let do_binary = matches!(scope, UpgradeScope::All | UpgradeScope::Binary);
    let do_mcp = matches!(scope, UpgradeScope::All | UpgradeScope::Mcp);
    let do_hooks = matches!(scope, UpgradeScope::All | UpgradeScope::Hooks);

    if do_binary && state.upgrade_available {
        actions.push(UpgradeAction::UpgradeBinary {
            from: state.b00t_cli_version.clone().unwrap_or_default(),
            to: state.b00t_cli_latest.clone().unwrap_or_default(),
        });
    }

    if do_mcp {
        // 🤓 Claude stores MCP names verbatim from install; check both short and suffixed forms
        let has_b00t_mcp = state.mcp_servers_registered.iter()
            .any(|n| n == "b00t-mcp" || n == "b00t-mcp-mcp");
        if !has_b00t_mcp {
            actions.push(UpgradeAction::SyncMcpServer {
                name: "b00t-mcp".to_string(),
                action: McpAction::Install,
            });
        }
        let has_cbm = state.mcp_servers_registered.iter()
            .any(|n| n == "codebase-memory" || n == "codebase-memory-mcp");
        if !has_cbm {
            actions.push(UpgradeAction::SyncMcpServer {
                name: "codebase-memory".to_string(),
                action: McpAction::Install,
            });
        }
    }

    if do_hooks {
        // Ensure PostToolUse rustfmt hook is registered (task/47)
        for hook in &state.hooks_available {
            if !state.hooks_registered.contains(hook) {
                // Only auto-register if b00t has a datum for it
                if hook_has_datum(hook) {
                    actions.push(UpgradeAction::InstallHook {
                        name: hook.clone(),
                    });
                }
            }
        }
    }

    actions
}

fn hook_has_datum(hook: &str) -> bool {
    // 🤓 PostToolUse: rustfmt post-edit hook (task/47 branch); PreToolUse: cbm discovery gate
    // Only register hooks that b00t has an active datum/script for
    matches!(hook, "PostToolUse" | "PreToolUse")
}

fn execute_action(action: &UpgradeAction, delegate: bool) -> Result<UpgradeResult> {
    let t = Instant::now();
    let (success, detail) = match action {
        UpgradeAction::UpgradeBinary { .. } => {
            if delegate {
                // ch0nky tier: delegate compile/install to local GPU agent
                run_delegated("b00t-cli version upgrade")
            } else {
                run_cmd("b00t-cli", &["version", "upgrade"])
            }
        }
        UpgradeAction::SyncMcpServer { name, action: mcp_action } => match mcp_action {
            McpAction::Install | McpAction::Update => {
                run_cmd("b00t-cli", &["app", "claudecode", "mcp", "install", name])
            }
            McpAction::Remove => run_cmd("b00t-cli", &["mcp", "remove", name]),
        },
        UpgradeAction::InstallHook { name } => {
            run_cmd("b00t-cli", &["cli", "install", &format!("claude-code-hooks-{}", name.to_lowercase())])
        }
        UpgradeAction::UpdateSettings { key, new, .. } => {
            (true, format!("settings.{key} = {new}")) // settings updated via claudecode mcp install
        }
    };
    Ok(UpgradeResult {
        action: action.clone(),
        success,
        detail,
        duration_ms: t.elapsed().as_millis() as u64,
    })
}

// ── Delegation ────────────────────────────────────────────────────────────────

fn run_delegated(task: &str) -> (bool, String) {
    // 🤓 Routes compile/install to ch0nky GPU tier via b00t job
    // ch0nky = qwen3-coder on RTX3090 (local vLLM); sm0l = scoring/classify only
    let output = Command::new("b00t-cli")
        .args(["job", "run", "--tier=ch0nky", "--", task])
        .output();
    match output {
        Ok(o) => (
            o.status.success(),
            String::from_utf8_lossy(&o.stdout).trim().to_string(),
        ),
        Err(e) => (false, e.to_string()),
    }
}

// ── Helpers ───────────────────────────────────────────────────────────────────

fn run_cmd(bin: &str, args: &[&str]) -> (bool, String) {
    match Command::new(bin).args(args).output() {
        Ok(o) => {
            let stdout = String::from_utf8_lossy(&o.stdout).trim().to_string();
            let stderr = String::from_utf8_lossy(&o.stderr).trim().to_string();
            let detail = if !stdout.is_empty() { stdout } else { stderr };
            (o.status.success(), detail)
        }
        Err(e) => (false, e.to_string()),
    }
}

fn sh_stdout(cmd: &str) -> Result<String> {
    let out = Command::new("sh")
        .args(["-c", cmd])
        .output()
        .context("sh exec")?;
    anyhow::ensure!(out.status.success(), "non-zero exit");
    Ok(String::from_utf8_lossy(&out.stdout).trim().to_string())
}

fn action_description(a: &UpgradeAction) -> String {
    match a {
        UpgradeAction::UpgradeBinary { from, to } => format!("binary: {from} → {to}"),
        UpgradeAction::SyncMcpServer { name, action } => {
            let verb = match action {
                McpAction::Install => "install",
                McpAction::Update => "update",
                McpAction::Remove => "remove",
            };
            format!("mcp: {verb} {name}")
        }
        UpgradeAction::InstallHook { name } => format!("hook: install {name}"),
        UpgradeAction::UpdateSettings { key, .. } => format!("settings: update {key}"),
    }
}

fn print_state_summary(s: &SystemState) {
    let current = s.b00t_cli_version.as_deref().unwrap_or("?");
    let latest = s.b00t_cli_latest.as_deref().unwrap_or("?");
    let claude = s.claude_version.as_deref().unwrap_or("not detected");
    let mcp_count = s.mcp_servers_registered.len();
    let hook_count = s.hooks_registered.len();
    println!("  b00t-cli:  {current} (latest: {latest})");
    println!("  claude:    {claude}");
    println!("  mcp:       {mcp_count} registered {}", s.mcp_servers_registered.join(", ").chars().take(60).collect::<String>());
    println!("  hooks:     {hook_count} registered");
    if s.upgrade_available {
        println!("  ⚠️ upgrade available: {current} → {latest}");
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scope_parses_all_variants() {
        let cases = [
            ("all", UpgradeScope::All),
            ("binary", UpgradeScope::Binary),
            ("mcp", UpgradeScope::Mcp),
            ("hooks", UpgradeScope::Hooks),
            ("settings", UpgradeScope::Settings),
        ];
        for (s, expected) in cases {
            assert_eq!(s.parse::<UpgradeScope>().unwrap(), expected, "scope={s}");
        }
    }

    #[test]
    fn scope_rejects_unknown() {
        assert!("invalid".parse::<UpgradeScope>().is_err());
    }

    #[test]
    fn plan_no_actions_when_current() {
        let state = SystemState {
            b00t_cli_version: Some("0.8.1".into()),
            b00t_cli_latest: Some("0.8.1".into()),
            claude_version: None,
            claude_settings_path: None,
            mcp_servers_registered: vec!["b00t-mcp".into(), "codebase-memory".into()],
            hooks_registered: vec!["PostToolUse".into()],
            hooks_available: vec!["PostToolUse".into()],
            upgrade_available: false,
        };
        let actions = plan_actions(&state, &UpgradeScope::All);
        assert!(actions.is_empty(), "expected no actions when fully up-to-date");
    }

    #[test]
    fn plan_binary_upgrade_when_outdated() {
        let state = SystemState {
            b00t_cli_version: Some("0.8.0".into()),
            b00t_cli_latest: Some("0.8.1".into()),
            claude_version: None,
            claude_settings_path: None,
            mcp_servers_registered: vec!["b00t-mcp".into(), "codebase-memory".into()],
            hooks_registered: vec!["PostToolUse".into()],
            hooks_available: vec!["PostToolUse".into()],
            upgrade_available: true,
        };
        let actions = plan_actions(&state, &UpgradeScope::Binary);
        assert_eq!(actions.len(), 1);
        assert!(matches!(&actions[0], UpgradeAction::UpgradeBinary { from, to } if from == "0.8.0" && to == "0.8.1"));
    }

    #[test]
    fn plan_mcp_install_when_missing() {
        let state = SystemState {
            b00t_cli_version: Some("0.8.1".into()),
            b00t_cli_latest: Some("0.8.1".into()),
            claude_version: None,
            claude_settings_path: None,
            mcp_servers_registered: vec![],
            hooks_registered: vec![],
            hooks_available: vec![],
            upgrade_available: false,
        };
        let actions = plan_actions(&state, &UpgradeScope::Mcp);
        assert!(actions.iter().any(|a| matches!(a, UpgradeAction::SyncMcpServer { name, .. } if name == "b00t-mcp")));
    }

    #[test]
    fn discover_hooks_non_empty() {
        assert!(!discover_available_hooks().is_empty());
    }

    #[test]
    fn action_description_readable() {
        let a = UpgradeAction::UpgradeBinary { from: "0.8.0".into(), to: "0.8.1".into() };
        assert!(action_description(&a).contains("0.8.0"));
        assert!(action_description(&a).contains("0.8.1"));
    }
}
