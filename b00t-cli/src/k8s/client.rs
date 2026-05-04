//! Kubernetes client wrapper for the b00t k8s subsystem

use k8s_openapi::api::core::v1::{Namespace, Pod};
use kube::{
    Client, Config,
    api::{Api, DeleteParams, ListParams, PostParams},
    config::{KubeConfigOptions, Kubeconfig},
};
use std::collections::BTreeMap;
use tracing::{debug, info, warn};

use crate::k8s::{
    MANAGED_BY_LABEL, MANAGED_BY_VALUE,
    config::K8sConfig,
    error::{Error, Result},
};

/// b00t Kubernetes client wrapper
///
/// Provides a simplified interface to kube-rs for common operations
/// while maintaining compatibility with the existing K8sDatum structure.
pub struct K8sClient {
    client: Client,
    config: K8sConfig,
}

impl K8sClient {
    /// Create a new K8sClient with default configuration
    pub async fn new() -> Result<Self> {
        let config = K8sConfig::default();
        Self::with_config(config).await
    }

    /// Create a new K8sClient with custom configuration
    pub async fn with_config(config: K8sConfig) -> Result<Self> {
        config.validate()?;

        let kube_config = Self::build_kube_config(&config).await?;
        let client = Client::try_from(kube_config).map_err(|e| Error::ClusterConnection {
            details: format!("Failed to create kube client: {}", e),
        })?;

        info!("Connected to Kubernetes cluster");
        debug!("Using namespace: {}", config.namespace);

        let k8s_client = Self { client, config };

        // Ensure namespace exists if auto_create_namespace is enabled
        if k8s_client.config.auto_create_namespace {
            k8s_client.ensure_namespace().await?;
        }

        Ok(k8s_client)
    }

    /// Build kube::Config from K8sConfig
    async fn build_kube_config(config: &K8sConfig) -> Result<Config> {
        let kube_config = if let Some(kubeconfig_path) = config.effective_kubeconfig_path() {
            let kubeconfig =
                Kubeconfig::read_from(kubeconfig_path).map_err(|e| Error::Configuration {
                    message: format!("Failed to read kubeconfig: {}", e),
                })?;

            let options = KubeConfigOptions {
                context: config.context.clone(),
                cluster: None,
                user: None,
            };

            Config::from_custom_kubeconfig(kubeconfig, &options)
                .await
                .map_err(|e| Error::Configuration {
                    message: format!("Failed to build config from kubeconfig: {}", e),
                })?
        } else {
            // Try in-cluster config
            Config::incluster().map_err(|e| Error::Configuration {
                message: format!("Failed to create in-cluster config: {}", e),
            })?
        };

        Ok(kube_config)
    }

    /// Ensure the configured namespace exists
    async fn ensure_namespace(&self) -> Result<()> {
        let namespaces: Api<Namespace> = Api::all(self.client.clone());

        // Check if namespace already exists
        if namespaces.get(&self.config.namespace).await.is_ok() {
            debug!("Namespace '{}' already exists", self.config.namespace);
            return Ok(());
        }

        // Create namespace
        let mut labels = BTreeMap::new();
        labels.insert(MANAGED_BY_LABEL.to_string(), MANAGED_BY_VALUE.to_string());
        labels.insert("b00t.type".to_string(), "auto-created".to_string());

        let namespace = Namespace {
            metadata: kube::api::ObjectMeta {
                name: Some(self.config.namespace.clone()),
                labels: Some(labels),
                ..Default::default()
            },
            ..Default::default()
        };

        namespaces
            .create(&PostParams::default(), &namespace)
            .await
            .map_err(|e| Error::Namespace {
                namespace: self.config.namespace.clone(),
                message: format!("Failed to create namespace: {}", e),
            })?;

        info!("Created namespace: {}", self.config.namespace);
        Ok(())
    }

    /// Get the current namespace
    pub fn namespace(&self) -> &str {
        &self.config.namespace
    }

    /// Get the underlying kube client
    pub fn client(&self) -> &Client {
        &self.client
    }

