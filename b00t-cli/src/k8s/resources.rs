//! Resource management for Kubernetes objects

use k8s_openapi::api::core::v1::{
    Container, ContainerPort, Pod, PodSpec, Service, ServicePort, ServiceSpec,
};
use k8s_openapi::apimachinery::pkg::apis::meta::v1::ObjectMeta;
use k8s_openapi::apimachinery::pkg::util::intstr::IntOrString;
use std::collections::BTreeMap;
use uuid::Uuid;

use crate::k8s::{
    LABEL_PREFIX, MANAGED_BY_LABEL, MANAGED_BY_VALUE,
    error::{Error, Result},
};

/// Builder for creating Kubernetes Pod specifications
#[derive(Debug, Clone)]
pub struct PodBuilder {
    name: String,
    namespace: String,
    labels: BTreeMap<String, String>,
    containers: Vec<Container>,
    restart_policy: Option<String>,
}

impl PodBuilder {
    /// Create a new PodBuilder
    pub fn new(name: impl Into<String>) -> Self {
        let mut labels = BTreeMap::new();
        labels.insert(MANAGED_BY_LABEL.to_string(), MANAGED_BY_VALUE.to_string());

        Self {
            name: name.into(),
            namespace: "default".to_string(),
            labels,
            containers: Vec::new(),
            restart_policy: None,
        }
    }

    /// Set the namespace
    pub fn namespace(mut self, namespace: impl Into<String>) -> Self {
        self.namespace = namespace.into();
        self
    }

    /// Add a label
    pub fn label(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.labels.insert(key.into(), value.into());
        self
    }

    /// Add multiple labels
    pub fn labels(mut self, labels: BTreeMap<String, String>) -> Self {
        self.labels.extend(labels);
        self
    }

    /// Add a container
    pub fn container(mut self, container: Container) -> Self {
        self.containers.push(container);
        self
    }

    /// Set restart policy
    pub fn restart_policy(mut self, policy: impl Into<String>) -> Self {
        self.restart_policy = Some(policy.into());
        self
    }

    /// Build the Pod
    pub fn build(self) -> Result<Pod> {
        if self.containers.is_empty() {
            return Err(Error::resource(
                &self.name,
                "Pod must have at least one container",
            ));
        }

        let metadata = ObjectMeta {
            name: Some(self.name.clone()),
            namespace: Some(self.namespace),
            labels: Some(self.labels),
            ..Default::default()
        };

        let spec = PodSpec {
            containers: self.containers,
            restart_policy: self.restart_policy,
            ..Default::default()
        };

        Ok(Pod {
            metadata,
            spec: Some(spec),
            ..Default::default()
        })
    }
}

/// Builder for creating Kubernetes Container specifications
#[derive(Debug, Clone)]
pub struct ContainerBuilder {
    name: String,
    image: String,
    ports: Vec<ContainerPort>,
    env: Vec<k8s_openapi::api::core::v1::EnvVar>,
    command: Option<Vec<String>>,
    args: Option<Vec<String>>,
    working_dir: Option<String>,
    image_pull_policy: Option<String>,
}

