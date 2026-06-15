use crate::BootDatum;
use anyhow::Result;
use shlex::Shlex;
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

// ── Sandbox type hierarchy ────────────────────────────────────────────────────

/// The isolation mechanism surrounding recipe execution.
/// `None` is a first-class value — direct execution with eBPF scope declared
/// by the datum's capabilities contract. The sandbox shapes the agent's
/// perception; it does not deny access.
#[derive(Debug, Clone, PartialEq)]
pub enum SandboxKind {
    /// Direct execution; eBPF scope applied at syscall level via capabilities contract
    None,
    /// Linux namespaces + seccomp-bpf + eBPF filesystem filter
    Ebpf,
    /// OCI container — image string (e.g. "app4dogacr.azurecr.io/ml/yolo-train:latest")
    Container(String),
    /// WASM module via WASI — strongest isolation, narrowest I/O surface
    Wasm,
    /// systemd transient scope — resource limits + audit via systemd journal
    SystemdRun,
}

impl SandboxKind {
    pub fn from_str(s: &str) -> Self {
        match s {
            "none" | "None" | "local" | "Local" => SandboxKind::None,
            "ebpf" | "Ebpf" | "eBPF" => SandboxKind::Ebpf,
            "wasm" | "Wasm" => SandboxKind::Wasm,
            s if s.starts_with("container:") => SandboxKind::Container(s[10..].to_string()),
            s if s.starts_with("container(") => SandboxKind::Container(
                s.trim_start_matches("container(")
                    .trim_end_matches(')')
                    .to_string(),
            ),
            other => SandboxKind::Container(other.to_string()),
        }
    }

    pub fn name(&self) -> &str {
        match self {
            SandboxKind::None => "none",
            SandboxKind::Ebpf => "ebpf",
            SandboxKind::Container(_) => "container",
            SandboxKind::Wasm => "wasm",
            SandboxKind::SystemdRun => "systemd-run",
        }
    }
}

/// I/O channel abstraction — how a sandbox exchanges data with the executor.
/// Each sandbox kind has a natural I/O method; some support multiple.
#[derive(Debug, Clone, PartialEq)]
pub enum IoMethod {
    /// Direct stdin/stdout/stderr — most executors, SandboxKind::None
    Stdio,
    /// Byte pipes — composable, suitable for pipeline chaining
    Pipe,
    /// File-based exchange — container sandboxes with mounted volumes
    File(PathBuf),
    /// WASI-scoped IO — restricted filesystem + network capability table
    Wasi,
}

impl IoMethod {
    /// Default I/O method for a given sandbox kind
    pub fn for_sandbox(kind: &SandboxKind) -> Self {
        match kind {
            SandboxKind::None => IoMethod::Stdio,
            SandboxKind::Ebpf => IoMethod::Stdio,
            SandboxKind::Container(_) => IoMethod::Pipe,
            SandboxKind::Wasm => IoMethod::Wasi,
            SandboxKind::SystemdRun => IoMethod::Stdio,
        }
    }
}

/// Abstract sandbox — scopes execution, owns I/O, implements the capabilities contract.
pub trait Sandbox: Send + Sync {
    fn kind(&self) -> &SandboxKind;
    fn io_method(&self) -> IoMethod;
    fn scope(&self, reqs: &SandboxRequirements) -> Result<()>;
    fn run(&self, plan: &ExecPlan) -> Result<ExecOutput<String>> {
        use std::time::Instant;
        let start = Instant::now();

        // Parse the command line into argv tokens without invoking a shell.
        let argv: Vec<String> = {
            let lexer = Shlex::new(&plan.command_line);
            lexer.collect()
        };

        if argv.is_empty() {
            return Err(anyhow::anyhow!(
                "sandbox run failed: empty command_line in ExecPlan"
            ));
        }

        let mut cmd = std::process::Command::new(&argv[0]);
        if argv.len() > 1 {
            cmd.args(&argv[1..]);
        }
        let output = cmd
            .current_dir(&plan.working_dir)
            .envs(plan.env.iter().map(|(k, v)| (k.as_str(), v.as_str())))
            .output()
            .map_err(|e| anyhow::anyhow!("sandbox run failed: {}", e))?;
        Ok(ExecOutput {
            value: String::from_utf8_lossy(&output.stdout).into_owned(),
            exit_code: output.status.code().unwrap_or(-1),
            duration_ms: start.elapsed().as_millis() as u64,
            sandbox: SandboxRequirements::default(),
            sandbox_kind: self.kind().clone(),
            io_method: self.io_method(),
            declared_effects: plan.declared_effects.clone(),
        })
    }
}

