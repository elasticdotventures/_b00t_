//! AAIII: Abstract AI Inference Interface
//!
//! 🤓 Meta-pattern for b00t AI tooling abstraction
//! Provides unified interface for multiple AI inference backends:
//! - Qwen Code CLI
//! - Claude Code
//! - OpenAI Codex
//! - Gemini CLI
//! - Amp
//! - OpenCode
//! - Mistral.rs (local)
//! - Pi (pi-coding-agent via local Gemma 4 / liter-llm gateway)
//!
//! AAIII enables:
//! - Service mesh interface for container sandbox wiring
//! - Pre-authorized .env for FaaS-style execution
//! - Async event-driven registration/dispatch
//! - State machine for tool capability simulation

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::env;
use std::process::Command;

/// AI Inference Backend types
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum AaiiiBackend {
    Qwen,      // Qwen Code CLI (priority)
    Claude,    // Claude Code
    Codex,     // OpenAI Codex
    Gemini,    // Gemini CLI
    Amp,       // Amp CLI
    OpenCode,  // OpenCode CLI
    MistralRs, // Local mistral.rs server
    Pi,        // pi-coding-agent via local Gemma 4 (env override, :1234 gateway, or :8001 direct)
    File,      // Fallback for testing
}

impl std::fmt::Display for AaiiiBackend {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AaiiiBackend::Qwen => write!(f, "Qwen"),
            AaiiiBackend::Claude => write!(f, "Claude"),
            AaiiiBackend::Codex => write!(f, "Codex"),
            AaiiiBackend::Gemini => write!(f, "Gemini"),
            AaiiiBackend::Amp => write!(f, "Amp"),
            AaiiiBackend::OpenCode => write!(f, "OpenCode"),
            AaiiiBackend::MistralRs => write!(f, "MistralRs"),
            AaiiiBackend::Pi => write!(f, "Pi"),
            AaiiiBackend::File => write!(f, "File"),
        }
    }
}

/// AAIII configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AaiiiConfig {
    pub backend: AaiiiBackend,
    pub api_key_env: Option<String>,
    pub base_url: Option<String>,
    pub model: Option<String>,
    pub sandbox_enabled: bool,
    pub mesh_endpoint: Option<String>,
}

impl Default for AaiiiConfig {
    fn default() -> Self {
        Self {
            backend: AaiiiBackend::File,
            api_key_env: None,
            base_url: None,
            model: None,
            sandbox_enabled: true,
            mesh_endpoint: None,
        }
    }
}

impl AaiiiConfig {
    /// Auto-detect available AI backend
    pub fn detect() -> Self {
        // Priority: Qwen > Claude > Codex > Gemini > Amp > OpenCode > MistralRs > File
        
        if Self::check_qwen() {
            eprintln!("🤖 AAIII backend detected: Qwen Code");
            return Self {
                backend: AaiiiBackend::Qwen,
                api_key_env: Some("DASHSCOPE_API_KEY".to_string()),
                ..Default::default()
            };
        }

        if Self::check_claude() {
            eprintln!("🤖 AAIII backend detected: Claude Code");
            return Self {
                backend: AaiiiBackend::Claude,
                api_key_env: Some("ANTHROPIC_API_KEY".to_string()),
                ..Default::default()
            };
        }

        if Self::check_codex() {
            eprintln!("🤖 AAIII backend detected: Codex");
            return Self {
                backend: AaiiiBackend::Codex,
                api_key_env: Some("OPENAI_API_KEY".to_string()),
                ..Default::default()
            };
        }

        if Self::check_gemini() {
            eprintln!("🤖 AAIII backend detected: Gemini");
            return Self {
                backend: AaiiiBackend::Gemini,
                api_key_env: Some("GEMINI_API_KEY".to_string()),
                ..Default::default()
            };
        }

        if Self::check_amp() {
            eprintln!("🤖 AAIII backend detected: Amp");
            return Self {
                backend: AaiiiBackend::Amp,
                ..Default::default()
            };
        }

        if Self::check_opencode() {
            eprintln!("🤖 AAIII backend detected: OpenCode");
            return Self {
                backend: AaiiiBackend::OpenCode,
                ..Default::default()
            };
        }

        if Self::check_mistralrs() {
            eprintln!("🤖 AAIII backend detected: Mistral.rs");
            return Self {
                backend: AaiiiBackend::MistralRs,
                base_url: Some("http://localhost:8181/v1".to_string()),
                ..Default::default()
            };
        }

        if Self::check_pi() {
            eprintln!("🤖 AAIII backend detected: Pi (local Gemma 4 ch0nky)");
            return Self {
                backend: AaiiiBackend::Pi,
                // 🤓 Prefer explicit env override, then gateway :1234 when healthy, then direct vLLM :8001.
                base_url: Self::detect_pi_base_url(),
                model: Some("ch0nky".to_string()),
                ..Default::default()
            };
        }

        eprintln!("🤖 AAIII backend: File (no AI CLI detected)");
        Self::default()
    }

