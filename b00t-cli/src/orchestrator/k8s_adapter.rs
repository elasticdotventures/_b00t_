// K8s adapter: Translates b00t datums to Kubernetes manifests + MCP commands
// Uses kubernetes-mcp server for execution

use anyhow::{Context, Result};
use serde_json::json;
use std::collections::HashMap;

use super::adapter::{
    AdapterMetadata, AdapterOutput, McpCommand, Orchestrator, OrchestratorAdapter,
};
use crate::datum_stack::{JobDatum, StackDatum};

pub struct K8sAdapter {
    namespace: String,
    use_kueue: bool, // Use Kueue for job queuing if available
}

impl K8sAdapter {
    pub fn new() -> Self {
        Self {
            namespace: "default".to_string(),
            use_kueue: Self::check_kueue_available(),
        }
    }

    pub fn with_namespace(mut self, namespace: String) -> Self {
        self.namespace = namespace;
        self
    }

    fn check_kueue_available() -> bool {
        // Check if Kueue CRDs are installed
        std::process::Command::new("kubectl")
            .args(&["get", "crd", "queues.kueue.x-k8s.io"])
            .output()
            .map(|out| out.status.success())
            .unwrap_or(false)
    }

    fn generate_job_manifest(&self, job: &JobDatum) -> Result<String> {
        let mut manifest = String::new();

        // Extract container spec
        let image = job
            .datum
            .image
            .as_ref()
            .context("Job datum missing image field")?;

        let command = job.datum.command.clone();
        let args = job.datum.args.clone();

        // Generate initContainers for service dependencies
        let init_containers = if let Some(orch) = &job.datum.orchestration {
            if let Some(stacks) = &orch.requires_stacks {
                self.generate_init_containers_for_stacks(stacks)?
            } else {
                String::new()
            }
        } else {
            String::new()
        };

        // Generate env vars
        let env_vars = self.generate_env_vars(&job.datum.env)?;

        // Generate resource requirements
        let resources = self.generate_resources(job)?;

        // Kueue integration
        let queue_label = if self.use_kueue {
            if let Some(orch) = &job.datum.orchestration {
                if let Some(queue) = &orch.queue_name {
                    format!("    kueue.x-k8s.io/queue-name: {}\n", queue)
                } else {
                    String::new()
                }
            } else {
                String::new()
            }
        } else {
            String::new()
        };

        // Build manifest
        manifest.push_str(&format!(
            r#"apiVersion: batch/v1
kind: Job
metadata:
  name: {}
  namespace: {}
  labels:
    app.kubernetes.io/name: {}
    app.kubernetes.io/managed-by: b00t
{}spec:
  template:
    metadata:
      labels:
        app.kubernetes.io/name: {}
    spec:
{}      containers:
      - name: {}
        image: {}
"#,
            job.datum.name,
            self.namespace,
            job.datum.name,
            queue_label,
            job.datum.name,
            init_containers,
            job.datum.name,
            image,
        ));

        // Add command if specified
        if let Some(cmd) = &command {
            manifest.push_str(&format!("        command: [\"{}\"]\n", cmd));
        }

        // Add args if specified
        if let Some(a) = &args {
            manifest.push_str(&format!(
                "        args: [{}]\n",
                a.iter()
                    .map(|s| format!("\"{}\"", s))
                    .collect::<Vec<_>>()
                    .join(", ")
            ));
        }

        // Add env vars
        if !env_vars.is_empty() {
            manifest.push_str(&format!("        env:\n{}", env_vars));
        }

        // Add resources
        manifest.push_str(&format!("{}", resources));

        // Restart policy for jobs
        manifest.push_str("      restartPolicy: Never\n");

        Ok(manifest)
    }

    fn generate_init_containers_for_stacks(&self, stacks: &[String]) -> Result<String> {
        // For each required stack, generate initContainer to wait for services
        // This is a k8s-specific implementation of abstract "wait_for_services"
        let mut init_containers = String::from("      initContainers:\n");

        for stack_name in stacks {
            // Assume stack has a service named after the stack
            // In reality, would query stack datum for actual service names
            init_containers.push_str(&format!(
                r#"      - name: wait-for-{}
        image: busybox:1.36
        command:
        - sh
        - -c
        - |
          echo "Waiting for {} services to be ready..."
          # Service discovery via k8s DNS
          until nc -z {}.{}.svc.cluster.local 80; do
            sleep 2
          done
          echo "{} is ready"
"#,
                stack_name.replace("_", "-"),
                stack_name,
                stack_name.replace("_", "-"),
                self.namespace,
                stack_name,
            ));
        }

        Ok(init_containers)
    }

