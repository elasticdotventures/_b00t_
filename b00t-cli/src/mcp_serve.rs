//! SP4-05/06 — `b00t mcp serve`: the on-demand MCP hosting control plane.
//!
//! An axum server the proxy calls to wake a backend `<svc>` MCP server:
//!
//! * `GET /_b00t/route/<svc>` — load the `<svc>.mcp_server` datum, run a
//!   pre-launch ledgrrr budget gate (SP4-06), `placement.ensure()` the backend,
//!   and return `{ "fqdn": ..., "warm": bool }`.
//! * `GET /_b00t/status` — the backend in use + every currently-warm service.
//!
//! A background reaper scales a service back to zero once it has been idle
//! longer than its datum's `idle_timeout_seconds`.
//!
//! Placement is provider-agnostic: `--backend podman` (dev, default) or
//! `--backend aca` (Azure Container Apps) — the datum's own
//! `placement.backend` is advisory here and only logged on a mismatch, because
//! one `serve` process drives one backend.

use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};

use anyhow::{Context, Result};
use axum::{
    Json, Router,
    extract::{Path as AxPath, Query, State},
    http::StatusCode,
    response::IntoResponse,
    routing::get,
};
use serde::Deserialize;
use serde_json::{Value, json};
use tokio::sync::RwLock;

use b00t_c0re_ledgrrr::{
    HttpSpendAuthorizer, LedgrrrMode, MockSpendAuthorizer, SpendAuthorizer,
};
use b00t_c0re_lib::{AcaPlacement, Endpoint, LaunchSpec, McpPlacement, PodmanPlacement};

use crate::datum_mcp_server::{McpServerSpec, PlacementBackend};
use crate::datum_utils::get_all_datums_with_paths;

/// Which placement backend a `serve` process drives.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ServeBackend {
    Podman,
    Aca,
}

impl ServeBackend {
    fn label(self) -> &'static str {
        match self {
            ServeBackend::Podman => "podman",
            ServeBackend::Aca => "aca",
        }
    }

    /// Does this backend match a datum's declared `placement.backend`?
    fn matches(self, declared: PlacementBackend) -> bool {
        matches!(
            (self, declared),
            (ServeBackend::Podman, PlacementBackend::Podman)
                | (ServeBackend::Aca, PlacementBackend::Aca)
        )
    }
}

impl std::str::FromStr for ServeBackend {
    type Err = anyhow::Error;
    fn from_str(s: &str) -> Result<Self> {
        match s.to_ascii_lowercase().as_str() {
            "podman" | "docker" => Ok(ServeBackend::Podman),
            "aca" | "azure" => Ok(ServeBackend::Aca),
            other => anyhow::bail!("unknown --backend '{other}' (expected: podman, aca)"),
        }
    }
}

struct WarmEntry {
    endpoint: Endpoint,
    last_hit: Instant,
    idle_timeout: Duration,
}

/// Shared control-plane state.
pub struct ControlPlane {
    b00t_path: String,
    backend: ServeBackend,
    placement: Box<dyn McpPlacement>,
    authorizer: Box<dyn SpendAuthorizer>,
    warm: RwLock<HashMap<String, WarmEntry>>,
}

impl ControlPlane {
    fn placement_for(backend: ServeBackend) -> Result<Box<dyn McpPlacement>> {
        Ok(match backend {
            ServeBackend::Podman => Box::new(PodmanPlacement::new()),
            ServeBackend::Aca => Box::new(
                AcaPlacement::from_env().context("--backend aca needs $B00T_ACA_RESOURCE_GROUP")?,
            ),
        })
    }

    fn authorizer_from_env() -> Box<dyn SpendAuthorizer> {
        match LedgrrrMode::from_env() {
            LedgrrrMode::Http => {
                let base = std::env::var("B00T_LEDGRRR_URL")
                    .unwrap_or_else(|_| "http://127.0.0.1:8787".to_string());
                Box::new(HttpSpendAuthorizer::new(base))
            }
            LedgrrrMode::Mock => Box::new(MockSpendAuthorizer::always_ok()),
        }
    }

