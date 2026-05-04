# SDD-004: b00t Install Dual-Method Pattern

> **Status:** IMPLEMENTED | **Confidence:** 95% | **Iteration:** 1
> **Stage Gates:** 5/5 passed | **Last Updated:** 2026-05-03

---

## 1. Problem Statement

`b00t install <tool>` failures are hard errors when the configured install method fails, even if alternative installation methods would succeed. Users experience non-resilient installs when:
- Primary package manager is unavailable or broken
- Network issues affect one method but not others
- Platform-specific methods fail on cross-platform tools

**Impact:** Poor developer experience — users must manually try alternative install methods or debug failures that could be auto-retried.

**Solution:** Implement a dual-method (fan-out) install pattern that:
1. Tries multiple install methods in parallel: `[cli, docker, package]`
2. First success wins — remaining attempts are cancelled
3. Failures are collected as warnings, not errors
4. Provides resilient, self-healing installation

---

## 2. Questions

### ✅ Resolved (increases confidence +5% each)
- Q: How should methods be executed? → A: `tokio::spawn` for parallel fan-out
- Q: What install methods to support initially? → A: Cli, Docker, Package, CurlBinary
- Q: Should failures be hard errors? → A: No — warnings only, first success wins
- Q: Should there be a single-method mode? → A: Yes — `InstallTarget::Preferred` for backward compat
- Q: How to report results? → A: `InstallResult` struct with method_used, warnings, elapsed_ms
- Q: How to cancel remaining attempts? → A: `CancellationToken` signals abort on first success

### ❓ Unresolved
- Q: Should per-method timeouts be configurable? (deferred — caller wraps with tokio::time::timeout)

---

## 3. Specification

### 3.1 Interface Contract
```
Input:  Vec<InstallMethod> + InstallTarget enum
Processing:
  - Preferred  → execute single method, return success/failure
  - All        → fan-out tokio::spawn all methods, first success wins
  - CancellationToken cancels remaining tasks on first success
  - Each method execution measured with Instant for elapsed_ms
Output: InstallResult { method_used, success, warnings, elapsed_ms }
Edge Cases:
  - Empty method list → returns failure with warning
  - All methods fail  → returns failure with all warnings collected
  - Timeout per method → caller wraps with tokio::time::timeout
```

### 3.2 Install Methods
```
InstallMethod enum variants:
  Cli { command: String }           — TOML datum install command (shell script)
  Docker { image, args }            — docker run --rm <image> [args]
  Package { manager, package }      — apt/brew/pip/cargo install <package>
  CurlBinary { url, dest }          — curl <url> → <dest>
```

### 3.3 Rust Pseudo-Code: InstallResult Enum

```rust
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

/// How to execute install methods
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InstallTarget {
    /// Only try the first (preferred) method
    Preferred,
    /// Fan-out all methods, first success wins (others cancelled)
    All,
}

/// A single install method that can be attempted
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InstallMethod {
    /// TOML datum install command (shell script)
    Cli { command: String },
    /// Docker-based install
    Docker { image: String, args: Vec<String> },
    /// Package manager install
    Package { manager: String, package: String },
    /// Direct binary download
    CurlBinary { url: String, dest: String },
}
```

### 3.4 Fan-Out Logic (Pseudo-Code)

```rust
async fn install_all(&self, methods: Vec<InstallMethod>) -> Result<InstallResult> {
    let cancel = CancellationToken::new();
    let (tx, mut rx) = tokio::sync::mpsc::channel(count);
    let mut warnings = Vec::new();

    // Spawn all methods in parallel
    for method in &methods {
        let cancel_child = cancel.clone();
        tokio::spawn(async move {
            if cancel_child.is_cancelled() { return; }  // Early exit
            
            let ok = execute_method(&method).await;
            tx.send((method, ok)).await;
        });
    }

    // First success wins
    while let Some((method, ok)) = rx.recv().await {
        if ok {
            cancel.cancel();  // Cancel remaining tasks
            return Ok(InstallResult::success(method.display_name()));
        } else {
            warnings.push(format!("{} failed", method.display_name()));
        }
    }

    // All failed
    Ok(InstallResult::failure(warnings))
}
```

### 3.5 Integration Points

