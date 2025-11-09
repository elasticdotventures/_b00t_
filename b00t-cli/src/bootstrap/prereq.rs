//! Prerequisite checker for b00t bootstrap
//!
//! Reads bootstrap.toml and validates that required binaries are installed
//! with correct versions.

use anyhow::{Context, Result};
use serde::Deserialize;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::process::Command;

/// Bootstrap configuration structure matching _b00t_/bootstrap.toml
#[derive(Debug, Deserialize)]
pub struct BootstrapConfig {
    pub bootstrap: BootstrapSection,
}

#[derive(Debug, Deserialize)]
pub struct BootstrapSection {
    pub required_bins: HashMap<String, BinarySpec>,
    #[serde(default)]
    pub optional_bins: HashMap<String, BinarySpec>,
}

#[derive(Debug, Deserialize, Clone)]
pub struct BinarySpec {
    pub version: String, // Format: ">=1.0.0"
    #[serde(default)]
    pub priority: u8,
    #[serde(default)]
    pub install_hint: Option<String>,
}

/// Result of prerequisite check for a single binary
#[derive(Debug)]
pub struct BinaryCheck {
    pub name: String,
    pub found: bool,
    pub installed_version: Option<String>,
    pub required_version: String,
    pub meets_requirement: bool,
    pub path: Option<PathBuf>,
    pub install_hint: Option<String>,
}

/// Overall prerequisite check result
#[derive(Debug)]
pub struct PrereqResult {
    pub required: Vec<BinaryCheck>,
    pub optional: Vec<BinaryCheck>,
    pub all_required_met: bool,
}

impl PrereqResult {
    /// Get list of missing required binaries
    pub fn missing_required(&self) -> Vec<&BinaryCheck> {
        self.required
            .iter()
            .filter(|b| !b.found || !b.meets_requirement)
            .collect()
    }

    /// Get list of missing optional binaries
    pub fn missing_optional(&self) -> Vec<&BinaryCheck> {
        self.optional
            .iter()
            .filter(|b| !b.found || !b.meets_requirement)
            .collect()
    }
}

/// Load bootstrap config from TOML file
fn load_config(config_path: &Path) -> Result<BootstrapConfig> {
    let content = std::fs::read_to_string(config_path)
        .with_context(|| format!("Failed to read {}", config_path.display()))?;

    toml::from_str(&content)
        .with_context(|| format!("Failed to parse bootstrap.toml"))
}

/// Check if binary exists in PATH
fn find_binary(name: &str) -> Option<PathBuf> {
    Command::new("which")
        .arg(name)
        .output()
        .ok()
        .and_then(|output| {
            if output.status.success() {
                String::from_utf8(output.stdout)
                    .ok()
                    .map(|s| PathBuf::from(s.trim()))
            } else {
                None
            }
        })
}

/// Get version of binary by running `<binary> --version`
fn get_version(name: &str) -> Option<String> {
    let output = Command::new(name)
        .arg("--version")
        .output()
        .ok()?;

    if !output.status.success() {
        return None;
    }

    let version_output = String::from_utf8(output.stdout).ok()?;
    extract_version(&version_output)
}

/// Check if docker binary is actually podman
/// 🤓 Checks both symlink target and version output
fn is_docker_actually_podman() -> bool {
    // Check if docker is symlinked to podman
    if let Some(docker_path) = find_binary("docker") {
        if let Ok(target) = std::fs::read_link(&docker_path) {
            if target.to_string_lossy().contains("podman") {
                return true;
            }
        }
        // Also resolve canonical path in case of nested symlinks
        if let Ok(canonical) = std::fs::canonicalize(&docker_path) {
            if canonical.to_string_lossy().contains("podman") {
                return true;
            }
        }
    }

    // Fallback: check version output
    if let Ok(output) = Command::new("docker").arg("--version").output() {
        if let Ok(version_output) = String::from_utf8(output.stdout) {
            if version_output.contains("podman") {
                return true;
            }
        }
    }

    false
}

/// Extract semantic version from version output
/// Handles various formats:
///   "git version 2.34.1" -> "2.34.1"
///   "docker version 20.10.0, build..." -> "20.10.0"
///   "just 1.5.0" -> "1.5.0"
fn extract_version(output: &str) -> Option<String> {
    // Common patterns: "X.Y.Z" or "vX.Y.Z"
    let re = regex::Regex::new(r"v?(\d+\.\d+\.\d+)").ok()?;
    re.captures(output)
        .and_then(|cap| cap.get(1))
        .map(|m| m.as_str().to_string())
}

