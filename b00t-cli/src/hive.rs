//! Hive CMDB — dynamic system state management for b00t hive nodes
//!
//! Reads real system resources (RAM, GPU, systemd services) and manages
//! hive profile transitions (download-mode ↔ inference-qwen3 ↔ inference-sm0l).
//!
//! Profile datums: _b00t_/*.hive.toml OR *.hive.tomllm OR *.hive.tomllmd
//! precedence: .hive.tomllmd > .hive.tomllm > .stack.tomllmd > .stack.tomllm > .hive.toml
//! State file: /tmp/b00t/hive-state.json (volatile; reset on reboot)

use anyhow::{Context, Result, bail};
use regex::Regex;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

pub const HIVE_STATE_PATH: &str = "/tmp/b00t/hive-state.json";
pub const HIVE_LEDGER_PATH: &str = "/tmp/b00t/hive-peers.json";

// ─── Peer Entry ──────────────────────────────────────────────────────────────

/// A peer node in the hive mesh
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PeerEntry {
    pub id: String,
    pub address: String,
    pub auth_type: String,
    pub zone: String,
    pub last_seen: String,
}

// ─── Accelerator ─────────────────────────────────────────────────────────────

/// A detected hardware accelerator (discrete GPU, integrated GPU, or NPU).
///
/// Probe chain: nvidia-smi → Mali device node → RKNPU2 driver package.
/// A node may surface several (e.g. an RK3588 reports both a Mali GPU and an RKNN NPU).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Accelerator {
    /// Coarse class: `"gpu"` (dedicated/integrated graphics) or `"npu"` (neural proc).
    pub class: String,
    /// How it was detected: `"nvidia-smi"` | `"mali-devnode"` | `"rknpu2-pkg"`.
    pub kind: String,
    /// Human-readable model name.
    pub name: String,
    /// Vendor (NVIDIA, ARM, Rockchip, …).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub vendor: Option<String>,
    /// Total VRAM in MB — `None` for shared-memory accelerators (NPU, iGPU).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub vram_total_mb: Option<u32>,
    /// Free VRAM in MB — `None` for shared-memory accelerators (NPU, iGPU).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub vram_free_mb: Option<u32>,
}

impl Accelerator {
    /// True if this is a graphics-class accelerator.
    pub fn is_gpu(&self) -> bool {
        self.class == "gpu"
    }
    /// True if this is a neural-processing-unit (shared DDR, no VRAM).
    pub fn is_npu(&self) -> bool {
        self.class == "npu"
    }
    /// One-line summary for the hive status display.
    pub fn summary(&self) -> String {
        match (self.vram_free_mb, self.vram_total_mb) {
            (Some(free), Some(total)) => {
                format!("{} — {}MB free / {}MB total", self.name, free, total)
            }
            _ => self.name.clone(),
        }
    }

    /// True if this accelerator satisfies a compatibility requirement token.
    ///
    /// Used to filter recipes/stacks to those compatible with a node's
    /// architecture (YAGNI: alpha-stage iterable filter). Tokens are
    /// case-insensitive:
    /// - `"gpu"` → any graphics-class accelerator
    /// - `"npu"` → any neural-processing-unit
    /// - a vendor/name/kind substring: `"nvidia"`, `"mali"`, `"rknn"`,
    ///   `"rknpu"`, or a probe kind verbatim (`"mali-devnode"`,
    ///   `"rknpu2-pkg"`, `"nvidia-smi"`)
    /// - empty → always satisfied (no requirement declared)
    pub fn satisfies(&self, req: &str) -> bool {
        let lower = req.trim().to_lowercase();
        let r = normalize_accel_token(&lower);
        if r.is_empty() {
            return true;
        }
        match r {
            "gpu" => self.is_gpu(),
            "npu" => self.is_npu(),
            _ => {
                // 🤓 substring over vendor+kind+name so the natural taxonomy
                //    ("nvidia", "mali", "rknpu", "rknpu2-pkg", …) resolves
                //    without an explicit token table — extensible for free.
                let hay = format!(
                    "{} {} {}",
                    self.vendor.as_deref().unwrap_or("").to_lowercase(),
                    self.kind,
                    self.name.to_lowercase()
                );
                hay.contains(r)
            }
        }
    }
}

/// Normalize common hardware synonyms so requirement tokens resolve naturally.
/// 🤓 RKNN (SDK) and RKNPU (driver package) are the same Rockchip NPU silicon.
fn normalize_accel_token(r: &str) -> &str {
    match r {
        "rknn" | "rknn2" => "rknpu",
        _ => r,
    }
}

/// Filter `accelerators` to those satisfying `requirement`.
/// Lazy iterator — the YAGNI surface for stack/recipe compatibility selection.
pub fn accelerators_matching<'a>(
    accels: &'a [Accelerator],
    requirement: &'a str,
) -> impl Iterator<Item = &'a Accelerator> {
    accels.iter().filter(move |a| a.satisfies(requirement))
}

/// True iff at least one accelerator satisfies *every* requirement token.
///
/// A node is "compatible" with a stack/recipe iff each declared requirement
/// (e.g. `["npu", "rknn"]`, `["gpu"]`) is met by some detected accelerator.
/// Empty requirements → compatible (no constraints declared).
pub fn satisfies_all_requirements(accels: &[Accelerator], requirements: &[String]) -> bool {
    requirements
        .iter()
        .all(|req| accels.iter().any(|a| a.satisfies(req)))
}

/// Derive the legacy single-GPU fields from a list of accelerators.
///
/// Prefers a GPU with its own VRAM (discrete). Returns `(name, total_mb, free_mb)`.
/// Pure helper so it is unit-testable independently of live probing.
fn derive_legacy_gpu_fields(accels: &[Accelerator]) -> (Option<String>, Option<u32>, Option<u32>) {
    let pick = accels
        .iter()
        .find(|a| a.is_gpu() && a.vram_total_mb.is_some())
        .or_else(|| accels.iter().find(|a| a.is_gpu()));
    match pick {
        Some(a) => (Some(a.name.clone()), a.vram_total_mb, a.vram_free_mb),
        None => (None, None, None),
    }
}

// ─── System Snapshot ─────────────────────────────────────────────────────────

