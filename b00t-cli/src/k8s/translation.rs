//! Translation engine for Docker → Kubernetes transformations

use k8s_openapi::api::core::v1::Pod;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

use crate::k8s::{
    error::{Error, Result},
    resources::{ContainerBuilder, PodBuilder, ResourceUtils, ServiceBuilder},
};

/// Simplified representation of a Dockerfile for translation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DockerfileSpec {
    /// Base image
    pub from: String,
    /// Working directory
    pub workdir: Option<String>,
    /// Exposed ports
    pub expose: Vec<u16>,
    /// Environment variables
    pub env: BTreeMap<String, String>,
    /// Command to run
    pub cmd: Option<Vec<String>>,
    /// Entry point
    pub entrypoint: Option<Vec<String>>,
    /// Copy/Add instructions (simplified)
    pub copy: Vec<CopyInstruction>,
    /// Run instructions (for metadata only)
    pub run: Vec<String>,
}

/// Represents a COPY or ADD instruction
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CopyInstruction {
    pub source: String,
    pub destination: String,
}

/// Simplified representation of docker-compose for translation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DockerComposeSpec {
    /// Service definitions
    pub services: BTreeMap<String, DockerComposeService>,
    /// Network definitions
    pub networks: Option<BTreeMap<String, DockerComposeNetwork>>,
    /// Volume definitions
    pub volumes: Option<BTreeMap<String, DockerComposeVolume>>,
}

