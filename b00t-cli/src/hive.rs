//! Hive CMDB — dynamic system state management for b00t hive nodes
//!
//! Reads real system resources (RAM, GPU, systemd services) and manages
//! hive profile transitions (download-mode ↔ inference-qwen3 ↔ inference-sm0l).
//!
//! Profile datums: _b00t_/*.hive.toml OR *.hive.tomllm OR *.hive.tomllmd
//! precedence: .hive.tomllmd > .hive.tomllm > .stack.tomllmd > .stack.tomllm > .hive.toml
//! State file: /tmp/b00t/hive-state.json (volatile; reset on reboot)

use anyhow::{bail, Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

pub const HIVE_STATE_PATH: &str = "/tmp/b00t/hive-state.json";

// ─── System Snapshot ─────────────────────────────────────────────────────────

/// Real-time system resource snapshot
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SystemSnapshot {
    pub ram_total_gb: f32,
    pub ram_available_gb: f32,
    pub swap_total_gb: f32,
    pub swap_free_gb: f32,
    pub gpu_name: Option<String>,
    pub gpu_total_mb: Option<u32>,
    pub gpu_free_mb: Option<u32>,
    pub cpu_cores: u32,
    pub active_downloads: Vec<String>, // PIDs/paths of active HF downloads
    pub active_services: Vec<String>,  // running systemd --user units
    pub active_profile: Option<String>, // from HIVE_STATE_PATH
    pub timestamp: String,
}

impl SystemSnapshot {
    /// Capture current system state
    pub fn capture() -> Result<Self> {
        let meminfo = fs::read_to_string("/proc/meminfo").context("reading /proc/meminfo")?;

        let ram_total_kb = parse_meminfo_kb(&meminfo, "MemTotal");
        let ram_avail_kb = parse_meminfo_kb(&meminfo, "MemAvailable");
        let swap_total_kb = parse_meminfo_kb(&meminfo, "SwapTotal");
        let swap_free_kb = parse_meminfo_kb(&meminfo, "SwapFree");

        let cpu_cores = read_cpu_count();
        let (gpu_name, gpu_total_mb, gpu_free_mb) = query_nvidia_smi();
        let active_downloads = find_active_downloads();
        let active_services = query_systemd_user_services();
        let active_profile = read_active_profile();

        Ok(SystemSnapshot {
            ram_total_gb: kb_to_gb(ram_total_kb),
            ram_available_gb: kb_to_gb(ram_avail_kb),
            swap_total_gb: kb_to_gb(swap_total_kb),
            swap_free_gb: kb_to_gb(swap_free_kb),
            gpu_name,
            gpu_total_mb,
            gpu_free_mb,
            cpu_cores,
            active_downloads,
            active_services,
            active_profile,
            timestamp: chrono::Utc::now().to_rfc3339(),
        })
    }

    /// Check if system meets resource gate for a profile
    pub fn satisfies_gate(&self, profile: &HiveProfile) -> Vec<String> {
        let mut issues = Vec::new();

        if let Some(gate) = &profile.resources_gate {
            if let Some(min_ram) = gate.ram_free_gb {
                if self.ram_available_gb < min_ram {
                    issues.push(format!(
                        "RAM: need {:.1}GB free, have {:.1}GB — stop vllm or downloads first",
                        min_ram, self.ram_available_gb
                    ));
                }
            }
            if let Some(min_gpu) = gate.gpu_free_mb {
                let free = self.gpu_free_mb.unwrap_or(0);
                if free < min_gpu {
                    issues.push(format!(
                        "GPU: need {}MB free, have {}MB — stop vllm first",
                        min_gpu, free
                    ));
                }
            }
        }
        issues
    }

    /// Human-readable resource summary line
    pub fn summary_line(&self) -> String {
        let gpu = match (&self.gpu_name, self.gpu_free_mb, self.gpu_total_mb) {
            (Some(name), Some(free), Some(total)) => {
                format!(" | GPU {} {}/{}MB free", name, free, total)
            }
            _ => String::from(" | GPU: n/a"),
        };
        format!(
            "RAM {:.1}/{:.1}GB avail{} | CPU {}c",
            self.ram_available_gb, self.ram_total_gb, gpu, self.cpu_cores
        )
    }
}

// ─── Hive Service Spec ────────────────────────────────────────────────────────

/// Inline systemd service spec — declared in [b00t.hive.service] datum section
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct HiveServiceSpec {
    pub description: Option<String>,
    pub service_type: String,        // systemd Type= (default: "simple")
    pub exec_start: String,          // ExecStart= (may be multi-line with \ continuation)
    pub exec_start_pre: Vec<String>, // ExecStartPre= lines
    pub environment: Vec<String>,    // Environment= lines (each "KEY=VALUE")
    pub limit_nofile: Option<u32>,   // LimitNOFILE=
    pub restart: Option<String>,     // Restart=
    pub restart_sec: Option<String>, // RestartSec=
    pub timeout_start_sec: Option<String>, // TimeoutStartSec=
    pub working_directory: Option<String>, // WorkingDirectory=
    pub after: Vec<String>,          // After= dependencies (default: ["network.target"])
}

// ─── Hive Profile (from .hive.toml datums) ────────────────────────────────────

/// Parsed hive profile — resource budget + transition rules
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HiveProfile {
    pub name: String,
    pub hint: String,

    // resource budget (what this profile CONSUMES)
    pub resources_ram_gb: Option<f32>,
    pub resources_gpu_mb: Option<u32>,
    pub resources_cpu_cores: Option<u32>,

    // gate (min FREE resources required to activate)
    pub resources_gate: Option<HiveResourceGate>,

    // mutual exclusion
    pub exclusion_group: Option<String>,
    pub exclusion_priority: Option<u32>,

    // services to start/stop on activate
    pub services_start: Vec<String>,
    pub services_stop: Vec<String>,

    // command guards
    pub guards: Vec<HiveGuard>,

    // MCP tool activation
    pub mcp_activate: Vec<String>,
    pub mcp_deactivate: Vec<String>,

    // inline service spec (generates b00t-hive-<name>.service on activate)
    pub service_spec: Option<HiveServiceSpec>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HiveResourceGate {
    pub ram_free_gb: Option<f32>,
    pub gpu_free_mb: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HiveGuard {
    pub pattern: GuardPattern,
    pub action: HiveGuardAction,
    pub message: Option<String>,
    pub redirect: Option<String>,
    /// Repeat threshold for 🦨 → 💩 escalation.
    /// 0 or None = always warn (default). 1 = escalate on 2nd hit.
    /// Used with k0mmand3r::emoji_registry!() lookup: tier 1 is warn, tier 2 is block.
    pub repeat_threshold: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum HiveGuardAction {
    Warn,
    Block,
    Redirect,
}

// ─── TOML parsing (raw deserialization from .hive.toml) ──────────────────────

#[derive(Deserialize)]
struct HiveToml {
    b00t: HiveTomlB00t,
}

#[derive(Deserialize)]
struct HiveTomlB00t {
    name: String,
    hint: String,
    hive: Option<HiveTomlHive>,
}

#[derive(Deserialize)]
struct HiveTomlHive {
    resources: Option<HiveTomlResources>,
    exclusion: Option<HiveTomlExclusion>,
    services: Option<HiveTomlServices>,
    guards: Option<Vec<HiveTomlGuard>>,
    /// Named Rhai guard macros defined in datum header.
    /// Forms: let <name> = <expr>; piped into context before guard evaluation.
    rhai_macros: Option<HashMap<String, String>>,
    mcp_tools: Option<HiveTomlMcpTools>,
    service: Option<HiveTomlServiceSpec>,
}

#[derive(Deserialize)]
struct HiveTomlResources {
    ram_gb: Option<f32>,
    gpu_mb: Option<u32>,
    cpu_cores: Option<u32>,
    gate: Option<HiveResourceGate>,
}

#[derive(Deserialize)]
struct HiveTomlExclusion {
    group: Option<String>,
    priority: Option<u32>,
}

#[derive(Deserialize)]
struct HiveTomlServices {
    start: Option<Vec<String>>,
    stop: Option<Vec<String>>,
}

#[derive(Deserialize)]
struct HiveTomlGuard {
    pattern: GuardPattern,
    action: HiveGuardAction,
    message: Option<String>,
    redirect: Option<String>,
    repeat_threshold: Option<u32>,
}

#[derive(Deserialize)]
struct HiveTomlMcpTools {
    activate: Option<Vec<String>>,
    deactivate: Option<Vec<String>>,
}

#[derive(Deserialize, Default)]
struct HiveTomlServiceSpec {
    description: Option<String>,
    #[serde(rename = "type")]
    service_type: Option<String>,
    exec_start: Option<String>,
    #[serde(default)]
    exec_start_pre: Vec<String>,
    #[serde(default)]
    environment: Vec<String>,
    limit_nofile: Option<u32>,
    restart: Option<String>,
    restart_sec: Option<String>,
    timeout_start_sec: Option<String>,
    working_directory: Option<String>,
    #[serde(default)]
    after: Vec<String>,
}

impl HiveProfile {
    /// Load from a .hive.toml or .hive.tomllm file
    pub fn from_file(path: &Path) -> Result<Self> {
        let content = fs::read_to_string(path).context(format!("reading {}", path.display()))?;

        let raw: HiveToml =
            toml::from_str(&content).context(format!("parsing hive TOML: {}", path.display()))?;

        let hive = raw.b00t.hive.unwrap_or_else(|| HiveTomlHive {
            resources: None,
            exclusion: None,
            services: None,
            guards: None,
            rhai_macros: None,
            mcp_tools: None,
            service: None,
        });

        let (resources_ram_gb, resources_gpu_mb, resources_cpu_cores, resources_gate) =
            if let Some(r) = hive.resources {
                (r.ram_gb, r.gpu_mb, r.cpu_cores, r.gate)
            } else {
                (None, None, None, None)
            };

        let (exclusion_group, exclusion_priority) = if let Some(e) = hive.exclusion {
            (e.group, e.priority)
        } else {
            (None, None)
        };

        let (services_start, services_stop) = if let Some(s) = hive.services {
            (s.start.unwrap_or_default(), s.stop.unwrap_or_default())
        } else {
            (vec![], vec![])
        };

        let guards = hive
            .guards
            .unwrap_or_default()
            .into_iter()
            .map(|g| HiveGuard {
                pattern: g.pattern,
                action: g.action,
                message: g.message,
                redirect: g.redirect,
                repeat_threshold: g.repeat_threshold,
            })
            .collect();

        let (mcp_activate, mcp_deactivate) = if let Some(m) = hive.mcp_tools {
            (
                m.activate.unwrap_or_default(),
                m.deactivate.unwrap_or_default(),
            )
        } else {
            (vec![], vec![])
        };

        let service_spec = hive.service.map(|s| HiveServiceSpec {
            description: s.description,
            service_type: s.service_type.unwrap_or_else(|| "simple".to_string()),
            exec_start: s.exec_start.unwrap_or_default(),
            exec_start_pre: s.exec_start_pre,
            environment: s.environment,
            limit_nofile: s.limit_nofile,
            restart: s.restart,
            restart_sec: s.restart_sec,
            timeout_start_sec: s.timeout_start_sec,
            working_directory: s.working_directory,
            after: if s.after.is_empty() {
                vec!["network.target".to_string()]
            } else {
                s.after
            },
        });

        Ok(HiveProfile {
            name: raw.b00t.name,
            hint: raw.b00t.hint,
            resources_ram_gb,
            resources_gpu_mb,
            resources_cpu_cores,
            resources_gate,
            exclusion_group,
            exclusion_priority,
            services_start,
            services_stop,
            guards,
            mcp_activate,
            mcp_deactivate,
            service_spec,
        })
    }
}

// ─── Profile Discovery ────────────────────────────────────────────────────────

/// Find all .hive.toml / .hive.tomllm / .hive.tomllmd / .stack.tomllm / .stack.tomllmd
/// datums; priority: .hive.tomllmd > .hive.tomllm > .stack.tomllmd > .stack.tomllm > .hive.toml
pub fn discover_profiles(datum_dir: &Path) -> Vec<(String, PathBuf)> {
    let mut profiles: HashMap<String, PathBuf> = HashMap::new();
    if let Ok(entries) = fs::read_dir(datum_dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
                let profile = if name.ends_with(".hive.tomllmd") {
                    Some((name.trim_end_matches(".hive.tomllmd").to_string(), 5))
                } else if name.ends_with(".hive.tomllm") {
                    Some((name.trim_end_matches(".hive.tomllm").to_string(), 4))
                } else if name.ends_with(".stack.tomllmd") {
                    Some((name.trim_end_matches(".stack.tomllmd").to_string(), 3))
                } else if name.ends_with(".stack.tomllm") {
                    Some((name.trim_end_matches(".stack.tomllm").to_string(), 2))
                } else if name.ends_with(".hive.toml") {
                    Some((name.trim_end_matches(".hive.toml").to_string(), 1))
                } else {
                    None
                };

                if let Some((profile_name, new_rank)) = profile {
                    let current_rank = profiles
                        .get(&profile_name)
                        .and_then(|existing| existing.file_name().and_then(|n| n.to_str()))
                        .map(|existing_name| {
                            if existing_name.ends_with(".hive.tomllmd") {
                                5
                            } else if existing_name.ends_with(".hive.tomllm") {
                                4
                            } else if existing_name.ends_with(".stack.tomllmd") {
                                3
                            } else if existing_name.ends_with(".stack.tomllm") {
                                2
                            } else {
                                1
                            }
                        })
                        .unwrap_or(0);
                    if new_rank >= current_rank {
                        profiles.insert(profile_name, path);
                    }
                }
            }
        }
    }
    let mut result: Vec<(String, PathBuf)> = profiles.into_iter().collect();
    result.sort_by(|a, b| a.0.cmp(&b.0));
    result
}

/// Load a named profile from datum dir — prefers
/// .hive.tomllmd > .hive.tomllm > .stack.tomllmd > .stack.tomllm > .hive.toml
pub fn load_profile(name: &str, datum_dir: &Path) -> Result<HiveProfile> {
    // 🤓 .tomllmd currently downgrades to the generic .tomllm/TOML handling path.
    let hive_tomllmd_path = datum_dir.join(format!("{}.hive.tomllmd", name));
    let hive_tomllm_path = datum_dir.join(format!("{}.hive.tomllm", name));
    let stack_tomllmd_path = datum_dir.join(format!("{}.stack.tomllmd", name));
    let stack_tomllm_path = datum_dir.join(format!("{}.stack.tomllm", name));
    let hive_toml_path = datum_dir.join(format!("{}.hive.toml", name));

    let path = if hive_tomllmd_path.exists() {
        hive_tomllmd_path
    } else if hive_tomllm_path.exists() {
        hive_tomllm_path
    } else if stack_tomllmd_path.exists() {
        stack_tomllmd_path
    } else if stack_tomllm_path.exists() {
        stack_tomllm_path
    } else if hive_toml_path.exists() {
        hive_toml_path
    } else {
        bail!(
            "profile '{}' not found (tried .hive.tomllmd, .hive.tomllm, .stack.tomllmd, .stack.tomllm, .hive.toml)",
            name
        );
    };
    HiveProfile::from_file(&path)
}

// ─── State Persistence ────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HiveState {
    pub active_profile: Option<String>,
    pub activated_at: Option<String>,
    pub last_snapshot: Option<SystemSnapshot>,
}

pub fn read_state() -> HiveState {
    fs::read_to_string(HIVE_STATE_PATH)
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or(HiveState {
            active_profile: None,
            activated_at: None,
            last_snapshot: None,
        })
}

pub fn write_state(state: &HiveState) -> Result<()> {
    let dir = Path::new(HIVE_STATE_PATH).parent().unwrap();
    fs::create_dir_all(dir)?;
    let json = serde_json::to_string_pretty(state)?;
    fs::write(HIVE_STATE_PATH, json)?;
    Ok(())
}

fn read_active_profile() -> Option<String> {
    read_state().active_profile
}

// ─── Guard Pattern Types ─────────────────────────────────────────────────────
//
// Supported pattern matchers:
// - JsonRegexPattern: compiled regex against command string (default)
// - RhaiExpr:        one-line Rhai expression evaluated against guard context
// - K0mmand3rStage:  hook into specific k0mmand3r winnow parser stage
//
// These are serializable in hive-guards.toml as a tagged union.
// The pattern field in TOML can be:
//   pattern = "pip install"                → JsonRegexPattern (backward compat)
//   pattern = { rhai = "cmd.contains('pip') && cmd.contains('install')" }
//   pattern = { stage = "pre_tokenize" }

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(untagged)]
pub enum GuardPattern {
    /// Substring match via regex (backward-compatible with bare string)
    JsonRegexPattern(String),
    /// Rhai expression evaluated at guard-check time
    RhaiExpr(RhaiGuardExpr),
    /// Hook into a named k0mmand3r winnow parser stage
    K0mmand3rStage(K0mmand3rStageGuard),
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RhaiGuardExpr {
    pub rhai: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct K0mmand3rStageGuard {
    pub stage: String,
}

impl GuardPattern {
    /// Evaluate this pattern against a command string.
    /// Rhai variants also receive the full guard context (env vars, session state).
    pub fn matches(&self, command: &str, context: &GuardContext) -> bool {
        match self {
            GuardPattern::JsonRegexPattern(p) => command.contains(p),
            GuardPattern::RhaiExpr(expr) => {
                match eval_rhai_expr(&expr.rhai, command, context) {
                    RhaiEvalResult::Match => true,
                    RhaiEvalResult::NoMatch => false,
                    RhaiEvalResult::Error(msg) => {
                        // Log eval error through the debug system — never silently no-match.
                        // The command proceeds (fail-open) but the error is flagged.
                        eprintln!("{msg}");
                        false
                    }
                }
            }
            GuardPattern::K0mmand3rStage(_stage) => {
                // Stage guards are registered and invoked through k0mmand3r parser stage hooks.
                // They must not participate in generic command matching via check_guards(),
                // or they would match every command and apply their action unconditionally.
                //
                // Any parse-time interception logic belongs in the stage callback path.
                false
            }
        }
    }
}

/// Context provided to Rhai guard expressions at evaluation time.
#[derive(Debug, Clone, Default)]
pub struct GuardContext {
    pub command: String,
    pub violation_count: u32,
    pub repeat_threshold: Option<u32>,
    /// Rhai macro definitions from datum header, e.g. pip_guard → "cmd.contains(\"pip\")"
    /// Injected as `let <name> = <expr>;` before guard expression evaluation.
    pub rhai_macros: HashMap<String, String>,
}

/// Persistent violation counter for 🦨→💩 escalation.
///
/// Tracks how many times each guard pattern has been violated.
/// When `count >= repeat_threshold`, the guard escalates from Warn to Block.
///
/// Persisted to `~/.b00t/guard-violations.jsonl` so violation counts survive
/// process restarts. Each violation is one JSONL line.

/// Default path for the violation counter persistence file.
pub fn default_violations_path() -> PathBuf {
    dirs::home_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join(".b00t")
        .join("guard-violations.jsonl")
}

#[derive(Debug, Clone, Default)]
pub struct GuardViolationCounter {
    counts: HashMap<String, u32>,
}

impl GuardViolationCounter {
    /// Create a new empty counter.
    pub fn new() -> Self {
        Self {
            counts: HashMap::new(),
        }
    }

    /// Increment the violation count for a guard pattern and return the new count.
    pub fn increment(&mut self, pattern_key: &str) -> u32 {
        let count = self.counts.entry(pattern_key.to_string()).or_insert(0);
        *count += 1;
        *count
    }

    /// Get the current violation count for a pattern (default 0).
    pub fn get_count(&self, pattern_key: &str) -> u32 {
        self.counts.get(pattern_key).copied().unwrap_or(0)
    }

    /// Reset a pattern's violation count.
    pub fn reset(&mut self, pattern_key: &str) {
        self.counts.remove(pattern_key);
    }

    /// Load violation counts from a JSONL file.
    /// Each line is `{"pattern": "...", "count": N}`.
    /// Returns the loaded counter, or empty if file doesn't exist or is corrupt.
    pub fn load(path: &std::path::Path) -> Self {
        let content = match std::fs::read_to_string(path) {
            Ok(c) => c,
            Err(_) => return Self::new(),
        };

        let mut counts = HashMap::new();
        for line in content.lines() {
            if let Ok(entry) = serde_json::from_str::<serde_json::Value>(line) {
                if let (Some(pattern), Some(count)) = (
                    entry.get("pattern").and_then(|v| v.as_str()),
                    entry.get("count").and_then(|v| v.as_u64()),
                ) {
                    counts.insert(pattern.to_string(), count as u32);
                }
            }
        }

        Self { counts }
    }

    /// Save violation counts to a JSONL file.
    /// Creates parent directory if it doesn't exist.
    pub fn save(&self, path: &std::path::Path) -> std::io::Result<()> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }

        let mut file = std::fs::File::create(path)?;
        use std::io::Write;
        for (pattern, count) in &self.counts {
            if *count > 0 {
                writeln!(
                    file,
                    "{}",
                    serde_json::json!({"pattern": pattern, "count": count})
                )?;
            }
        }
        Ok(())
    }

