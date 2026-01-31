use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use shellexpand;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::process::Command;

/// Declarative Ansible playbook metadata for datums.
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Default)]
pub struct AnsibleConfig {
    /// Path to the playbook file (relative to the workspace or absolute).
    pub playbook: String,
    /// Optional inventory file path.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub inventory: Option<String>,
    /// Extra variables passed via `-e key=value` (sorted by key for reproducibility).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub extra_vars: Option<HashMap<String, String>>,
    /// Any additional CLI arguments to forward to `ansible-playbook`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub extra_args: Option<Vec<String>>,
}

impl AnsibleConfig {
    fn resolve_path<'a>(value: &str, workspace: Option<&'a Path>) -> PathBuf {
        let expanded = shellexpand::tilde(value);
        let relative = Path::new(expanded.as_ref());
        if relative.is_absolute() || workspace.is_none() {
            relative.to_path_buf()
        } else {
            workspace.unwrap().join(relative)
        }
    }
}

/// Execute the configured playbook using the given workspace root as base for relative paths.
pub fn run_playbook(config: &AnsibleConfig, workspace: Option<&Path>) -> Result<()> {
    if config.playbook.trim().is_empty() {
        anyhow::bail!("Ansible playbook path cannot be empty");
    }

    let playbook_path = AnsibleConfig::resolve_path(&config.playbook, workspace);
    let inventory_path = config
        .inventory
        .as_ref()
        .map(|inv| AnsibleConfig::resolve_path(inv, workspace));

    let args = build_ansible_args(
        &playbook_path,
        inventory_path.as_deref(),
        config.extra_args.as_ref(),
        config.extra_vars.as_ref(),
    );

    println!("🥾 Running ansible-playbook {}", playbook_path.display());

    let status = Command::new("ansible-playbook")
        .args(&args)
        .current_dir(workspace.unwrap_or_else(|| Path::new(".")))
        .status()
        .context("Failed to launch ansible-playbook")?;

    if !status.success() {
        anyhow::bail!("ansible-playbook failed with status {}", status);
    }

    Ok(())
}

fn build_ansible_args(
    playbook: &Path,
    inventory: Option<&Path>,
    extra_args: Option<&Vec<String>>,
    extra_vars: Option<&HashMap<String, String>>,
) -> Vec<String> {
    let mut args = Vec::new();

    if let Some(inv) = inventory {
        args.push("-i".to_string());
        args.push(inv.display().to_string());
    }

    if let Some(extra_args) = extra_args {
        args.extend(extra_args.iter().cloned());
    }

    if let Some(vars) = extra_vars {
        let mut sorted: Vec<_> = vars.iter().collect();
        sorted.sort_by(|(a, _), (b, _)| a.cmp(b));
        for (key, value) in sorted {
            args.push("-e".to_string());
            args.push(format!("{}={}", key, value));
        }
    }

    args.push(playbook.display().to_string());
    args
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    #[test]
    fn build_args_includes_inventory_and_vars() {
        let mut vars = HashMap::new();
        vars.insert("answer".to_string(), "42".to_string());
        vars.insert("foo".to_string(), "bar".to_string());

        let config = AnsibleConfig {
            playbook: "playbook.yaml".to_string(),
            inventory: Some("inventory.yaml".to_string()),
            extra_vars: Some(vars),
            extra_args: Some(vec!["-v".to_string(), "--check".to_string()]),
        };

        let playbook_path = Path::new("playbook.yaml");
        let inventory_path = Path::new("inventory.yaml");
        let args = build_ansible_args(
            playbook_path,
            Some(inventory_path),
            config.extra_args.as_ref(),
            config.extra_vars.as_ref(),
        );

        assert_eq!(args[0], "-i");
        assert_eq!(args[1], "inventory.yaml");
        assert!(args.contains(&"-e".to_string()));
        assert!(args.contains(&"answer=42".to_string()));
        assert!(args.contains(&"foo=bar".to_string()));
        assert_eq!(args.last().unwrap(), "playbook.yaml");
        assert!(args.contains(&"--check".to_string()));
        assert!(args.contains(&"-v".to_string()));
    }

    #[test]
    fn resolve_relative_paths_against_workspace() {
        let workspace = Path::new("/workspace/root");
        let playbook = AnsibleConfig::resolve_path("relative/playbook.yml", Some(workspace));
        assert_eq!(playbook, workspace.join("relative/playbook.yml"));

        let inventory = AnsibleConfig::resolve_path("~/inventory.yml", Some(workspace));
        assert!(inventory.ends_with("inventory.yml"));
    }
}