    pub fn from_env(b00t_path: impl Into<String>, backend: ServeBackend) -> Result<Arc<Self>> {
        Ok(Arc::new(Self {
            b00t_path: b00t_path.into(),
            backend,
            placement: Self::placement_for(backend)?,
            authorizer: Self::authorizer_from_env(),
            warm: RwLock::new(HashMap::new()),
        }))
    }

    /// Load the `<svc>.mcp_server` datum's launch spec.
    fn load_spec(&self, svc: &str) -> Result<McpServerSpec> {
        let datums = get_all_datums_with_paths(&self.b00t_path, Some(4))
            .context("scan _b00t_ datums")?;
        let hit = datums
            .get(svc)
            .or_else(|| datums.get(&format!("{svc}.mcp_server")))
            .with_context(|| format!("no datum '{svc}' found"))?;
        hit.0
            .mcp_server
            .clone()
            .with_context(|| format!("datum '{svc}' is not an McpServer (no [b00t.mcp_server])"))
    }
}

fn launch_spec(spec: &McpServerSpec) -> LaunchSpec {
    LaunchSpec {
        image: spec.image.clone(),
        port: spec.port,
        env: spec.env.clone(),
        cpu: spec.cpu,
        memory: spec.memory.clone(),
        idle_timeout_seconds: spec.idle_timeout_seconds,
    }
}

#[derive(Debug, Deserialize)]
struct RouteQuery {
    /// Billing tenant for the pre-launch budget gate.
    tenant: Option<String>,
    /// Billing agent for the pre-launch budget gate.
    agent: Option<String>,
}

async fn route_svc(
    State(cp): State<Arc<ControlPlane>>,
    AxPath(svc): AxPath<String>,
    Query(q): Query<RouteQuery>,
) -> Response {
    // already warm? bump last_hit and return.
    {
        let mut warm = cp.warm.write().await;
        if let Some(e) = warm.get_mut(&svc) {
            e.last_hit = Instant::now();
            return Json(json!({ "fqdn": e.endpoint.base_url, "warm": e.endpoint.warm })).into_response();
        }
    }

    let spec = match cp.load_spec(&svc) {
        Ok(s) => s,
        Err(e) => {
            return (
                StatusCode::NOT_FOUND,
                Json(json!({ "error": "unknown_service", "detail": e.to_string() })),
            )
                .into_response();
        }
    };

    if !cp.backend.matches(spec.placement.backend) {
        tracing::warn!(
            svc = %svc,
            declared = ?spec.placement.backend,
            running = cp.backend.label(),
            "datum placement.backend differs from the serving backend"
        );
    }

    // SP4-06 — pre-launch budget gate. A flat launch cost unless the datum
    // pins its own `budget_ceiling`.
    let cost = if spec.budget_ceiling > 0 {
        spec.budget_ceiling
    } else {
        1
    };
    let tenant = q.tenant.as_deref().unwrap_or("platform");
    let agent = q.agent.as_deref().unwrap_or("control-plane");
    let idem = format!("launch:{svc}:{}", chrono::Utc::now().format("%Y-%m-%d"));
    match cp.authorizer.authorize_spend(tenant, agent, cost, &idem).await {
        Ok(r) if r.ok => {}
        Ok(r) => {
            return (
                StatusCode::SERVICE_UNAVAILABLE,
                Json(json!({
                    "error": "budget_exceeded",
                    "reason": r.reason,
                    "budget_remaining": r.budget_remaining,
                })),
            )
                .into_response();
        }
        Err(e) => {
            return (
                StatusCode::SERVICE_UNAVAILABLE,
                Json(json!({ "error": "budget_check_failed", "detail": e.to_string() })),
            )
                .into_response();
        }
    }

    // launch (or attach to an already-running container).
    let endpoint = match cp.placement.ensure(&svc, &launch_spec(&spec)).await {
        Ok(ep) => ep,
        Err(e) => {
            return (
                StatusCode::BAD_GATEWAY,
                Json(json!({ "error": "launch_failed", "detail": e.to_string() })),
            )
                .into_response();
        }
    };

    cp.warm.write().await.insert(
        svc.clone(),
        WarmEntry {
            endpoint: endpoint.clone(),
            last_hit: Instant::now(),
            idle_timeout: Duration::from_secs(spec.idle_timeout_seconds as u64),
        },
    );

    Json(json!({ "fqdn": endpoint.base_url, "warm": endpoint.warm })).into_response()
}

