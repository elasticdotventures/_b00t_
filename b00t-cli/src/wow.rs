//! WOW — Way of Working invariant checks.
//!
//! Each check implements one of five category traits and produces a `CheckResult`.
//! Checks are registered via `wow_check!` macro, which generates both a unit test
//! and a documentation example.
//!
//! # Usage
//! ```rust
//! use crate::wow::{BuildIntegrityCheck, CheckResult};
//!
//! struct CandleBuildCheck;
//! impl BuildIntegrityCheck for CandleBuildCheck {
//!     fn name(&self) -> &str { "candle feature compiles" }
//!     fn run(&self) -> CheckResult { /* ... */ }
//! }
//! ```

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

// ─── CheckResult — the spline primitive ──────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CheckResult {
    pub name: String,
    pub category: CheckCategory,
    pub passed: bool,
    pub detail: String,
}

/// MECE categories — mutually exclusive, collectively exhaustive.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum CheckCategory {
    BuildIntegrity,
    TypeInvariant,
    Boundary,
    Deployment,
    DesignHeuristic,
    SessionHygiene,
}

impl std::fmt::Display for CheckCategory {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::BuildIntegrity => write!(f, "A:BuildIntegrity"),
            Self::TypeInvariant => write!(f, "B:TypeInvariant"),
            Self::Boundary => write!(f, "C:Boundary"),
            Self::Deployment => write!(f, "D:Deployment"),
            Self::DesignHeuristic => write!(f, "E:DesignHeuristic"),
            Self::SessionHygiene => write!(f, "F:SessionHygiene"),
        }
    }
}

// ─── Category traits ────────────────────────────────────────────────────────

/// Resolve a path relative to the project root (CARGO_MANIFEST_DIR or cwd).
#[allow(dead_code)]
fn project_path(path: &str) -> std::path::PathBuf {
    let p = std::path::Path::new(path);
    if p.exists() { return p.to_path_buf(); }
    if let Ok(manifest) = std::env::var("CARGO_MANIFEST_DIR") {
        for candidate in &[
            std::path::Path::new(&manifest).join("..").join(path),
            std::path::Path::new(&manifest).join("..").join("..").join(path),
        ] {
            if candidate.exists() { return candidate.to_path_buf(); }
        }
    }
    p.to_path_buf()
}

/// A check that verifies the build is healthy before proceeding.
pub trait BuildIntegrityCheck: Send + Sync {
    fn name(&self) -> &str;
    fn run(&self) -> CheckResult;
}

/// A check that verifies type invariants haven't been violated.
pub trait TypeInvariantCheck: Send + Sync {
    fn name(&self) -> &str;
    fn run(&self) -> CheckResult;
}

/// A check that verifies file/system boundary ownership.
pub trait BoundaryCheck: Send + Sync {
    fn name(&self) -> &str;
    fn run(&self) -> CheckResult;
}

/// A check that verifies deployment topology (dual runtime, polyseme).
pub trait DeploymentCheck: Send + Sync {
    fn name(&self) -> &str;
    fn run(&self) -> CheckResult;
}

/// A check that verifies design heuristics (module sizing, command count).
pub trait DesignHeuristicCheck: Send + Sync {
    fn name(&self) -> &str;
    fn run(&self) -> CheckResult;
}

// ─── Concrete check implementations ──────────────────────────────────────────

pub struct CandleBuildCheck;
impl BuildIntegrityCheck for CandleBuildCheck {
    fn name(&self) -> &str { "candle feature compiles" }
    fn run(&self) -> CheckResult {
        let status = std::process::Command::new("cargo")
            .args(["check", "--features", "candle", "-p", "b00t-cli"])
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .status();
        match status {
            Ok(s) if s.success() => CheckResult {
                name: self.name().into(), category: CheckCategory::BuildIntegrity,
                passed: true, detail: "cargo check --features candle succeeded".into(),
            },
            Ok(s) => CheckResult {
                name: self.name().into(), category: CheckCategory::BuildIntegrity,
                passed: false, detail: format!("exit code {:?}", s.code()),
            },
            Err(e) => CheckResult {
                name: self.name().into(), category: CheckCategory::BuildIntegrity,
                passed: false, detail: format!("spawn failed: {e}"),
            },
        }
    }
}