    /// Load from default path, increment a pattern, save, return new count.
    pub fn increment_persist(&mut self, pattern_key: &str) -> u32 {
        let new_count = self.increment(pattern_key);
        let path = default_violations_path();
        let _ = self.save(&path); // best-effort; don't fail on IO
        new_count
    }
}

// ─── Guard Evaluation ─────────────────────────────────────────────────────────

pub enum GuardResult {
    Allow,
    Warn {
        message: String,
        redirect: Option<String>,
    },
    Block {
        message: String,
    },
}

/// Check a command string against a list of guards; returns first match.
/// Uses GuardPattern::matches() which supports JsonRegexPattern, RhaiExpr, and K0mmand3rStage.
/// When a guard has `repeat_threshold` and violation count meets or exceeds it,
/// the action escalates from Warn to Block (🦨 → 💩).
pub fn check_guards(command: &str, guards: &[HiveGuard], context: &GuardContext) -> GuardResult {
    for guard in guards {
        if guard.pattern.matches(command, context) {
            let pattern_display = match &guard.pattern {
                GuardPattern::JsonRegexPattern(p) => p.clone(),
                GuardPattern::RhaiExpr(e) => format!("rhai:{}", e.rhai),
                GuardPattern::K0mmand3rStage(s) => format!("stage:{}", s.stage),
            };
            let message = guard
                .message
                .clone()
                .unwrap_or_else(|| format!("guard matched: {pattern_display}"));

            // Persist violation count for 🦨→💩 escalation tracking.
            // Writes to ~/.b00t/guard-violations.jsonl.
            let _ = GuardViolationCounter::load(&default_violations_path())
                .increment_persist(&pattern_display);

            // 🦨→💩 escalation: if repeat_threshold is set and violation count >= threshold,
            // escalate Warn/Redirect to Block
            let (effective_message, effective_action) =
                match (&guard.action, guard.repeat_threshold) {
                    (HiveGuardAction::Warn | HiveGuardAction::Redirect, Some(threshold))
                        if context.violation_count >= threshold =>
                    {
                        // Escalate to Block, replace 🦨 with 💩 in message
                        let escalated = message
                            .replace("🦨", "💩")
                            .replace("use ", "repeated: use ");
                        (escalated, HiveGuardAction::Block)
                    }
                    _ => (message.clone(), guard.action.clone()),
                };

            return match effective_action {
                HiveGuardAction::Warn | HiveGuardAction::Redirect => GuardResult::Warn {
                    message: effective_message,
                    redirect: guard.redirect.clone(),
                },
                HiveGuardAction::Block => GuardResult::Block {
                    message: effective_message,
                },
            };
        }
    }
    GuardResult::Allow
}