impl ContainerBuilder {
    /// Create a new ContainerBuilder
    pub fn new(name: impl Into<String>, image: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            image: image.into(),
            ports: Vec::new(),
            env: Vec::new(),
            command: None,
            args: None,
            working_dir: None,
            image_pull_policy: None,
        }
    }

    /// Add a port
    pub fn port(mut self, container_port: i32, name: Option<String>) -> Self {
        self.ports.push(ContainerPort {
            container_port,
            name,
            protocol: Some("TCP".to_string()),
            ..Default::default()
        });
        self
    }

    /// Add an environment variable
    pub fn env(mut self, name: impl Into<String>, value: impl Into<String>) -> Self {
        self.env.push(k8s_openapi::api::core::v1::EnvVar {
            name: name.into(),
            value: Some(value.into()),
            ..Default::default()
        });
        self
    }

    /// Set command
    pub fn command(mut self, command: Vec<String>) -> Self {
        self.command = Some(command);
        self
    }

    /// Set args
    pub fn args(mut self, args: Vec<String>) -> Self {
        self.args = Some(args);
        self
    }

    /// Set working directory
    pub fn working_dir(mut self, working_dir: impl Into<String>) -> Self {
        self.working_dir = Some(working_dir.into());
        self
    }

    /// Set image pull policy
    pub fn image_pull_policy(mut self, policy: impl Into<String>) -> Self {
        self.image_pull_policy = Some(policy.into());
        self
    }

    /// Build the Container
    pub fn build(self) -> Container {
        Container {
            name: self.name,
            image: Some(self.image),
            ports: if self.ports.is_empty() {
                None
            } else {
                Some(self.ports)
            },
            env: if self.env.is_empty() {
                None
            } else {
                Some(self.env)
            },
            command: self.command,
            args: self.args,
            working_dir: self.working_dir,
            image_pull_policy: self.image_pull_policy,
            ..Default::default()
        }
    }
}

/// Builder for creating Kubernetes Service specifications
#[derive(Debug, Clone)]
pub struct ServiceBuilder {
    name: String,
    namespace: String,
    labels: BTreeMap<String, String>,
    selector: BTreeMap<String, String>,
    ports: Vec<ServicePort>,
    service_type: Option<String>,
}

impl ServiceBuilder {
    /// Create a new ServiceBuilder
    pub fn new(name: impl Into<String>) -> Self {
        let mut labels = BTreeMap::new();
        labels.insert(MANAGED_BY_LABEL.to_string(), MANAGED_BY_VALUE.to_string());

        Self {
            name: name.into(),
            namespace: "default".to_string(),
            labels,
            selector: BTreeMap::new(),
            ports: Vec::new(),
            service_type: None,
        }
    }

    /// Set the namespace
    pub fn namespace(mut self, namespace: impl Into<String>) -> Self {
        self.namespace = namespace.into();
        self
    }

    /// Add a label
    pub fn label(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.labels.insert(key.into(), value.into());
        self
    }

    /// Set selector
    pub fn selector(mut self, selector: BTreeMap<String, String>) -> Self {
        self.selector = selector;
        self
    }

    /// Add a port mapping
    pub fn port(mut self, port: i32, target_port: Option<i32>, name: Option<String>) -> Self {
        self.ports.push(ServicePort {
            port,
            target_port: target_port.map(IntOrString::Int),
            name,
            protocol: Some("TCP".to_string()),
            ..Default::default()
        });
        self
    }

    /// Set service type
    pub fn service_type(mut self, service_type: impl Into<String>) -> Self {
        self.service_type = Some(service_type.into());
        self
    }

    /// Build the Service
    pub fn build(self) -> Result<Service> {
        if self.ports.is_empty() {
            return Err(Error::resource(
                &self.name,
                "Service must have at least one port",
            ));
        }

        if self.selector.is_empty() {
            return Err(Error::resource(&self.name, "Service must have a selector"));
        }

        let metadata = ObjectMeta {
            name: Some(self.name.clone()),
            namespace: Some(self.namespace),
            labels: Some(self.labels),
            ..Default::default()
        };

        let spec = ServiceSpec {
            selector: Some(self.selector),
            ports: Some(self.ports),
            type_: self.service_type,
            ..Default::default()
        };

        Ok(Service {
            metadata,
            spec: Some(spec),
            ..Default::default()
        })
    }
}

/// Utilities for working with Kubernetes resources
pub struct ResourceUtils;

impl ResourceUtils {
    /// Generate a unique name with prefix
    pub fn generate_name(prefix: &str) -> String {
        let uuid = Uuid::new_v4();
        let short_uuid = &uuid.to_string()[..8];
        format!("{}-{}", prefix, short_uuid)
    }