    fn check_qwen() -> bool {
        Command::new("qwen")
            .arg("--version")
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false)
    }

    fn check_claude() -> bool {
        Command::new("claude")
            .arg("--version")
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false)
    }

    fn check_codex() -> bool {
        Command::new("codex")
            .arg("--version")
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false)
    }

    fn check_gemini() -> bool {
        Command::new("gemini")
            .arg("--version")
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false)
    }

    fn check_amp() -> bool {
        Command::new("amp")
            .arg("--version")
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false)
    }

    fn check_opencode() -> bool {
        Command::new("opencode")
            .arg("--version")
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false)
    }

    fn check_mistralrs() -> bool {
        reqwest::blocking::get("http://localhost:8181/v1/models")
            .map(|r| r.status().is_success())
            .unwrap_or(false)
    }

    fn check_pi() -> bool {
        // 🤓 pi requires: (a) pi binary present AND (b) a reachable OpenAI-compatible local endpoint.
        let binary_ok = Command::new("pi")
            .arg("--version")
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false);
        binary_ok && Self::detect_pi_base_url().is_some()
    }

    fn detect_pi_base_url() -> Option<String> {
        Self::pi_base_url_candidates()
            .into_iter()
            .find(|base_url| Self::models_endpoint_ok(base_url))
    }

    fn pi_base_url_candidates() -> Vec<String> {
        let mut candidates = Vec::new();

        for key in ["B00T_AI_CH0NKY_BASE", "PI_BASE_URL", "LLAMA_CPP_BASE_URL", "OPENAI_BASE_URL"] {
            if let Ok(value) = env::var(key) {
                let trimmed = value.trim();
                if !trimmed.is_empty() && !candidates.iter().any(|existing| existing == trimmed) {
                    candidates.push(trimmed.to_string());
                }
            }
        }

        for default_url in ["http://localhost:1234/v1", "http://localhost:8001/v1"] {
            if !candidates.iter().any(|existing| existing == default_url) {
                candidates.push(default_url.to_string());
            }
        }

        candidates
    }

    fn models_endpoint_ok(base_url: &str) -> bool {
        let models_url = format!("{}/models", base_url.trim_end_matches('/'));
        reqwest::blocking::get(&models_url)
            .map(|r| r.status().is_success())
            .unwrap_or(false)
    }
}

/// Tool capability for pre-simulation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolCapability {
    pub name: String,
    pub description: String,
    pub commands: Vec<String>,
    pub env_vars: Vec<String>,
    pub skill_tags: Vec<String>, // #tags for skill-based feature flags
}

/// AAIII runtime for async event dispatch
pub struct AaiiiRuntime {
    config: AaiiiConfig,
    capabilities: HashMap<String, ToolCapability>,
    event_handlers: HashMap<String, Vec<Box<dyn Fn(&str) + Send + Sync>>>,
}

impl AaiiiRuntime {
    pub fn new(config: AaiiiConfig) -> Self {
        Self {
            config,
            capabilities: HashMap::new(),
            event_handlers: HashMap::new(),
        }
    }

    pub fn with_auto_detect() -> Self {
        Self::new(AaiiiConfig::detect())
    }

    /// Register a tool capability
    pub fn register_capability(&mut self, cap: ToolCapability) {
        self.capabilities.insert(cap.name.clone(), cap);
    }

    /// Register an event handler
    pub fn register_handler<F>(&mut self, event: &str, handler: F)
    where
        F: Fn(&str) + Send + Sync + 'static,
    {
        self.event_handlers
            .entry(event.to_string())
            .or_default()
            .push(Box::new(handler));
    }