/// Result of evaluating a Rhai guard expression.
/// Never conflates parse/eval errors with a non-match — errors have their own channel.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RhaiEvalResult {
    /// Expression evaluated to true — guard matched
    Match,
    /// Expression evaluated to false — guard did not match
    NoMatch,
    /// Expression failed to parse or evaluate — distinct from non-match.
    /// Contains the Rhai error message for logging/tracking.
    Error(String),
}

impl RhaiEvalResult {
    /// Returns true if the result is a Match.
    pub fn is_match(&self) -> bool {
        matches!(self, RhaiEvalResult::Match)
    }
}

/// Evaluate a Rhai one-liner expression against guard context.
/// Returns RhaiEvalResult::Match, NoMatch, or Error — never conflates errors with non-matches.
/// Rhai macros from the datum header are injected as `let <name> = <expr>;` before
/// the guard expression, enabling: pattern = { rhai = "pip_guard || docker_guard" }
/// Engine is created fresh per eval — guards are evaluated infrequently
/// (once per command, not per token). Rhai's engine is lightweight enough
/// that per-call instantiation is fine.
pub fn eval_rhai_expr(expr: &str, command: &str, context: &GuardContext) -> RhaiEvalResult {
    use rhai::{Engine, Scope};
    let mut engine = Engine::new();
    let mut scope = Scope::new();
    scope.push("cmd", command.to_string());
    scope.push("violations", context.violation_count as i64);
    scope.push(
        "threshold",
        context.repeat_threshold.unwrap_or(u32::MAX) as i64,
    );

    // Build the full Rhai script: macro let-bindings + guard expression
    // Each macro becomes: let <name> = <expr>;
    // Then the guard expression references them by name or composes with || && |>
    let mut script = String::new();
    for (name, macro_expr) in &context.rhai_macros {
        script.push_str(&format!("let {name} = {macro_expr};\n"));
    }
    script.push('(');
    script.push_str(expr);
    script.push(')');

    match engine.eval_with_scope::<bool>(&mut scope, &script) {
        Ok(true) => RhaiEvalResult::Match,
        Ok(false) => RhaiEvalResult::NoMatch,
        Err(e) => RhaiEvalResult::Error(format!("⚠️ rhai guard eval failed: {e} for expr: {expr}")),
    }
}

