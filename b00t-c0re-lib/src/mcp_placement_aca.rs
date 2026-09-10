//! SP4-04 — Azure Container Apps backend for [`McpPlacement`].
//!
//! The per-`<svc>` container app is DECLARED by Terraform
//! (`infra terraform/b00t/mcp-servers.tf`, a `for_each` over the
//! `standing-mcp-server` module). This backend only toggles the warm/cold
//! state of an already-`terraform`'d app via `az containerapp update
//! --min-replicas`, and reads its FQDN — it never `terraform apply`s.
//!
//! Idle-to-zero is the ACA module's own `idle_timeout_seconds` (KEDA HTTP
//! scale rule); `stop()` forces cold immediately.

use anyhow::{Context, Result};

use crate::mcp_placement::{Endpoint, LaunchSpec, McpPlacement, PlacementStatus};

/// Placement onto a pre-declared ACA app named `<svc>` in a resource group.
pub struct AcaPlacement {
    /// Resource group holding the container apps (`$B00T_ACA_RESOURCE_GROUP`).
    pub resource_group: String,
    /// Optional subscription id (`$B00T_ACA_SUBSCRIPTION`); `None` = az default.
    pub subscription: Option<String>,
    /// How long to wait for `/health` to go 200 after a warm.
    pub health_deadline: std::time::Duration,
}

impl AcaPlacement {
    /// From `$B00T_ACA_RESOURCE_GROUP` / `$B00T_ACA_SUBSCRIPTION`.
    pub fn from_env() -> Result<Self> {
        Ok(Self {
            resource_group: std::env::var("B00T_ACA_RESOURCE_GROUP")
                .context("B00T_ACA_RESOURCE_GROUP is not set")?,
            subscription: std::env::var("B00T_ACA_SUBSCRIPTION").ok(),
            health_deadline: std::time::Duration::from_secs(45),
        })
    }

    fn base_args(&self, svc: &str) -> Vec<String> {
        let mut a = vec![
            "containerapp".to_string(),
            "show".to_string(),
            "-n".to_string(),
            svc.to_string(),
            "-g".to_string(),
            self.resource_group.clone(),
        ];
        if let Some(sub) = &self.subscription {
            a.push("--subscription".to_string());
            a.push(sub.clone());
        }
        a
    }

    async fn az(&self, args: &[String]) -> Result<std::process::Output> {
        tokio::process::Command::new("az")
            .args(args)
            .output()
            .await
            .with_context(|| format!("run `az {}`", args.join(" ")))
    }

    async fn set_min_replicas(&self, svc: &str, n: u32) -> Result<()> {
        let mut a = vec![
            "containerapp".to_string(),
            "update".to_string(),
            "-n".to_string(),
            svc.to_string(),
            "-g".to_string(),
            self.resource_group.clone(),
            "--min-replicas".to_string(),
            n.to_string(),
        ];
        if let Some(sub) = &self.subscription {
            a.push("--subscription".to_string());
            a.push(sub.clone());
        }
        let out = self.az(&a).await?;
        if !out.status.success() {
            anyhow::bail!(
                "az containerapp update failed: {}",
                String::from_utf8_lossy(&out.stderr)
            );
        }
        Ok(())
    }

    async fn fqdn(&self, svc: &str) -> Result<String> {
        let mut a = self.base_args(svc);
        a.push("--query".to_string());
        a.push("properties.configuration.ingress.fqdn".to_string());
        a.push("-o".to_string());
        a.push("tsv".to_string());
        let out = self.az(&a).await?;
        if !out.status.success() {
            anyhow::bail!(
                "az containerapp show failed: {}",
                String::from_utf8_lossy(&out.stderr)
            );
        }
        let fqdn = String::from_utf8_lossy(&out.stdout).trim().to_string();
        if fqdn.is_empty() {
            anyhow::bail!("container app '{svc}' has no ingress FQDN");
        }
        Ok(fqdn)
    }

    async fn health_ok(base_url: &str) -> bool {
        reqwest::Client::new()
            .get(format!("{base_url}/health"))
            .send()
            .await
            .map(|r| r.status().is_success())
            .unwrap_or(false)
    }
}

#[async_trait::async_trait]
impl McpPlacement for AcaPlacement {
    async fn ensure(&self, svc: &str, _spec: &LaunchSpec) -> Result<Endpoint> {
        // warm it (idempotent) and read where it lives
        self.set_min_replicas(svc, 1).await?;
        let base_url = format!("https://{}", self.fqdn(svc).await?);

        let start = std::time::Instant::now();
        while start.elapsed() < self.health_deadline {
            if Self::health_ok(&base_url).await {
                return Ok(Endpoint { base_url, warm: true });
            }
            tokio::time::sleep(std::time::Duration::from_secs(2)).await;
        }
        Ok(Endpoint {
            base_url,
            warm: false,
        })
    }

    async fn stop(&self, svc: &str) -> Result<()> {
        self.set_min_replicas(svc, 0).await
    }

    async fn status(&self, svc: &str) -> Result<PlacementStatus> {
        let mut a = self.base_args(svc);
        a.push("--query".to_string());
        a.push("properties.runningStatus".to_string());
        a.push("-o".to_string());
        a.push("tsv".to_string());
        let out = self.az(&a).await?;
        if !out.status.success() {
            return Ok(PlacementStatus::Unknown);
        }
        Ok(match String::from_utf8_lossy(&out.stdout).trim() {
            "Running" | "Progressing" => PlacementStatus::Running,
            "" => PlacementStatus::Unknown,
            _ => PlacementStatus::Stopped,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn base_args_include_group_and_optional_subscription() {
        let p = AcaPlacement {
            resource_group: "rg1".into(),
            subscription: Some("sub1".into()),
            health_deadline: std::time::Duration::from_secs(1),
        };
        let a = p.base_args("gh");
        assert!(a.contains(&"gh".to_string()));
        assert!(a.contains(&"rg1".to_string()));
        assert!(a.contains(&"sub1".to_string()));

        let p2 = AcaPlacement {
            resource_group: "rg1".into(),
            subscription: None,
            health_deadline: std::time::Duration::from_secs(1),
        };
        assert!(!p2.base_args("gh").contains(&"--subscription".to_string()));
    }

    #[test]
    fn from_env_requires_the_resource_group() {
        // (B00T_ACA_RESOURCE_GROUP is not set in the test env)
        assert!(AcaPlacement::from_env().is_err());
    }
}
