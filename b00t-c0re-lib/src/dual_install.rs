//! Dual-install method client — fan-out across multiple install strategies
//!
//! # Default behavior
//! `InstallTarget::All` fans out via `tokio::spawn` to all provided methods.
//! Each method can fail independently — first success wins, others cancelled.
//! Failures are surfaced as warnings, not hard errors.
//!
//! # Target selection
//! CLI flag `--install` maps to `InstallTarget`:
//!   absent              → `Preferred` (backward compat — single method)
//!   `--install=preferred` → `Preferred`
//!   `--install=all`       → `All` (fan-out, first success cancels rest)
//!
//! # Testing failure paths
//! Create a test client with `DualInstallClient::test_with(failing_executor)`
//! to verify error handling without executing real shell commands.

use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use std::time::Instant;
use tokio::task::JoinHandle;
use tokio_util::sync::CancellationToken;
use tracing::{debug, info, warn};

// ── Install method variants ──────────────────────────────────────────────────

/// A single install method that can be attempted
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum InstallMethod {
    /// TOML datum install command (shell script)
    Cli { command: String },
    /// Docker-based install
    Docker {
        image: String,
        args: Vec<String>,
    },
    /// Package manager install
    Package {
        manager: String,
        package: String,
    },
    /// Direct binary download
    CurlBinary {
        url: String,
        dest: String,
    },
}

impl InstallMethod {
    /// Human-readable name for logging/display
    pub fn display_name(&self) -> &'static str {
        match self {
            Self::Cli { .. } => "cli",
            Self::Docker { .. } => "docker",
            Self::Package { .. } => "package",
            Self::CurlBinary { .. } => "curl-binary",
        }
    }

    /// Short description of what this method would do
    pub fn description(&self) -> String {
        match self {
            Self::Cli { command } => format!("Shell: {}", command),
            Self::Docker { image, args } => {
                if args.is_empty() {
                    format!("Docker: {}", image)
                } else {
                    format!("Docker: {} {:?}", image, args)
                }
            }
            Self::Package {
                manager,
                package,
            } => format!("{} install {}", manager, package),
            Self::CurlBinary { url, dest } => {
                format!("curl {} → {}", url, dest)
            }
        }
    }

    /// Extract the package name for Package variant, or method name otherwise
    pub fn package_name(&self) -> String {
        match self {
            Self::Package { package, .. } => package.clone(),
            Self::Docker { image, .. } => image.clone(),
            Self::Cli { command } => {
                command.split_whitespace().next().unwrap_or("cli").to_string()
            }
            Self::CurlBinary { url, .. } => {
                url.rsplit('/').next().unwrap_or("binary").to_string()
            }
        }
    }
}

// ── Target selector ──────────────────────────────────────────────────────────

/// How to execute install methods
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum InstallTarget {
    /// Only try the first (preferred) method
    Preferred,
    /// Fan-out all methods, first success wins (others cancelled)
    All,
}

impl InstallTarget {
    pub fn display_name(self) -> &'static str {
        match self {
            Self::Preferred => "preferred",
            Self::All => "all",
        }
    }

    /// Parse from `--install` flag value. Returns `Preferred` for absent/empty.
    pub fn from_flag(raw: Option<&str>) -> Result<Self> {
        match raw {
            None | Some("") | Some("preferred") => Ok(Self::Preferred),
            Some("all") => Ok(Self::All),
            Some(other) => Err(anyhow::anyhow!(
                "Unknown --install target '{}'. Valid: preferred, all",
                other
            )),
        }
    }
}

// ── Result type ──────────────────────────────────────────────────────────────

/// Result of an install fan-out operation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InstallResult {
    /// Which method succeeded (empty string if all failed)
    pub method_used: String,
    /// Overall success status
    pub success: bool,
    /// Warnings from failed methods (empty if success)
    pub warnings: Vec<String>,
    /// Elapsed time for the winning method (or total if all failed), in ms
    pub elapsed_ms: u64,
}