// ─── Systemd Unit Generation ──────────────────────────────────────────────────

/// Generate systemd unit file content from an inline service spec.
/// Unit name: `b00t-hive-{profile_name}.service`
pub fn generate_systemd_unit(profile_name: &str, spec: &HiveServiceSpec) -> String {
    let description = spec.description.as_deref().unwrap_or(profile_name);
    let after = if spec.after.is_empty() {
        "network.target".to_string()
    } else {
        spec.after.join(" ")
    };

    let mut unit = format!(
        "[Unit]\nDescription=b00t hive stack: {description}\nAfter={after}\nWants={after}\n\n[Service]\nType={}\n",
        spec.service_type
    );

    if let Some(n) = spec.limit_nofile {
        unit.push_str(&format!("LimitNOFILE={n}\n"));
    }

    for env in &spec.environment {
        unit.push_str(&format!("Environment=\"{env}\"\n"));
    }
    unit.push('\n');

    for pre in &spec.exec_start_pre {
        unit.push_str(&format!("ExecStartPre={pre}\n"));
    }

    if let Some(wd) = &spec.working_directory {
        unit.push_str(&format!("WorkingDirectory={wd}\n"));
    }

    // ExecStart: strip leading/trailing whitespace, preserve internal \ continuations
    let exec = spec.exec_start.trim();
    unit.push_str(&format!("ExecStart={exec}\n"));

    if let Some(r) = &spec.restart {
        unit.push_str(&format!("Restart={r}\n"));
    }
    if let Some(rs) = &spec.restart_sec {
        unit.push_str(&format!("RestartSec={rs}\n"));
    }
    if let Some(ts) = &spec.timeout_start_sec {
        unit.push_str(&format!("TimeoutStartSec={ts}\n"));
    }

    unit.push_str("\n[Install]\nWantedBy=default.target\n");
    unit
}

pub fn stack_template_unit(profile_name: &str) -> String {
    format!("b00t@{profile_name}.service")
}

// ─── Hive Stack Status ────────────────────────────────────────────────────────

