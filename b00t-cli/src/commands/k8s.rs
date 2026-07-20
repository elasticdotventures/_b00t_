use anyhow::{Context, Result};
use clap::Parser;

/// KubeClient wraps kube-rs for b00t k8s operations.
/// 🤓 kube 4.x API: Api<T>::list() returns ObjectList<T>, logs() returns LogStream.
struct KubeClient {
    client: kube::Client,
}

impl KubeClient {
    async fn new() -> Result<Self> {
        let client = kube::Client::try_default().await.context(
            "failed to create kube client — is kubectl configured? (check ~/.kube/config)",
        )?;
        Ok(KubeClient { client })
    }

    /// List pods, optionally filtered by namespace and b00t-managed pods only.
    async fn list_pods(&self, namespace: Option<&str>, all: bool) -> Result<Vec<PodSummary>> {
        use kube::api::ListParams;
        let ns = namespace.unwrap_or("default");
        let api: kube::Api<k8s_openapi::api::core::v1::Pod> =
            kube::Api::namespaced(self.client.clone(), ns);
        let pods = api
            .list(&ListParams::default())
            .await
            .context("failed to list pods")?;
        Ok(pods
            .items
            .into_iter()
            .filter(|p| {
                all || p
                    .metadata
                    .labels
                    .as_ref()
                    .map(|l| {
                        l.contains_key("app.kubernetes.io/managed-by")
                            || l.contains_key("b00t.io/managed")
                    })
                    .unwrap_or(false)
            })
            .map(|p| {
                let name = p.metadata.name.unwrap_or_default();
                let status = p
                    .status
                    .as_ref()
                    .and_then(|s| s.phase.as_deref())
                    .unwrap_or("Unknown")
                    .to_string();
                let namespace = p.metadata.namespace.unwrap_or_default();
                let ready = p
                    .status
                    .as_ref()
                    .and_then(|s| s.container_statuses.as_ref())
                    .map(|cs| cs.iter().filter(|c| c.ready).count())
                    .unwrap_or(0);
                let total = p
                    .status
                    .as_ref()
                    .and_then(|s| s.container_statuses.as_ref())
                    .map(|cs| cs.len())
                    .unwrap_or(0);
                PodSummary {
                    name,
                    namespace,
                    status,
                    ready,
                    total,
                }
            })
            .collect())
    }

    /// Get logs for a specific pod.
    async fn get_logs(
        &self,
        pod_name: &str,
        namespace: Option<&str>,
        previous: bool,
    ) -> Result<()> {
        use kube::api::LogParams;
        let ns = namespace.unwrap_or("default");
        let api: kube::Api<k8s_openapi::api::core::v1::Pod> =
            kube::Api::namespaced(self.client.clone(), ns);
        let params = LogParams {
            follow: false, // streaming follow requires async stdout, deferred to Task 4.4
            previous,
            ..LogParams::default()
        };
        let logs = api.logs(pod_name, &params).await.context(format!(
            "failed to get logs for pod '{pod_name}' in namespace '{ns}'"
        ))?;
        print!("{}", logs);
        Ok(())
    }
}

#[derive(serde::Serialize)]
struct PodSummary {
    name: String,
    namespace: String,
    status: String,
    ready: usize,
    total: usize,
}