    /// Get the k8s configuration
    pub fn config(&self) -> &K8sConfig {
        &self.config
    }

    /// Create a namespaced API instance for a specific resource type
    pub fn api<K>(&self) -> Api<K>
    where
        K: Clone
            + serde::de::DeserializeOwned
            + kube::Resource<Scope = kube::core::NamespaceResourceScope>,
        <K as kube::Resource>::DynamicType: Default,
    {
        Api::namespaced(self.client.clone(), &self.config.namespace)
    }

    /// Create a cluster-wide API instance for a specific resource type
    pub fn api_all<K>(&self) -> Api<K>
    where
        K: Clone
            + serde::de::DeserializeOwned
            + kube::Resource<Scope = kube::core::ClusterResourceScope>,
        <K as kube::Resource>::DynamicType: Default,
    {
        Api::all(self.client.clone())
    }

    /// Deploy a pod with b00t-specific labels and defaults
    pub async fn deploy_pod(&self, mut pod: Pod) -> Result<Pod> {
        // Ensure metadata exists
        if pod.metadata.labels.is_none() {
            pod.metadata.labels = Some(BTreeMap::new());
        }

        // Add b00t-managed labels
        let labels = pod.metadata.labels.as_mut().unwrap();
        for (key, value) in &self.config.default_labels {
            labels.insert(key.clone(), value.clone());
        }

        // Apply resource defaults if not specified
        if let Some(ref mut spec) = pod.spec {
            for container in &mut spec.containers {
                if container.resources.is_none() {
                    container.resources = Some(k8s_openapi::api::core::v1::ResourceRequirements {
                        requests: self.build_resource_map(true),
                        limits: self.build_resource_map(false),
                        ..Default::default()
                    });
                }
            }
        }

        let pods: Api<Pod> = self.api();
        let pod_name = pod
            .metadata
            .name
            .as_ref()
            .ok_or_else(|| Error::PodDeployment {
                pod_name: "unnamed".to_string(),
                reason: "Pod must have a name".to_string(),
            })?;

        debug!("Deploying pod: {}", pod_name);

        let result = pods
            .create(&PostParams::default(), &pod)
            .await
            .map_err(|e| Error::PodDeployment {
                pod_name: pod_name.clone(),
                reason: format!("Kubernetes API error: {}", e),
            })?;

        info!("Successfully deployed pod: {}", pod_name);
        Ok(result)
    }

    /// List all pods managed by b00t
    pub async fn list_b00t_pods(&self) -> Result<Vec<Pod>> {
        let pods: Api<Pod> = self.api();
        let list_params =
            ListParams::default().labels(&format!("{}={}", MANAGED_BY_LABEL, MANAGED_BY_VALUE));

        let pod_list = pods.list(&list_params).await.map_err(|e| Error::Resource {
            resource_name: "pods".to_string(),
            message: format!("Failed to list pods: {}", e),
        })?;

        debug!("Found {} b00t-managed pods", pod_list.items.len());
        Ok(pod_list.items)
    }

    /// List all pods in a namespace (or all namespaces if None)
    pub async fn list_all_pods(&self, namespace: Option<&str>) -> Result<Vec<Pod>> {
        let list_params = ListParams::default();

        let pods: Api<Pod> = if let Some(ns) = namespace {
            Api::namespaced(self.client.clone(), ns)
        } else {
            Api::all(self.client.clone())
        };

        let pod_list = pods.list(&list_params).await.map_err(|e| Error::Resource {
            resource_name: "pods".to_string(),
            message: format!("Failed to list all pods: {}", e),
        })?;

        debug!("Found {} total pods", pod_list.items.len());
        Ok(pod_list.items)
    }

    /// Get a specific pod by name
    pub async fn get_pod(&self, name: &str) -> Result<Pod> {
        let pods: Api<Pod> = self.api();
        pods.get(name).await.map_err(|e| Error::Resource {
            resource_name: format!("pod/{}", name),
            message: format!("Failed to get pod: {}", e),
        })
    }

