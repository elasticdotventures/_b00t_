//! b00t-lsp-proxy: Abstract LSP wrapper for multiple tools
//!
//! 🤓 Proxies existing LSP servers (just-mcp, datum-lsp, etc.)
//! with unified interface for:
//! - IaC serialization with variable pipelines
//! - Feature-flagged .tomllm/.tomllmd LSP (macro-generated from AST)
//! - Dynamic option population via jpath/tomllm-path
//! - SLO/SLI budget enforcement (time/cost)
//! - Sandbox lifetime management
//! - Signal pattern detection (activity proxy)
//! - WASM compilation for third-party tools

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::process::Stdio;
use tokio::process::{Child, Command};

/// LSP Proxy configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LspProxyConfig {
    /// Name of proxied tool (e.g., "just-mcp", "datum-lsp")
    pub tool_name: String,
    /// Command to spawn the LSP server
    pub command: String,
    /// Command arguments
    pub args: Vec<String>,
    /// Feature flags (e.g., "tomllm-ast", "tomllmd-diagram-dsl", "slo-enforcement")
    pub features: Vec<String>,
    /// SLO/SLI budgets
    pub budgets: SloBudgets,
    /// WASM compilation settings
    pub wasm: WasmConfig,
}

/// SLO/SLI Budget configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SloBudgets {
    /// Time budget in seconds (max sandbox lifetime)
    pub time_seconds: u64,
    /// Cost budget in cents (API calls, compute)
    pub cost_cents: u64,
    /// No-output timeout (kill if silent too long)
    pub no_output_timeout_secs: u64,
    /// Signal change threshold (min changes/sec to stay alive)
    pub signal_changes_per_sec: f64,
}

impl Default for SloBudgets {
    fn default() -> Self {
        Self {
            time_seconds: 3600,          // 1 hour default
            cost_cents: 1000,            // $10 default
            no_output_timeout_secs: 300, // 5 minutes
            signal_changes_per_sec: 0.1, // 1 change per 10 seconds
        }
    }
}

/// WASM compilation configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WasmConfig {
    /// Enable WASM compilation
    pub enabled: bool,
    /// Target WASM platform (wasm32-wasi, wasm32-unknown-unknown)
    pub target: String,
    /// Optimization level (0, 1, 2, 3, s, z)
    pub opt_level: String,
    /// Third-party crates to compile
    pub crates: Vec<String>,
}

impl Default for WasmConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            target: "wasm32-wasi".to_string(),
            opt_level: "z".to_string(),
            crates: vec![],
        }
    }
}

/// LSP Proxy instance
pub struct LspProxy {
    config: LspProxyConfig,
    process: Option<Child>,
    signal_history: Vec<u64>,
    last_output_time: std::time::Instant,
    cost_tracker: CostTracker,
    start_time: std::time::Instant,
}

/// Cost tracker for SLO enforcement
#[derive(Debug, Default)]
pub struct CostTracker {
    api_calls: u64,
    compute_seconds: u64,
    estimated_cents: u64,
}

impl LspProxy {
    pub fn new(config: LspProxyConfig) -> Self {
        Self {
            config,
            process: None,
            signal_history: Vec::new(),
            last_output_time: std::time::Instant::now(),
            cost_tracker: CostTracker::default(),
            start_time: std::time::Instant::now(),
        }
    }

    /// Spawn the proxied LSP server
    pub async fn spawn(&mut self) -> Result<()> {
        let mut cmd = Command::new(&self.config.command);
        cmd.args(&self.config.args);
        cmd.stdin(Stdio::piped());
        cmd.stdout(Stdio::piped());
        cmd.stderr(Stdio::piped());

        // 🤓 Feature flags as environment variables
        for feature in &self.config.features {
            cmd.env(format!("B00T_FEATURE_{}", feature.to_uppercase()), "1");
        }

        // 🤓 WASM mode if enabled
        if self.config.wasm.enabled {
            cmd.env("B00T_WASM_ENABLED", "1");
            cmd.env("B00T_WASM_TARGET", &self.config.wasm.target);
        }

        let child = cmd.spawn().context("Failed to spawn LSP proxy")?;
        self.process = Some(child);

        eprintln!("🤖 LSP proxy spawned: {}", self.config.tool_name);
        Ok(())
    }

    /// Check if sandbox should be terminated (SLO/SLI violation)
    pub fn should_terminate(&self) -> Option<TerminationReason> {
        let elapsed = self.start_time.elapsed();

        // Time budget exceeded
        if elapsed.as_secs() > self.config.budgets.time_seconds {
            return Some(TerminationReason::TimeBudgetExceeded);
        }

        // Cost budget exceeded
        if self.cost_tracker.estimated_cents > self.config.budgets.cost_cents {
            return Some(TerminationReason::CostBudgetExceeded);
        }

        // No output timeout
        if self.last_output_time.elapsed().as_secs() > self.config.budgets.no_output_timeout_secs {
            return Some(TerminationReason::NoOutputTimeout);
        }

        // Signal pattern detection (too little activity)
        if self.signal_history.len() > 10 {
            let elapsed_secs = elapsed.as_secs_f64();
            if elapsed_secs > 0.0 {
                let recent_rate = self.signal_history.len() as f64 / elapsed_secs;
                if recent_rate < self.config.budgets.signal_changes_per_sec {
                    return Some(TerminationReason::LowActivity);
                }
            }
        }

        None
    }

    /// Record signal change (for activity detection)
    pub fn record_signal(&mut self, change_hash: u64) {
        self.signal_history.push(change_hash);
        // Keep last 100 signals
        if self.signal_history.len() > 100 {
            self.signal_history.remove(0);
        }
        self.last_output_time = std::time::Instant::now();
    }