/// No-op sandbox — direct execution; scope is enforced at eBPF level externally.
pub struct NoSandbox;

impl Sandbox for NoSandbox {
    fn kind(&self) -> &SandboxKind {
        &SandboxKind::None
    }
    fn io_method(&self) -> IoMethod {
        IoMethod::Stdio
    }
    fn scope(&self, _reqs: &SandboxRequirements) -> Result<()> {
        Ok(())
    }
}

/// OCI container sandbox — wraps execution in `docker run`.
pub struct ContainerSandbox {
    pub image: String,
    pub extra_flags: Vec<String>,
    kind: SandboxKind,
}

impl ContainerSandbox {
    pub fn new(image: impl Into<String>) -> Self {
        let image = image.into();
        Self {
            kind: SandboxKind::Container(image.clone()),
            image,
            extra_flags: vec!["--rm".to_string()],
        }
    }
}

impl Sandbox for ContainerSandbox {
    fn kind(&self) -> &SandboxKind {
        &self.kind
    }
    fn io_method(&self) -> IoMethod {
        IoMethod::Pipe
    }
    fn scope(&self, _reqs: &SandboxRequirements) -> Result<()> {
        Ok(())
    }
    fn run(&self, plan: &ExecPlan) -> Result<ExecOutput<String>> {
        use std::time::Instant;
        let start = Instant::now();
        let mut docker_args: Vec<String> = vec!["run".to_string()];
        docker_args.extend(self.extra_flags.clone());
        let wd = plan.working_dir.display().to_string();
        docker_args.extend_from_slice(&[
            "-v".to_string(),
            format!("{}:/workspace", wd),
            "-w".to_string(),
            "/workspace".to_string(),
        ]);
        for (k, v) in &plan.env {
            docker_args.extend_from_slice(&["-e".to_string(), format!("{}={}", k, v)]);
        }
        docker_args.push(self.image.clone());
        docker_args.extend_from_slice(&[
            "sh".to_string(),
            "-c".to_string(),
            plan.command_line.clone(),
        ]);
        let output = std::process::Command::new("podman")
            .args(&docker_args)
            .output()
            .map_err(|e| anyhow::anyhow!("docker run failed: {}", e))?;
        Ok(ExecOutput {
            value: String::from_utf8_lossy(&output.stdout).into_owned(),
            exit_code: output.status.code().unwrap_or(-1),
            duration_ms: start.elapsed().as_millis() as u64,
            sandbox: SandboxRequirements::default(),
            sandbox_kind: SandboxKind::Container(self.image.clone()),
            io_method: IoMethod::Pipe,
            declared_effects: plan.declared_effects.clone(),
        })
    }
}

/// systemd transient scope sandbox — wraps command in 'systemd-run --user --scope'.
/// Provides resource limits via systemd unit properties + audit via journal.
pub struct SystemdRunSandbox {
    /// Max memory in bytes (None = no limit)
    pub mem_limit: Option<u64>,
    /// Max CPU weight 1-10000 (None = default 100)
    pub cpu_weight: Option<u32>,
}

impl Default for SystemdRunSandbox {
    fn default() -> Self {
        Self { mem_limit: None, cpu_weight: None }
    }
}

impl Sandbox for SystemdRunSandbox {
    fn kind(&self) -> &SandboxKind {
        &SandboxKind::SystemdRun
    }
    fn io_method(&self) -> IoMethod {
        IoMethod::Stdio
    }
    fn scope(&self, _reqs: &SandboxRequirements) -> Result<()> {
        Ok(())
    }
    fn run(&self, plan: &ExecPlan) -> Result<ExecOutput<String>> {
        use std::time::Instant;
        let start = Instant::now();
        let unit_name = format!("b00t-exec-{}", std::process::id());
        let mut args: Vec<String> = vec![
            "--user".to_string(),
            "--scope".to_string(),
            format!("--unit={}", unit_name),
        ];
        if let Some(mem) = self.mem_limit {
            args.push(format!("--property=MemoryMax={}", mem));
        }
        if let Some(cpu) = self.cpu_weight {
            args.push(format!("--property=CPUWeight={}", cpu));
        }
        args.push("--".to_string());
        args.push("sh".to_string());
        args.push("-c".to_string());
        args.push(plan.command_line.clone());

        let output = std::process::Command::new("systemd-run")
            .args(&args)
            .current_dir(&plan.working_dir)
            .envs(plan.env.iter().map(|(k, v)| (k.as_str(), v.as_str())))
            .output()
            .map_err(|e| anyhow::anyhow!("systemd-run failed: {}", e))?;

        Ok(ExecOutput {
            value: String::from_utf8_lossy(&output.stdout).into_owned(),
            exit_code: output.status.code().unwrap_or(-1),
            duration_ms: start.elapsed().as_millis() as u64,
            sandbox: SandboxRequirements::default(),
            sandbox_kind: SandboxKind::SystemdRun,
            io_method: IoMethod::Stdio,
            declared_effects: plan.declared_effects.clone(),
        })
    }
}