pub struct DefaultBuildCheck;
impl BuildIntegrityCheck for DefaultBuildCheck {
    fn name(&self) -> &str { "default build compiles" }
    fn run(&self) -> CheckResult {
        let status = std::process::Command::new("cargo")
            .args(["check", "-p", "b00t-cli"])
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .status();
        match status {
            Ok(s) if s.success() => CheckResult {
                name: self.name().into(), category: CheckCategory::BuildIntegrity,
                passed: true, detail: "cargo check succeeded".into(),
            },
            Ok(s) => CheckResult {
                name: self.name().into(), category: CheckCategory::BuildIntegrity,
                passed: false, detail: format!("exit code {:?}", s.code()),
            },
            Err(e) => CheckResult {
                name: self.name().into(), category: CheckCategory::BuildIntegrity,
                passed: false, detail: format!("spawn failed: {e}"),
            },
        }
    }
}

pub struct KnownRoleCheck;
impl TypeInvariantCheck for KnownRoleCheck {
    fn name(&self) -> &str { "KnownRole enum is exhaustive" }
    fn run(&self) -> CheckResult {
        let path = if let Ok(manifest) = std::env::var("CARGO_MANIFEST_DIR") {
            std::path::Path::new(&manifest).join("..").join("b00t-cli/src/agentic_role.rs")
        } else {
            std::path::PathBuf::from("b00t-cli/src/agentic_role.rs")
        };
        let src = match std::fs::read_to_string(&path) {
            Ok(s) => s, Err(e) => return CheckResult {
                name: self.name().into(), category: CheckCategory::TypeInvariant,
                passed: false, detail: format!("read agentic_role.rs: {e}"),
            },
        };
        let has_worker = src.contains("Worker(RoleRef<Worker>)");
        let has_exec = src.contains("Executive(RoleRef<Executive>)");
        let has_op = src.contains("Operator(RoleRef<Operator>)");
        let has_provider = src.contains("AppProvider(RoleRef<AppProvider>)");
        let all = has_worker && has_exec && has_op && has_provider;
        CheckResult {
            name: self.name().into(), category: CheckCategory::TypeInvariant,
            passed: all,
            detail: if all { "4 variants present".into() }
                    else { format!("worker={has_worker} exec={has_exec} op={has_op} provider={has_provider}") },
        }
    }
}

pub struct VendorDockerfileCheck;
impl BoundaryCheck for VendorDockerfileCheck {
    fn name(&self) -> &str { "vendor dockerfile exists" }
    fn run(&self) -> CheckResult {
        let path = if let Ok(manifest) = std::env::var("CARGO_MANIFEST_DIR") {
            std::path::Path::new(&manifest).join("..").join("vendor/l3dg3rr/Dockerfile.ledgerr-mcp")
        } else {
            std::path::PathBuf::from("vendor/l3dg3rr/Dockerfile.ledgerr-mcp")
        };
        let exists = path.exists();
        CheckResult {
            name: self.name().into(), category: CheckCategory::Boundary,
            passed: exists,
            detail: if exists { "Dockerfile.ledgerr-mcp present".into() }
                    else { "vendor/l3dg3rr/Dockerfile.ledgerr-mcp not found".into() },
        }
    }
}