impl InstallResult {
    /// Construct a success result
    pub fn success(method: &str, elapsed_ms: u64) -> Self {
        Self {
            method_used: method.to_string(),
            success: true,
            warnings: Vec::new(),
            elapsed_ms,
        }
    }

    /// Construct a failure result
    pub fn failure(warnings: Vec<String>, elapsed_ms: u64) -> Self {
        Self {
            method_used: String::new(),
            success: false,
            warnings,
            elapsed_ms,
        }
    }
}

// ── Executor type for test injection ─────────────────────────────────────────

/// A function that executes an install method and returns success/failure.
/// Used for both real execution and test mocking.
type ExecutorFn = Arc<dyn Fn(&InstallMethod) -> JoinHandle<bool> + Send + Sync>;

// ── Dual install client ──────────────────────────────────────────────────────

/// Fan-out install client dispatching across multiple install strategies
///
/// Uses `tokio::select!` for "first success wins, cancel rest" behavior.
/// Test failure paths by creating a client with `DualInstallClient::test_with(executor)`.
pub struct DualInstallClient {
    /// Override executor (None = use default execute_method).
    /// Used for testing failure paths without real shell commands.
    executor: Option<ExecutorFn>,
}

impl DualInstallClient {
    pub fn new() -> Self {
        Self { executor: None }
    }

    /// Create a test client with a custom executor function.
    /// The executor is called for each method; returning a JoinHandle that
    /// resolves to `true` (success) or `false` (failure).
    ///
    /// # Example - all methods fail:
    /// ```rust
    /// let client = DualInstallClient::test_with(Arc::new(|_m| {
    ///     tokio::spawn(async { false })
    /// }));
    /// let result = client.install(methods, InstallTarget::All).await.unwrap();
    /// assert!(!result.success); // all methods failed
    /// ```
    pub fn test_with(executor: ExecutorFn) -> Self {
        Self {
            executor: Some(executor),
        }
    }

    /// Execute install with the specified target strategy.
    ///
    /// ## Preferred mode
    /// Attempts only the first method. Returns result (success or failure).
    ///
    /// ## All mode
    /// Spawns all methods in parallel with `tokio::spawn`.
    /// First success wins; remaining tasks are cancelled via CancellationToken.
    /// If all fail, returns failure with all warnings.
    pub async fn install(
        &self,
        methods: Vec<InstallMethod>,
        target: InstallTarget,
    ) -> Result<InstallResult> {
        if methods.is_empty() {
            return Ok(InstallResult::failure(
                vec!["No install methods provided".to_string()],
                0,
            ));
        }

        match target {
            InstallTarget::Preferred => self.install_preferred(&methods[0]).await,
            InstallTarget::All => self.install_all(methods).await,
        }
    }

    /// Execute a single preferred method
    async fn install_preferred(&self, method: &InstallMethod) -> Result<InstallResult> {
        let start = Instant::now();
        info!(
            "install_preferred: trying {} ({})",
            method.display_name(),
            method.description()
        );

        let success = self.spawn_exec(method).await;
        let elapsed_ms = start.elapsed().as_millis() as u64;

        if success {
            info!("install: {} succeeded in {}ms", method.display_name(), elapsed_ms);
            Ok(InstallResult::success(method.display_name(), elapsed_ms))
        } else {
            let msg = format!("{} failed", method.display_name());
            warn!("install: {}", msg);
            Ok(InstallResult::failure(vec![msg], elapsed_ms))
        }
    }