    /// Record API call for cost tracking
    pub fn record_api_call(&mut self, estimated_cost_cents: u64) {
        self.cost_tracker.api_calls += 1;
        self.cost_tracker.estimated_cents += estimated_cost_cents;
    }

    /// Record compute time for cost tracking
    pub fn record_compute(&mut self, seconds: u64, cost_per_sec_cents: u64) {
        self.cost_tracker.compute_seconds += seconds;
        self.cost_tracker.estimated_cents += seconds * cost_per_sec_cents;
    }

    /// Get remaining budget
    pub fn remaining_budget(&self) -> BudgetStatus {
        let elapsed = self.start_time.elapsed();
        BudgetStatus {
            time_remaining_secs: self
                .config
                .budgets
                .time_seconds
                .saturating_sub(elapsed.as_secs()),
            cost_remaining_cents: self
                .config
                .budgets
                .cost_cents
                .saturating_sub(self.cost_tracker.estimated_cents),
            no_output_remaining_secs: self
                .config
                .budgets
                .no_output_timeout_secs
                .saturating_sub(self.last_output_time.elapsed().as_secs()),
        }
    }

    /// Terminate the sandbox
    pub async fn terminate(&mut self) -> Result<()> {
        if let Some(mut child) = self.process.take() {
            child.kill().await.context("Failed to kill LSP process")?;
            eprintln!("🛑 LSP proxy terminated");
        }
        Ok(())
    }
}

/// Termination reason
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TerminationReason {
    TimeBudgetExceeded,
    CostBudgetExceeded,
    NoOutputTimeout,
    LowActivity,
    Manual,
}

/// Budget status
#[derive(Debug, Clone)]
pub struct BudgetStatus {
    pub time_remaining_secs: u64,
    pub cost_remaining_cents: u64,
    pub no_output_remaining_secs: u64,
}

/// TOMLLM LSP configuration (feature-flagged)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TomllmLspConfig {
    /// Enable .tomllm / downgraded .tomllmd LSP
    pub enabled: bool,
    /// Generate LSP from TOMLLM AST
    pub generate_from_ast: bool,
    /// JPath expressions for dynamic options
    pub jpath_expressions: HashMap<String, String>,
    /// Feature flags for macro introspection
    pub feature_flags: Vec<String>,
}

impl Default for TomllmLspConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            generate_from_ast: true,
            jpath_expressions: HashMap::new(),
            feature_flags: vec!["tomllm-ast".to_string()],
        }
    }
}

/// IaC Serialization for instantiation recipes
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IacRecipe {
    /// Recipe name
    pub name: String,
    /// Variable pipeline (functional composition)
    pub variables: Vec<VariableTransform>,
    /// Target infrastructure (docker, k8s, podman)
    pub target: String,
    /// Generated code (serialized)
    pub serialized: String,
}

/// Variable transformation in pipeline
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VariableTransform {
    /// Variable name
    pub name: String,
    /// Transformation function
    pub transform: String,
    /// Input from previous step
    pub input: Option<String>,
    /// Output to next step
    pub output: String,
}

/// Compile third-party crate to WASM
pub async fn compile_to_wasm(crate_name: &str, target: &str, opt_level: &str) -> Result<String> {
    eprintln!(
        "🔧 Compiling {} to WASM ({}, opt={})",
        crate_name, target, opt_level
    );

    // 🤓 This would invoke cargo build --target <target> --release
    // For now, placeholder for actual WASM compilation logic
    let wasm_path = format!("/tmp/{}.wasm", crate_name.replace('-', "_"));

    // Simulate compilation
    tokio::time::sleep(std::time::Duration::from_millis(100)).await;

    Ok(wasm_path)
}

/// Generate LSP from TOMLLM AST (macro introspection)
pub fn generate_lsp_from_ast(_tomllm_content: &str, features: &[String]) -> Result<LspProxyConfig> {
    // 🤓 Parse TOMLLM, extract AST, generate LSP handlers
    // This is where feature flags control which handlers are generated

    let mut args = vec!["--stdio".to_string()];

    // Feature-flagged handlers
    if features.contains(&"tomllm-ast".to_string()) {
        args.push("--tomllm-ast".to_string());
    }

    Ok(LspProxyConfig {
        tool_name: "tomllm-lsp".to_string(),
        command: "b00t-lsp".to_string(),
        args,
        features: features.to_vec(),
        budgets: SloBudgets::default(),
        wasm: WasmConfig::default(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_budget_tracking() {
        let config = LspProxyConfig {
            tool_name: "test".to_string(),
            command: "echo".to_string(),
            args: vec![],
            features: vec![],
            budgets: SloBudgets {
                time_seconds: 60,
                cost_cents: 100,
                no_output_timeout_secs: 10,
                signal_changes_per_sec: 0.5,
            },
            wasm: WasmConfig::default(),
        };

        let mut proxy = LspProxy::new(config);
        proxy.record_api_call(50);
        proxy.record_compute(10, 3);

        let status = proxy.remaining_budget();
        assert!(status.cost_remaining_cents < 100);
        assert!(status.time_remaining_secs <= 60);
    }

    #[test]
    fn test_signal_detection() {
        let config = LspProxyConfig {
            tool_name: "test".to_string(),
            command: "echo".to_string(),
            args: vec![],
            features: vec![],
            budgets: SloBudgets::default(),
            wasm: WasmConfig::default(),
        };

        let mut proxy = LspProxy::new(config);

        // Record some signals
        for i in 0..20 {
            proxy.record_signal(i);
        }

        // Should have activity
        assert!(proxy.signal_history.len() > 0);
    }
}