#[derive(Parser)]
pub enum K8sCommands {
    #[clap(
        about = "Deploy a pod from Dockerfile or docker-compose",
        long_about = "Deploy a pod from Dockerfile or docker-compose.\n\nExamples:\n  b00t-cli k8s deploy --from-dockerfile ./Dockerfile --name web-server\n  b00t-cli k8s deploy --from-compose ./docker-compose.yaml\n  b00t-cli k8s deploy --image nginx:latest --name nginx-test"
    )]
    Deploy {
        #[clap(long, help = "Deploy from Dockerfile", conflicts_with_all = &["from_compose", "image"])]
        from_dockerfile: Option<String>,
        #[clap(long, help = "Deploy from docker-compose.yaml", conflicts_with_all = &["from_dockerfile", "image"])]
        from_compose: Option<String>,
        #[clap(long, help = "Deploy from container image", conflicts_with_all = &["from_dockerfile", "from_compose"])]
        image: Option<String>,
        #[clap(long, help = "Pod name (required for dockerfile/image deployment)")]
        name: Option<String>,
        #[clap(long, help = "Namespace (default: default)")]
        namespace: Option<String>,
        #[clap(long, help = "Environment variables in KEY=VALUE format")]
        env: Vec<String>,
    },
    #[clap(
        about = "Deploy MCP server as Kubernetes pod",
        long_about = "Deploy MCP server as Kubernetes pod.\n\nExamples:\n  b00t-cli k8s deploy-mcp --server filesystem\n  b00t-cli k8s deploy-mcp --server brave-search --namespace mcp-servers"
    )]
    DeployMcp {
        #[clap(long, help = "MCP server name from b00t configuration")]
        server: String,
        #[clap(long, help = "Namespace (default: default)")]
        namespace: Option<String>,
        #[clap(long, help = "Override pod name")]
        name: Option<String>,
    },
    #[clap(
        about = "List running pods",
        long_about = "List running pods.\n\nExamples:\n  b00t-cli k8s list\n  b00t-cli k8s list --namespace kube-system\n  b00t-cli k8s list --all"
    )]
    List {
        #[clap(long, help = "Show pods in specific namespace")]
        namespace: Option<String>,
        #[clap(long, help = "Show all pods (not just b00t-managed)")]
        all: bool,
        #[clap(long, help = "Output in JSON format")]
        json: bool,
    },
    #[clap(
        about = "Show pod logs",
        long_about = "Show logs for a specific pod.\n\nExamples:\n  b00t-cli k8s logs web-server\n  b00t-cli k8s logs --follow web-server\n  b00t-cli k8s logs --previous web-server"
    )]
    Logs {
        #[clap(help = "Pod name")]
        pod_name: String,
        #[clap(long, help = "Namespace (default: default)")]
        namespace: Option<String>,
        #[clap(long, help = "Follow log output")]
        follow: bool,
        #[clap(long, help = "Show previous container logs")]
        previous: bool,
    },
    #[clap(
        about = "Delete resources",
        long_about = "Delete Kubernetes resources.\n\nExamples:\n  b00t-cli k8s delete pod web-server\n  b00t-cli k8s delete --all pods\n  b00t-cli k8s delete service web-service"
    )]
    Delete {
        #[clap(help = "Resource type (pod, service, deployment)")]
        resource_type: String,
        #[clap(help = "Resource name (or --all for all resources)")]
        resource_name: Option<String>,
        #[clap(long, help = "Delete all resources of the specified type")]
        all: bool,
        #[clap(long, help = "Namespace (default: default)")]
        namespace: Option<String>,
    },
}

impl K8sCommands {
    pub fn execute(&self, _path: &str) -> Result<()> {
        let rt = tokio::runtime::Runtime::new().context("failed to create async runtime")?;

        match self {
            K8sCommands::List {
                namespace,
                all,
                json,
            } => {
                let client = rt.block_on(KubeClient::new())?;
                let pods = rt.block_on(client.list_pods(namespace.as_deref(), *all))?;
                if *json {
                    println!("{}", serde_json::to_string_pretty(&pods)?);
                } else if pods.is_empty() {
                    println!("no pods found");
                } else {
                    println!(
                        "{:<40} {:<15} {:<12} {:<8}",
                        "NAME", "NAMESPACE", "STATUS", "READY"
                    );
                    for pod in &pods {
                        println!(
                            "{:<40} {:<15} {:<12} {}/{}",
                            pod.name, pod.namespace, pod.status, pod.ready, pod.total
                        );
                    }
                }
                Ok(())
            }
            K8sCommands::Logs {
                pod_name,
                namespace,
                follow: _,
                previous,
            } => {
                let client = rt.block_on(KubeClient::new())?;
                rt.block_on(client.get_logs(pod_name, namespace.as_deref(), *previous))
            }
            K8sCommands::Deploy { .. } => {
                println!("🚀 K8s deploy — requires helm chart transmutation pipeline (see #83)");
                Ok(())
            }
            K8sCommands::DeployMcp { .. } => {
                println!("🚀 K8s deploy-mcp — requires MCP→helm chart mapping (see #83)");
                Ok(())
            }
            K8sCommands::Delete { .. } => {
                println!("🗑️ K8s delete — requires resource safety gates (see #83)");
                Ok(())
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_k8s_commands_parse() {
        let deploy_cmd = K8sCommands::Deploy {
            from_dockerfile: Some("Dockerfile".to_string()),
            from_compose: None,
            image: None,
            name: Some("test-pod".to_string()),
            namespace: None,
            env: vec![],
        };
        assert!(matches!(deploy_cmd, K8sCommands::Deploy { .. }));
    }

    #[test]
    fn test_pod_summary_serialize() {
        let pod = PodSummary {
            name: "test".into(),
            namespace: "default".into(),
            status: "Running".into(),
            ready: 1,
            total: 1,
        };
        let json = serde_json::to_string(&pod).unwrap();
        assert!(json.contains("Running"));
    }
}