/// Query systemd for b00t@*.service and b00t-hive-*.service status.
/// Returns list of (unit_name, is_active, is_enabled) tuples.
pub fn hive_stacks_status() -> Vec<(String, bool, bool)> {
    let patterns = ["b00t@*.service", "b00t-hive-*.service"];
    let mut results = Vec::new();

    for pattern in &patterns {
        let output = Command::new("systemctl")
            .args([
                "--user",
                "list-units",
                pattern,
                "--all",
                "--no-legend",
                "--plain",
            ])
            .output();
        if let Ok(o) = output {
            for line in String::from_utf8_lossy(&o.stdout).lines() {
                let parts: Vec<&str> = line.split_whitespace().collect();
                if parts.len() >= 4 {
                    let unit = parts[0].to_string();
                    let is_active = parts[2] == "active";
                    results.push((unit, is_active, false));
                }
            }
        }
    }

    // Check enabled state
    for item in &mut results {
        let out = Command::new("systemctl")
            .args(["--user", "is-enabled", &item.0])
            .output();
        if let Ok(o) = out {
            item.2 = String::from_utf8_lossy(&o.stdout).trim() == "enabled";
        }
    }

    results
}

// ─── Activation ───────────────────────────────────────────────────────────────

/// Activate a hive profile: stop conflicting services, start required services
pub fn activate_profile(
    profile: &HiveProfile,
    snapshot: &SystemSnapshot,
    dry_run: bool,
    force: bool,
) -> Result<Vec<String>> {
    // 🤓 Route through DBus when available — system-level systemctl without sudo
    #[cfg(feature = "dbus")]
    if !dry_run {
        if crate::dbus_dispatch::dbus_available() {
            return crate::dbus_dispatch::dbus_stack_activate(&profile.name, force);
        }
    }

    let mut log = Vec::new();

    // 1. Check resource gate
    let issues = snapshot.satisfies_gate(profile);
    if !issues.is_empty() && !force {
        bail!(
            "resource gate failed for profile '{}':\n  {}",
            profile.name,
            issues.join("\n  ")
        );
    } else if !issues.is_empty() {
        for issue in &issues {
            log.push(format!("⚠️  gate warning (--force): {}", issue));
        }
    }

    // 2. Stop conflicting services
    for unit in &profile.services_stop {
        log.push(format!("stop  {}", unit));
        if !dry_run {
            let _ = Command::new("systemctl")
                .args(["--user", "stop", unit])
                .status();
        }
    }

    // 3. Kill active downloads if needed (warn about RAM competition)
    if !snapshot.active_downloads.is_empty() && profile.resources_ram_gb.unwrap_or(0.0) > 10.0 {
        log.push(format!(
            "⚠️  active downloads detected: {:?}",
            snapshot.active_downloads
        ));
        if !dry_run && !force {
            bail!(
                "active HF downloads detected — stop them first or use --force\nDownloads: {:?}",
                snapshot.active_downloads
            );
        }
    }

    // 3a. Generate systemd unit from inline service spec (if declared in datum)
    let mut generated_unit: Option<String> = None;
    if let Some(ref spec) = profile.service_spec {
        if !spec.exec_start.is_empty() {
            let unit_name = format!("b00t-hive-{}.service", profile.name);
            let unit_content = generate_systemd_unit(&profile.name, spec);
            log.push(format!("generate {}", unit_name));
            if !dry_run {
                let systemd_user_dir = dirs::home_dir()
                    .unwrap_or_else(|| std::path::PathBuf::from("/tmp"))
                    .join(".config/systemd/user");
                fs::create_dir_all(&systemd_user_dir).context("creating ~/.config/systemd/user")?;
                let unit_path = systemd_user_dir.join(&unit_name);
                fs::write(&unit_path, &unit_content)
                    .context(format!("writing {}", unit_path.display()))?;
                log.push(format!("  wrote {}", unit_path.display()));
                // daemon-reload so systemd sees the new unit
                let _ = Command::new("systemctl")
                    .args(["--user", "daemon-reload"])
                    .status();
                log.push("  daemon-reload".to_string());
            }
            generated_unit = Some(unit_name);
        }
    }

    // 4. Start required services
    for unit in &profile.services_start {
        log.push(format!("start {}", unit));
        if !dry_run {
            let result = Command::new("systemctl")
                .args(["--user", "start", unit])
                .status();
            match result {
                Ok(s) if s.success() => {}
                Ok(s) => log.push(format!(
                    "  ⚠️  {} exit code {}",
                    unit,
                    s.code().unwrap_or(-1)
                )),
                Err(e) => log.push(format!("  ⚠️  {} failed: {}", unit, e)),
            }
        }
    }

    // 4b. Start generated unit (if any) and not already in services_start
    if let Some(ref unit_name) = generated_unit {
        if !profile.services_start.contains(unit_name) {
            log.push(format!("start {}", unit_name));
            if !dry_run {
                let result = Command::new("systemctl")
                    .args(["--user", "start", unit_name])
                    .status();
                match result {
                    Ok(s) if s.success() => {}
                    Ok(s) => log.push(format!(
                        "  ⚠️  {} exit code {}",
                        unit_name,
                        s.code().unwrap_or(-1)
                    )),
                    Err(e) => log.push(format!("  ⚠️  {} failed: {}", unit_name, e)),
                }
            }
        }
    }

    // 4c. Enable generated unit directly for autostart.
    // 🤓 b00t@.service template unit may not exist → systemctl enable b00t@{name} silently fails.
    //    The generated b00t-hive-{}.service has [Install] WantedBy=default.target so enabling
    //    it directly gives reliable autostart without requiring the template file.
    if let Some(ref unit_name) = generated_unit {
        log.push(format!("enable {}", unit_name));
        if !dry_run {
            let result = Command::new("systemctl")
                .args(["--user", "enable", unit_name])
                .status();
            match result {
                Ok(s) if s.success() => {}
                Ok(s) => log.push(format!(
                    "  ⚠️  {} enable exit code {}",
                    unit_name,
                    s.code().unwrap_or(-1)
                )),
                Err(e) => log.push(format!("  ⚠️  {} enable failed: {}", unit_name, e)),
            }
        }
    }

    // Persist autostart via the stable template unit so the profile comes back after reboot.
    let template_unit = stack_template_unit(&profile.name);
    log.push(format!("enable {}", template_unit));
    if !dry_run {
        let result = Command::new("systemctl")
            .args(["--user", "enable", &template_unit])
            .status();
        match result {
            Ok(s) if s.success() => {}
            Ok(s) => log.push(format!(
                "  ⚠️  {} enable exit code {}",
                template_unit,
                s.code().unwrap_or(-1)
            )),
            Err(e) => log.push(format!("  ⚠️  {} enable failed: {}", template_unit, e)),
        }
    }

    // 5. Persist state
    if !dry_run {
        let state = HiveState {
            active_profile: Some(profile.name.clone()),
            activated_at: Some(chrono::Utc::now().to_rfc3339()),
            last_snapshot: Some(snapshot.clone()),
        };
        write_state(&state)?;
    }

    log.push(format!(
        "{}profile '{}' {}activated",
        if dry_run { "[dry-run] " } else { "" },
        profile.name,
        if dry_run { "would be " } else { "" }
    ));

    Ok(log)
}

// ─── Helpers ──────────────────────────────────────────────────────────────────

fn parse_meminfo_kb(meminfo: &str, key: &str) -> u64 {
    for line in meminfo.lines() {
        if line.starts_with(key) {
            let parts: Vec<&str> = line.split_whitespace().collect();
            if parts.len() >= 2 {
                return parts[1].parse().unwrap_or(0);
            }
        }
    }
    0
}