/// Check if installed version meets requirement
/// Parses requirement like ">=1.0.0" and compares versions
fn version_meets_requirement(installed: &str, requirement: &str) -> Result<bool> {
    // Parse requirement (e.g., ">=1.0.0")
    let requirement = requirement.trim();

    let (op, required_ver_str) = if requirement.starts_with(">=") {
        (">=", &requirement[2..])
    } else if requirement.starts_with("<=") {
        ("<=", &requirement[2..])
    } else if requirement.starts_with('>') {
        (">", &requirement[1..])
    } else if requirement.starts_with('<') {
        ("<", &requirement[1..])
    } else if requirement.starts_with('=') {
        ("=", &requirement[1..])
    } else {
        ("=", requirement)
    };

    let installed_ver = semver::Version::parse(installed.trim())
        .with_context(|| format!("Failed to parse installed version: {}", installed))?;

    let required_ver = semver::Version::parse(required_ver_str.trim())
        .with_context(|| format!("Failed to parse required version: {}", required_ver_str))?;

    Ok(match op {
        ">=" => installed_ver >= required_ver,
        "<=" => installed_ver <= required_ver,
        ">" => installed_ver > required_ver,
        "<" => installed_ver < required_ver,
        "=" => installed_ver == required_ver,
        _ => false,
    })
}

/// Check a single binary against its specification
/// 🤓 Supports alternatives (e.g., "docker" can be satisfied by "podman")
fn check_binary(name: &str, spec: &BinarySpec) -> BinaryCheck {
    // Check primary binary first
    let mut path = find_binary(name);
    let mut found = path.is_some();
    let mut actual_name = name.to_string();
    let mut using_alternative = false;

    // Check if docker is actually podman (symlink or wrapper)
    if name == "docker" && found && is_docker_actually_podman() {
        actual_name = "docker (via podman)".to_string();
        using_alternative = true;
    }

    // If not found, check alternatives (docker -> podman)
    if !found && name == "docker" {
        if let Some(podman_path) = find_binary("podman") {
            path = Some(podman_path);
            found = true;
            actual_name = "podman (docker alternative)".to_string();
            using_alternative = true;
        }
    }

    let (installed_version, meets_requirement) = if found {
        // Get version from the actual binary found (podman if docker not found)
        let binary_for_version = if name == "docker" && find_binary("docker").is_none() {
            "podman"
        } else {
            name
        };

        if let Some(version) = get_version(binary_for_version) {
            // 🤓 If using alternative (podman for docker), assume it meets requirement
            let meets = if using_alternative {
                true
            } else {
                version_meets_requirement(&version, &spec.version).unwrap_or(false)
            };
            (Some(version), meets)
        } else {
            // 🤓 Binary found but version unknown - assume OK
            (Some("unknown".to_string()), true)
        }
    } else {
        (None, false)
    };

    BinaryCheck {
        name: actual_name,
        found,
        installed_version,
        required_version: spec.version.clone(),
        meets_requirement,
        path,
        install_hint: spec.install_hint.clone(),
    }
}

/// Check all prerequisites from bootstrap config
pub fn check_prerequisites(config_path: &Path) -> Result<PrereqResult> {
    let config = load_config(config_path)?;

    let mut required_checks = Vec::new();
    let mut optional_checks = Vec::new();

    // Check required binaries
    for (name, spec) in &config.bootstrap.required_bins {
        required_checks.push(check_binary(name, spec));
    }

    // Sort by priority (lower number = higher priority)
    required_checks.sort_by_key(|check| {
        config.bootstrap.required_bins
            .get(&check.name)
            .map(|spec| spec.priority)
            .unwrap_or(99)
    });

    // Check optional binaries
    for (name, spec) in &config.bootstrap.optional_bins {
        optional_checks.push(check_binary(name, spec));
    }

    let all_required_met = required_checks.iter()
        .all(|check| check.found && check.meets_requirement);

    Ok(PrereqResult {
        required: required_checks,
        optional: optional_checks,
        all_required_met,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_version() {
        assert_eq!(
            extract_version("git version 2.34.1"),
            Some("2.34.1".to_string())
        );
        assert_eq!(
            extract_version("docker version 20.10.0, build..."),
            Some("20.10.0".to_string())
        );
        assert_eq!(
            extract_version("just 1.5.0"),
            Some("1.5.0".to_string())
        );
        assert_eq!(
            extract_version("v3.2.1"),
            Some("3.2.1".to_string())
        );
    }

    #[test]
    fn test_version_comparison() {
        assert!(version_meets_requirement("2.34.1", ">=2.30.0").unwrap());
        assert!(!version_meets_requirement("2.29.0", ">=2.30.0").unwrap());
        assert!(version_meets_requirement("1.0.0", "=1.0.0").unwrap());
        assert!(!version_meets_requirement("1.0.1", "=1.0.0").unwrap());
    }
}