/// Individual service in docker-compose
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DockerComposeService {
    /// Container image
    pub image: Option<String>,
    /// Build context (for dockerfile-based services)
    pub build: Option<String>,
    /// Port mappings
    pub ports: Option<Vec<String>>,
    /// Environment variables
    pub environment: Option<BTreeMap<String, String>>,
    /// Volume mounts
    pub volumes: Option<Vec<String>>,
    /// Command override
    pub command: Option<Vec<String>>,
    /// Depends on other services
    pub depends_on: Option<Vec<String>>,
    /// Restart policy
    pub restart: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DockerComposeNetwork {
    pub driver: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DockerComposeVolume {
    pub driver: Option<String>,
}

/// Translation result containing generated Kubernetes resources
#[derive(Debug, Clone)]
pub struct TranslationResult {
    /// Generated pods
    pub pods: Vec<Pod>,
    /// Generated services
    pub services: Vec<k8s_openapi::api::core::v1::Service>,
    /// Translation metadata
    pub metadata: TranslationMetadata,
}

/// Metadata about the translation process
#[derive(Debug, Clone)]
pub struct TranslationMetadata {
    /// Source type (dockerfile, compose, etc.)
    pub source_type: String,
    /// Generated resource names
    pub resource_names: Vec<String>,
    /// Warnings or notes
    pub warnings: Vec<String>,
}

/// Main translation engine
pub struct TranslationEngine {
    /// Whether to auto-generate services for exposed ports
    pub auto_generate_services: bool,
    /// Default image pull policy
    pub default_image_pull_policy: String,
    /// Default restart policy
    pub default_restart_policy: String,
    /// Default namespace
    pub namespace: String,
}

impl Default for TranslationEngine {
    fn default() -> Self {
        Self {
            auto_generate_services: true,
            default_image_pull_policy: "IfNotPresent".to_string(),
            default_restart_policy: "Always".to_string(),
            namespace: "default".to_string(),
        }
    }
}

impl TranslationEngine {
    /// Create a new translation engine
    pub fn new() -> Self {
        Self::default()
    }

    /// Configure the translation engine
    pub fn with_namespace(mut self, namespace: impl Into<String>) -> Self {
        self.namespace = namespace.into();
        self
    }

    /// Configure auto service generation
    pub fn with_auto_services(mut self, auto_generate: bool) -> Self {
        self.auto_generate_services = auto_generate;
        self
    }

    /// Translate a Dockerfile specification to Kubernetes resources
    pub fn translate_dockerfile(
        &self,
        name: &str,
        dockerfile: &DockerfileSpec,
    ) -> Result<TranslationResult> {
        let mut warnings = Vec::new();
        let pod_name = self.sanitize_name(name);

        // Build container from dockerfile
        let mut container_builder = ContainerBuilder::new(&pod_name, &dockerfile.from)
            .image_pull_policy(&self.default_image_pull_policy);

        // Add exposed ports
        for &port in &dockerfile.expose {
            container_builder = container_builder.port(port as i32, Some(format!("port-{}", port)));
        }

        // Add environment variables
        for (key, value) in &dockerfile.env {
            container_builder = container_builder.env(key, value);
        }

        // Set working directory
        if let Some(ref workdir) = dockerfile.workdir {
            container_builder = container_builder.working_dir(workdir);
        }

        // Set command/entrypoint
        if let Some(ref entrypoint) = dockerfile.entrypoint {
            container_builder = container_builder.command(entrypoint.clone());
            if let Some(ref cmd) = dockerfile.cmd {
                container_builder = container_builder.args(cmd.clone());
            }
        } else if let Some(ref cmd) = dockerfile.cmd {
            container_builder = container_builder.command(cmd.clone());
        }

        let container = container_builder.build();

        // Build pod
        let labels = ResourceUtils::standard_labels(&pod_name);
        let pod = PodBuilder::new(&pod_name)
            .namespace(&self.namespace)
            .labels(labels.clone())
            .container(container)
            .restart_policy(&self.default_restart_policy)
            .build()?;

        let pods = vec![pod];
        let mut services = Vec::new();

        // Generate service if ports are exposed and auto-generation is enabled
        if self.auto_generate_services && !dockerfile.expose.is_empty() {
            let service_name = format!("{}-service", pod_name);
            let mut service_builder = ServiceBuilder::new(&service_name)
                .namespace(&self.namespace)
                .selector(labels);

            for &port in &dockerfile.expose {
                service_builder = service_builder.port(
                    port as i32,
                    Some(port as i32),
                    Some(format!("port-{}", port)),
                );
            }

            match service_builder.build() {
                Ok(service) => services.push(service),
                Err(e) => warnings.push(format!("Failed to generate service: {}", e)),
            }
        }

        // Handle COPY instructions (add warnings since we can't actually copy files)
        if !dockerfile.copy.is_empty() {
            warnings.push("COPY instructions detected - you may need to build a custom image or use init containers".to_string());
        }

        // Handle RUN instructions
        if !dockerfile.run.is_empty() {
            warnings.push(
                "RUN instructions detected - these should be part of the image build process"
                    .to_string(),
            );
        }

        let resource_names = pods
            .iter()
            .filter_map(|p| p.metadata.name.clone())
            .chain(services.iter().filter_map(|s| s.metadata.name.clone()))
            .collect();

        Ok(TranslationResult {
            pods,
            services,
            metadata: TranslationMetadata {
                source_type: "dockerfile".to_string(),
                resource_names,
                warnings,
            },
        })
    }

    /// Translate a docker-compose specification to Kubernetes resources
    pub fn translate_compose(&self, compose: &DockerComposeSpec) -> Result<TranslationResult> {
        let mut pods = Vec::new();
        let mut services = Vec::new();
        let mut warnings = Vec::new();
        let mut resource_names = Vec::new();

        for (service_name, service_spec) in &compose.services {
            let pod_name = self.sanitize_name(service_name);

            // Determine the image
            let image = if let Some(ref img) = service_spec.image {
                img.clone()
            } else if service_spec.build.is_some() {
                warnings.push(format!("Service '{}' uses build context - you'll need to build and push the image first", service_name));
                format!("{}:latest", pod_name) // placeholder
            } else {
                return Err(Error::translation(
                    "docker-compose",
                    format!("Service '{}' has no image or build context", service_name),
                ));
            };

            // Build container
            let mut container_builder = ContainerBuilder::new(&pod_name, &image)
                .image_pull_policy(&self.default_image_pull_policy);

            // Add environment variables
            if let Some(ref env) = service_spec.environment {
                for (key, value) in env {
                    container_builder = container_builder.env(key, value);
                }
            }

            // Parse ports
            let mut exposed_ports = Vec::new();
            if let Some(ref ports) = service_spec.ports {
                for port_spec in ports {
                    if let Some(port) = self.parse_port_spec(port_spec) {
                        container_builder = container_builder.port(
                            port.container_port,
                            Some(format!("port-{}", port.container_port)),
                        );
                        exposed_ports.push(port);
                    } else {
                        warnings.push(format!(
                            "Could not parse port spec '{}' for service '{}'",
                            port_spec, service_name
                        ));
                    }
                }
            }

            // Set command
            if let Some(ref command) = service_spec.command {
                container_builder = container_builder.command(command.clone());
            }

            let container = container_builder.build();

            // Build pod
            let labels = ResourceUtils::standard_labels(&pod_name);
            let restart_policy = service_spec
                .restart
                .as_deref()
                .unwrap_or(&self.default_restart_policy);

            let pod = PodBuilder::new(&pod_name)
                .namespace(&self.namespace)
                .labels(labels.clone())
                .container(container)
                .restart_policy(restart_policy)
                .build()?;

            pods.push(pod);
            resource_names.push(pod_name.clone());

            // Generate service if ports are exposed
            if self.auto_generate_services && !exposed_ports.is_empty() {
                let service_name = format!("{}-service", pod_name);
                let mut service_builder = ServiceBuilder::new(&service_name)
                    .namespace(&self.namespace)
                    .selector(labels);

                for port in exposed_ports {
                    service_builder = service_builder.port(
                        port.host_port.unwrap_or(port.container_port),
                        Some(port.container_port),
                        Some(format!("port-{}", port.container_port)),
                    );
                }

                match service_builder.build() {
                    Ok(service) => {
                        services.push(service);
                        resource_names.push(service_name);
                    }
                    Err(e) => warnings.push(format!(
                        "Failed to generate service for '{}': {}",
                        service_name, e
                    )),
                }
            }

            // Handle volumes (add warnings)
            if let Some(ref _volumes) = service_spec.volumes {
                warnings.push(format!("Service '{}' has volume mounts - you may need to create PersistentVolumes manually", service_name));
            }

            // Handle depends_on (add warnings)
            if let Some(ref deps) = service_spec.depends_on {
                warnings.push(format!(
                    "Service '{}' has dependencies {:?} - ensure proper startup order",
                    service_name, deps
                ));
            }
        }

        Ok(TranslationResult {
            pods,
            services,
            metadata: TranslationMetadata {
                source_type: "docker-compose".to_string(),
                resource_names,
                warnings,
            },
        })
    }

    /// Parse a docker-compose port specification
    fn parse_port_spec(&self, port_spec: &str) -> Option<PortMapping> {
        // Handle formats like "80:8080", "8080", "127.0.0.1:80:8080"
        let parts: Vec<&str> = port_spec.split(':').collect();

        match parts.len() {
            1 => {
                // Just container port: "8080"
                if let Ok(port) = parts[0].parse::<i32>() {
                    Some(PortMapping {
                        container_port: port,
                        host_port: Some(port),
                    })
                } else {
                    None
                }
            }
            2 => {
                // Host:container: "80:8080"
                if let (Ok(host_port), Ok(container_port)) =
                    (parts[0].parse::<i32>(), parts[1].parse::<i32>())
                {
                    Some(PortMapping {
                        container_port,
                        host_port: Some(host_port),
                    })
                } else {
                    None
                }
            }
            3 => {
                // IP:host:container: "127.0.0.1:80:8080" (ignore IP for now)
                if let (Ok(host_port), Ok(container_port)) =
                    (parts[1].parse::<i32>(), parts[2].parse::<i32>())
                {
                    Some(PortMapping {
                        container_port,
                        host_port: Some(host_port),
                    })
                } else {
                    None
                }
            }
            _ => None,
        }
    }

    /// Sanitize a name to be valid for Kubernetes
    fn sanitize_name(&self, name: &str) -> String {
        name.to_lowercase()
            .chars()
            .map(|c| {
                if c.is_alphanumeric() || c == '-' {
                    c
                } else {
                    '-'
                }
            })
            .collect::<String>()
            .trim_matches('-')
            .to_string()
    }
}

/// Port mapping information
#[derive(Debug, Clone)]
struct PortMapping {
    container_port: i32,
    host_port: Option<i32>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_dockerfile_translation() {
        let dockerfile = DockerfileSpec {
            from: "nginx:latest".to_string(),
            workdir: Some("/app".to_string()),
            expose: vec![80, 443],
            env: {
                let mut env = BTreeMap::new();
                env.insert("NODE_ENV".to_string(), "production".to_string());
                env
            },
            cmd: Some(vec![
                "nginx".to_string(),
                "-g".to_string(),
                "daemon off;".to_string(),
            ]),
            entrypoint: None,
            copy: vec![],
            run: vec![],
        };

        let engine = TranslationEngine::new();
        let result = engine
            .translate_dockerfile("web-server", &dockerfile)
            .unwrap();

        assert_eq!(result.pods.len(), 1);
        assert_eq!(result.services.len(), 1); // Auto-generated service
        assert_eq!(result.metadata.source_type, "dockerfile");

        let pod = &result.pods[0];
        assert_eq!(pod.metadata.name, Some("web-server".to_string()));
        assert!(pod.spec.is_some());
    }

    #[test]
    fn test_compose_translation() {
        let mut services = BTreeMap::new();
        services.insert(
            "web".to_string(),
            DockerComposeService {
                image: Some("nginx:latest".to_string()),
                build: None,
                ports: Some(vec!["80:8080".to_string()]),
                environment: Some({
                    let mut env = BTreeMap::new();
                    env.insert("ENV".to_string(), "prod".to_string());
                    env
                }),
                volumes: None,
                command: None,
                depends_on: None,
                restart: Some("always".to_string()),
            },
        );

        let compose = DockerComposeSpec {
            services,
            networks: None,
            volumes: None,
        };

        let engine = TranslationEngine::new();
        let result = engine.translate_compose(&compose).unwrap();

        assert_eq!(result.pods.len(), 1);
        assert_eq!(result.services.len(), 1);
        assert_eq!(result.metadata.source_type, "docker-compose");
    }

    #[test]
    fn test_port_parsing() {
        let engine = TranslationEngine::new();

        let port1 = engine.parse_port_spec("8080").unwrap();
        assert_eq!(port1.container_port, 8080);
        assert_eq!(port1.host_port, Some(8080));

        let port2 = engine.parse_port_spec("80:8080").unwrap();
        assert_eq!(port2.container_port, 8080);
        assert_eq!(port2.host_port, Some(80));

        let port3 = engine.parse_port_spec("127.0.0.1:80:8080").unwrap();
        assert_eq!(port3.container_port, 8080);
        assert_eq!(port3.host_port, Some(80));
    }

    #[test]
    fn test_name_sanitization() {
        let engine = TranslationEngine::new();

        assert_eq!(engine.sanitize_name("Web_Server!"), "web-server");
        assert_eq!(engine.sanitize_name("my-app"), "my-app");
        assert_eq!(engine.sanitize_name("App123"), "app123");
    }
}
