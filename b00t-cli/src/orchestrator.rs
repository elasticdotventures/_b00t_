//! Agent orchestrator - silently manages service dependencies
//!
//! Solves the chicken-egg problem where b00t commands need services running.
//! Automatically starts dependencies before executing commands.

use crate::dependency_resolver::DependencyResolver;
use crate::{get_config, BootDatum, DatumType};
use anyhow::{Context, Result};
use std::collections::HashMap;
use std::path::Path;
use std::process::Command;
use std::time::Duration;
use tokio::time::sleep;

/// Orchestrator manages dependencies and ensures services are running
pub struct Orchestrator {
    datums: HashMap<String, BootDatum>,
}

impl Orchestrator {
    /// Create new orchestrator by loading all datums from directory
    pub fn new(path: &str) -> Result<Self> {
        let all_datums = Self::load_all_datums(path)?;

        Ok(Self {
            datums: all_datums,
        })
    }

    /// Load all datums from _b00t_/ directory
    fn load_all_datums(path: &str) -> Result<HashMap<String, BootDatum>> {
        let mut datums = HashMap::new();
        let b00t_path = Path::new(path);

        if !b00t_path.exists() {
            anyhow::bail!("b00t path does not exist: {}", path);
        }

        // Scan for all .toml files
        for entry in std::fs::read_dir(b00t_path)? {
            let entry = entry?;
            let file_path = entry.path();

            if !file_path.is_file() {
                continue;
            }

            let file_name = file_path.file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("");

            // Parse datum from each .{type}.toml file
            if let Some(name) = Self::extract_datum_name(file_name) {
                if let Ok((config, _)) = get_config(&name, path) {
                    let datum = config.b00t;
                    let key = Self::make_key(&datum);
                    datums.insert(key, datum);
                }
            }
        }

        Ok(datums)
    }

    /// Extract datum name from filename (e.g., "qdrant.docker.toml" -> "qdrant")
    fn extract_datum_name(filename: &str) -> Option<String> {
        if !filename.ends_with(".toml") {
            return None;
        }

        // Remove .toml extension
        let without_ext = filename.strip_suffix(".toml")?;

        // Remove type suffix (e.g., .docker, .mcp, .cli, .api)
        let types = ["docker", "mcp", "cli", "bash", "vscode", "k8s", "apt", "ai", "api", "stack"];
        for typ in types {
            let suffix = format!(".{}", typ);
            if let Some(name) = without_ext.strip_suffix(&suffix) {
                return Some(name.to_string());
            }
        }

        None
    }

    /// Make datum key from name and type
    fn make_key(datum: &BootDatum) -> String {
        let type_str = datum.datum_type.as_ref()
            .map(|t| format!("{:?}", t).to_lowercase())
            .unwrap_or_else(|| "unknown".to_string());
        format!("{}.{}", datum.name, type_str)
    }

    /// Ensure all dependencies for a datum are running
    /// Returns list of services that were started
    pub async fn ensure_dependencies(&self, datum_key: &str) -> Result<Vec<String>> {
        let mut started = Vec::new();

        // Get datum and its dependencies
        let datum = self.datums.get(datum_key)
            .ok_or_else(|| anyhow::anyhow!("Datum not found: {}", datum_key))?
            .clone();

        // Handle explicit dependencies from depends_on field
        let dep_keys = datum.depends_on.clone().unwrap_or_default();
        for dep_key in dep_keys {
            if let Some(dep_datum) = self.datums.get(&dep_key) {
                if self.needs_start(dep_datum).await? {
                    self.start_service(dep_datum).await
                        .context(format!("Failed to start {}", dep_key))?;
                    started.push(dep_key.clone());
                }
            }
        }

        // Handle capability-based dependencies from requires field
        if let Some(requires) = &datum.requires {
            for (req_name, requirement) in requires {
                if let Some(capability) = &requirement.capability {
                    // Resolve capability to concrete API implementations
                    let resolved = self.resolve_capability(capability, requirement).await?;
                    started.extend(resolved);
                }
            }
        }

        Ok(started)
    }