async fn status(State(cp): State<Arc<ControlPlane>>) -> Json<Value> {
    let warm = cp.warm.read().await;
    let services: Vec<Value> = warm
        .iter()
        .map(|(svc, e)| {
            json!({
                "svc": svc,
                "fqdn": e.endpoint.base_url,
                "warm": e.endpoint.warm,
                "idle_seconds": e.last_hit.elapsed().as_secs(),
                "idle_timeout_seconds": e.idle_timeout.as_secs(),
            })
        })
        .collect();
    Json(json!({ "backend": cp.backend.label(), "warm": services }))
}

/// axum's [`Response`] type alias, spelled once.
type Response = axum::response::Response;

/// Scale idle services back to zero.
async fn reaper(cp: Arc<ControlPlane>) {
    let mut tick = tokio::time::interval(Duration::from_secs(30));
    loop {
        tick.tick().await;
        let expired: Vec<String> = {
            let warm = cp.warm.read().await;
            warm.iter()
                .filter(|(_, e)| e.last_hit.elapsed() > e.idle_timeout)
                .map(|(svc, _)| svc.clone())
                .collect()
        };
        for svc in expired {
            if let Err(e) = cp.placement.stop(&svc).await {
                tracing::warn!(svc = %svc, error = %e, "idle reaper: stop failed");
            } else {
                tracing::info!(svc = %svc, "idle reaper: scaled to zero");
            }
            cp.warm.write().await.remove(&svc);
        }
    }
}

/// `b00t mcp serve` entry point.
pub async fn serve(b00t_path: &str, backend: ServeBackend, port: u16) -> Result<()> {
    let cp = ControlPlane::from_env(b00t_path, backend)?;

    tokio::spawn(reaper(cp.clone()));

    let app = Router::new()
        .route("/_b00t/route/:svc", get(route_svc))
        .route("/_b00t/status", get(status))
        .route("/health", get(|| async { "ok" }))
        .with_state(cp);

    let addr = std::net::SocketAddr::from(([127, 0, 0, 1], port));
    let listener = tokio::net::TcpListener::bind(addr)
        .await
        .with_context(|| format!("bind {addr}"))?;
    tracing::info!(%addr, backend = backend.label(), "b00t mcp serve — control plane up");
    axum::serve(listener, app).await.context("axum serve")?;
    Ok(())
}