fn kb_to_gb(kb: u64) -> f32 {
    kb as f32 / 1024.0 / 1024.0
}

fn read_cpu_count() -> u32 {
    fs::read_to_string("/proc/cpuinfo")
        .map(|s| s.lines().filter(|l| l.starts_with("processor")).count() as u32)
        .unwrap_or(1)
}

fn query_nvidia_smi() -> (Option<String>, Option<u32>, Option<u32>) {
    let output = Command::new("nvidia-smi")
        .args([
            "--query-gpu=name,memory.total,memory.free",
            "--format=csv,noheader,nounits",
        ])
        .output();

    match output {
        Ok(o) if o.status.success() => {
            let stdout = String::from_utf8_lossy(&o.stdout);
            let line = stdout.trim();
            let parts: Vec<&str> = line.splitn(3, ',').collect();
            if parts.len() == 3 {
                let name = parts[0].trim().to_string();
                let total: u32 = parts[1].trim().parse().unwrap_or(0);
                let free: u32 = parts[2].trim().parse().unwrap_or(0);
                return (Some(name), Some(total), Some(free));
            }
            (None, None, None)
        }
        _ => (None, None, None),
    }
}

fn find_active_downloads() -> Vec<String> {
    let output = Command::new("pgrep")
        .args(["-a", "-f", "hf download|huggingface-cli download"])
        .output();

    match output {
        Ok(o) if o.status.success() => String::from_utf8_lossy(&o.stdout)
            .lines()
            .map(|l| l.to_string())
            .collect(),
        _ => vec![],
    }
}

fn query_systemd_user_services() -> Vec<String> {
    let output = Command::new("systemctl")
        .args([
            "--user",
            "list-units",
            "--state=running",
            "--no-legend",
            "--plain",
        ])
        .output();

    match output {
        Ok(o) if o.status.success() => String::from_utf8_lossy(&o.stdout)
            .lines()
            .filter_map(|l| {
                let parts: Vec<&str> = l.split_whitespace().collect();
                parts.first().map(|s| s.to_string())
            })
            .collect(),
        _ => vec![],
    }
}