// ── Execution types ───────────────────────────────────────────────────────────

/// Declared sandbox capabilities — a tested contract, not a request.
/// An eBPF sandbox uses these to deterministically scope the agent's view.
/// Undeclared paths are absent, not denied.
#[derive(Debug, Clone, Default)]
pub struct SandboxRequirements {
    pub network: bool,
    pub filesystem: Vec<PathBuf>,
    pub env_vars: Vec<String>,
    pub secrets: Vec<String>,
    pub max_duration: Option<Duration>,
}

impl SandboxRequirements {
    /// Merge two requirement sets — union of capabilities.
    /// Used when composing recipe pipelines.
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

#[derive(Debug, Clone)]
pub struct CommandSignature {
    pub name: String,
    pub description: Option<String>,
    pub parameters: Vec<ParameterSignature>,
    pub dependencies: Vec<String>,
    pub private: bool,
}

#[derive(Debug, Clone)]
pub struct ParameterSignature {
    pub name: String,
    pub default_value: Option<String>,
    pub required: bool,
    /// "singular" | "plus" (one-or-more) | "star" (zero-or-more)
    pub kind: String,
}

#[derive(Debug, Clone)]
pub struct ExecPlan {
    pub command_line: String,
    pub working_dir: PathBuf,
    pub env: Vec<(String, String)>,
    pub declared_effects: Vec<String>,
    pub sandbox_kind: SandboxKind,
    pub io_method: IoMethod,
}

/// Monadic execution result — wraps output with full provenance.
pub struct ExecOutput<T> {
    pub value: T,
    pub exit_code: i32,
    pub duration_ms: u64,
    pub sandbox: SandboxRequirements,
    pub sandbox_kind: SandboxKind,
    pub io_method: IoMethod,
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
            sandbox_kind: self.sandbox_kind,
            io_method: self.io_method,
            declared_effects: self.declared_effects,
        }
    }
}

/// Polymorphic execution trait.
/// Implemented by JustfileDatum, CliDatum, BashDatum.
/// The sandbox_kind and io_method fields on ExecOutput carry provenance
/// through monadic chains.
pub trait CliExecutor: DatumProvider {
    fn execute(&self, args: &[String]) -> Result<ExecOutput<String>>;
    fn dry_run(&self, args: &[String]) -> Result<ExecPlan>;
    fn list_commands(&self) -> Result<Vec<CommandSignature>>;
    fn sandbox_requirements(&self) -> SandboxRequirements;

    /// Subset vector of sandbox kinds this executor is compatible with.
    /// Ordered by preference — the first kind the runtime supports is used.
    /// `[SandboxKind::None]` = runs directly anywhere.
    fn allowed_sandboxes(&self) -> Vec<SandboxKind>;

    /// Preferred I/O method for this executor (defaults to Stdio).
    fn io_method(&self) -> IoMethod {
        IoMethod::for_sandbox(&SandboxKind::None)
    }