/// SP4-10 — `b00t mcp servers`: every declared `McpServer` datum, plus live
/// warm/cold status when `$B00T_MCP_CONTROL_URL` points at a running
/// `b00t mcp serve`.
pub async fn list_servers(b00t_path: &str, json_out: bool) -> Result<()> {
    let datums = get_all_datums_with_paths(b00t_path, Some(4)).context("scan _b00t_ datums")?;
    let mut rows: Vec<(String, McpServerSpec)> = datums
        .into_iter()
        .filter_map(|(key, (d, _))| {
            let svc = key
                .strip_suffix(".mcp_server")
                .map(str::to_string)
                .unwrap_or(key);
            d.mcp_server.map(|s| (svc, s))
        })
        .collect();
    rows.sort_by(|a, b| a.0.cmp(&b.0));

    // best-effort live status
    let warm: HashMap<String, Value> = match std::env::var("B00T_MCP_CONTROL_URL") {
        Ok(base) => {
            let url = format!("{}/_b00t/status", base.trim_end_matches('/'));
            match reqwest::get(&url).await {
                Ok(r) => r
                    .json::<Value>()
                    .await
                    .ok()
                    .and_then(|v| v.get("warm").cloned())
                    .and_then(|w| w.as_array().cloned())
                    .unwrap_or_default()
                    .into_iter()
                    .filter_map(|e| {
                        e.get("svc")
                            .and_then(|s| s.as_str())
                            .map(|s| (s.to_string(), e.clone()))
                    })
                    .collect(),
                Err(_) => HashMap::new(),
            }
        }
        Err(_) => HashMap::new(),
    };

    if json_out {
        let out: Vec<Value> = rows
            .iter()
            .map(|(svc, spec)| {
                json!({
                    "svc": svc,
                    "image": spec.image,
                    "backend": format!("{:?}", spec.placement.backend).to_lowercase(),
                    "port": spec.port,
                    "idle_timeout_seconds": spec.idle_timeout_seconds,
                    "digest_pinned": spec.is_digest_pinned(),
                    "endpoint": crate::datum_mcp_server::default_endpoint(svc),
                    "warm": warm.get(svc).map(|e| e.get("warm").cloned().unwrap_or(Value::Bool(true))),
                })
            })
            .collect();
        println!("{}", serde_json::to_string_pretty(&out)?);
        return Ok(());
    }

    if rows.is_empty() {
        println!("no McpServer datums declared (create _b00t_/<svc>.mcp_server.toml)");
        return Ok(());
    }
    println!("{:<16} {:<10} {:<7} {:<48} {}", "SVC", "BACKEND", "WARM", "IMAGE", "ENDPOINT");
    for (svc, spec) in &rows {
        let warm_s = match warm.get(svc) {
            Some(_) => "yes",
            None => "-",
        };
        println!(
            "{:<16} {:<10} {:<7} {:<48} {}",
            svc,
            format!("{:?}", spec.placement.backend).to_lowercase(),
            warm_s,
            spec.image,
            crate::datum_mcp_server::default_endpoint(svc),
        );
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn backend_parses_and_labels() {
        assert_eq!("podman".parse::<ServeBackend>().unwrap(), ServeBackend::Podman);
        assert_eq!("ACA".parse::<ServeBackend>().unwrap(), ServeBackend::Aca);
        assert!("fly".parse::<ServeBackend>().is_err());
        assert_eq!(ServeBackend::Aca.label(), "aca");
    }

    #[test]
    fn backend_match_is_exact() {
        assert!(ServeBackend::Podman.matches(PlacementBackend::Podman));
        assert!(!ServeBackend::Podman.matches(PlacementBackend::Aca));
        assert!(ServeBackend::Aca.matches(PlacementBackend::Aca));
    }

    #[test]
    fn launch_spec_projects_the_datum_spec() {
        let mut spec = McpServerSpec::default();
        spec.image = "x@sha256:1".into();
        spec.port = 9000;
        spec.idle_timeout_seconds = 42;
        let ls = launch_spec(&spec);
        assert_eq!(ls.image, "x@sha256:1");
        assert_eq!(ls.port, 9000);
        assert_eq!(ls.idle_timeout_seconds, 42);
    }

    /// CP-3 — a budget-denied launch returns 503 and never calls `ensure()`.
    #[tokio::test]
    async fn budget_denied_route_is_503_and_does_not_launch() {
        use axum::body::Body;
        use axum::http::Request;
        use tower::ServiceExt;

        let tmp = tempfile::tempdir().unwrap();
        std::fs::write(
            tmp.path().join("demo.mcp_server.toml"),
            "[b00t]\nname = \"demo\"\ntype = \"mcp_server\"\n\n[b00t.mcp_server]\nimage = \"local/x:dev\"\n",
        )
        .unwrap();

        let cp = Arc::new(ControlPlane {
            b00t_path: tmp.path().to_string_lossy().into_owned(),
            backend: ServeBackend::Podman,
            placement: Box::new(PanicPlacement),
            authorizer: Box::new(MockSpendAuthorizer::always_deny()),
            warm: RwLock::new(HashMap::new()),
        });

        let app = Router::new()
            .route("/_b00t/route/:svc", get(route_svc))
            .with_state(cp);

        let res = app
            .oneshot(
                Request::builder()
                    .uri("/_b00t/route/demo")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(res.status(), StatusCode::SERVICE_UNAVAILABLE);
    }

    /// A placement impl that fails the test if it is ever asked to launch.
    struct PanicPlacement;

    #[async_trait::async_trait]
    impl McpPlacement for PanicPlacement {
        async fn ensure(&self, _svc: &str, _spec: &LaunchSpec) -> Result<Endpoint> {
            panic!("ensure() must not be called when the budget gate denies the launch");
        }
        async fn stop(&self, _svc: &str) -> Result<()> {
            Ok(())
        }
        async fn status(
            &self,
            _svc: &str,
        ) -> Result<b00t_c0re_lib::PlacementStatus> {
            Ok(b00t_c0re_lib::PlacementStatus::Unknown)
        }
    }
}