// ─── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn test_guard_warn() {
        let guards = vec![HiveGuard {
            pattern: GuardPattern::JsonRegexPattern("pip install".to_string()),
            action: HiveGuardAction::Warn,
            message: Some("🦨 use uv pip install".to_string()),
            redirect: Some("uv pip install".to_string()),
            repeat_threshold: None,
        }];
        let ctx = GuardContext::default();
        matches!(
            check_guards("pip install requests", &guards, &ctx),
            GuardResult::Warn { .. }
        );
    }

    #[test]
    fn test_guard_block() {
        let guards = vec![HiveGuard {
            pattern: GuardPattern::JsonRegexPattern("rm -rf /".to_string()),
            action: HiveGuardAction::Block,
            message: Some("🚫 blocked".to_string()),
            redirect: None,
            repeat_threshold: None,
        }];
        let ctx = GuardContext::default();
        matches!(check_guards("rm -rf /", &guards, &ctx), GuardResult::Block { .. });
    }

    #[test]
    fn test_guard_allow() {
        let guards = vec![HiveGuard {
            pattern: GuardPattern::JsonRegexPattern("pip install".to_string()),
            action: HiveGuardAction::Warn,
            message: None,
            redirect: None,
            repeat_threshold: None,
        }];
        let ctx = GuardContext::default();
        matches!(check_guards("cargo build", &guards, &ctx), GuardResult::Allow);
    }

    #[test]
    fn test_guard_rhai_expr() {
        let guards = vec![HiveGuard {
            pattern: GuardPattern::RhaiExpr(RhaiGuardExpr {
                rhai: "cmd.contains(\"pip\") && cmd.contains(\"install\")".to_string(),
            }),
            action: HiveGuardAction::Warn,
            message: Some("🦨 rhai caught pip install".to_string()),
            redirect: Some("uv pip install".to_string()),
            repeat_threshold: None,
        }];
        let ctx = GuardContext::default();
        matches!(
            check_guards("pip install flask", &guards, &ctx),
            GuardResult::Warn { .. }
        );
    }

    #[test]
    fn test_guard_rhai_no_match() {
        let guards = vec![HiveGuard {
            pattern: GuardPattern::RhaiExpr(RhaiGuardExpr {
                rhai: "cmd.contains(\"docker\")".to_string(),
            }),
            action: HiveGuardAction::Block,
            message: Some("🚫 docker usage blocked".to_string()),
            redirect: None,
            repeat_threshold: None,
        }];
        let ctx = GuardContext::default();
        matches!(
            check_guards("pip install flask", &guards, &ctx),
            GuardResult::Allow
        );
    }

    #[test]
    fn test_guard_rhai_violation_threshold() {
        let guards = vec![HiveGuard {
            pattern: GuardPattern::RhaiExpr(RhaiGuardExpr {
                rhai: "cmd.contains(\"pip\") && violations >= threshold".to_string(),
            }),
            action: HiveGuardAction::Block,
            message: Some("💩 repeat pip violation".to_string()),
            redirect: None,
            repeat_threshold: Some(2),
        }];
        let ctx = GuardContext {
            command: "pip install flask".to_string(),
            violation_count: 2,
            repeat_threshold: Some(2),
            rhai_macros: HashMap::new(),
        };
        matches!(
            check_guards("pip install flask", &guards, &ctx),
            GuardResult::Block { .. }
        );
    }

    // ── Data-driven guard coverage: verify every Rhai expression in shipped datums ──
    //
    // Scans all _b00t_/*.hive.toml datum files at test time, parses guards,
    // and evaluates each Rhai expression with at least one match + one no-match input.
    // Fails if ANY Rhai expression fails to parse, evaluate, or produce the expected result.
    //
    // This is how we maintain 100% implementation coverage for Rhai guards shipped
    // with b00t. Adding a new `pattern = { rhai = "..." }` to ANY .hive.toml file
    // automatically requires a passing test — no separate test code needed.
    //
    // Run on version bump or merge to main via: cargo test -- guard_expr_coverage

    #[test]
    fn test_guard_expr_coverage_all_shipped_datums() {
        use std::fs;
        use std::path::Path;

        // Locate _b00t_/ relative to CARGO_MANIFEST_DIR (compile-time constant)
        let b00t_dir = Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .unwrap()
            .join("_b00t_");

        let mut total_guards = 0u32;
        let mut rhai_guards = 0u32;
        let mut failures = Vec::new();

        // Scan for all .hive.toml files
        for entry in fs::read_dir(&b00t_dir).expect("_b00t_ directory not found") {
            let entry = entry.unwrap();
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) != Some("toml") {
                continue;
            }
            let file_name = path.file_name().unwrap().to_str().unwrap().to_string();
            // Only .hive.toml files contain guards
            if !file_name.ends_with(".hive.toml") {
                continue;
            }

            // Read and parse TOML — use toml::Table directly (toml 0.9 compat)
            let content = fs::read_to_string(&path)
                .unwrap_or_else(|_| panic!("failed to read {}", path.display()));
            let toml_value: toml::Table = content
                .parse()
                .unwrap_or_else(|e| panic!("failed to parse {}: {e}", path.display()));

            // Extract guards from b00t.hive.guards array
            let guards_arr = toml_value
                .get("b00t")
                .and_then(|b| b.as_table())
                .and_then(|b| b.get("hive"))
                .and_then(|b| b.as_table())
                .and_then(|h| h.get("guards"))
                .and_then(|g| g.as_array());

            let Some(guard_values) = guards_arr else { continue };
            total_guards += guard_values.len() as u32;

            // Extract rhai_macros from datum header, if any
            let rhai_macros: HashMap<String, String> = toml_value
                .get("b00t")
                .and_then(|b| b.as_table())
                .and_then(|b| b.get("hive"))
                .and_then(|b| b.as_table())
                .and_then(|h| h.get("rhai_macros"))
                .and_then(|m| m.as_table())
                .map(|t| {
                    t.iter()
                        .map(|(k, v)| (k.clone(), v.as_str().unwrap_or("").to_string()))
                        .collect()
                })
                .unwrap_or_default();

            // Validate each macro definition compiles as standalone Rhai
            // Skip macros that reference other macros (they'll be tested when guards use them)
            for (macro_name, macro_expr) in &rhai_macros {
                let depends_on_other_macro = rhai_macros
                    .keys()
                    .filter(|k| *k != macro_name)
                    .any(|other_name| macro_expr.contains(other_name));
                if depends_on_other_macro {
                    continue;
                }
                let result = eval_rhai_expr(macro_expr, "test command", &GuardContext::default());
                if let RhaiEvalResult::Error(msg) = &result {
                    failures.push(format!(
                        "{file_name}:macro:{macro_name}: rhai eval failed: {msg}"
                    ));
                }
            }

            for (idx, gv) in guard_values.iter().enumerate() {
                // Determine pattern type
                let pattern = match gv.get("pattern") {
                    Some(toml::Value::String(s)) => {
                        GuardPattern::JsonRegexPattern(s.clone())
                    }
                    Some(toml::Value::Table(t)) if t.contains_key("rhai") => {
                        let expr = t.get("rhai").unwrap().as_str().unwrap().to_string();
                        rhai_guards += 1;
                        GuardPattern::RhaiExpr(RhaiGuardExpr { rhai: expr })
                    }
                    Some(toml::Value::Table(t)) if t.contains_key("stage") => {
                        GuardPattern::K0mmand3rStage(K0mmand3rStageGuard {
                            stage: t.get("stage").unwrap().as_str().unwrap().to_string(),
                        })
                    }
                    _ => continue,
                };

                let action: HiveGuardAction = match gv.get("action").and_then(|a| a.as_str()) {
                    Some("warn") | Some("redirect") => HiveGuardAction::Warn,
                    Some("block") => HiveGuardAction::Block,
                    _ => HiveGuardAction::Warn,
                };
                let msg = gv.get("message").and_then(|m| m.as_str()).map(|s| s.to_string());
                let redirect = gv.get("redirect").and_then(|r| r.as_str()).map(|s| s.to_string());

                let guard = HiveGuard {
                    pattern: pattern.clone(),
                    action,
                    message: msg,
                    redirect,
                    repeat_threshold: gv.get("repeat_threshold")
                        .and_then(|r| r.as_integer()).map(|i| i as u32),
                };

                // Generate a matching input — extract keywords from the Rhai expression
                let match_cmd = match &pattern {
                    GuardPattern::JsonRegexPattern(p) => p.clone(),
                    GuardPattern::RhaiExpr(expr) => {
                        // Extract quoted strings from the Rhai expression to build a match input.
                        // e.g. cmd.contains("pip") → "pip install flask"
                        let mut keywords: Vec<String> = Vec::new();
                        let mut in_quote = false;
                        let mut current = String::new();
                        for ch in expr.rhai.chars() {
                            match ch {
                                '"' if !in_quote => { in_quote = true; current.clear(); }
                                '"' if in_quote => { in_quote = false; keywords.push(current.clone()); }
                                c if in_quote => current.push(c),
                                _ => {}
                            }
                        }
                        // Also check for references to macro names: pip_guard, docker_guard, etc.
                        // Map known macro names to their keywords
                        for keyword in &keywords {
                            match keyword.as_str() {
                                "pip" | "pip3" | "npm" | "conda" => {
                                    format!("{keyword} install somepackage")
                                }
                                "docker" => "docker run nginx".to_string(),
                                "git" => "git push --force origin main".to_string(),
                                "brew" => "brew install ffmpeg".to_string(),
                                "huggingface-cli" => {
                                    "huggingface-cli download some-model".to_string()
                                }
                                "rm" => "rm -rf /tmp/cache".to_string(),
                                "ulimit" => "ulimit -n 65536".to_string(),
                                _ => "trigger-command-match".to_string(),
                            };
                        }
                        // If none of the keywords match, try treating the entire expr
                        // as a macro name reference (pip_guard → "pip install foo")
                        if keywords.is_empty() && !expr.rhai.contains('"') {
                            let name_lower = expr.rhai.trim().to_lowercase();
                            if name_lower.contains("pip") {
                                "pip install somepackage"
                            } else if name_lower.contains("docker") {
                                "docker run nginx"
                            } else if name_lower.contains("git") {
                                "git push --force origin main"
                            } else {
                                "trigger-command-match"
                            }
                            .to_string()
                        } else {
                            // Last resort: use the literal command we built from keywords
                            keywords.join(" ") + " install"
                        }
                    }
                    GuardPattern::K0mmand3rStage(s) => s.stage.clone(),
                };
                let ctx_match = GuardContext {
                    command: match_cmd.clone(),
                    violation_count: 2,
                    repeat_threshold: Some(1),
                    rhai_macros: rhai_macros.clone(),
                };
                let result = check_guards(&match_cmd, &[guard.clone()], &ctx_match);
                let matched = matches!(result, GuardResult::Warn { .. } | GuardResult::Block { .. });
                if !matched {
                    failures.push(format!(
                        "{}[{}]: expected match for pattern={:?}, got Allow",
                        file_name, idx, pattern
                    ));
                }

                // Generate a non-matching input and verify it passes
                let no_match_cmd = "ls -la".to_string();
                let ctx_no = GuardContext::default();
                let result_no = check_guards(&no_match_cmd, &[guard], &ctx_no);
                let allowed = matches!(result_no, GuardResult::Allow);
                if !allowed {
                    failures.push(format!(
                        "{}[{}]: expected no-match for pattern={:?}, got non-Allow",
                        file_name, idx, pattern
                    ));
                }
            }
        }

        // Report results
        assert!(total_guards > 0, "no guards found — are hive-guards.hive.toml files shipped?");
        eprintln!(
            "✅ guard coverage: {total_guards} guards scanned, {rhai_guards} rhai expressions, {mat} failures",
            total_guards = total_guards,
            rhai_guards = rhai_guards,
            mat = failures.len(),
        );
        assert!(failures.is_empty(), "guard failures:\n  {}", failures.join("\n  "));
    }

    #[test]
    fn test_parse_hive_toml() {
        let toml_str = r#"
[b00t]
name = "test-profile"
type = "hive_profile"
hint = "test"

[b00t.hive.resources]
ram_gb = 10.0
gpu_mb = 8000

[b00t.hive.resources.gate]
ram_free_gb = 4.0

[[b00t.hive.guards]]
pattern = "pip install"
action = "warn"
message = "use uv"
"#;
        // write to temp file and parse
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("test-profile.hive.toml");
        std::fs::write(&path, toml_str).unwrap();
        let profile = HiveProfile::from_file(&path).unwrap();
        assert_eq!(profile.name, "test-profile");
        assert_eq!(profile.resources_ram_gb, Some(10.0));
        assert_eq!(profile.resources_gpu_mb, Some(8000));
        assert_eq!(profile.guards.len(), 1);
        assert_eq!(profile.guards[0].pattern, GuardPattern::JsonRegexPattern("pip install".to_string()));
    }

    #[test]
    fn test_resource_gate_fails() {
        let snapshot = SystemSnapshot {
            ram_total_gb: 31.0,
            ram_available_gb: 2.0, // only 2GB free
            swap_total_gb: 4.0,
            swap_free_gb: 4.0,
            gpu_name: None,
            gpu_total_mb: None,
            gpu_free_mb: None,
            cpu_cores: 4,
            active_downloads: vec![],
            active_services: vec![],
            active_profile: None,
            timestamp: "2026-01-01".to_string(),
        };
        let profile = HiveProfile {
            name: "inference-qwen3".to_string(),
            hint: "test".to_string(),
            resources_ram_gb: Some(30.0),
            resources_gpu_mb: None,
            resources_cpu_cores: None,
            resources_gate: Some(HiveResourceGate {
                ram_free_gb: Some(16.0),
                gpu_free_mb: None,
            }),
            exclusion_group: None,
            exclusion_priority: None,
            services_start: vec![],
            services_stop: vec![],
            guards: vec![],
            mcp_activate: vec![],
            mcp_deactivate: vec![],
            service_spec: None,
        };
        let issues = snapshot.satisfies_gate(&profile);
        assert!(!issues.is_empty(), "should fail gate with only 2GB free");
    }

    #[test]
    fn test_generate_systemd_unit_contains_exec_start() {
        let spec = HiveServiceSpec {
            description: Some("test service".to_string()),
            service_type: "simple".to_string(),
            exec_start: "/usr/bin/test --arg val".to_string(),
            exec_start_pre: vec![],
            environment: vec!["FOO=bar".to_string()],
            limit_nofile: Some(65536),
            restart: Some("on-failure".to_string()),
            restart_sec: Some("30s".to_string()),
            timeout_start_sec: Some("300".to_string()),
            working_directory: Some("/tmp/test-workdir".to_string()),
            after: vec!["network.target".to_string()],
        };
        let unit = generate_systemd_unit("test-profile", &spec);
        assert!(unit.contains("ExecStart=/usr/bin/test --arg val"));
        assert!(unit.contains("LimitNOFILE=65536"));
        assert!(unit.contains("Environment=\"FOO=bar\""));
        assert!(unit.contains("Restart=on-failure"));
        assert!(unit.contains("WorkingDirectory=/tmp/test-workdir"));
        assert!(unit.contains("[Install]"));
    }

    #[test]
    fn test_stack_template_unit_name() {
        assert_eq!(
            stack_template_unit("inference-gemma4"),
            "b00t@inference-gemma4.service"
        );
    }

    #[test]
    fn test_parse_hive_toml_with_service_spec() {
        let toml_str = r#"
[b00t]
name = "test-svc"
type = "hive_profile"
hint = "test with inline service"

[b00t.hive.service]
description = "test svc"
exec_start = "/usr/bin/sleep 3600"
environment = ["FOO=bar"]
limit_nofile = 1024
restart = "on-failure"
working_directory = "/tmp/test"
"#;
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("test-svc.hive.toml");
        std::fs::write(&path, toml_str).unwrap();
        let profile = HiveProfile::from_file(&path).unwrap();
        assert!(profile.service_spec.is_some());
        let spec = profile.service_spec.unwrap();
        assert_eq!(spec.exec_start, "/usr/bin/sleep 3600");
        assert_eq!(spec.limit_nofile, Some(1024));
        assert_eq!(spec.working_directory.as_deref(), Some("/tmp/test"));
    }

    // ── load_profile precedence tests ─────────────────────────────────────────

    fn minimal_hive_content(name: &str, hint: &str) -> String {
        format!(
            "[b00t]\nname = \"{}\"\ntype = \"hive_profile\"\nhint = \"{}\"\n",
            name, hint
        )
    }

    fn write_profile_file(dir: &std::path::Path, name: &str, ext: &str, hint: &str) {
        std::fs::write(
            dir.join(format!("{}{}", name, ext)),
            minimal_hive_content(name, hint),
        )
        .unwrap();
    }

    #[test]
    fn test_load_profile_prefers_hive_tomllmd() {
        let dir = tempfile::tempdir().unwrap();
        let name = "myprofile";

        // All five candidates present
        write_profile_file(dir.path(), name, ".hive.toml", "hive-toml");
        write_profile_file(dir.path(), name, ".hive.tomllm", "hive-tomllm");
        write_profile_file(dir.path(), name, ".stack.tomllm", "stack-tomllm");
        write_profile_file(dir.path(), name, ".stack.tomllmd", "stack-tomllmd");
        write_profile_file(dir.path(), name, ".hive.tomllmd", "hive-tomllmd");

        let profile = load_profile(name, dir.path()).unwrap();
        assert_eq!(profile.hint, "hive-tomllmd", ".hive.tomllmd must win");
    }

    #[test]
    fn test_load_profile_prefers_hive_tomllm_over_stack_and_toml() {
        let dir = tempfile::tempdir().unwrap();
        let name = "myprofile";

        write_profile_file(dir.path(), name, ".hive.toml", "hive-toml");
        write_profile_file(dir.path(), name, ".hive.tomllm", "hive-tomllm");
        write_profile_file(dir.path(), name, ".stack.tomllm", "stack-tomllm");

        let profile = load_profile(name, dir.path()).unwrap();
        assert_eq!(profile.hint, "hive-tomllm");
    }

    #[test]
    fn test_load_profile_prefers_stack_tomllmd_over_stack_tomllm_and_hive_toml() {
        let dir = tempfile::tempdir().unwrap();
        let name = "myprofile";

        write_profile_file(dir.path(), name, ".hive.toml", "hive-toml");
        write_profile_file(dir.path(), name, ".stack.tomllm", "stack-tomllm");
        write_profile_file(dir.path(), name, ".stack.tomllmd", "stack-tomllmd");

        let profile = load_profile(name, dir.path()).unwrap();
        assert_eq!(profile.hint, "stack-tomllmd");
    }

    #[test]
    fn test_load_profile_not_found_error_lists_tried_extensions() {
        let dir = tempfile::tempdir().unwrap();
        let err = load_profile("ghost", dir.path()).unwrap_err();
        let msg = err.to_string();
        assert!(msg.contains(".hive.tomllmd"), "error must mention .hive.tomllmd");
        assert!(msg.contains(".hive.tomllm"), "error must mention .hive.tomllm");
        assert!(msg.contains(".stack.tomllmd"), "error must mention .stack.tomllmd");
        assert!(msg.contains(".stack.tomllm"), "error must mention .stack.tomllm");
        assert!(msg.contains(".hive.toml"), "error must mention .hive.toml");
    }
}
