//! Skeleton generator for ~/.b00t directory structure
//!
//! Creates directories specified in bootstrap.toml

use anyhow::{Context, Result};
use serde::Deserialize;
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

/// Bootstrap configuration (subset for directories)
#[derive(Debug, Deserialize)]
struct BootstrapConfig {
    bootstrap: BootstrapDirectories,
}

#[derive(Debug, Deserialize)]
struct BootstrapDirectories {
    #[serde(default)]
    directories: HashMap<String, String>,
}

/// Result of skeleton generation
#[derive(Debug)]
pub struct SkeletonResult {
    pub created: Vec<PathBuf>,
    pub already_existed: Vec<PathBuf>,
    pub errors: Vec<(PathBuf, String)>,
}

impl SkeletonResult {
    /// Check if all directories were successfully created or existed
    pub fn is_success(&self) -> bool {
        self.errors.is_empty()
    }

    /// Get total count of directories processed
    pub fn total_count(&self) -> usize {
        self.created.len() + self.already_existed.len() + self.errors.len()
    }
}

/// Expand path with tilde (~) to home directory
fn expand_path(path: &str) -> PathBuf {
    if path.starts_with('~') {
        if let Some(home) = dirs::home_dir() {
            let remainder = &path[1..]; // Remove leading ~
            let remainder = remainder.trim_start_matches('/'); // Remove leading /
            return home.join(remainder);
        }
    }
    PathBuf::from(path)
}

/// Create a single directory with proper error handling
fn create_directory(path: &Path) -> Result<bool> {
    if path.exists() {
        if path.is_dir() {
            Ok(false) // Already exists
        } else {
            anyhow::bail!("Path exists but is not a directory: {}", path.display());
        }
    } else {
        fs::create_dir_all(path)
            .with_context(|| format!("Failed to create directory: {}", path.display()))?;
        Ok(true) // Newly created
    }
}

/// Create skeleton directory structure from bootstrap config
pub fn create_skeleton(config_path: &Path) -> Result<SkeletonResult> {
    // Read and parse config
    let content = std::fs::read_to_string(config_path)
        .with_context(|| format!("Failed to read {}", config_path.display()))?;

    let config: BootstrapConfig = toml::from_str(&content)
        .context("Failed to parse bootstrap.toml")?;

    let mut created = Vec::new();
    let mut already_existed = Vec::new();
    let mut errors = Vec::new();

    // Process each directory
    for (name, path_str) in &config.bootstrap.directories {
        let path = expand_path(path_str);

        match create_directory(&path) {
            Ok(true) => {
                created.push(path);
            }
            Ok(false) => {
                already_existed.push(path);
            }
            Err(e) => {
                errors.push((path, e.to_string()));
            }
        }
    }

    Ok(SkeletonResult {
        created,
        already_existed,
        errors,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_expand_path() {
        let path = expand_path("~/.b00t/config");
        assert!(path.to_string_lossy().contains(".b00t/config"));
        assert!(!path.to_string_lossy().contains('~'));
    }

    #[test]
    fn test_expand_path_non_tilde() {
        let path = expand_path("/tmp/test");
        assert_eq!(path, PathBuf::from("/tmp/test"));
    }
}