    /// Execute within a specific sandbox context.
    /// Dispatches to sandbox.run() — ContainerSandbox wraps in docker run.
    fn execute_in(&self, args: &[String], sandbox: &dyn Sandbox) -> Result<ExecOutput<String>> {
        sandbox.scope(&self.sandbox_requirements())?;
        let plan = self.dry_run(args)?;
        let mut output = sandbox.run(&plan)?;
        output.sandbox = output.sandbox.merge(&self.sandbox_requirements());
        Ok(output)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn sandbox_kind_local_maps_to_none() {
        assert_eq!(SandboxKind::from_str("local"), SandboxKind::None);
        assert_eq!(SandboxKind::from_str("Local"), SandboxKind::None);
    }
    #[test]
    fn sandbox_kind_known_values() {
        assert_eq!(SandboxKind::from_str("none"), SandboxKind::None);
        assert_eq!(SandboxKind::from_str("ebpf"), SandboxKind::Ebpf);
        assert_eq!(SandboxKind::from_str("wasm"), SandboxKind::Wasm);
        assert_eq!(
            SandboxKind::from_str("container:img"),
            SandboxKind::Container("img".to_string())
        );
    }
    #[test]
    fn no_sandbox_run_executes_command() {
        let plan = ExecPlan {
            command_line: "echo no-sandbox-works".to_string(),
            working_dir: PathBuf::from("."),
            env: vec![],
            declared_effects: vec![],
            sandbox_kind: SandboxKind::None,
            io_method: IoMethod::Stdio,
        };
        let output = NoSandbox.run(&plan).unwrap();
        assert_eq!(output.exit_code, 0);
        assert!(output.value.contains("no-sandbox-works"));
    }
    #[test]
    fn container_sandbox_image_in_kind() {
        let s = ContainerSandbox::new("alpine:latest");
        assert_eq!(s.image, "alpine:latest");
    }
    fn make_output(value: &str, network: bool, fs: Vec<&str>) -> ExecOutput<String> {
        ExecOutput {
            value: value.to_string(),
            exit_code: 0,
            duration_ms: 1,
            sandbox: SandboxRequirements {
                network,
                filesystem: fs.into_iter().map(PathBuf::from).collect(),
                env_vars: vec![],
                secrets: vec![],
                max_duration: None,
            },
            sandbox_kind: SandboxKind::None,
            io_method: IoMethod::Stdio,
            declared_effects: vec![],
        }
    }
    #[test]
    fn and_then_merges_sandbox_requirements() {
        let a = make_output("a", true, vec!["/workspace"]);
        let result = a
            .and_then(|_| Ok(make_output("b", false, vec!["/data"])))
            .unwrap();
        assert!(result.sandbox.network);
        assert!(
            result
                .sandbox
                .filesystem
                .contains(&PathBuf::from("/workspace"))
        );
        assert!(result.sandbox.filesystem.contains(&PathBuf::from("/data")));
    }
    #[test]
    fn and_then_no_duplicate_paths() {
        let a = make_output("a", false, vec!["/shared"]);
        let result = a
            .and_then(|_| Ok(make_output("b", false, vec!["/shared", "/extra"])))
            .unwrap();
        let count = result
            .sandbox
            .filesystem
            .iter()
            .filter(|p| *p == &PathBuf::from("/shared"))
            .count();
        assert_eq!(count, 1);
    }
    #[test]
    fn sandbox_kind_name_includes_systemd_run() {
        assert_eq!(SandboxKind::None.name(), "none");
        assert_eq!(SandboxKind::SystemdRun.name(), "systemd-run");
        assert_eq!(SandboxKind::Container("img".to_string()).name(), "container");
    }

    #[test]
    fn io_method_for_sandbox_systemd_run_is_stdio() {
        assert_eq!(IoMethod::for_sandbox(&SandboxKind::SystemdRun), IoMethod::Stdio);
    }

    #[test]
    fn systemd_run_sandbox_kind_is_correct() {
        let s = SystemdRunSandbox::default();
        assert_eq!(s.kind().name(), "systemd-run");
        assert_eq!(s.io_method(), IoMethod::Stdio);
    }

    #[test]
    fn sandbox_kind_from_str_roundtrip() {
        let k = SandboxKind::from_str("none");
        assert_eq!(k.name(), "none");
        let k2 = SandboxKind::from_str("wasm");
        assert_eq!(k2.name(), "wasm");
    }

    #[test]
    fn no_sandbox_run_executes_echo() {
        let plan = ExecPlan {
            command_line: "echo sandbox-provider-test".to_string(),
            working_dir: PathBuf::from("."),
            env: vec![],
            declared_effects: vec![],
            sandbox_kind: SandboxKind::None,
            io_method: IoMethod::Stdio,
        };
        let output = NoSandbox.run(&plan).unwrap();
        assert_eq!(output.exit_code, 0);
        assert!(output.value.contains("sandbox-provider-test"));
    }

}