    fn generate_env_vars(&self, env: &Option<HashMap<String, String>>) -> Result<String> {
        if let Some(env_map) = env {
            let mut env_yaml = String::new();
            for (key, value) in env_map {
                // Handle abstract service references: ${STACK:name:service:endpoint}
                let resolved_value = self.resolve_service_reference(value)?;
                env_yaml.push_str(&format!(
                    "        - name: {}\n          value: \"{}\"\n",
                    key, resolved_value
                ));
            }
            Ok(env_yaml)
        } else {
            Ok(String::new())
        }
    }

    fn resolve_service_reference(&self, value: &str) -> Result<String> {
        // Parse ${STACK:stack-name:service:endpoint} syntax
        if value.starts_with("${STACK:") && value.ends_with('}') {
            let inner = &value[8..value.len() - 1]; // Remove ${STACK: and }
            let parts: Vec<&str> = inner.split(':').collect();

            if parts.len() >= 2 {
                let _stack_name = parts[0];
                let service_name = parts[1];
                let port = parts.get(2).unwrap_or(&"80");

                // k8s-specific: Service DNS name
                let k8s_service = format!(
                    "http://{}.{}.svc.cluster.local:{}",
                    service_name, self.namespace, port
                );
                return Ok(k8s_service);
            }
        }

        // Not a service reference, return as-is
        Ok(value.to_string())
    }

    fn generate_resources(&self, job: &JobDatum) -> Result<String> {
        let mut resources = String::from("        resources:\n");

        if let Some(orch) = &job.datum.orchestration {
            // CPU and memory
            if let Some(res_req) = &orch.resource_requirements {
                resources.push_str("          requests:\n");
                if let Some(cpu) = res_req.get("cpu") {
                    resources.push_str(&format!("            cpu: \"{}\"\n", cpu));
                }
                if let Some(memory) = res_req.get("memory") {
                    resources.push_str(&format!("            memory: \"{}\"\n", memory));
                }
            }

            // GPU requirements
            if let Some(gpu) = &orch.gpu_requirements {
                if let Some(count) = gpu.count {
                    resources.push_str(&format!("            nvidia.com/gpu: {}\n", count));
                }
            }
        }

        Ok(resources)
    }

    fn generate_mcp_commands(&self, manifest: &str) -> Vec<McpCommand> {
        vec![McpCommand {
            server: "kubernetes-mcp".to_string(),
            tool: "kubectl_apply".to_string(),
            arguments: json!({
                "manifest": manifest,
                "namespace": self.namespace,
            }),
        }]
    }
}

impl Default for K8sAdapter {
    fn default() -> Self {
        Self::new()
    }
}

impl OrchestratorAdapter for K8sAdapter {
    fn translate_job(&self, job: &JobDatum) -> Result<AdapterOutput> {
        let manifest = self.generate_job_manifest(job)?;
        let mcp_commands = self.generate_mcp_commands(&manifest);

        let mut metadata = AdapterMetadata::default();

        // Add warnings for budget constraints
        if let Some(orch) = &job.datum.orchestration {
            if orch.budget_constraint.is_some() {
                metadata.warnings.push(
                    "Budget constraints require b00t budget controller to be running".to_string(),
                );
            }
        }

        // Warn if Kueue not available but queue requested
        if !self.use_kueue {
            if let Some(orch) = &job.datum.orchestration {
                if orch.queue_name.is_some() {
                    metadata.warnings.push(
                        "Kueue queue requested but Kueue CRDs not found in cluster".to_string(),
                    );
                }
            }
        }

        Ok(AdapterOutput {
            orchestrator: Orchestrator::Kubernetes,
            manifests: vec![manifest],
            mcp_commands,
            metadata,
        })
    }

    fn translate_stack(&self, _stack: &StackDatum) -> Result<AdapterOutput> {
        // Use existing kompose integration from stack.rs
        // This would call stack.generate_docker_compose() then run kompose

        let mut metadata = AdapterMetadata::default();
        metadata
            .warnings
            .push("Stack translation via kompose not yet implemented in adapter".to_string());

        // Placeholder - would integrate with existing stack to-k8s command
        Ok(AdapterOutput {
            orchestrator: Orchestrator::Kubernetes,
            manifests: vec![],
            mcp_commands: vec![],
            metadata,
        })
    }

    fn orchestrator(&self) -> Orchestrator {
        Orchestrator::Kubernetes
    }

    fn is_available(&self) -> bool {
        // Check if kubectl is configured
        std::process::Command::new("kubectl")
            .args(&["config", "current-context"])
            .output()
            .map(|out| out.status.success())
            .unwrap_or(false)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_service_reference_resolution() {
        let adapter = K8sAdapter::new();

        // Test abstract service reference
        let result = adapter
            .resolve_service_reference("${STACK:my-stack:n8n:5678}")
            .unwrap();
        assert_eq!(result, "http://n8n.default.svc.cluster.local:5678");

        // Test plain value
        let result = adapter
            .resolve_service_reference("http://example.com")
            .unwrap();
        assert_eq!(result, "http://example.com");
    }
}