pub struct DualRuntimeCheck;
impl DeploymentCheck for DualRuntimeCheck {
    fn name(&self) -> &str { "dual runtime datum" }
    fn run(&self) -> CheckResult {
        let path = if let Ok(manifest) = std::env::var("CARGO_MANIFEST_DIR") {
            std::path::Path::new(&manifest).join("..").join("_b00t_/ledgrrr.cli.toml")
        } else {
            std::path::PathBuf::from("_b00t_/ledgrrr.cli.toml")
        };
        let datum = match std::fs::read_to_string(&path) {
            Ok(s) => s, Err(e) => return CheckResult {
                name: self.name().into(), category: CheckCategory::Deployment,
                passed: false, detail: format!("read datum: {e}"),
            },
        };
        let has_section = datum.contains("[b00t.providers.runtime]");
        let has_detect = datum.contains("detect_order");
        CheckResult {
            name: self.name().into(), category: CheckCategory::Deployment,
            passed: has_section && has_detect,
            detail: if has_section && has_detect { "detect_order declared".into() }
                    else { "missing [b00t.providers.runtime] or detect_order".into() },
        }
    }
}

pub struct JustModulesCheck;
impl DesignHeuristicCheck for JustModulesCheck {
    fn name(&self) -> &str { "justfile modules exist" }
    fn run(&self) -> CheckResult {
        // CARGO_MANIFEST_DIR/../justfile when running from tests
        let path = if let Ok(manifest) = std::env::var("CARGO_MANIFEST_DIR") {
            std::path::Path::new(&manifest).join("..").join("justfile")
        } else {
            std::path::PathBuf::from("justfile")
        };
        let just = match std::fs::read_to_string(&path) {
            Ok(s) => s, Err(e) => return CheckResult {
                name: self.name().into(), category: CheckCategory::DesignHeuristic,
                passed: false, detail: format!("read justfile: {e}"),
            },
        };
        let all = just.contains("mod b00t") && just.contains("mod ledgrrr") && just.contains("mod irontology");
        CheckResult {
            name: self.name().into(), category: CheckCategory::DesignHeuristic,
            passed: all,
            detail: if all { "b00t, ledgrrr, irontology modules declared".into() }
                    else { "missing one or more module declarations".into() },
        }
    }
}

// ─── Registry ────────────────────────────────────────────────────────────────

/// A registered WOW check — wraps any category trait into a uniform callable.
pub struct WowCheck {
    pub name: String,
    pub category: CheckCategory,
    runner: Box<dyn Fn() -> CheckResult + Send + Sync>,
}

impl WowCheck {
    pub fn run(&self) -> CheckResult { (self.runner)() }
}

static WOW_REGISTRY: std::sync::OnceLock<std::sync::Mutex<Vec<WowCheck>>> =
    std::sync::OnceLock::new();

fn registry() -> &'static std::sync::Mutex<Vec<WowCheck>> {
    WOW_REGISTRY.get_or_init(|| std::sync::Mutex::new(Vec::new()))
}

/// Register a build integrity check.
pub fn register_build<C: BuildIntegrityCheck + 'static>(check: C) {
    let name = check.name().to_string();
    if let Ok(mut reg) = registry().lock() {
        reg.push(WowCheck {
            name: format!("A: {name}"),
            category: CheckCategory::BuildIntegrity,
            runner: Box::new(move || check.run()),
        });
    }
}

/// Register a type invariant check.
pub fn register_type<C: TypeInvariantCheck + 'static>(check: C) {
    let name = check.name().to_string();
    if let Ok(mut reg) = registry().lock() {
        reg.push(WowCheck {
            name: format!("B: {name}"),
            category: CheckCategory::TypeInvariant,
            runner: Box::new(move || check.run()),
        });
    }
}

/// Register a boundary check.
pub fn register_boundary<C: BoundaryCheck + 'static>(check: C) {
    let name = check.name().to_string();
    if let Ok(mut reg) = registry().lock() {
        reg.push(WowCheck {
            name: format!("C: {name}"),
            category: CheckCategory::Boundary,
            runner: Box::new(move || check.run()),
        });
    }
}

/// Register a deployment check.
pub fn register_deployment<C: DeploymentCheck + 'static>(check: C) {
    let name = check.name().to_string();
    if let Ok(mut reg) = registry().lock() {
        reg.push(WowCheck {
            name: format!("D: {name}"),
            category: CheckCategory::Deployment,
            runner: Box::new(move || check.run()),
        });
    }
}