    /// Create standard b00t labels
    pub fn standard_labels(app_name: &str) -> BTreeMap<String, String> {
        let mut labels = BTreeMap::new();
        labels.insert(MANAGED_BY_LABEL.to_string(), MANAGED_BY_VALUE.to_string());
        labels.insert("app.kubernetes.io/name".to_string(), app_name.to_string());
        labels.insert(format!("{}/app", LABEL_PREFIX), app_name.to_string());
        labels
    }

    /// Create labels for a specific instance
    pub fn instance_labels(app_name: &str, instance: &str) -> BTreeMap<String, String> {
        let mut labels = Self::standard_labels(app_name);
        labels.insert(
            "app.kubernetes.io/instance".to_string(),
            instance.to_string(),
        );
        labels.insert(format!("{}/instance", LABEL_PREFIX), instance.to_string());
        labels
    }

    /// Extract app name from labels
    pub fn extract_app_name(labels: &BTreeMap<String, String>) -> Option<String> {
        labels
            .get("app.kubernetes.io/name")
            .cloned()
            .or_else(|| labels.get(&format!("{}/app", LABEL_PREFIX)).cloned())
    }

    /// Check if resource is managed by b00t
    pub fn is_b00t_managed(labels: &BTreeMap<String, String>) -> bool {
        labels
            .get(MANAGED_BY_LABEL)
            .map(|v| v == MANAGED_BY_VALUE)
            .unwrap_or(false)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pod_builder() {
        let container = ContainerBuilder::new("test-container", "nginx:latest")
            .port(80, Some("http".to_string()))
            .env("ENV_VAR", "value")
            .build();

        let pod = PodBuilder::new("test-pod")
            .namespace("test-namespace")
            .label("app", "test-app")
            .container(container)
            .restart_policy("Always")
            .build()
            .unwrap();

        assert_eq!(pod.metadata.name, Some("test-pod".to_string()));
        assert_eq!(pod.metadata.namespace, Some("test-namespace".to_string()));
        assert!(pod.metadata.labels.as_ref().unwrap().contains_key("app"));
        assert!(pod.spec.is_some());
    }

    #[test]
    fn test_container_builder() {
        let container = ContainerBuilder::new("test", "nginx:latest")
            .port(80, Some("http".to_string()))
            .env("TEST", "value")
            .command(vec!["sh".to_string()])
            .args(vec!["-c".to_string(), "echo hello".to_string()])
            .working_dir("/app")
            .image_pull_policy("IfNotPresent")
            .build();

        assert_eq!(container.name, "test");
        assert_eq!(container.image, Some("nginx:latest".to_string()));
        assert!(container.ports.is_some());
        assert!(container.env.is_some());
        assert!(container.command.is_some());
        assert!(container.args.is_some());
        assert_eq!(container.working_dir, Some("/app".to_string()));
    }

    #[test]
    fn test_service_builder() {
        let mut selector = BTreeMap::new();
        selector.insert("app".to_string(), "test-app".to_string());

        let service = ServiceBuilder::new("test-service")
            .namespace("test-namespace")
            .selector(selector)
            .port(80, Some(8080), Some("http".to_string()))
            .service_type("ClusterIP")
            .build()
            .unwrap();

        assert_eq!(service.metadata.name, Some("test-service".to_string()));
        assert_eq!(
            service.metadata.namespace,
            Some("test-namespace".to_string())
        );
        assert!(service.spec.is_some());
        assert!(service.spec.as_ref().unwrap().ports.is_some());
    }

    #[test]
    fn test_resource_utils() {
        let name = ResourceUtils::generate_name("test");
        assert!(name.starts_with("test-"));
        assert!(name.len() > 5);

        let labels = ResourceUtils::standard_labels("my-app");
        assert_eq!(
            labels.get(MANAGED_BY_LABEL),
            Some(&MANAGED_BY_VALUE.to_string())
        );
        assert_eq!(
            labels.get("app.kubernetes.io/name"),
            Some(&"my-app".to_string())
        );

        assert!(ResourceUtils::is_b00t_managed(&labels));

        let app_name = ResourceUtils::extract_app_name(&labels);
        assert_eq!(app_name, Some("my-app".to_string()));
    }
}
