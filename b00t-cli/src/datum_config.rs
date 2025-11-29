use crate::traits::*;
use crate::{BootDatum, DatumType};
use anyhow::Result;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

/// b00t configuration file (_b00t_.toml) structure
#[derive(Deserialize, Serialize, Debug, Clone)]
pub struct B00tConfig {
    /// Version of b00t CLI that created/last modified this config
    pub version: String,

    /// When this config was initialized
    pub initialized: DateTime<Utc>,

    /// Preferred installation methods (in order of preference)
    /// Default: ["docker", "pkgx", "apt", "curl"]
    pub install_methods: Vec<String>,

    /// List of datums to install/maintain (supports wildcards)
    /// Format: "name.type" or "pattern.*" or "*.type"
    pub datums: Vec<String>,

    /// Installation history log
    #[serde(default)]
    pub history: Vec<InstallHistoryEntry>,

    /// Optional per-host check registry (true=enable, false=ignore)
    #[serde(default)]
    pub checks: HashMap<String, bool>,

    /// Optional metadata
    #[serde(flatten)]
    pub metadata: Option<HashMap<String, serde_json::Value>>,
}

/// Installation history entry
#[derive(Deserialize, Serialize, Debug, Clone)]
pub struct InstallHistoryEntry {
    pub datum: String,
    pub version: Option<String>,
    pub timestamp: DateTime<Utc>,
    pub method: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub status: Option<String>, // success, failed, skipped
}

impl B00tConfig {
    /// Create a new default configuration
    pub fn new() -> Self {
        Self {
            version: b00t_c0re_lib::version::VERSION.to_string(),
            initialized: Utc::now(),
            install_methods: vec![
                "docker".to_string(),
                "pkgx".to_string(),
                "apt".to_string(),
                "curl".to_string(),
            ],
            datums: Vec::new(),
            history: Vec::new(),
            checks: HashMap::new(),
            metadata: None,
        }
    }

    /// Load configuration from file
    pub fn load(path: &Path) -> Result<Self> {
        let contents = fs::read_to_string(path)?;
        let config: B00tConfig = toml::from_str(&contents)?;
        Ok(config)
    }

    /// Save configuration to file
    pub fn save(&self, path: &Path) -> Result<()> {
        let toml_string = toml::to_string_pretty(self)?;
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::write(path, toml_string)?;
        Ok(())
    }

    /// Find the appropriate config file path
    /// Priority:
    /// 1. If in git repo: <repo_root>/_b00t_.toml (project-specific)
    /// 2. ~/.b00t/_b00t_.toml (user-level)
    ///
    /// Note: Projects may have a _b00t_/ directory for project-specific datums,
    /// but the config file is always _b00t_.toml at the repo root.
    pub fn find_config_path() -> Result<PathBuf> {
        // Check if we're in a git repo - use project-specific config at repo root
        if let Ok(repo_root) = Self::find_git_root() {
            let config_path = repo_root.join("_b00t_.toml");
            return Ok(config_path);
        }

        // Fall back to user-level config
        if let Ok(home) = std::env::var("HOME") {
            let b00t_path = PathBuf::from(home).join(".b00t/_b00t_.toml");
            return Ok(b00t_path);
        }

        anyhow::bail!("Could not determine config file path");
    }

    /// Find git repository root by looking for .git directory
    fn find_git_root() -> Result<PathBuf> {
        let current_dir = std::env::current_dir()?;
        let mut dir = current_dir.as_path();

        loop {
            let git_dir = dir.join(".git");
            if git_dir.exists() {
                return Ok(dir.to_path_buf());
            }

            match dir.parent() {
                Some(parent) => dir = parent,
                None => anyhow::bail!("Not in a git repository"),
            }
        }
    }

    /// Load or create config from the appropriate location
    pub fn load_or_create() -> Result<(Self, PathBuf)> {
        let config_path = Self::find_config_path()?;

        if config_path.exists() {
            let config = Self::load(&config_path)?;
            Ok((config, config_path))
        } else {
            let config = Self::new();
            Ok((config, config_path))
        }
    }

    /// Add a datum to the config
    pub fn add_datum(&mut self, datum_spec: String) {
        if !self.datums.contains(&datum_spec) {
            self.datums.push(datum_spec);
        }
    }

    /// Remove a datum from the config
    pub fn remove_datum(&mut self, datum_spec: &str) -> bool {
        if let Some(pos) = self.datums.iter().position(|d| d == datum_spec) {
            self.datums.remove(pos);
            true
        } else {
            false
        }
    }

    /// Add an installation history entry
    pub fn add_history(
        &mut self,
        datum: String,
        version: Option<String>,
        method: String,
        status: Option<String>,
    ) {
        self.history.push(InstallHistoryEntry {
            datum,
            version,
            timestamp: Utc::now(),
            method,
            status,
        });
    }

    /// Match a datum specification against a pattern (supports wildcards)
    /// Patterns: "name.type", "name.*", "*.type", "*"
    pub fn matches_pattern(datum_spec: &str, pattern: &str) -> bool {
        if pattern == "*" {
            return true;
        }

        let datum_parts: Vec<&str> = datum_spec.split('.').collect();
        let pattern_parts: Vec<&str> = pattern.split('.').collect();

        if datum_parts.len() != pattern_parts.len() {
            return false;
        }

        datum_parts
            .iter()
            .zip(pattern_parts.iter())
            .all(|(d, p)| *p == "*" || d == p)
    }