    /// Fan-out across all methods with "first success wins, cancel rest" semantics.
    ///
    /// Uses CancellationToken to abort remaining tasks after first success.
    /// Each task's completion is sent through a oneshot channel so the
    /// select loop can pick the first success immediately.
    async fn install_all(&self, methods: Vec<InstallMethod>) -> Result<InstallResult> {
        let total_start = Instant::now();
        let count = methods.len();
        info!("install_all: fan-out {} methods", count);

        let cancel = CancellationToken::new();
        let mut warnings: Vec<String> = Vec::new();

        // Spawn all methods; each sends result via oneshot channel
        let (tx, mut rx) = tokio::sync::mpsc::channel::<(InstallMethod, bool, u64)>(count);

        for method in &methods {
            let m = method.clone();
            let tx = tx.clone();
            let cancel_child = cancel.clone();
            let executor = self.executor.clone();

            tokio::spawn(async move {
                // Early exit if already cancelled by another success
                if cancel_child.is_cancelled() {
                    return;
                }

                let start = Instant::now();
                debug!(
                    "install_all: starting {} ({})",
                    m.display_name(),
                    m.description()
                );

                let ok = match &executor {
                    Some(f) => f(&m).await.unwrap_or(false),
                    None => DualInstallClient::execute_method(&m).await,
                };
                let elapsed_ms = start.elapsed().as_millis() as u64;

                let _ = tx.send((m, ok, elapsed_ms)).await;
            });
        }

        // Drop the original sender so the loop terminates when all tasks complete
        drop(tx);

        let mut completed = 0;
        while let Some((method, ok, elapsed_ms)) = rx.recv().await {
            completed += 1;

            if ok {
                // First success — cancel remaining tasks
                info!(
                    "install_all: {} succeeded in {}ms (cancelling {} remaining)",
                    method.display_name(),
                    elapsed_ms,
                    count - completed
                );
                cancel.cancel();
                return Ok(InstallResult::success(method.display_name(), elapsed_ms));
            } else {
                warn!("install_all: {} failed", method.display_name());
                warnings.push(format!("{} failed", method.display_name()));

                // If all tasks completed without success, return failure
                if completed >= count {
                    let elapsed_ms = total_start.elapsed().as_millis() as u64;
                    warn!(
                        "install_all: all {} methods failed in {}ms",
                        count, elapsed_ms
                    );
                    return Ok(InstallResult::failure(warnings, elapsed_ms));
                }
            }
        }

        // Should not reach here if completed >= count check works, but safety net:
        let elapsed_ms = total_start.elapsed().as_millis() as u64;
        Ok(InstallResult::failure(warnings, elapsed_ms))
    }

    /// Spawn the executor (either overridden or default).
    /// Returns true on success, false on failure.
    async fn spawn_exec(&self, method: &InstallMethod) -> bool {
        match &self.executor {
            Some(f) => match f(method).await {
                Ok(ok) => ok,
                Err(e) => {
                    tracing::warn!("spawn_exec: executor JoinError: {}", e);
                    false
                }
            },
            None => Self::execute_method(method).await,
        }
    }

    /// Execute an install method.
    ///
    /// Real implementation dispatches based on variant:
    async fn execute_method(method: &InstallMethod) -> bool {
        match method {
            InstallMethod::Cli { command } => {
                let parts = shell_words::split(command).unwrap_or_else(|_| vec![command.clone()]);
                tokio::process::Command::new(&parts[0])
                    .args(&parts[1..])
                    .output()
                    .await
                    .map(|o| o.status.success())
                    .unwrap_or(false)
            }
            InstallMethod::Docker { image, args } => {
                let mut cmd = tokio::process::Command::new("docker");
                cmd.arg("run").arg("--rm").arg(image);
                for a in args {
                    cmd.arg(a);
                }
                cmd.output().await.map(|o| o.status.success()).unwrap_or(false)
            }
            InstallMethod::Package { manager, package } => {
                let (cmd, args) = match manager.as_str() {
                    "apt" => ("apt-get", vec!["install", "-y", package.as_str()]),
                    "brew" => ("brew", vec!["install", package.as_str()]),
                    "pkgx" => (package.as_str(), vec![]),
                    "aptitude" => ("aptitude", vec!["install", "-y", package.as_str()]),
                    "pip" | "pip3" => (manager.as_str(), vec!["install", package.as_str()]),
                    "cargo" => ("cargo", vec!["install", package.as_str()]),
                    _ => {
                        debug!("Unsupported package manager: {}", manager);
                        return false;
                    }
                };
                tokio::process::Command::new(cmd)
                    .args(args)
                    .output()
                    .await
                    .map(|o| o.status.success())
                    .unwrap_or(false)
            }
            InstallMethod::CurlBinary { url, dest } => {
                match reqwest::get(url).await {
                    Ok(resp) => {
                        if let Ok(bytes) = resp.bytes().await {
                            std::fs::create_dir_all(
                                std::path::Path::new(dest)
                                    .parent()
                                    .unwrap_or_else(|| std::path::Path::new("/tmp")),
                            )
                            .ok();
                            std::fs::write(dest, &bytes).is_ok()
                        } else {
                            false
                        }
                    }
                    Err(_) => false,
                }
            }
        }
    }
}