| Existing Component | How It's Used | Modification Needed |
|---|---|---|
| `b00t.cli.toml` datum `install` field | Parsed into `InstallMethod::Cli` | None |
| `DualInstallClient` | Library struct in b00t-c0re-lib | Add to re-exports |
| `b00t-cli install` command | Calls `DualInstallClient::install()` | Add `--install=all` flag |

### 3.6 Fallback Chain
```
InstallTarget::All fan-out:
  tokio::spawn(cli_task)     → tries TOML datum install command
  tokio::spawn(docker_task)  → tries docker run
  tokio::spawn(pkg_task)     → tries apt/dnf/brew/pkg

  First Ok(()) → InstallResult{success: true, method_used: "cli|docker|package"}
  All Err      → InstallResult{success: false, warnings: [all errors]}
```

### 3.7 Termination Conditions
- **Success:** First method returns Ok → short-circuit, cancel others via CancellationToken
- **Total failure:** All methods return Err → return failure with all warnings
- **Empty input:** No methods provided → immediate failure warning
- **Timeout:** Individual methods may hang — caller wraps with `tokio::time::timeout`

### 3.8 Debug Levels

| --debug N | Output |
|---|---|
| 0 | Off — no output |
| 1 | lifecycle — "Starting fan-out for N methods", "X succeeded" |
| 2 | verbose — per-method start/complete/fail with elapsed times |
| 3 | trace/rotel — raw command output, spawn details, channel debug |

### 3.9 Confidence Tracker

| Iteration | Change | Confidence | Rationale |
|---|---|---|---|
| 1 | Initial spec + impl + tests | 95% | Pattern proven, tests pass, implementation verified |

---

## 4. Stage Gates

### Gate 1: SDD Written ✅
- [x] Problem statement documented
- [x] Interface contract defined
- [x] Rust pseudo-code for InstallResult enum
- [x] Integration points mapped

### Gate 2: Rust Types Implemented ✅
- [x] `InstallMethod` enum with all 4 variants
- [x] `InstallTarget` enum (Preferred | All)
- [x] `InstallResult` struct
- [x] `DualInstallClient` struct with fan-out logic

### Gate 3: Tests Written & Passing ✅
- [x] Unit tests for InstallMethod variants
- [x] Unit tests for InstallTarget parsing
- [x] Integration test: single method success/failure
- [x] Integration test: fan-out first success wins
- [x] Integration test: all methods fail → warnings collected

### Gate 4: Library Integration ✅
- [x] `mod dual_install;` in b00t-c0re-lib/lib.rs
- [x] Re-exports for `DualInstallClient`, `InstallMethod`, `InstallResult`, `InstallTarget`
- [x] `cargo build` succeeds
- [x] `cargo test` passes

### Gate 5: CLI Integration ✅
- [x] `b00t install <tool>` uses DualInstallClient
- [x] `--install=all` flag triggers fan-out mode
- [x] `--install=preferred` (default) uses single-method mode

---

## 5. Retrospective

### Iteration 1
- **Attempted:** Full spec + impl + tests in TDD order
- **Result:** PASS — all tests pass, build succeeds
- **Confidence Change:** +45% → 95%
- **Root Cause (if fail):** N/A
- **Spec Updates:** None — design matched implementation

---

## 6. Implementation Reference

**File:** `b00t-c0re-lib/src/dual_install.rs`

Key types:
- `InstallMethod` — enum for cli/docker/package/curl-binary
- `InstallTarget` — Preferred | All
- `InstallResult` — result struct with method_used, warnings, elapsed_ms
- `DualInstallClient` — fan-out client with test injection support

**Test Injection:**
```rust
// Create a test client where all methods fail
let client = DualInstallClient::test_with(Arc::new(|_m| {
    tokio::spawn(async { false })
}));
let result = client.install(methods, InstallTarget::All).await?;
assert!(!result.success);
```

---

### b00t:map v1
# summary: SDD-004 — Fan-out install pattern: parallel methods, first success wins, warnings not errors
# tags: install, fan-out, resilience, dual-method, cancellation, tokio
# tier: ch0nky
# cmds: b00t install <tool> --install=all, b00t install <tool> --install=preferred
# complexity: 4
# confidence: 95%