    /// Delete a pod by name
    pub async fn delete_pod(&self, name: &str) -> Result<()> {
        let pods: Api<Pod> = self.api();
        pods.delete(name, &DeleteParams::default())
            .await
            .map_err(|e| Error::Resource {
                resource_name: format!("pod/{}", name),
                message: format!("Failed to delete pod: {}", e),
            })?;

        info!("Deleted pod: {}", name);
        Ok(())
    }

    /// Get logs from a pod
    pub async fn get_pod_logs(&self, name: &str) -> Result<String> {
        let pods: Api<Pod> = self.api();
        let logs = pods
            .logs(name, &Default::default())
            .await
            .map_err(|e| Error::Resource {
                resource_name: format!("pod/{}", name),
                message: format!("Failed to get logs: {}", e),
            })?;

        Ok(logs)
    }

    /// Check if a pod is running
    pub async fn is_pod_running(&self, name: &str) -> Result<bool> {
        let pod = self.get_pod(name).await?;

        if let Some(status) = pod.status {
            if let Some(phase) = status.phase {
                return Ok(phase == "Running");
            }
        }

        Ok(false)
    }

    /// Wait for a pod to be ready
    pub async fn wait_for_pod_ready(&self, name: &str) -> Result<()> {
        use tokio::time::{Duration, sleep};

        let timeout = Duration::from_secs(self.config.timeout_seconds);
        let poll_interval = Duration::from_secs(2);
        let start = std::time::Instant::now();

        loop {
            if start.elapsed() > timeout {
                return Err(Error::Timeout {
                    timeout_seconds: self.config.timeout_seconds,
                    operation: format!("waiting for pod '{}' to be ready", name),
                });
            }

            match self.get_pod(name).await {
                Ok(pod) => {
                    if let Some(status) = pod.status {
                        if let Some(conditions) = status.conditions {
                            let ready = conditions
                                .iter()
                                .any(|c| c.type_ == "Ready" && c.status == "True");

                            if ready {
                                info!("Pod '{}' is ready", name);
                                return Ok(());
                            }
                        }
                    }
                }
                Err(e) => {
                    warn!("Error checking pod status: {}", e);
                }
            }

            sleep(poll_interval).await;
        }
    }

    /// Helper to build resource requirements map
    fn build_resource_map(
        &self,
        requests: bool,
    ) -> Option<BTreeMap<String, k8s_openapi::apimachinery::pkg::api::resource::Quantity>> {
        let mut map = BTreeMap::new();
        let defaults = &self.config.resource_defaults;

        if requests {
            if let Some(ref cpu) = defaults.cpu_request {
                map.insert(
                    "cpu".to_string(),
                    k8s_openapi::apimachinery::pkg::api::resource::Quantity(cpu.clone()),
                );
            }
            if let Some(ref memory) = defaults.memory_request {
                map.insert(
                    "memory".to_string(),
                    k8s_openapi::apimachinery::pkg::api::resource::Quantity(memory.clone()),
                );
            }
        } else {
            if let Some(ref cpu) = defaults.cpu_limit {
                map.insert(
                    "cpu".to_string(),
                    k8s_openapi::apimachinery::pkg::api::resource::Quantity(cpu.clone()),
                );
            }
            if let Some(ref memory) = defaults.memory_limit {
                map.insert(
                    "memory".to_string(),
                    k8s_openapi::apimachinery::pkg::api::resource::Quantity(memory.clone()),
                );
            }
        }

        if map.is_empty() { None } else { Some(map) }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::k8s::DEFAULT_NAMESPACE;

    #[test]
    fn test_config_validation() {
        let config = K8sConfig::default();
        assert!(config.validate().is_ok());
    }

    #[test]
    fn test_resource_map_building() {
        let client_config = K8sConfig::default();
        // Note: We can't easily test K8sClient without a real cluster
        // These tests would be better in integration tests
        assert_eq!(client_config.namespace, DEFAULT_NAMESPACE);
    }

    // Integration tests would go here and require a test cluster
    // For now, focusing on unit tests for the configuration logic
}
