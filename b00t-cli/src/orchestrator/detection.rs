// Orchestrator auto-detection
// Detects which orchestrators are available on the system

use anyhow::Result;
use std::process::Command;

use super::adapter::Orchestrator;

/// Detect the best available orchestrator
pub fn detect_orchestrator() -> Result<Orchestrator> {
    // Priority order: Kubernetes > Docker Compose > Nomad > Direct

    // 1. Check for Kubernetes (kubectl context configured)
    if check_kubernetes() {
        return Ok(Orchestrator::Kubernetes);
    }

    // 2. Check for Docker Compose
    if check_docker_compose() {
        return Ok(Orchestrator::DockerCompose);
    }

    // 3. Check for Nomad
    if check_nomad() {
        return Ok(Orchestrator::Nomad);
    }

    // 4. Fallback to direct execution (always available)
    Ok(Orchestrator::Direct)
}

/// List all available orchestrators
pub fn list_available_orchestrators() -> Vec<(Orchestrator, bool)> {
    vec![
        (Orchestrator::Kubernetes, check_kubernetes()),
        (Orchestrator::DockerCompose, check_docker_compose()),
        (Orchestrator::Nomad, check_nomad()),
        (Orchestrator::Direct, true), // Always available
    ]
}

fn check_kubernetes() -> bool {
    // Check if kubectl is installed and context is configured
    Command::new("kubectl")
        .args(&["config", "current-context"])
        .output()
        .map(|out| out.status.success())
        .unwrap_or(false)
}

fn check_docker_compose() -> bool {
    // Check if docker-compose is installed
    Command::new("docker-compose")
        .arg("version")
        .output()
        .map(|out| out.status.success())
        .unwrap_or_else(|_| {
            // Try docker compose (v2 syntax)
            Command::new("docker")
                .args(&["compose", "version"])
                .output()
                .map(|out| out.status.success())
                .unwrap_or(false)
        })
}

fn check_nomad() -> bool {
    // Check if nomad is installed
    Command::new("nomad")
        .arg("version")
        .output()
        .map(|out| out.status.success())
        .unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_detect_orchestrator() {
        // Should always return something
        let result = detect_orchestrator();
        assert!(result.is_ok());
    }

    #[test]
    fn test_list_available() {
        let list = list_available_orchestrators();
        assert_eq!(list.len(), 4);

        // Direct should always be available
        let (orch, available) = list.last().unwrap();
        assert_eq!(*orch, Orchestrator::Direct);
        assert!(available);
    }
}