impl Default for DualInstallClient {
    fn default() -> Self {
        Self::new()
    }
}

// ── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use tokio::sync::Mutex;

    // ── InstallMethod tests ──────────────────────────────────────────────

    #[test]
    fn test_method_display_name_cli() {
        let m = InstallMethod::Cli {
            command: "echo hello".to_string(),
        };
        assert_eq!(m.display_name(), "cli");
    }

    #[test]
    fn test_method_display_name_docker() {
        let m = InstallMethod::Docker {
            image: "ubuntu".to_string(),
            args: vec!["bash".to_string()],
        };
        assert_eq!(m.display_name(), "docker");
    }

    #[test]
    fn test_method_display_name_package() {
        let m = InstallMethod::Package {
            manager: "brew".to_string(),
            package: "go".to_string(),
        };
        assert_eq!(m.display_name(), "package");
    }

    #[test]
    fn test_method_display_name_curl_binary() {
        let m = InstallMethod::CurlBinary {
            url: "https://example.com/go.tar.gz".to_string(),
            dest: "/usr/local/bin/go".to_string(),
        };
        assert_eq!(m.display_name(), "curl-binary");
    }

    #[test]
    fn test_method_description() {
        let m = InstallMethod::Package {
            manager: "apt".to_string(),
            package: "curl".to_string(),
        };
        assert_eq!(m.description(), "apt install curl");
    }

    #[test]
    fn test_method_package_name() {
        let m = InstallMethod::Package {
            manager: "brew".to_string(),
            package: "ripgrep".to_string(),
        };
        assert_eq!(m.package_name(), "ripgrep");

        let m = InstallMethod::Docker {
            image: "nginx:latest".to_string(),
            args: vec![],
        };
        assert_eq!(m.package_name(), "nginx:latest");
    }

    // ── InstallTarget tests ──────────────────────────────────────────────

    #[test]
    fn test_target_display_name() {
        assert_eq!(InstallTarget::Preferred.display_name(), "preferred");
        assert_eq!(InstallTarget::All.display_name(), "all");
    }

    #[test]
    fn test_target_from_flag_absent() {
        assert_eq!(
            InstallTarget::from_flag(None).unwrap(),
            InstallTarget::Preferred
        );
    }

    #[test]
    fn test_target_from_flag_preferred() {
        assert_eq!(
            InstallTarget::from_flag(Some("preferred")).unwrap(),
            InstallTarget::Preferred
        );
    }

    #[test]
    fn test_target_from_flag_all() {
        assert_eq!(
            InstallTarget::from_flag(Some("all")).unwrap(),
            InstallTarget::All
        );
    }

    #[test]
    fn test_target_from_flag_invalid() {
        assert!(InstallTarget::from_flag(Some("docker-only")).is_err());
    }

    // ── InstallResult tests ──────────────────────────────────────────────

    #[test]
    fn test_install_result_success() {
        let r = InstallResult::success("cli", 42);
        assert!(r.success);
        assert_eq!(r.method_used, "cli");
        assert_eq!(r.elapsed_ms, 42);
        assert!(r.warnings.is_empty());
    }

    #[test]
    fn test_install_result_failure() {
        let r = InstallResult::failure(
            vec!["docker failed".to_string(), "cli failed".to_string()],
            100,
        );
        assert!(!r.success);
        assert!(r.method_used.is_empty());
        assert_eq!(r.elapsed_ms, 100);
        assert_eq!(r.warnings.len(), 2);
    }

    #[test]
    fn test_install_result_serde_roundtrip() {
        let r = InstallResult::success("docker", 150);
        let json = serde_json::to_string(&r).unwrap();
        let restored: InstallResult = serde_json::from_str(&json).unwrap();
        assert_eq!(restored.method_used, "docker");
        assert!(restored.success);
        assert_eq!(restored.elapsed_ms, 150);
    }

    // ── InstallMethod serde tests ────────────────────────────────────────

    #[test]
    fn test_install_method_serde_all_variants() {
        // Test Cli
        let m = InstallMethod::Cli { command: "echo test".to_string() };
        let json = serde_json::to_string(&m).unwrap();
        let restored: InstallMethod = serde_json::from_str(&json).unwrap();
        assert_eq!(m, restored);

        // Test Package
        let m = InstallMethod::Package {
            manager: "apt".to_string(),
            package: "curl".to_string(),
        };
        let json = serde_json::to_string(&m).unwrap();
        let restored: InstallMethod = serde_json::from_str(&json).unwrap();
        assert_eq!(m, restored);

        // Test CurlBinary
        let m = InstallMethod::CurlBinary {
            url: "https://golang.org/dl/go".to_string(),
            dest: "/usr/local/bin/go".to_string(),
        };
        let json = serde_json::to_string(&m).unwrap();
        let restored: InstallMethod = serde_json::from_str(&json).unwrap();
        assert_eq!(m, restored);
    }

    #[test]
    fn test_install_target_serde_roundtrip() {
        let t = InstallTarget::All;
        let json = serde_json::to_string(&t).unwrap();
        let restored: InstallTarget = serde_json::from_str(&json).unwrap();
        assert_eq!(t, restored);
    }

    // ── DualInstallClient tests ──────────────────────────────────────────

    #[test]
    fn test_new_does_not_panic() {
        let _ = DualInstallClient::new();
    }

    #[tokio::test]
    async fn test_empty_methods_fails() {
        let client = DualInstallClient::new();
        let result = client.install(vec![], InstallTarget::All).await.unwrap();
        assert!(!result.success);
        assert_eq!(result.warnings.len(), 1);
        assert_eq!(result.warnings[0], "No install methods provided");
    }

    #[tokio::test]
    async fn test_single_method_success() {
        let client = DualInstallClient::new();
        let methods = vec![InstallMethod::Cli {
            command: "echo hello".to_string(),
        }];
        let result = client
            .install(methods, InstallTarget::Preferred)
            .await
            .unwrap();
        assert!(result.success);
        assert_eq!(result.method_used, "cli");
        assert!(result.warnings.is_empty());
    }

    #[tokio::test]
    async fn test_single_method_failure() {
        // Test failure path by using a mock executor that always fails
        let client = DualInstallClient::test_with(Arc::new(|_m| {
            tokio::spawn(async { false })
        }));

        let methods = vec![InstallMethod::Cli {
            command: "false".to_string(),
        }];
        let result = client
            .install(methods, InstallTarget::Preferred)
            .await
            .unwrap();

        assert!(!result.success);
        assert_eq!(result.method_used, "");
        assert_eq!(result.warnings.len(), 1);
        assert_eq!(result.warnings[0], "cli failed");
    }

    #[tokio::test]
    async fn test_all_mode_first_success() {
        let client = DualInstallClient::new();
        let methods = vec![
            InstallMethod::Cli {
                command: "echo hi".to_string(),
            },
            InstallMethod::Package {
                manager: "brew".to_string(),
                package: "go".to_string(),
            },
        ];
        let result = client.install(methods, InstallTarget::All).await.unwrap();
        assert!(result.success);
        assert!(!result.method_used.is_empty());
        assert!(result.warnings.is_empty());
    }

    #[tokio::test]
    async fn test_all_mode_all_fail() {
        // Test that when ALL methods fail, we get a failure result
        let call_count = Arc::new(AtomicUsize::new(0));

        let client = DualInstallClient::test_with({
            let count = Arc::clone(&call_count);
            Arc::new(move |_m| {
                let count = Arc::clone(&count);
                tokio::spawn(async move {
                    count.fetch_add(1, Ordering::SeqCst);
                    false
                })
            })
        });

        let methods = vec![
            InstallMethod::Cli { command: "false".to_string() },
            InstallMethod::Docker {
                image: "notexist/broken".to_string(),
                args: vec![],
            },
            InstallMethod::Package {
                manager: "fake-pkg".to_string(),
                package: "nope".to_string(),
            },
        ];

        let result = client.install(methods, InstallTarget::All).await.unwrap();

        assert!(!result.success);
        assert_eq!(result.method_used, "");
        assert_eq!(result.warnings.len(), 3);
        // All three methods should have been attempted
        assert_eq!(call_count.load(Ordering::SeqCst), 3);
    }

    #[tokio::test]
    async fn test_all_mode_partial_success() {
        // Test that only one success is needed even when others fail
        let call_order = Arc::new(Mutex::new(Vec::new()));

        let client = DualInstallClient::test_with({
            let order = Arc::clone(&call_order);
            Arc::new(move |m| {
                let order = Arc::clone(&order);
                let name = m.display_name().to_string();
                tokio::spawn(async move {
                    let is_docker = name == "docker";
                    order.lock().await.push(name);
                    is_docker // Only docker succeeds
                })
            })
        });

        let methods = vec![
            InstallMethod::Cli { command: "false".to_string() },
            InstallMethod::Docker {
                image: "working/image".to_string(),
                args: vec![],
            },
            InstallMethod::Package {
                manager: "broken".to_string(),
                package: "fail".to_string(),
            },
        ];

        let result = client.install(methods, InstallTarget::All).await.unwrap();

        assert!(result.success);
        assert_eq!(result.method_used, "docker");
        // Cli and Package failed; docker succeeded
        let order = call_order.lock().await;
        assert!(order.contains(&"cli".to_string()));
        assert!(order.contains(&"docker".to_string()));
        assert!(order.contains(&"package".to_string()));
    }

    #[tokio::test]
    async fn test_default_impl() {
        let client = DualInstallClient::default();
        let result = client
            .install(
                vec![InstallMethod::Cli {
                    command: "true".to_string(),
                }],
                InstallTarget::Preferred,
            )
            .await
            .unwrap();
        assert!(result.success);
    }

    #[tokio::test]
    async fn test_all_mode_multiple_methods() {
        let exec_count = Arc::new(AtomicUsize::new(0));
        let client = DualInstallClient::test_with({
            let count = Arc::clone(&exec_count);
            Arc::new(move |_m| {
                let count = Arc::clone(&count);
                tokio::spawn(async move {
                    count.fetch_add(1, Ordering::SeqCst);
                    true // All succeed
                })
            })
        });
        let methods = vec![
            InstallMethod::Cli {
                command: "true".to_string(),
            },
            InstallMethod::Docker {
                image: "ubuntu".to_string(),
                args: vec!["echo".to_string(), "hello".to_string()],
            },
            InstallMethod::Package {
                manager: "apt".to_string(),
                package: "curl".to_string(),
            },
            InstallMethod::CurlBinary {
                url: "https://example.com/tool".to_string(),
                dest: "/tmp/tool".to_string(),
            },
        ];
        let result = client.install(methods, InstallTarget::All).await.unwrap();
        assert!(result.success);
        assert!(!result.method_used.is_empty());
        // Due to parallel execution, exact winner is non-deterministic
        assert!(result.warnings.is_empty());
        // All methods were started (though only first success matters)
        assert!(exec_count.load(Ordering::SeqCst) >= 1);
    }
}