/// Real-time system resource snapshot
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SystemSnapshot {
    pub ram_total_gb: f32,
    pub ram_available_gb: f32,
    pub swap_total_gb: f32,
    pub swap_free_gb: f32,
    /// Legacy single-GPU fields — kept for backwards-compatible JSON/serde.
    /// Derived from [`SystemSnapshot::accelerators`] (first GPU-class entry).
    pub gpu_name: Option<String>,
    pub gpu_total_mb: Option<u32>,
    pub gpu_free_mb: Option<u32>,
    /// All detected accelerators (discrete GPU, iGPU, NPU, …).
    #[serde(default)]
    pub accelerators: Vec<Accelerator>,
    pub cpu_cores: u32,
    pub active_downloads: Vec<String>, // PIDs/paths of active HF downloads
    pub active_services: Vec<String>,  // running systemd --user units
    pub active_profile: Option<String>, // from HIVE_STATE_PATH
    pub hive_ledger_path: Option<String>, // path to peer ledger JSON
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
        let accelerators = detect_accelerators();
        // 🤓 back-compat: surface the first GPU-class accelerator into the legacy
        //    gpu_* fields so existing JSON consumers + gate logic keep working.
        let (gpu_name, gpu_total_mb, gpu_free_mb) = derive_legacy_gpu_fields(&accelerators);
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
            accelerators,
            cpu_cores,
            active_downloads,
            active_services,
            active_profile,
            hive_ledger_path: Some(HIVE_LEDGER_PATH.to_string()),
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
        let accel = if self.accelerators.is_empty() {
            String::new()
        } else {
            let npus = self.accelerators.iter().filter(|a| a.is_npu()).count();
            let gpus = self.accelerators.iter().filter(|a| a.is_gpu()).count();
            format!(" | accel {}gpu/{}npu", gpus, npus)
        };
        format!(
            "RAM {:.1}/{:.1}GB avail{} | CPU {}c",
            self.ram_available_gb, self.ram_total_gb, accel, self.cpu_cores
        )
    }

    /// Stable hardware-identity fingerprint for drift detection (P3).
    ///
    /// Captures *identity* only — CPU cores, total RAM, and the set of
    /// accelerator classes/kinds. Excludes volatile values (free RAM, temps,
    /// driver versions) so routine load/driver updates don't trigger false drift.
    /// Two snapshots with the same fingerprint represent the same hardware.
    pub fn fingerprint(&self) -> String {
        let mut accels: Vec<String> = self
            .accelerators
            .iter()
            .map(|a| format!("{}:{}", a.class, a.kind))
            .collect();
        accels.sort();
        accels.dedup();
        format!(
            "{}c/{:.0}GB/{}",
            self.cpu_cores,
            self.ram_total_gb,
            accels.join(",")
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

    // Named Rhai macros used by command guards.
    pub rhai_macros: HashMap<String, String>,

    // MCP tool activation
    pub mcp_activate: Vec<String>,
    pub mcp_deactivate: Vec<String>,

    // inline service spec (generates b00t-hive-<name>.service on activate)
    pub service_spec: Option<HiveServiceSpec>,

    // datum keys this profile depends on (b00t hive status --datum-crossref)
    #[serde(default)]
    pub depends_on: Vec<String>,
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
    #[serde(default)]
    depends_on: Vec<String>,
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

        let rhai_macros = hive.rhai_macros.unwrap_or_default();

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
            rhai_macros,
            mcp_activate,
            mcp_deactivate,
            service_spec,
            depends_on: raw.b00t.depends_on,
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

// ─── Datum crossref (b00t hive status --datum-crossref) ───────────────────────

/// Result of cross-referencing a single hive profile's `depends_on` datums
/// against their `BootDatum.status`.
#[derive(Debug, Clone, Serialize)]
pub struct ProfileCrossrefResult {
    pub profile: String,
    pub healthy: bool,
    /// Reasons the profile is degraded, e.g. "dep agent-qwen disabled" or
    /// "dep agent-qwen missing". Empty when healthy.
    pub reasons: Vec<String>,
}

/// Cross-reference a hive profile's `depends_on` datum keys against their
/// `BootDatum.status`/`BootDatum.enabled` in the `_b00t_` datum store.
///
/// A profile is degraded when any dependency's backing datum is disabled, or
/// when no datum matches the dependency at all. A profile with an empty
/// `depends_on` (or where every dependency resolves to an enabled datum) is
/// healthy.
///
/// "Disabled" is `enabled == Some(false)` OR `status == Some("disabled")`.
/// The former is the real-world signal: datums in this repo are disabled via
/// `.gitattributes` (`b00t.enabled=false`), almost always paired with
/// `status=sunset` rather than a literal `status=disabled` — see
/// `_b00t_/.gitattributes`. Checking `status` alone would silently miss
/// every disabled datum actually in use. `status == "disabled"` is kept too,
/// for hand-authored datums that set it directly without a git-attribute
/// override.
pub fn crossref_datum_status(profile: &HiveProfile, b00t_path: &str) -> ProfileCrossrefResult {
    let mut reasons = Vec::new();

    for dep in &profile.depends_on {
        match crate::datum_utils::find_datum_by_pattern(b00t_path, dep) {
            Ok(Some(datum)) => {
                if datum.enabled == Some(false) || datum.status.as_deref() == Some("disabled") {
                    reasons.push(format!("dep {} disabled", dep));
                }
            }
            Ok(None) => {
                reasons.push(format!("dep {} missing", dep));
            }
            Err(_) => {
                reasons.push(format!("dep {} missing", dep));
            }
        }
    }

    ProfileCrossrefResult {
        profile: profile.name.clone(),
        healthy: reasons.is_empty(),
        reasons,
    }
}

/// Load a named profile from datum dir — prefers
/// .hive.tomllmd > .hive.tomllm > .stack.tomllmd > .stack.tomllm > .hive.toml > .agent.toml
///
/// 🤓 (#860) `*.agent.toml` datums (e.g. `opencode.agent.toml`) may carry an inline
///    `[b00t.hive.service]` table instead of a sibling `.hive.toml`. `HiveProfile::from_file`
///    already only reads the `[b00t]`/`[b00t.hive.*]` shape (serde ignores the surrounding
///    `[b00t.agent]`/`[b00t.env]`/`[[b00t.usage]]` sections), so an agent.toml is a drop-in
///    hive-profile source — it's a resolution gap, not a parsing one. The systemd naming
///    convention is `b00t@<base>-agent.service` for datum file `<base>.agent.toml` (see
///    `opencode.agent.toml` → `b00t@opencode-agent.service`), so a requested name ending in
///    "-agent" also probes the suffix-stripped base filename.
pub fn load_profile(name: &str, datum_dir: &Path) -> Result<HiveProfile> {
    // 🤓 .tomllmd currently downgrades to the generic .tomllm/TOML handling path.
    let hive_tomllmd_path = datum_dir.join(format!("{}.hive.tomllmd", name));
    let hive_tomllm_path = datum_dir.join(format!("{}.hive.tomllm", name));
    let stack_tomllmd_path = datum_dir.join(format!("{}.stack.tomllmd", name));
    let stack_tomllm_path = datum_dir.join(format!("{}.stack.tomllm", name));
    let hive_toml_path = datum_dir.join(format!("{}.hive.toml", name));
    let agent_toml_direct_path = datum_dir.join(format!("{}.agent.toml", name));
    let agent_toml_suffixed_path = name
        .strip_suffix("-agent")
        .map(|base| datum_dir.join(format!("{}.agent.toml", base)));

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
    } else if agent_toml_direct_path.exists() {
        agent_toml_direct_path
    } else if agent_toml_suffixed_path.as_ref().is_some_and(|p| p.exists()) {
        agent_toml_suffixed_path.unwrap()
    } else {
        bail!(
            "profile '{}' not found (tried .hive.tomllmd, .hive.tomllm, .stack.tomllmd, .stack.tomllm, .hive.toml, .agent.toml [b00t.hive.service])",
            name
        );
    };

    let is_agent_toml = path
        .file_name()
        .and_then(|f| f.to_str())
        .is_some_and(|f| f.ends_with(".agent.toml"));
    let mut profile = HiveProfile::from_file(&path)?;
    if is_agent_toml {
        // agent.toml's own [b00t].name is the agent identity (e.g. "opencode"), not the
        // hive/systemd profile identifier ("opencode-agent"). Re-stamp to the requested
        // name so downstream generated unit names (b00t-hive-<name>.service) match what
        // b00t@<name>.service's PropagatesStopTo expects.
        profile.name = name.to_string();
    }
    Ok(profile)
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
            GuardPattern::JsonRegexPattern(p) => {
                // Compile and apply regex; fall back to substring match for invalid patterns
                match regex::Regex::new(p) {
                    Ok(re) => re.is_match(command),
                    Err(_) => command.contains(p.as_str()),
                }
            }
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

/// Path for session-scoped guards (agent-adjustable at runtime).
pub fn session_guards_path() -> PathBuf {
    dirs::home_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join(".b00t")
        .join("session-guards.json")
}

/// Load agent-added session guards from ~/.b00t/session-guards.json.
/// Returns empty vec if file missing or malformed — never errors.
pub fn load_session_guards() -> Vec<HiveGuard> {
    #[derive(serde::Deserialize)]
    struct SessionGuardEntry {
        pattern: String,
        #[serde(default = "default_sg_action")]
        action: String,
        message: Option<String>,
        threshold: Option<u32>,
    }
    fn default_sg_action() -> String {
        "warn".to_string()
    }

    let path = session_guards_path();
    let content = match std::fs::read_to_string(&path) {
        Ok(c) => c,
        Err(_) => return vec![],
    };
    let entries: Vec<SessionGuardEntry> = match serde_json::from_str(&content) {
        Ok(v) => v,
        Err(_) => return vec![],
    };
    entries
        .into_iter()
        .map(|sg| {
            let action = match sg.action.as_str() {
                "block" => HiveGuardAction::Block,
                "redirect" => HiveGuardAction::Redirect,
                _ => HiveGuardAction::Warn,
            };
            HiveGuard {
                pattern: GuardPattern::JsonRegexPattern(sg.pattern),
                action,
                message: sg.message,
                redirect: None,
                repeat_threshold: sg.threshold,
            }
        })
        .collect()
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

    /// Load from default path, increment a pattern, append to file, return new count.
    /// Uses append-mode IO — each violation is one JSONL line.
    /// Does NOT rewrite the file (O(1) per violation).
    pub fn increment_persist(&mut self, pattern_key: &str) -> u32 {
        let new_count = self.increment(pattern_key);
        let path = default_violations_path();
        if let Some(parent) = path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        if let Ok(mut file) = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&path)
        {
            use std::io::Write;
            let _ = writeln!(
                file,
                "{}",
                serde_json::json!({"pattern": pattern_key, "count": new_count})
            );
        }
        // Also write to unified events.jsonl with consistent schema
        let home = std::env::var("HOME").unwrap_or_default();
        let events_path = std::path::Path::new(&home)
            .join(".b00t")
            .join("events.jsonl");
        if let Some(parent) = events_path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        if let Ok(mut file) = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&events_path)
        {
            use std::io::Write;
            let _ = writeln!(
                file,
                "{}",
                serde_json::json!({
                    "ts": chrono::Utc::now().to_rfc3339(),
                    "event": "guard",
                    "detail": pattern_key,
                    "action": if new_count > 1 { "block" } else { "warn" },
                    "pid": std::process::id(),
                })
            );
        }
        new_count
    }

    /// Compact the JSONL file by merging duplicate pattern counts.
    /// Reads all lines, sums counts per pattern, rewrites with current in-memory counts.
    pub fn compact(&self) -> std::io::Result<()> {
        self.save(&default_violations_path())
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
            let new_count = GuardViolationCounter::load(&default_violations_path())
                .increment_persist(&pattern_display);

            // 🦨→💩 escalation: if repeat_threshold is set and persisted violation
            // count exceeds the threshold, escalate Warn/Redirect to Block.
            // repeat_threshold=1 means: warn on 1st hit, block from 2nd hit onward.
            let (effective_message, effective_action) =
                match (&guard.action, guard.repeat_threshold) {
                    (HiveGuardAction::Warn | HiveGuardAction::Redirect, Some(threshold))
                        if new_count > threshold =>
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
    // Register regex_match(cmd, pattern) for guard pattern matching.
    // Allows guards like: pattern = { rhai = "regex_match(cmd, 'git checkout -b (feat|fix)/')" }
    engine.register_fn("regex_match", |s: &str, pattern: &str| -> bool {
        Regex::new(pattern)
            .map(|re| re.is_match(s))
            .unwrap_or(false)
    });
    let mut scope = Scope::new();
    scope.push("cmd", command.to_string());
    scope.push("violations", context.violation_count as i64);
    scope.push(
        "threshold",
        context.repeat_threshold.unwrap_or(u32::MAX) as i64,
    );

    // Build the full Rhai script: macro let-bindings + guard expression.
    // Each macro becomes: let <name> = <expr>;
    // Then the guard expression references them by name or composes with || && |>
    // Sort macros so those without dependencies come first (e.g. docker_guard before docker_run_guard)
    let mut script = String::new();
    let mut macro_vec: Vec<(&String, &String)> = context.rhai_macros.iter().collect();
    macro_vec.sort_by(|(_, a_expr), (_, b_expr)| {
        let a_dep = context
            .rhai_macros
            .keys()
            .any(|k| a_expr.contains(k.as_str()));
        let b_dep = context
            .rhai_macros
            .keys()
            .any(|k| b_expr.contains(k.as_str()));
        a_dep.cmp(&b_dep)
    });
    for (name, macro_expr) in &macro_vec {
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

/// Detect all hardware accelerators on this node.
///
/// Probe chain (every probe runs; a node may report several accelerators):
/// 1. NVIDIA discrete GPU — `nvidia-smi`
/// 2. ARM Mali integrated GPU — `/dev/mali0` device node
/// 3. Rockchip RKNN NPU — `rknpu2` driver package / DRM `*npu*` render node
pub fn detect_accelerators() -> Vec<Accelerator> {
    let mut accels = Vec::new();
    if let Some(a) = probe_nvidia_smi() {
        accels.push(a);
    }
    if let Some(a) = probe_mali() {
        accels.push(a);
    }
    if let Some(a) = probe_rknn_npu() {
        accels.push(a);
    }
    accels
}

/// NVIDIA discrete GPU via `nvidia-smi`. Returns None if nvidia-smi is absent/fails.
fn probe_nvidia_smi() -> Option<Accelerator> {
    let output = Command::new("nvidia-smi")
        .args([
            "--query-gpu=name,memory.total,memory.free",
            "--format=csv,noheader,nounits",
        ])
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let line = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if line.is_empty() {
        return None;
    }
    let parts: Vec<&str> = line.splitn(3, ',').collect();
    if parts.len() != 3 {
        return None;
    }
    let name = parts[0].trim().to_string();
    let total: u32 = parts[1].trim().parse().unwrap_or(0);
    let free: u32 = parts[2].trim().parse().unwrap_or(0);
    Some(Accelerator {
        class: "gpu".into(),
        kind: "nvidia-smi".into(),
        name,
        vendor: Some("NVIDIA".into()),
        vram_total_mb: Some(total),
        vram_free_mb: Some(free),
    })
}

/// ARM Mali integrated GPU via `/dev/mali0` device-node presence.
/// 🤓 Mali has no standard VRAM query; it shares system RAM → vram fields are None.
fn probe_mali() -> Option<Accelerator> {
    if Path::new("/dev/mali0").exists() {
        return Some(Accelerator {
            class: "gpu".into(),
            kind: "mali-devnode".into(),
            name: "ARM Mali (integrated)".into(),
            vendor: Some("ARM".into()),
            vram_total_mb: None,
            vram_free_mb: None,
        });
    }
    None
}

/// Rockchip RKNN NPU (RK3588/RK3576/…).
///
/// 🤓 Detection signals (either is sufficient):
/// - DRM render node whose name contains `npu` (vendor kernel exposes the NPU as
///   `/dev/dri/platform-fdab0000.npu-render` on RK3588); the legacy `/dev/rknpu`
///   char device is often absent.
/// - `rknn_server` runtime binary on PATH.
/// The driver version is read best-effort from dpkg (`rknpu2-rk3588` / `rknpu2`).
/// NPU shares DDR → no VRAM fields.
fn probe_rknn_npu() -> Option<Accelerator> {
    let has_drm_npu = fs::read_dir("/dev/dri")
        .ok()
        .map(|entries| {
            entries
                .filter_map(|e| e.ok())
                .any(|e| e.file_name().to_string_lossy().contains("npu"))
        })
        .unwrap_or(false);
    let has_rknn_server = which("rknn_server");
    if !has_drm_npu && !has_rknn_server {
        return None;
    }
    let ver = dpkg_version("rknpu2-rk3588").or_else(|| dpkg_version("rknpu2"));
    let name = match ver {
        Some(v) => format!("Rockchip RKNPU2 NPU ({v})"),
        None => "Rockchip RKNN NPU".to_string(),
    };
    Some(Accelerator {
        class: "npu".into(),
        kind: "rknpu2-pkg".into(),
        name,
        vendor: Some("Rockchip".into()),
        vram_total_mb: None,
        vram_free_mb: None,
    })
}

/// `true` if `bin` resolves on PATH (non-throwing `command -v`).
fn which(bin: &str) -> bool {
    Command::new("sh")
        .args(["-c", &format!("command -v {bin}")])
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

/// Best-effort installed version of a dpkg package, or None if absent/unknown.
fn dpkg_version(pkg: &str) -> Option<String> {
    let out = Command::new("dpkg-query")
        .args(["-W", "-f=${Version}", pkg])
        .output()
        .ok()?;
    if !out.status.success() {
        return None;
    }
    let v = String::from_utf8_lossy(&out.stdout).trim().to_string();
    if v.is_empty() { None } else { Some(v) }
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

    // ─── Accelerator probe logic ───────────────────────────────────────────────

    fn gpu(name: &str, total: u32, free: u32) -> Accelerator {
        Accelerator {
            class: "gpu".into(),
            kind: "nvidia-smi".into(),
            name: name.into(),
            vendor: Some("NVIDIA".into()),
            vram_total_mb: Some(total),
            vram_free_mb: Some(free),
        }
    }

    fn npu(name: &str) -> Accelerator {
        Accelerator {
            class: "npu".into(),
            kind: "rknpu2-pkg".into(),
            name: name.into(),
            vendor: Some("Rockchip".into()),
            vram_total_mb: None,
            vram_free_mb: None,
        }
    }

    #[test]
    fn accelerator_class_predicates() {
        assert!(gpu("x", 1, 1).is_gpu());
        assert!(!gpu("x", 1, 1).is_npu());
        assert!(npu("y").is_npu());
        assert!(!npu("y").is_gpu());
    }

    #[test]
    fn accelerator_summary_with_vram() {
        assert_eq!(
            gpu("RTX 3090", 24576, 8000).summary(),
            "RTX 3090 — 8000MB free / 24576MB total"
        );
    }

    #[test]
    fn accelerator_summary_without_vram() {
        // NPU / iGPU: no VRAM → name only
        assert_eq!(
            npu("Rockchip RKNPU2 NPU (2.3.0)").summary(),
            "Rockchip RKNPU2 NPU (2.3.0)"
        );
    }

    #[test]
    fn legacy_gpu_fields_prefer_discrete_gpu() {
        // RK3588 node: an NPU + an integrated Mali (no VRAM). Legacy fields stay None
        // because no GPU has its own VRAM — gate logic must not falsely report free VRAM.
        let mali = Accelerator {
            class: "gpu".into(),
            kind: "mali-devnode".into(),
            name: "ARM Mali".into(),
            vendor: Some("ARM".into()),
            vram_total_mb: None,
            vram_free_mb: None,
        };
        let accels = vec![npu("RKNPU2"), mali];
        let (name, total, free) = derive_legacy_gpu_fields(&accels);
        // Mali is a GPU but has no VRAM → we still surface its name, but VRAM stays None
        assert_eq!(name.as_deref(), Some("ARM Mali"));
        assert_eq!(total, None);
        assert_eq!(free, None);
    }

    #[test]
    fn legacy_gpu_fields_picks_nvidia_over_npu() {
        // A node with BOTH an NVIDIA GPU and an NPU: legacy fields must reflect the GPU.
        let accels = vec![gpu("RTX 3090", 24576, 8000), npu("RKNPU2")];
        let (name, total, free) = derive_legacy_gpu_fields(&accels);
        assert_eq!(name.as_deref(), Some("RTX 3090"));
        assert_eq!(total, Some(24576));
        assert_eq!(free, Some(8000));
    }

    #[test]
    fn legacy_gpu_fields_none_when_only_npu() {
        let accels = vec![npu("RKNPU2")];
        let (name, total, free) = derive_legacy_gpu_fields(&accels);
        assert_eq!(name, None);
        assert_eq!(total, None);
        assert_eq!(free, None);
    }

    #[test]
    fn snapshot_summary_line_counts_accelerators() {
        let snap = SystemSnapshot {
            ram_total_gb: 16.0,
            ram_available_gb: 14.0,
            swap_total_gb: 0.0,
            swap_free_gb: 0.0,
            gpu_name: None,
            gpu_total_mb: None,
            gpu_free_mb: None,
            accelerators: vec![gpu("Mali", 0, 0), npu("RKNPU2")],
            cpu_cores: 8,
            active_downloads: vec![],
            active_services: vec![],
            active_profile: None,
            hive_ledger_path: None,
            timestamp: "t".into(),
        };
        assert!(snap.summary_line().contains("1gpu/1npu"));
    }

    #[test]
    fn fingerprint_is_stable_and_sorted() {
        // a Mali iGPU (as probe_mali() would emit) + an RKNN NPU
        let mali = Accelerator {
            class: "gpu".into(),
            kind: "mali-devnode".into(),
            name: "ARM Mali".into(),
            vendor: Some("ARM".into()),
            vram_total_mb: None,
            vram_free_mb: None,
        };
        // same hardware → same fingerprint, regardless of accel vector order
        let mk = |accels: Vec<Accelerator>| SystemSnapshot {
            ram_total_gb: 16.0,
            ram_available_gb: 1.0,
            swap_total_gb: 0.0,
            swap_free_gb: 0.0,
            gpu_name: None,
            gpu_total_mb: None,
            gpu_free_mb: None,
            accelerators: accels,
            cpu_cores: 8,
            active_downloads: vec![],
            active_services: vec![],
            active_profile: None,
            hive_ledger_path: None,
            timestamp: "t".into(),
        };
        let a = mk(vec![npu("RKNPU2"), mali.clone()]);
        let b = mk(vec![mali, npu("RKNPU2")]);
        assert_eq!(a.fingerprint(), b.fingerprint());
        assert!(a.fingerprint().contains("8c/16GB/"));
        // class:kind only — versions excluded so driver updates don't drift
        assert!(a.fingerprint().contains("gpu:mali-devnode"));
        assert!(a.fingerprint().contains("npu:rknpu2-pkg"));
    }

    #[test]
    fn fingerprint_excludes_volatile_values() {
        // free RAM differs wildly between two captures of the same hardware →
        // fingerprint must be identical (it tracks total, not free).
        let base = || SystemSnapshot {
            ram_total_gb: 16.0,
            ram_available_gb: 14.0,
            swap_total_gb: 0.0,
            swap_free_gb: 0.0,
            gpu_name: None,
            gpu_total_mb: None,
            gpu_free_mb: None,
            accelerators: vec![npu("RKNPU2")],
            cpu_cores: 8,
            active_downloads: vec![],
            active_services: vec![],
            active_profile: None,
            hive_ledger_path: None,
            timestamp: "t".into(),
        };
        let mut loaded = base();
        loaded.ram_available_gb = 2.0; // nearly full RAM
        assert_eq!(base().fingerprint(), loaded.fingerprint());
    }

    #[test]
    fn snapshot_summary_line_no_accelerators() {
        let snap = SystemSnapshot {
            ram_total_gb: 16.0,
            ram_available_gb: 14.0,
            swap_total_gb: 0.0,
            swap_free_gb: 0.0,
            gpu_name: None,
            gpu_total_mb: None,
            gpu_free_mb: None,
            accelerators: vec![],
            cpu_cores: 8,
            active_downloads: vec![],
            active_services: vec![],
            active_profile: None,
            hive_ledger_path: None,
            timestamp: "t".into(),
        };
        assert!(!snap.summary_line().contains("accel"));
    }

    #[test]
    fn detect_accelerators_does_not_panic() {
        // Live probe on whatever host runs the test — must never panic, may be empty.
        let accels = detect_accelerators();
        // every entry must be self-consistent
        for a in &accels {
            assert!(a.is_gpu() || a.is_npu());
            assert!(!a.name.is_empty());
        }
    }

    // ─── compatibility filter (B) ──────────────────────────────────────────────

    #[test]
    fn satisfies_class_tokens() {
        let g = gpu("RTX 3090", 24576, 8000);
        let n = npu("RKNPU2");
        assert!(g.satisfies("gpu"));
        assert!(!g.satisfies("npu"));
        assert!(n.satisfies("npu"));
        assert!(!n.satisfies("gpu"));
    }

    #[test]
    fn satisfies_substring_tokens() {
        // vendor/name/kind substring matching
        let mali = Accelerator {
            class: "gpu".into(),
            kind: "mali-devnode".into(),
            name: "ARM Mali (integrated)".into(),
            vendor: Some("ARM".into()),
            vram_total_mb: None,
            vram_free_mb: None,
        };
        let rknn = npu("Rockchip RKNPU2 NPU (2.3.0)");
        assert!(mali.satisfies("mali"));
        assert!(mali.satisfies("mali-devnode"));
        assert!(rknn.satisfies("rknn"));
        assert!(rknn.satisfies("rknpu"));
        assert!(rknn.satisfies("rknpu2-pkg"));
        assert!(rknn.satisfies("rockchip"));
        // negative: a Mali does not satisfy an NPU requirement
        assert!(!mali.satisfies("rknn"));
    }

    #[test]
    fn satisfies_empty_is_always_true() {
        // no requirement declared → compatible
        assert!(gpu("x", 1, 1).satisfies(""));
        assert!(npu("y").satisfies(""));
    }

    #[test]
    fn accelerators_matching_is_lazy_iter() {
        let accels = vec![gpu("RTX 3090", 0, 0), npu("RKNPU2")];
        let gpus: Vec<_> = accelerators_matching(&accels, "gpu").collect();
        assert_eq!(gpus.len(), 1);
        assert_eq!(gpus[0].name, "RTX 3090");
    }

    #[test]
    fn satisfies_all_requirements_node_compat() {
        // RK3588 node: Mali + RKNN NPU
        let mali = Accelerator {
            class: "gpu".into(),
            kind: "mali-devnode".into(),
            name: "ARM Mali".into(),
            vendor: Some("ARM".into()),
            vram_total_mb: None,
            vram_free_mb: None,
        };
        let accels = vec![mali, npu("RKNPU2")];
        // a stack needing an NPU → compatible
        assert!(satisfies_all_requirements(&accels, &["npu".into()]));
        assert!(satisfies_all_requirements(&accels, &["rknn".into()]));
        // a stack needing a discrete NVIDIA GPU → NOT compatible (only Mali here)
        assert!(!satisfies_all_requirements(&accels, &["nvidia".into()]));
        // needs both gpu + npu → compatible (Mali covers gpu)
        assert!(satisfies_all_requirements(
            &accels,
            &["gpu".into(), "npu".into()]
        ));
        // empty requirements → compatible
        assert!(satisfies_all_requirements(&accels, &[]));
    }

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
        matches!(
            check_guards("rm -rf /", &guards, &ctx),
            GuardResult::Block { .. }
        );
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
        matches!(
            check_guards("cargo build", &guards, &ctx),
            GuardResult::Allow
        );
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

            let Some(guard_values) = guards_arr else {
                continue;
            };
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
                    Some(toml::Value::String(s)) => GuardPattern::JsonRegexPattern(s.clone()),
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

                // Validate K0mmand3rStage stage names against known ParseStage values.
                // Catches typos in TOML stage definitions at test time.
                if let GuardPattern::K0mmand3rStage(ref stage_guard) = pattern {
                    if k0mmand3r::parser_stages::ParseStage::from_name(&stage_guard.stage).is_none()
                    {
                        failures.push(format!(
                            "{file_name}[{idx}]: unknown stage '{}' — must be one of: pre_parse, pre_verb, post_verb, pre_params, post_params, pre_content, post_content, post_parse",
                            stage_guard.stage
                        ));
                    }
                }

                let action: HiveGuardAction = match gv.get("action").and_then(|a| a.as_str()) {
                    Some("warn") | Some("redirect") => HiveGuardAction::Warn,
                    Some("block") => HiveGuardAction::Block,
                    _ => HiveGuardAction::Warn,
                };
                let msg = gv
                    .get("message")
                    .and_then(|m| m.as_str())
                    .map(|s| s.to_string());
                let redirect = gv
                    .get("redirect")
                    .and_then(|r| r.as_str())
                    .map(|s| s.to_string());

                let guard = HiveGuard {
                    pattern: pattern.clone(),
                    action,
                    message: msg,
                    redirect,
                    repeat_threshold: gv
                        .get("repeat_threshold")
                        .and_then(|r| r.as_integer())
                        .map(|i| i as u32),
                };

                // Generate a matching input — extract keywords from the Rhai expression
                let match_cmd = match &pattern {
                    GuardPattern::JsonRegexPattern(p) => p.clone(),
                    GuardPattern::RhaiExpr(expr) => {
                        // Extract quoted strings from the Rhai expression to build a match input.
                        // e.g. cmd.contains("pip") → "pip install flask"
                        // Skip quoted strings in negated contexts: !cmd.contains("/")
                        let mut keywords: Vec<String> = Vec::new();
                        let mut in_quote = false;
                        let mut current = String::new();
                        let mut inter_quote_buf = String::new(); // chars between quotes (for negation detection)
                        for ch in expr.rhai.chars() {
                            match ch {
                                '"' if !in_quote => {
                                    in_quote = true;
                                    current.clear();
                                }
                                '"' if in_quote => {
                                    in_quote = false;
                                    // Skip quoted strings in negated contexts (!cmd.contains(...))
                                    let is_negated = inter_quote_buf.contains("!cmd.contains(");
                                    if !is_negated {
                                        keywords.push(current.clone());
                                    }
                                    inter_quote_buf.clear();
                                }
                                c if in_quote => current.push(c),
                                c => {
                                    inter_quote_buf.push(c);
                                }
                            }
                        }
                        // Also check for references to macro names: pip_guard, docker_guard, etc.
                        // Map known macro names to their keywords.
                        // First matching keyword wins, then falls through to macro name fallback.
                        let mut keyword_cmd: Option<String> = None;
                        for keyword in &keywords {
                            let cmd = match keyword.as_str() {
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
                                _ => continue,
                            };
                            keyword_cmd = Some(cmd);
                            break;
                        }
                        if let Some(cmd) = keyword_cmd {
                            cmd
                        } else if keywords.is_empty() && !expr.rhai.contains('"') {
                            // Bare macro name reference (e.g. "ledgerr_mcp_guard") or
                            // a composition of macros without literal strings.
                            // Try to resolve through rhai_macros to extract keywords.
                            let macro_resolved = if !rhai_macros.is_empty() {
                                let trimmed = expr.rhai.trim();
                                if let Some(macro_expr) = rhai_macros.get(trimmed) {
                                    // Extract quoted strings from the macro definition —
                                    // same negation-skip logic as the outer keyword-extraction
                                    // loop above: a quoted string appearing inside
                                    // !cmd.contains("...") must NOT be included in the
                                    // synthesized match command, or the synthesized command
                                    // will always fail its own negation clause. This loop was
                                    // missing that check (only the outer, non-macro-reference
                                    // path had it), so any bare macro-name-reference guard
                                    // whose expression combines a positive cmd.contains(...)
                                    // with a negated !cmd.contains(...) — e.g.
                                    // podman_build_uncapped/podman_run_uncapped — always
                                    // synthesized a command containing the negated keyword too,
                                    // making the guard always evaluate to "Allow" in this test.
                                    let mut macro_keywords: Vec<String> = Vec::new();
                                    let mut in_q = false;
                                    let mut cur = String::new();
                                    let mut macro_inter_quote_buf = String::new();
                                    for ch in macro_expr.chars() {
                                        match ch {
                                            '"' if !in_q => {
                                                in_q = true;
                                                cur.clear();
                                            }
                                            '"' if in_q => {
                                                in_q = false;
                                                let is_negated = macro_inter_quote_buf
                                                    .contains("!cmd.contains(");
                                                if !is_negated {
                                                    macro_keywords.push(cur.clone());
                                                }
                                                macro_inter_quote_buf.clear();
                                            }
                                            c if in_q => cur.push(c),
                                            c => macro_inter_quote_buf.push(c),
                                        }
                                    }
                                    if !macro_keywords.is_empty() {
                                        Some(macro_keywords.join(" "))
                                    } else {
                                        None
                                    }
                                } else {
                                    None
                                }
                            } else {
                                None
                            };
                            if let Some(cmd) = macro_resolved {
                                cmd
                            } else {
                                let name_lower = expr.rhai.trim().to_lowercase();
                                if name_lower.contains("pip") {
                                    "pip install somepackage".to_string()
                                } else if name_lower.contains("docker") {
                                    "docker run nginx".to_string()
                                } else if name_lower.contains("git") {
                                    "git push --force origin main".to_string()
                                } else {
                                    "trigger-command-match".to_string()
                                }
                            }
                        } else {
                            // Last resort: use the literal command we built from keywords
                            keywords.join(" ") + " install"
                        }
                    }
                    GuardPattern::K0mmand3rStage(s) => s.stage.clone(),
                };
                // K0mmand3rStage guards don't match via check_guards() — they're
                // triggered by the k0mmand3r parser stage hooks. Skip them here.
                if matches!(pattern, GuardPattern::K0mmand3rStage(_)) {
                    continue;
                }
                // Try multiple command variations to find one that matches the guard.
                // Rhai guards have specific patterns (e.g. cmd.contains("git push") && cmd.contains("origin main"))
                // that a single generic command may not satisfy.
                // Check if the guard relates to git — peek via the message hint
                // Detect git-related guards by scanning the rhai expression for "git"
                let has_git = match &pattern {
                    GuardPattern::RhaiExpr(e) => e.rhai.contains("git"),
                    _ => false,
                };
                let _match_candidates = if has_git {
                    vec![
                        match_cmd.clone(),
                        "git checkout master".to_string(),
                        "git push --force origin main".to_string(),
                        "git commit -m 'simple message'".to_string(), // no : for guard 19
                        "git checkout -b feat/new-thing".to_string(),
                        "git checkout -b main".to_string(), // no / — matches guard 18
                        "git merge feature-branch".to_string(),
                    ]
                } else {
                    vec![match_cmd.clone()]
                };
                let ctx_match = GuardContext {
                    command: match_cmd.clone(),
                    rhai_macros: rhai_macros.clone(),
                    violation_count: 0,
                    repeat_threshold: None,
                };
                let result = check_guards(&match_cmd, &[guard.clone()], &ctx_match);
                // K0mmand3rStage guards can't be tested via check_guards (they return false
                // since they're parser-stage hooks). Skip the match assertion for them.
                if !matches!(pattern, GuardPattern::K0mmand3rStage(_)) {
                    let matched =
                        matches!(result, GuardResult::Warn { .. } | GuardResult::Block { .. });
                    if !matched {
                        failures.push(format!(
                            "{}[{}]: expected match for pattern={:?}, got Allow",
                            file_name, idx, pattern
                        ));
                    }
                }

                // Generate a non-matching input and verify it passes.
                // Skip no-match test for K0mmand3rStage guards (they always match).
                if !matches!(pattern, GuardPattern::K0mmand3rStage(_)) {
                    let no_match_cmd = "ls -la".to_string();
                    let ctx_no = GuardContext {
                        command: no_match_cmd.clone(),
                        rhai_macros: rhai_macros.clone(),
                        violation_count: 0,
                        repeat_threshold: None,
                    };
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
        }

        // Report results
        assert!(
            total_guards > 0,
            "no guards found — are hive-guards.hive.toml files shipped?"
        );
        eprintln!(
            "✅ guard coverage: {total_guards} guards scanned, {rhai_guards} rhai expressions, {mat} failures",
            total_guards = total_guards,
            rhai_guards = rhai_guards,
            mat = failures.len(),
        );
        assert!(
            failures.is_empty(),
            "guard failures:\n  {}",
            failures.join("\n  ")
        );
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

[b00t.hive.rhai_macros]
ledgerr_mcp_guard = "cmd.contains(\"ledgerr-mcp\")"

[[b00t.hive.guards]]
pattern = "pip install"
action = "warn"
message = "use uv"

[[b00t.hive.guards]]
pattern = { rhai = "ledgerr_mcp_guard" }
action = "block"
message = "ledgerr-mcp requires supervised execution"
"#;
        // write to temp file and parse
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("test-profile.hive.toml");
        std::fs::write(&path, toml_str).unwrap();
        let profile = HiveProfile::from_file(&path).unwrap();
        assert_eq!(profile.name, "test-profile");
        assert_eq!(profile.resources_ram_gb, Some(10.0));
        assert_eq!(profile.resources_gpu_mb, Some(8000));
        assert_eq!(profile.guards.len(), 2);
        assert_eq!(
            profile.guards[0].pattern,
            GuardPattern::JsonRegexPattern("pip install".to_string())
        );
        assert_eq!(
            profile
                .rhai_macros
                .get("ledgerr_mcp_guard")
                .map(String::as_str),
            Some("cmd.contains(\"ledgerr-mcp\")")
        );
        let ctx = GuardContext {
            command: "ledgerr-mcp status".to_string(),
            rhai_macros: profile.rhai_macros.clone(),
            violation_count: 0,
            repeat_threshold: None,
        };
        assert!(matches!(
            check_guards("ledgerr-mcp status", &profile.guards, &ctx),
            GuardResult::Block { .. }
        ));
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
            accelerators: vec![],
            cpu_cores: 4,
            active_downloads: vec![],
            active_services: vec![],
            active_profile: None,
            hive_ledger_path: None,
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
            rhai_macros: HashMap::new(),
            mcp_activate: vec![],
            mcp_deactivate: vec![],
            service_spec: None,
            depends_on: vec![],
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

    // ── datum crossref tests (b00t hive status --datum-crossref, #713) ─────────

    #[test]
    fn test_hive_profile_depends_on_parses_from_toml() {
        let toml_str = r#"
[b00t]
name = "test-crossref-parse"
type = "hive_profile"
hint = "test depends_on parsing"
depends_on = ["agent-qwen", "serena.mcp"]
"#;
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("test-crossref-parse.hive.toml");
        std::fs::write(&path, toml_str).unwrap();
        let profile = HiveProfile::from_file(&path).unwrap();
        assert_eq!(
            profile.depends_on,
            vec!["agent-qwen".to_string(), "serena.mcp".to_string()]
        );
    }

    #[test]
    fn test_datum_crossref_degraded_when_dep_disabled() {
        let dir = tempfile::tempdir().unwrap();

        // backing datum: disabled
        std::fs::write(
            dir.path().join("agent-qwen.mcp.toml"),
            r#"
[b00t]
name = "agent-qwen"
type = "mcp"
hint = "qwen inference agent"
status = "disabled"
"#,
        )
        .unwrap();

        let toml_str = r#"
[b00t]
name = "serena"
type = "hive_profile"
hint = "test profile with disabled dep"
depends_on = ["agent-qwen.mcp"]
"#;
        let profile_path = dir.path().join("serena.hive.toml");
        std::fs::write(&profile_path, toml_str).unwrap();
        let profile = HiveProfile::from_file(&profile_path).unwrap();

        let result = crossref_datum_status(&profile, dir.path().to_str().unwrap());
        assert!(!result.healthy);
        assert_eq!(
            result.reasons,
            vec!["dep agent-qwen.mcp disabled".to_string()]
        );
    }

    #[test]
    fn test_datum_crossref_degraded_when_dep_enabled_false() {
        // Real-world disabled datums in this repo use `enabled = false`
        // (typically paired with `status = "sunset"`, set via
        // `.gitattributes` — see _b00t_/.gitattributes), NOT a literal
        // `status = "disabled"`. Regression guard: crossref must catch this
        // shape too, not just the synthetic status="disabled" case above.
        let dir = tempfile::tempdir().unwrap();

        std::fs::write(
            dir.path().join("agent-qwen.mcp.toml"),
            r#"
[b00t]
name = "agent-qwen"
type = "mcp"
hint = "qwen inference agent"
status = "sunset"
enabled = false
"#,
        )
        .unwrap();

        let toml_str = r#"
[b00t]
name = "serena"
type = "hive_profile"
hint = "test profile with sunset+enabled=false dep"
depends_on = ["agent-qwen.mcp"]
"#;
        let profile_path = dir.path().join("serena.hive.toml");
        std::fs::write(&profile_path, toml_str).unwrap();
        let profile = HiveProfile::from_file(&profile_path).unwrap();

        let result = crossref_datum_status(&profile, dir.path().to_str().unwrap());
        assert!(!result.healthy);
        assert_eq!(
            result.reasons,
            vec!["dep agent-qwen.mcp disabled".to_string()]
        );
    }

    #[test]
    fn test_datum_crossref_healthy_when_deps_enabled_or_empty() {
        let dir = tempfile::tempdir().unwrap();

        // backing datum: enabled (no status field means not disabled)
        std::fs::write(
            dir.path().join("agent-qwen.mcp.toml"),
            r#"
[b00t]
name = "agent-qwen"
type = "mcp"
hint = "qwen inference agent"
"#,
        )
        .unwrap();

        let toml_str = r#"
[b00t]
name = "serena"
type = "hive_profile"
hint = "test profile with healthy dep"
depends_on = ["agent-qwen.mcp"]
"#;
        let profile_path = dir.path().join("serena.hive.toml");
        std::fs::write(&profile_path, toml_str).unwrap();
        let profile = HiveProfile::from_file(&profile_path).unwrap();

        let result = crossref_datum_status(&profile, dir.path().to_str().unwrap());
        assert!(result.healthy);
        assert!(result.reasons.is_empty());

        // empty depends_on is also healthy
        let toml_str_empty = r#"
[b00t]
name = "no-deps"
type = "hive_profile"
hint = "profile with no dependencies"
"#;
        let empty_path = dir.path().join("no-deps.hive.toml");
        std::fs::write(&empty_path, toml_str_empty).unwrap();
        let empty_profile = HiveProfile::from_file(&empty_path).unwrap();
        let empty_result = crossref_datum_status(&empty_profile, dir.path().to_str().unwrap());
        assert!(empty_result.healthy);
        assert!(empty_result.reasons.is_empty());
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
        assert!(
            msg.contains(".hive.tomllmd"),
            "error must mention .hive.tomllmd"
        );
        assert!(
            msg.contains(".hive.tomllm"),
            "error must mention .hive.tomllm"
        );
        assert!(
            msg.contains(".stack.tomllmd"),
            "error must mention .stack.tomllmd"
        );
        assert!(
            msg.contains(".stack.tomllm"),
            "error must mention .stack.tomllm"
        );
        assert!(msg.contains(".hive.toml"), "error must mention .hive.toml");
        assert!(
            msg.contains(".agent.toml"),
            "error must mention .agent.toml (#860)"
        );
    }

    // ── load_profile .agent.toml resolution tests (#860) ─────────────────────

    /// Minimal fixture mirroring the real `opencode.agent.toml` shape: `[b00t]`
    /// name/hint + surrounding `[b00t.agent]`/`[b00t.env]`/`[[b00t.usage]]` noise
    /// that must be ignored, plus an inline `[b00t.hive.service]` table.
    fn agent_toml_with_inline_service(agent_name: &str) -> String {
        format!(
            r#"
[b00t]
name = "{agent_name}"
type = "agent"
hint = "{agent_name} coding agent — systemd-managed ACP server"

[b00t.agent]
pid = "{agent_name}-001"
model = "qwen36-local/ch0nky"

[b00t.hive.service]
description = "{agent_name} ACP server"
service_type = "simple"
restart = "on-failure"
restart_sec = "10s"
timeout_start_sec = "60"
after = ["network.target"]
environment = ["FOO=bar"]
exec_start = "{agent_name} serve --port 3000"

[b00t.hive.resources]
ram_gb = 1

[b00t.env]
AGENT_ID = "{agent_name}"

[[b00t.usage]]
description = "Start {agent_name} agent service"
command = "systemctl --user start b00t@{agent_name}-agent.service"
"#
        )
    }

    #[test]
    fn test_load_profile_resolves_agent_toml_via_suffix_stripped_base() {
        // systemd instance "opencode-agent" → datum file "opencode.agent.toml"
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(
            dir.path().join("opencode.agent.toml"),
            agent_toml_with_inline_service("opencode"),
        )
        .unwrap();

        let profile = load_profile("opencode-agent", dir.path())
            .expect("opencode-agent must resolve via opencode.agent.toml (#860)");

        // Re-stamped to the requested systemd/hive identifier, not the agent's own name.
        assert_eq!(profile.name, "opencode-agent");
        let spec = profile
            .service_spec
            .expect("[b00t.hive.service] must be extracted from the agent.toml");
        assert_eq!(spec.exec_start, "opencode serve --port 3000");
        assert_eq!(spec.restart.as_deref(), Some("on-failure"));
        assert_eq!(spec.after, vec!["network.target".to_string()]);
        assert_eq!(spec.environment, vec!["FOO=bar".to_string()]);
    }

    #[test]
    fn test_load_profile_resolves_agent_toml_direct_name() {
        // A profile requested by the bare agent.toml basename (no "-agent" suffix)
        // also resolves directly.
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(
            dir.path().join("moltis.agent.toml"),
            agent_toml_with_inline_service("moltis"),
        )
        .unwrap();

        let profile = load_profile("moltis", dir.path()).unwrap();
        assert!(profile.service_spec.is_some());
        assert_eq!(profile.name, "moltis");
    }

    #[test]
    fn test_load_profile_still_prefers_hive_toml_over_agent_toml() {
        // Regression guard: existing suffix precedence must not be disturbed by
        // adding .agent.toml as a fallback.
        let dir = tempfile::tempdir().unwrap();
        let name = "myprofile";
        write_profile_file(dir.path(), name, ".hive.toml", "hive-toml");
        std::fs::write(
            dir.path().join(format!("{}.agent.toml", name)),
            agent_toml_with_inline_service(name),
        )
        .unwrap();

        let profile = load_profile(name, dir.path()).unwrap();
        assert_eq!(profile.hint, "hive-toml", ".hive.toml must still win");
    }

    #[test]
    fn test_load_profile_resolves_real_opencode_agent_datum() {
        // End-to-end proof against the REAL production datum shipped in this repo
        // (_b00t_/opencode.agent.toml) — the exact repro from issue #860:
        // `systemctl --user start b00t@opencode-agent.service` failed because the
        // resolver never looked at *.agent.toml files.
        let b00t_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .unwrap()
            .join("_b00t_");
        if !b00t_dir.join("opencode.agent.toml").exists() {
            // Datum not present in this checkout (e.g. sparse worktree) — skip rather
            // than fail the suite on an environment difference.
            return;
        }

        let profile = load_profile("opencode-agent", &b00t_dir)
            .expect("real opencode.agent.toml must resolve as profile 'opencode-agent'");

        assert_eq!(profile.name, "opencode-agent");
        let spec = profile
            .service_spec
            .expect("real opencode.agent.toml [b00t.hive.service] must be extracted");
        assert_eq!(
            spec.exec_start,
            "opencode serve --port 3000 --model qwen36-local/ch0nky"
        );
        assert_eq!(spec.restart.as_deref(), Some("on-failure"));
        assert!(spec.after.contains(&"network.target".to_string()));
        assert!(
            spec.environment
                .iter()
                .any(|e| e.starts_with("OPENCODE_CONFIG="))
        );
    }
}