/// Register a design heuristic check.
pub fn register_design<C: DesignHeuristicCheck + 'static>(check: C) {
    let name = check.name().to_string();
    if let Ok(mut reg) = registry().lock() {
        reg.push(WowCheck {
            name: format!("E: {name}"),
            category: CheckCategory::DesignHeuristic,
            runner: Box::new(move || check.run()),
        });
    }
}

/// Run all registered checks, return results.
pub fn run_all() -> Vec<CheckResult> {
    registry().lock().map(|reg| reg.iter().map(|c| c.run()).collect()).unwrap_or_default()
}

/// Generate a spline report string from check results.
pub fn format_spline(results: &[CheckResult]) -> String {
    let total = results.len();
    let passed = results.iter().filter(|r| r.passed).count();
    let mut out = format!("\nWOW integrity spline: {passed}/{total} passed\n");
    out.push_str(&"─".repeat(42));
    out.push('\n');
    for r in results {
        let mark = if r.passed { "✅" } else { "❌" };
        out.push_str(&format!(" {mark} [{}] {}\n", r.category, r.name));
        if !r.passed {
            out.push_str(&format!("      ↳ {}\n", r.detail));
        }
    }
    out.push_str(&"─".repeat(42));
    out.push('\n');
    out
}

/// Initialize default checks. Called at startup.
pub fn init_default_checks() {
    register_build(CandleBuildCheck);
    register_build(DefaultBuildCheck);
    register_type(KnownRoleCheck);
    register_boundary(VendorDockerfileCheck);
    register_deployment(DualRuntimeCheck);
    register_design(JustModulesCheck);
}

// ─── wow_test! macro ─────────────────────────────────────────────────────────

/// Generate a test + doc example from a WOW check struct.
///
/// ```rust
/// wow_test!(CandleBuildCheck, BuildIntegrityCheck, "candle feature compiles");
/// ```
///
/// Expands to:
/// - A `#[test]` that instantiates and runs the check
/// - A doc example showing the check name and expected output
#[macro_export]
macro_rules! wow_test {
    ($check_name:ident, $struct:ident, $trait:ident, $desc:expr) => {
        #[doc = concat!("WOW check: ", $desc)]
        #[test]
        fn $check_name() {
            let check_obj = $struct;
            let result = $trait::run(&check_obj);
            assert!(result.passed, "WOW check '{}' [{}] failed: {}", result.name, result.category, result.detail);
        }
    };
}

#[cfg(test)]
mod tests {
    use super::*;

    // Register checks and run spline
    fn setup() {
        // Only register once (registry is static)
        if registry().lock().map(|r| r.is_empty()).unwrap_or(false) {
            init_default_checks();
        }
    }

    #[test]
    fn test_wow_spline_all_pass() {
        setup();
        let results = run_all();
        let passed = results.iter().filter(|r| r.passed).count();
        let total = results.len();
        let spline = format_spline(&results);
        println!("{spline}");
        assert_eq!(passed, total, "all WOW checks must pass:\n{spline}");
    }

    // Individual wow_test! invocations — each generates a #[test] + doc example
    wow_test!(test_candle_build, CandleBuildCheck, BuildIntegrityCheck, "candle feature compiles");
    wow_test!(test_default_build, DefaultBuildCheck, BuildIntegrityCheck, "default build compiles");
    wow_test!(test_known_role, KnownRoleCheck, TypeInvariantCheck, "KnownRole enum is exhaustive");
    wow_test!(test_vendor_dockerfile, VendorDockerfileCheck, BoundaryCheck, "vendor dockerfile exists");
    wow_test!(test_dual_runtime, DualRuntimeCheck, DeploymentCheck, "dual runtime datum");
    wow_test!(test_just_modules, JustModulesCheck, DesignHeuristicCheck, "justfile modules exist");
}
