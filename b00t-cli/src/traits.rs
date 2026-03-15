use crate::BootDatum;
use anyhow::Result;
use std::path::PathBuf;
use std::time::Duration;

/// Display trait for generating k8s CRDs from b00t datums
/// Enables MBSE-based stack → pod transformation
pub trait DatumCrdDisplay {
    /// Generate complete k8s Custom Resource Definition YAML
    fn to_crd_template(&self) -> Result<String>;

    /// Generate k8s Pod spec YAML
    fn to_pod_spec(&self) -> Result<String>;

    /// Get resource requirements (CPU, memory, GPU)
    fn get_resource_requirements(&self) -> ResourceRequirements;

    /// Get affinity rules for GPU batching and scheduling
    fn get_affinity_rules(&self) -> AffinityRules;

    /// Get budget constraints if defined
    fn get_budget_constraints(&self) -> Option<BudgetConstraints>;
}

/// Resource requirements for k8s pod scheduling
#[derive(Debug, Clone, Default)]
pub struct ResourceRequirements {
    pub cpu: Option<String>,
    pub memory: Option<String>,
    pub gpu_count: Option<u32>,
    pub gpu_memory: Option<String>,
    pub gpu_type: Option<String>,
}

/// Affinity rules for pod scheduling
#[derive(Debug, Clone)]
pub struct AffinityRules {
    pub strategy: AffinityStrategy,
    pub gpu_batch_group: Option<String>,
    pub topology_key: Option<String>,
}

impl Default for AffinityRules {
    fn default() -> Self {
        Self {
            strategy: AffinityStrategy::None,
            gpu_batch_group: None,
            topology_key: None,
        }
    }
}

/// Affinity scheduling strategies
#[derive(Debug, Clone, PartialEq)]
pub enum AffinityStrategy {
    None,
    GpuAffinity,     // Batch jobs to minimize GPU load/unload
    CostOptimized,   // Batch by budget constraints
    TimeEpoch,       // Batch by time windows
    ResourceSharing, // Allow multiple jobs on same GPU
}

/// Budget constraints for cost-aware scheduling
#[derive(Debug, Clone)]
pub struct BudgetConstraints {
    pub daily_limit: f64,
    pub cost_per_job: f64,
    pub currency: String,
    pub on_exceeded: String, // defer, alert, cancel
}

#[derive(Debug, Clone, PartialEq)]
pub enum VersionStatus {
    Match,   // 👍🏻
    Newer,   // 🐣
    Older,   // 😭
    Missing, // 😱
    Unknown, // ⏹️
}

impl VersionStatus {
    pub fn emoji(&self) -> &'static str {
        match self {
            VersionStatus::Match => "👍🏻",
            VersionStatus::Newer => "🐣",
            VersionStatus::Older => "😭",
            VersionStatus::Missing => "😱",
            VersionStatus::Unknown => "⏹️",
        }
    }
}

pub trait DatumChecker {
    fn is_installed(&self) -> bool;
    fn current_version(&self) -> Option<String>;
    fn desired_version(&self) -> Option<String>;
    fn version_status(&self) -> VersionStatus;
}

pub trait StatusProvider: DatumChecker {
    fn name(&self) -> &str;
    fn subsystem(&self) -> &str;
    fn hint(&self) -> &str;
    fn is_disabled(&self) -> bool;
}

pub trait FilterLogic {
    fn is_available(&self) -> bool;
    fn prerequisites_satisfied(&self) -> bool;
    fn evaluate_constraints(&self, require: &[String]) -> bool;
}

pub trait DatumProvider: DatumChecker + StatusProvider + FilterLogic + Send + Sync {
    /// Used by ConstraintEvaluator trait methods (compiler doesn't detect indirect usage)
    #[allow(dead_code)]
    fn datum(&self) -> &BootDatum;
}

/// Factory-style trait for interactive datum creation
pub trait DatumCreator {
    fn create_interactive(name: &str, path: &str) -> Result<BootDatum>;
    fn file_extension() -> &'static str {
        ".toml"
    }
    fn type_name() -> &'static str {
        "datum"
    }
}

// Base implementation for common constraint evaluation
pub trait ConstraintEvaluator {
    fn datum(&self) -> &BootDatum;

    fn has_any_env_vars(&self) -> bool {
        if let Some(env) = &self.datum().env {
            env.keys().any(|key| std::env::var(key).is_ok())
        } else {
            false
        }
    }

    fn has_all_env_vars(&self) -> bool {
        if let Some(env) = &self.datum().env {
            env.keys().all(|key| std::env::var(key).is_ok())
        } else {
            true // No env vars = satisfied
        }
    }

    fn check_os_requirement(&self, os: &str) -> bool {
        match os {
            "ubuntu" | "debian" => std::fs::read_to_string("/etc/os-release")
                .map(|content| content.contains("ubuntu") || content.contains("debian"))
                .unwrap_or(false),
            "macos" => cfg!(target_os = "macos"),
            "windows" => cfg!(target_os = "windows"),
            "linux" => cfg!(target_os = "linux"),
            _ => false,
        }
    }

