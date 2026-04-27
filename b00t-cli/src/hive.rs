//! Hive CMDB — dynamic system state management for b00t hive nodes
//!
//! Reads real system resources (RAM, GPU, systemd services) and manages
//! hive profile transitions (download-mode ↔ inference-qwen3 ↔ inference-sm0l).
//!
//! Profile datums: _b00t_/*.hive.toml  OR  _b00t_/*.hive.tomllm  (.tomllm wins)
//! State file: /tmp/b00t/hive-state.json (volatile; reset on reboot)

use anyhow::{Context, Result, bail};
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
    pub pattern: String,
    pub action: HiveGuardAction,
    pub message: Option<String>,
    pub redirect: Option<String>,
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
    pattern: String,
    action: HiveGuardAction,
    message: Option<String>,
    redirect: Option<String>,
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

/// Find all .hive.toml / .hive.tomllm / .stack.tomllm datums; priority: .hive.tomllm > .stack.tomllm > .hive.toml
pub fn discover_profiles(datum_dir: &Path) -> Vec<(String, PathBuf)> {
    let mut profiles: HashMap<String, PathBuf> = HashMap::new();
    if let Ok(entries) = fs::read_dir(datum_dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
                if name.ends_with(".hive.tomllm") {
                    let profile_name = name.trim_end_matches(".hive.tomllm").to_string();
                    profiles.insert(profile_name, path); // .hive.tomllm wins
                } else if name.ends_with(".stack.tomllm") {
                    let profile_name = name.trim_end_matches(".stack.tomllm").to_string();
                    profiles.entry(profile_name).or_insert(path); // .stack.tomllm only if no .hive.tomllm
                } else if name.ends_with(".hive.toml") {
                    let profile_name = name.trim_end_matches(".hive.toml").to_string();
                    profiles.entry(profile_name).or_insert(path); // .toml only if no .tomllm variants
                }
            }
        }
    }
    let mut result: Vec<(String, PathBuf)> = profiles.into_iter().collect();
    result.sort_by(|a, b| a.0.cmp(&b.0));
    result
}

/// Load a named profile from datum dir — prefers .hive.tomllm > .stack.tomllm > .hive.toml
pub fn load_profile(name: &str, datum_dir: &Path) -> Result<HiveProfile> {
    // 🤓 priority: .hive.tomllm (explicit hive) > .stack.tomllm (sysconfig profile with inline service spec) > .hive.toml (fallback)
    let hive_tomllm_path = datum_dir.join(format!("{}.hive.tomllm", name));
    let stack_tomllm_path = datum_dir.join(format!("{}.stack.tomllm", name));
    let hive_toml_path = datum_dir.join(format!("{}.hive.toml", name));

    let path = if hive_tomllm_path.exists() {
        hive_tomllm_path
    } else if stack_tomllm_path.exists() {
        stack_tomllm_path
    } else if hive_toml_path.exists() {
        hive_toml_path
    } else {
        bail!(
            "profile '{}' not found (tried .hive.tomllm, .stack.tomllm, .hive.toml)",
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

/// Check a command string against a list of guards; returns first match
pub fn check_guards(command: &str, guards: &[HiveGuard]) -> GuardResult {
    for guard in guards {
        if command.contains(&guard.pattern) {
            let message = guard
                .message
                .clone()
                .unwrap_or_else(|| format!("guard matched: {}", guard.pattern));
            return match guard.action {
                HiveGuardAction::Warn | HiveGuardAction::Redirect => GuardResult::Warn {
                    message,
                    redirect: guard.redirect.clone(),
                },
                HiveGuardAction::Block => GuardResult::Block { message },
            };
        }
    }
    GuardResult::Allow
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
            pattern: "pip install".to_string(),
            action: HiveGuardAction::Warn,
            message: Some("🦨 use uv pip install".to_string()),
            redirect: Some("uv pip install".to_string()),
        }];
        matches!(
            check_guards("pip install requests", &guards),
            GuardResult::Warn { .. }
        );
    }

    #[test]
    fn test_guard_block() {
        let guards = vec![HiveGuard {
            pattern: "rm -rf /".to_string(),
            action: HiveGuardAction::Block,
            message: Some("🚫 blocked".to_string()),
            redirect: None,
        }];
        matches!(check_guards("rm -rf /", &guards), GuardResult::Block { .. });
    }

    #[test]
    fn test_guard_allow() {
        let guards = vec![HiveGuard {
            pattern: "pip install".to_string(),
            action: HiveGuardAction::Warn,
            message: None,
            redirect: None,
        }];
        matches!(check_guards("cargo build", &guards), GuardResult::Allow);
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
        assert_eq!(profile.guards[0].pattern, "pip install");
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
            after: vec!["network.target".to_string()],
        };
        let unit = generate_systemd_unit("test-profile", &spec);
        assert!(unit.contains("ExecStart=/usr/bin/test --arg val"));
        assert!(unit.contains("LimitNOFILE=65536"));
        assert!(unit.contains("Environment=\"FOO=bar\""));
        assert!(unit.contains("Restart=on-failure"));
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
"#;
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("test-svc.hive.toml");
        std::fs::write(&path, toml_str).unwrap();
        let profile = HiveProfile::from_file(&path).unwrap();
        assert!(profile.service_spec.is_some());
        let spec = profile.service_spec.unwrap();
        assert_eq!(spec.exec_start, "/usr/bin/sleep 3600");
        assert_eq!(spec.limit_nofile, Some(1024));
    }
}
