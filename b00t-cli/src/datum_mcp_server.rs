//! SP4 — `DatumType::McpServer` payload: the on-demand *launch* spec for a
//! remote MCP server. The connect URL is DERIVED
//! (`https://<svc>.<base-domain>/mcp`), never stored — an `McpServer` datum
//! that carries a `url` is a mistake (that's an `Mcp` httpstream datum).

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

/// The default base domain for on-demand MCP services (matches
/// `b00t_c0re_lib::remote_mcp_proxy::DEFAULT_BASE_DOMAIN`).
pub const DEFAULT_BASE_DOMAIN: &str = "b00t.promptexecution.com";

/// Where an `McpServer` gets launched. Additive — the routing URL never names
/// the provider (D3).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum PlacementBackend {
    /// Azure Container Apps via `terraform/azure/modules/standing-mcp-server`.
    Aca,
    /// Local `podman run` — dev / CI.
    #[default]
    Podman,
    Fly,
    CloudRun,
}

/// Placement policy for a launch.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub struct Placement {
    #[serde(default)]
    pub backend: PlacementBackend,
    /// Preferred region (backend-specific; `None` = backend default).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub region: Option<String>,
    /// Free-form backend knobs (e.g. `min_replicas`, `sku`).
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub extra: BTreeMap<String, String>,
}

/// The body of a `DatumType::McpServer` datum (`[b00t.mcp_server]`).
/// (`PartialEq` only — `cpu` is `f32`.)
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
pub struct McpServerSpec {
    /// OCI image reference. Production refs MUST be digest-pinned
    /// (`repo/name@sha256:…`) — see [`Self::is_digest_pinned`].
    pub image: String,
    /// vCPUs for the main container.
    #[serde(default = "default_cpu")]
    pub cpu: f32,
    /// Memory for the main container (e.g. `"1Gi"`).
    #[serde(default = "default_memory")]
    pub memory: String,
    /// HTTP port the server listens on (`/mcp` + `/health`).
    #[serde(default = "default_port")]
    pub port: u16,
    /// Non-secret env vars for the container.
    #[serde(default)]
    pub env: BTreeMap<String, String>,
    /// Secret *names* — resolved by the placement backend from its own store,
    /// never carried as values here.
    #[serde(default)]
    pub secrets_ref: Vec<String>,
    /// Seconds of inactivity before the backend scales to zero.
    #[serde(default = "default_idle")]
    pub idle_timeout_seconds: u32,
    /// Per-launch budget ceiling, in cake (checked against ledgrrr before a
    /// cold launch).
    #[serde(default)]
    pub budget_ceiling: u64,
    /// Where / how to launch.
    #[serde(default)]
    pub placement: Placement,
}

fn default_cpu() -> f32 {
    0.5
}
fn default_memory() -> String {
    "1Gi".to_string()
}
fn default_port() -> u16 {
    8080
}
fn default_idle() -> u32 {
    300
}

impl McpServerSpec {
    /// A production image ref must be digest-pinned.
    pub fn is_digest_pinned(&self) -> bool {
        self.image.contains("@sha256:")
    }
}

/// SP4-02 — the derived connect endpoint for a service. MUST match
/// `b00t_c0re_lib::remote_mcp_proxy::RemoteMcpProxy::route`'s host exactly.
pub fn derive_endpoint(svc: &str, base_domain: &str) -> String {
    format!("https://{svc}.{}/mcp", base_domain.trim_matches('.'))
}

/// Convenience: the endpoint on the default base domain.
pub fn default_endpoint(svc: &str) -> String {
    derive_endpoint(svc, DEFAULT_BASE_DOMAIN)
}

#[cfg(test)]
mod tests {
    use super::*;

    const FIXTURE: &str = r#"
image = "ghcr.io/promptexecution/gh-mcp@sha256:aaaa"
cpu = 1.0
memory = "2Gi"
idle_timeout_seconds = 600
budget_ceiling = 5000

[env]
GH_HOST = "github.com"

[placement]
backend = "aca"
region = "australiaeast"
"#;

    #[test]
    fn fixture_deserialises() {
        let spec: McpServerSpec = toml::from_str(FIXTURE).unwrap();
        assert!(spec.is_digest_pinned());
        assert_eq!(spec.cpu, 1.0);
        assert_eq!(spec.port, 8080); // default
        assert_eq!(spec.idle_timeout_seconds, 600);
        assert_eq!(spec.env.get("GH_HOST").map(String::as_str), Some("github.com"));
        assert_eq!(spec.placement.backend, PlacementBackend::Aca);
        assert_eq!(spec.placement.region.as_deref(), Some("australiaeast"));
    }

    #[test]
    fn empty_spec_defaults() {
        let spec: McpServerSpec =
            toml::from_str("image = \"local/x:dev\"").unwrap();
        assert!(!spec.is_digest_pinned());
        assert_eq!(spec.cpu, 0.5);
        assert_eq!(spec.port, 8080);
        assert_eq!(spec.idle_timeout_seconds, 300);
        assert_eq!(spec.placement.backend, PlacementBackend::Podman); // default
    }

    #[test]
    fn a_url_field_is_rejected() {
        // an McpServer datum must NOT carry a connect url — that's an Mcp datum.
        assert!(toml::from_str::<McpServerSpec>(
            "image = \"x@sha256:1\"\nurl = \"https://x/mcp\""
        )
        .is_err());
    }

    #[test]
    fn derive_endpoint_matches_the_proxy_host() {
        assert_eq!(
            derive_endpoint("gh", "b00t.promptexecution.com"),
            "https://gh.b00t.promptexecution.com/mcp"
        );
        assert_eq!(default_endpoint("gh"), "https://gh.b00t.promptexecution.com/mcp");
    }
}