    /// Dispatch an event to registered handlers
    pub fn dispatch(&self, event: &str, payload: &str) -> Result<()> {
        if let Some(handlers) = self.event_handlers.get(event) {
            for handler in handlers {
                handler(payload);
            }
        }
        Ok(())
    }

    /// Get capabilities for a given skill tag
    pub fn get_capabilities_by_tag(&self, tag: &str) -> Vec<&ToolCapability> {
        self.capabilities
            .values()
            .filter(|cap| cap.skill_tags.iter().any(|t| t == tag))
            .collect()
    }

    /// Pre-simulate tool capabilities for AI agent
    pub fn simulate_capabilities(&self) -> Vec<String> {
        self.capabilities
            .values()
            .map(|cap| {
                format!(
                    "{}: {} (commands: {})",
                    cap.name,
                    cap.description,
                    cap.commands.join(", ")
                )
            })
            .collect()
    }

    /// Get backend type
    pub fn backend(&self) -> AaiiiBackend {
        self.config.backend
    }
}

/// Feature flag based on skill #tags
pub struct SkillFeatureFlags {
    enabled_tags: Vec<String>,
}

impl SkillFeatureFlags {
    pub fn new(tags: Vec<String>) -> Self {
        Self { enabled_tags: tags }
    }

    /// Check if a feature is enabled based on skill tags
    pub fn is_enabled(&self, feature_tag: &str) -> bool {
        self.enabled_tags.iter().any(|t| t == feature_tag)
    }

    /// Get enabled features for a role
    pub fn for_role(role: &str) -> Self {
        // 🤓 Role-based feature flags from datum skills
        match role {
            "executive" => Self::new(vec![
                "hive-cmdb".to_string(),
                "agent-delegation".to_string(),
                "redis-coordination".to_string(),
            ]),
            "developer" => Self::new(vec![
                "git".to_string(),
                "rust".to_string(),
                "testing".to_string(),
            ]),
            _ => Self::new(vec![]),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_aaiii_config_detect() {
        let config = AaiiiConfig::detect();
        // Should always return a valid config
        assert!(matches!(
            config.backend,
            AaiiiBackend::Qwen
                | AaiiiBackend::Claude
                | AaiiiBackend::Codex
                | AaiiiBackend::Gemini
                | AaiiiBackend::Amp
                | AaiiiBackend::OpenCode
                | AaiiiBackend::MistralRs
                | AaiiiBackend::Pi
                | AaiiiBackend::File
        ));
    }

    #[test]
    fn test_pi_base_url_candidates_prioritize_env_override() {
        let key = "B00T_AI_CH0NKY_BASE";
        let previous = std::env::var(key).ok();
        unsafe {
            std::env::set_var(key, "http://127.0.0.1:8001/v1");
        }

        let candidates = AaiiiConfig::pi_base_url_candidates();
        assert_eq!(candidates.first().map(String::as_str), Some("http://127.0.0.1:8001/v1"));
        assert!(candidates.iter().any(|candidate| candidate == "http://localhost:1234/v1"));

        if let Some(value) = previous {
            unsafe {
                std::env::set_var(key, value);
            }
        } else {
            unsafe {
                std::env::remove_var(key);
            }
        }
    }

    #[test]
    fn test_pi_base_url_candidates_include_direct_gemma4_fallback() {
        for key in ["B00T_AI_CH0NKY_BASE", "PI_BASE_URL", "LLAMA_CPP_BASE_URL", "OPENAI_BASE_URL"] {
            unsafe {
                std::env::remove_var(key);
            }
        }

        let candidates = AaiiiConfig::pi_base_url_candidates();
        assert!(candidates.iter().any(|candidate| candidate == "http://localhost:1234/v1"));
        assert!(candidates.iter().any(|candidate| candidate == "http://localhost:8001/v1"));
    }

    #[test]
    fn test_skill_feature_flags() {
        let flags = SkillFeatureFlags::for_role("executive");
        assert!(flags.is_enabled("hive-cmdb"));
        assert!(flags.is_enabled("redis-coordination"));
        assert!(!flags.is_enabled("rust"));
    }
}