    /// Resolve a capability requirement to concrete API implementations
    /// Returns list of services that were started
    fn resolve_capability<'a>(&'a self, capability: &'a str, requirement: &'a crate::CapabilityRequirement) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<Vec<String>>> + 'a>> {
        Box::pin(async move {
            let mut started = Vec::new();

            // Find API datums that provide this capability
            let mut candidates: Vec<_> = self.datums.iter()
                .filter(|(_, datum)| {
                    datum.datum_type == Some(crate::DatumType::Api) &&
                    datum.provides.as_ref()
                        .and_then(|p| p.capability.as_ref())
                        .map(|c| c == capability)
                        .unwrap_or(false)
                })
                .collect();

            // Sort by preference if specified
            if let Some(prefer) = &requirement.prefer {
                candidates.sort_by_key(|(key, _)| {
                    prefer.iter()
                        .position(|p| key.starts_with(p))
                        .unwrap_or(prefer.len())
                });
            }

            // Try preferred APIs first, fall back if needed
            for (api_key, api_datum) in candidates {
                // Recursively ensure API dependencies
                match self.ensure_api_dependencies(api_datum).await {
                    Ok(mut api_started) => {
                        started.append(&mut api_started);
                        return Ok(started); // Success, use this API
                    }
                    Err(e) => {
                        // Try next candidate or fallback
                        eprintln!("Warning: Failed to start {}: {}", api_key, e);
                        continue;
                    }
                }
            }

            // If all failed and fallback specified, try that
            if let Some(fallback) = &requirement.fallback {
                if let Some(api_datum) = self.datums.values()
                    .find(|d| d.name == *fallback && d.datum_type == Some(crate::DatumType::Api))
                {
                    let mut api_started = self.ensure_api_dependencies(api_datum).await?;
                    started.append(&mut api_started);
                    return Ok(started);
                }
            }

            anyhow::bail!("Failed to resolve capability: {}", capability)
        })
    }

    /// Ensure API dependencies (infrastructure + recursive API requirements)
    fn ensure_api_dependencies<'a>(&'a self, api_datum: &'a BootDatum) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<Vec<String>>> + 'a>> {
        Box::pin(async move {
            let mut started = Vec::new();

            // Start infrastructure dependencies (e.g., Docker containers)
            if let Some(depends_on) = &api_datum.depends_on {
                for dep_key in depends_on {
                    if let Some(dep_datum) = self.datums.get(dep_key) {
                        if self.needs_start(dep_datum).await? {
                            self.start_service(dep_datum).await
                                .context(format!("Failed to start {}", dep_key))?;
                            started.push(dep_key.clone());
                        }
                    }
                }
            }

            // Recursively resolve API requirements (e.g., composite APIs)
            if let Some(requires) = &api_datum.requires {
                for (_, requirement) in requires {
                    if let Some(capability) = &requirement.capability {
                        let mut resolved = self.resolve_capability(capability, requirement).await?;
                        started.append(&mut resolved);
                    }
                }
            }

            Ok(started)
        })
    }

    /// Check if a service needs to be started
    async fn needs_start(&self, datum: &BootDatum) -> Result<bool> {
        match datum.datum_type.as_ref() {
            Some(crate::DatumType::Docker) => {
                Ok(!self.is_docker_running(&datum.name).await?)
            }
            Some(crate::DatumType::Mcp) => {
                // MCP servers managed by session, not orchestrator
                Ok(false)
            }
            _ => Ok(false), // Other types don't need orchestration
        }
    }

    /// Check if docker container is running
    async fn is_docker_running(&self, name: &str) -> Result<bool> {
        let runtime = self.get_container_runtime()?;

        let output = Command::new(&runtime)
            .args(&["ps", "--filter", &format!("name={}", name), "--format", "{{.Names}}"])
            .output()?;

        let running = String::from_utf8_lossy(&output.stdout)
            .lines()
            .any(|line| line.contains(name));

        Ok(running)
    }

    /// Start a service based on its datum type
    async fn start_service(&self, datum: &BootDatum) -> Result<()> {
        match datum.datum_type.as_ref() {
            Some(crate::DatumType::Docker) => {
                self.start_docker_service(datum).await
            }
            _ => Ok(()), // No-op for other types
        }
    }

    /// Start docker container using datum configuration
    async fn start_docker_service(&self, datum: &BootDatum) -> Result<()> {
        let runtime = self.get_container_runtime()?;
        let image = datum.image.as_ref()
            .ok_or_else(|| anyhow::anyhow!("Docker datum missing image field"))?;

        // Build docker run command from datum
        let mut args = vec!["run", "-d", "--name", &datum.name];

        // Add port mappings and volume mounts from docker_args
        if let Some(docker_args) = &datum.docker_args {
            for arg in docker_args {
                args.push(arg);
            }
        }

        // Add environment variables
        let mut env_args = Vec::new();
        if let Some(env) = &datum.env {
            for (key, value) in env {
                env_args.push("-e".to_string());
                env_args.push(format!("{}={}", key, value));
            }
        }

        for env_arg in &env_args {
            args.push(env_arg);
        }

        args.push(image);

        // Execute docker run (silently)
        let output = Command::new(&runtime)
            .args(&args)
            .output()
            .context(format!("Failed to run {} container", datum.name))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            // Check if already running
            if stderr.contains("already in use") || stderr.contains("Conflict") {
                return Ok(()); // Already running, success
            }
            anyhow::bail!("Failed to start {}: {}", datum.name, stderr);
        }

        // Wait for service to be ready
        self.wait_for_ready(datum).await?;

        Ok(())
    }

    /// Wait for service to be ready (basic health check)
    async fn wait_for_ready(&self, datum: &BootDatum) -> Result<()> {
        let max_attempts = 30;
        let delay = Duration::from_millis(200);

        for _ in 0..max_attempts {
            if self.is_docker_running(&datum.name).await? {
                // Additional check: if env has URL, could ping it
                // For now, just verify container started
                sleep(Duration::from_millis(500)).await; // Grace period
                return Ok(());
            }
            sleep(delay).await;
        }

        anyhow::bail!("Timeout waiting for {} to start", datum.name)
    }

    /// Get container runtime (docker or podman)
    fn get_container_runtime(&self) -> Result<String> {
        if self.is_command_available("docker") {
            Ok("docker".to_string())
        } else if self.is_command_available("podman") {
            Ok("podman".to_string())
        } else {
            anyhow::bail!("Neither docker nor podman available")
        }
    }

    /// Check if command is in PATH
    fn is_command_available(&self, cmd: &str) -> bool {
        Command::new("which")
            .arg(cmd)
            .output()
            .map(|output| output.status.success())
            .unwrap_or(false)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_orchestrator_creation() {
        let path = std::env::var("_B00T_Path").unwrap_or_else(|_| "~/.b00t/_b00t_".to_string());
        let result = Orchestrator::new(&path);
        assert!(result.is_ok());
    }
}