    /// Get all datums that match any of the configured patterns
    pub fn get_matching_datums(&self, all_datums: &[(String, DatumType)]) -> Vec<String> {
        let mut matched = Vec::new();

        for (name, dtype) in all_datums {
            let datum_spec = format!("{}.{}", name, Self::datum_type_str(dtype));

            for pattern in &self.datums {
                if Self::matches_pattern(&datum_spec, pattern) {
                    matched.push(datum_spec.clone());
                    break;
                }
            }
        }

        matched
    }

    /// Convert DatumType to string for pattern matching
    fn datum_type_str(dtype: &DatumType) -> &'static str {
        match dtype {
            DatumType::Agent => "agent",
            DatumType::Cli => "cli",
            DatumType::Mcp => "mcp",
            DatumType::Ai => "ai",
            DatumType::AiModel => "ai_model",
            DatumType::Docker => "docker",
            DatumType::K8s => "k8s",
            DatumType::Apt => "apt",
            DatumType::Nix => "nix",
            DatumType::Vscode => "vscode",
            DatumType::Bash => "bash",
            DatumType::Stack => "stack",
            DatumType::Api => "api",
            DatumType::Config => "config",
            DatumType::Job => "job",
            DatumType::Unknown => "unknown",
            _ => "unknown",
        }
    }
}

impl Default for B00tConfig {
    fn default() -> Self {
        Self::new()
    }
}

/// ConfigDatum wrapper for b00t datum system
pub struct ConfigDatum {
    pub datum: BootDatum,
    pub config: B00tConfig,
}

impl ConfigDatum {
    pub fn from_config(path: &str) -> Result<Self> {
        let config_path = PathBuf::from(shellexpand::tilde(path).to_string()).join("_b00t_.toml");

        let config = if config_path.exists() {
            B00tConfig::load(&config_path)?
        } else {
            B00tConfig::new()
        };

        let datum = BootDatum {
            name: "_b00t_".to_string(),
            datum_type: Some(DatumType::Config),
            desires: Some(b00t_c0re_lib::version::VERSION.to_string()),
            hint: "b00t configuration file".to_string(),
            keywords: Some(vec!["config".to_string(), "b00t".to_string()]),
            ..BootDatum::default()
        };

        Ok(Self { datum, config })
    }
}

impl DatumChecker for ConfigDatum {
    fn is_installed(&self) -> bool {
        B00tConfig::find_config_path()
            .map(|p| p.exists())
            .unwrap_or(false)
    }

    fn current_version(&self) -> Option<String> {
        if self.is_installed() {
            Some(self.config.version.clone())
        } else {
            None
        }
    }

    fn desired_version(&self) -> Option<String> {
        Some(b00t_c0re_lib::version::VERSION.to_string())
    }

    fn version_status(&self) -> VersionStatus {
        match (self.current_version(), self.desired_version()) {
            (Some(current), Some(desired)) if current == desired => VersionStatus::Match,
            (Some(_), Some(_)) => VersionStatus::Older,
            (None, Some(_)) => VersionStatus::Missing,
            _ => VersionStatus::Unknown,
        }
    }
}

impl StatusProvider for ConfigDatum {
    fn name(&self) -> &str {
        &self.datum.name
    }

    fn subsystem(&self) -> &str {
        "config"
    }

    fn hint(&self) -> &str {
        &self.datum.hint
    }

    fn is_disabled(&self) -> bool {
        false
    }
}

impl FilterLogic for ConfigDatum {
    fn is_available(&self) -> bool {
        true
    }

    fn prerequisites_satisfied(&self) -> bool {
        true
    }

    fn evaluate_constraints(&self, _require: &[String]) -> bool {
        true
    }
}

impl DatumProvider for ConfigDatum {
    fn datum(&self) -> &BootDatum {
        &self.datum
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pattern_matching() {
        assert!(B00tConfig::matches_pattern("git.cli", "git.cli"));
        assert!(B00tConfig::matches_pattern("git.cli", "*.cli"));
        assert!(B00tConfig::matches_pattern("git.cli", "git.*"));
        assert!(B00tConfig::matches_pattern("git.cli", "*"));
        assert!(!B00tConfig::matches_pattern("git.cli", "rust.cli"));
        assert!(!B00tConfig::matches_pattern("git.cli", "git.docker"));
    }

    #[test]
    fn test_new_config() {
        let config = B00tConfig::new();
        assert_eq!(
            config.install_methods,
            vec!["docker", "pkgx", "apt", "curl"]
        );
        assert!(config.datums.is_empty());
        assert!(config.history.is_empty());
    }

    #[test]
    fn test_add_remove_datum() {
        let mut config = B00tConfig::new();
        config.add_datum("git.cli".to_string());
        assert_eq!(config.datums.len(), 1);
        assert!(config.datums.contains(&"git.cli".to_string()));

        assert!(config.remove_datum("git.cli"));
        assert!(config.datums.is_empty());

        assert!(!config.remove_datum("nonexistent.datum"));
    }
}