    fn evaluate_constraints_default(&self, require: &[String]) -> bool {
        if require.is_empty() {
            // Default behavior: NEEDS_ALL_ENV for datums with env vars
            return self.has_all_env_vars();
        }

        require.iter().all(|constraint| {
            match constraint.as_str() {
                "NEEDS_ANY_ENV" => self.has_any_env_vars(),
                "NEEDS_ALL_ENV" => self.has_all_env_vars(),
                constraint if constraint.starts_with("OS:") => {
                    self.check_os_requirement(&constraint[3..])
                }
                constraint if constraint.starts_with("CMD:") => {
                    crate::check_command_available(&constraint[4..])
                }
                _ => true, // Unknown constraints default to true
            }
        })
    }
}

// ── CliExecutor: polymorphic execution trait ──────────────────────────────────

/// Declared sandbox capabilities — a tested contract, not a request.
/// An eBPF sandbox uses these to deterministically scope the agent's view:
/// undeclared paths are absent, not denied.
#[derive(Debug, Clone, Default)]
pub struct SandboxRequirements {
    /// Network egress permitted
    pub network: bool,
    /// Filesystem paths visible to the agent (absolute, canonicalized)
    pub filesystem: Vec<PathBuf>,
    /// Environment variable patterns readable by the recipe (globs)
    pub env_vars: Vec<String>,
    /// Secrets injected by sandbox, never logged or echoed
    pub secrets: Vec<String>,
    /// Wall-clock execution limit
    pub max_duration: Option<Duration>,
}

impl SandboxRequirements {
    /// Merge two requirement sets — union of capabilities.
    /// Used when composing recipe pipelines: the merged set is the minimum
    /// sandbox required to run the full chain.
    pub fn merge(mut self, other: &SandboxRequirements) -> Self {
        self.network = self.network || other.network;
        for p in &other.filesystem {
            if !self.filesystem.contains(p) {
                self.filesystem.push(p.clone());
            }
        }
        for e in &other.env_vars {
            if !self.env_vars.contains(e) {
                self.env_vars.push(e.clone());
            }
        }
        for s in &other.secrets {
            if !self.secrets.contains(s) {
                self.secrets.push(s.clone());
            }
        }
        if let Some(other_dur) = other.max_duration {
            self.max_duration = Some(match self.max_duration {
                Some(d) => d.max(other_dur),
                None => other_dur,
            });
        }
        self
    }
}

/// Metadata describing a single executable command or recipe.
#[derive(Debug, Clone)]
pub struct CommandSignature {
    pub name: String,
    pub description: Option<String>,
    pub parameters: Vec<ParameterSignature>,
    pub dependencies: Vec<String>,
}

/// A single parameter in a command signature.
#[derive(Debug, Clone)]
pub struct ParameterSignature {
    pub name: String,
    pub default_value: Option<String>,
    pub required: bool,
}

/// What would execute — returned by `dry_run` without side effects.
#[derive(Debug, Clone)]
pub struct ExecPlan {
    pub command_line: String,
    pub working_dir: PathBuf,
    pub env: Vec<(String, String)>,
    pub declared_effects: Vec<String>,
}

/// Monadic execution result — wraps output with provenance metadata.
/// Use `and_then` to chain executions; sandbox requirements propagate through the chain.
pub struct ExecOutput<T> {
    pub value: T,
    pub exit_code: i32,
    pub duration_ms: u64,
    pub sandbox: SandboxRequirements,
    pub declared_effects: Vec<String>,
}

impl<T> ExecOutput<T> {
    /// Monadic bind — chain executions, merge sandbox requirements.
    pub fn and_then<U, F>(self, f: F) -> Result<ExecOutput<U>>
    where
        F: FnOnce(T) -> Result<ExecOutput<U>>,
    {
        let sandbox = self.sandbox;
        let mut next = f(self.value)?;
        next.sandbox = next.sandbox.merge(&sandbox);
        Ok(next)
    }

    /// Map over value, preserve metadata.
    pub fn map<U, F: FnOnce(T) -> U>(self, f: F) -> ExecOutput<U> {
        ExecOutput {
            value: f(self.value),
            exit_code: self.exit_code,
            duration_ms: self.duration_ms,
            sandbox: self.sandbox,
            declared_effects: self.declared_effects,
        }
    }
}

/// Polymorphic execution trait — implemented by justfile, CLI, and bash datums.
/// Provides a commodity interface: discover → plan → execute → compose.
pub trait CliExecutor: DatumProvider {
    /// Execute with the given arguments, returning output wrapped in provenance.
    fn execute(&self, args: &[String]) -> Result<ExecOutput<String>>;

    /// Dry-run: describe what would execute without triggering side effects.
    fn dry_run(&self, args: &[String]) -> Result<ExecPlan>;

    /// List available commands/recipes for agent discovery.
    fn list_commands(&self) -> Result<Vec<CommandSignature>>;

    /// Declare sandbox requirements for this executor.
    /// Used by the eBPF sandbox to scope the agent's filesystem view.
    fn sandbox_requirements(&self) -> SandboxRequirements;
}
