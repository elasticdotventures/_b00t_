//! SP4-03 — where an on-demand MCP server runs. `McpPlacement` is the
//! provider-agnostic control surface; `PodmanPlacement` is the local dev impl.
//! The Azure Container Apps impl is SP4-04 (`mcp_placement_aca.rs`).
//!
//! `LaunchSpec` is the subset of `b00t-cli`'s `McpServerSpec` a backend needs —
//! defined here so `b00t-c0re-lib` takes no dependency on `b00t-cli`.

use std::collections::BTreeMap;

use anyhow::{Context, Result};

/// The launch inputs a placement backend needs (a projection of
/// `b00t_cli::datum_mcp_server::McpServerSpec`).
#[derive(Debug, Clone, PartialEq)]
pub struct LaunchSpec {
    /// OCI image ref (digest-pinned in prod).
    pub image: String,
    /// Port the server listens on (`/mcp` + `/health`).
    pub port: u16,
    /// Non-secret env vars.
    pub env: BTreeMap<String, String>,
    /// vCPUs.
    pub cpu: f32,
    /// Memory, e.g. `"1Gi"`.
    pub memory: String,
    /// Idle seconds before the backend scales to zero.
    pub idle_timeout_seconds: u32,
}

impl Default for LaunchSpec {
    fn default() -> Self {
        Self {
            image: String::new(),
            port: 8080,
            env: BTreeMap::new(),
            cpu: 0.5,
            memory: "1Gi".to_string(),
            idle_timeout_seconds: 300,
        }
    }
}

/// A reachable MCP server endpoint.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Endpoint {
    /// Base URL, e.g. `http://127.0.0.1:49xxx` or
    /// `https://gh.b00t.promptexecution.com`. Append `/mcp` to call it.
    pub base_url: String,
    /// Whether `/health` answered 200 before this returned.
    pub warm: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlacementStatus {
    Running,
    Stopped,
    Unknown,
}

/// Bring an MCP server up on demand, take it down, report its state.
#[async_trait::async_trait]
pub trait McpPlacement: Send + Sync {
    /// Ensure `<svc>` is placed and `/health`-warm; return how to reach it.
    async fn ensure(&self, svc: &str, spec: &LaunchSpec) -> Result<Endpoint>;
    /// Scale `<svc>` to zero / remove it.
    async fn stop(&self, svc: &str) -> Result<()>;
    /// Current state of `<svc>`.
    async fn status(&self, svc: &str) -> Result<PlacementStatus>;
}

// ── PodmanPlacement — local dev ─────────────────────────────────────────────

/// Runs each `<svc>` as a `podman` container labelled `b00t.mcp=<svc>`, with a
/// random host port mapped to the container port. `/health` is polled before
/// `ensure` returns.
pub struct PodmanPlacement {
    /// `podman` (default) or a full path / `docker`.
    pub engine: String,
    /// How long to wait for `/health` to go 200.
    pub health_deadline: std::time::Duration,
}

impl Default for PodmanPlacement {
    fn default() -> Self {
        Self {
            engine: std::env::var("B00T_MCP_CONTAINER_ENGINE")
                .unwrap_or_else(|_| "podman".to_string()),
            health_deadline: std::time::Duration::from_secs(30),
        }
    }
}

impl PodmanPlacement {
    pub fn new() -> Self {
        Self::default()
    }

    fn container_name(svc: &str) -> String {
        format!("b00t-mcp-{}", svc.replace(['/', ':', '.'], "-"))
    }

    async fn run(&self, args: &[&str]) -> Result<std::process::Output> {
        tokio::process::Command::new(&self.engine)
            .args(args)
            .output()
            .await
            .with_context(|| format!("run `{} {}`", self.engine, args.join(" ")))
    }

    /// The mapped host port for the container's `port`, if running.
    async fn host_port(&self, svc: &str, port: u16) -> Result<Option<u16>> {
        let name = Self::container_name(svc);
        let out = self
            .run(&["port", &name, &format!("{port}/tcp")])
            .await?;
        if !out.status.success() {
            return Ok(None);
        }
        // "0.0.0.0:49153" (last line)
        let text = String::from_utf8_lossy(&out.stdout);
        let mapped = text
            .lines()
            .filter_map(|l| l.rsplit(':').next())
            .filter_map(|p| p.trim().parse::<u16>().ok())
            .next();
        Ok(mapped)
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
impl McpPlacement for PodmanPlacement {
    async fn ensure(&self, svc: &str, spec: &LaunchSpec) -> Result<Endpoint> {
        let name = Self::container_name(svc);

        // already running?
        if let Some(hp) = self.host_port(svc, spec.port).await? {
            let base_url = format!("http://127.0.0.1:{hp}");
            let warm = Self::health_ok(&base_url).await;
            return Ok(Endpoint { base_url, warm });
        }

        // start it
        let port_map = format!("{}", spec.port);
        let mut args: Vec<String> = vec![
            "run".into(),
            "-d".into(),
            "--rm".into(),
            "--name".into(),
            name.clone(),
            "--label".into(),
            format!("b00t.mcp={svc}"),
            "-p".into(),
            format!("0:{port_map}"),
        ];
        for (k, v) in &spec.env {
            args.push("-e".into());
            args.push(format!("{k}={v}"));
        }
        args.push(spec.image.clone());
        let arg_refs: Vec<&str> = args.iter().map(String::as_str).collect();
        let out = self.run(&arg_refs).await?;
        if !out.status.success() {
            anyhow::bail!(
                "podman run failed: {}",
                String::from_utf8_lossy(&out.stderr)
            );
        }

        // resolve the mapped port, then poll /health
        let hp = self
            .host_port(svc, spec.port)
            .await?
            .context("container started but no host port was mapped")?;
        let base_url = format!("http://127.0.0.1:{hp}");
        let start = std::time::Instant::now();
        while start.elapsed() < self.health_deadline {
            if Self::health_ok(&base_url).await {
                return Ok(Endpoint { base_url, warm: true });
            }
            tokio::time::sleep(std::time::Duration::from_millis(500)).await;
        }
        Ok(Endpoint {
            base_url,
            warm: false,
        })
    }

    async fn stop(&self, svc: &str) -> Result<()> {
        let name = Self::container_name(svc);
        let _ = self.run(&["stop", "-t", "2", &name]).await;
        Ok(())
    }

    async fn status(&self, svc: &str) -> Result<PlacementStatus> {
        let name = Self::container_name(svc);
        let out = self
            .run(&["ps", "--filter", &format!("name={name}"), "--format", "{{.Names}}"])
            .await?;
        let running = String::from_utf8_lossy(&out.stdout)
            .lines()
            .any(|l| l.trim() == name);
        Ok(if running {
            PlacementStatus::Running
        } else {
            PlacementStatus::Stopped
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn container_name_is_sanitised() {
        assert_eq!(
            PodmanPlacement::container_name("gh/list:v1.0"),
            "b00t-mcp-gh-list-v1-0"
        );
    }

    #[test]
    fn launch_spec_defaults() {
        let s = LaunchSpec::default();
        assert_eq!(s.port, 8080);
        assert_eq!(s.idle_timeout_seconds, 300);
        assert_eq!(s.memory, "1Gi");
    }

    #[test]
    fn endpoint_eq() {
        let a = Endpoint { base_url: "http://x".into(), warm: true };
        let b = a.clone();
        assert_eq!(a, b);
    }
}
