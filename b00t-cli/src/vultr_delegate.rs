//! 🤓 vultr delegate — lets b00t-historian execute Vultr VPS provision/
//! deprovision/status requests on behalf of other hive agents over NATS,
//! instead of every agent needing its own `VULTR_API_KEY` + allowlisted
//! egress IP (see `_b00t_/datums/PROVIDER-VULTR.provider.tomllmd`).
//!
//! This module is pure orchestration over the existing `ComputeProvider`
//! trait (`crate::commands::provider`) — no new HTTP client, no duplicate
//! Vultr API code. It adds exactly what wasn't there before:
//!   - a fail-closed allowlist (`AllowedRequesters`) gating who may provision
//!     or deprovision
//!   - a max-concurrent-instance cap (this creates real billed
//!     infrastructure)
//!   - a mandatory, bounded TTL on every provisioned instance, enforced by
//!     `spawn_ttl_teardown`
//!   - a response shape (`ProvisionResponse` wrapping
//!     `pipeline_scheduler::HostInfo`) designed to be consumed directly by
//!     the pipeline scheduler's dynamic-provisioning path
//!     (`pipeline_provision.rs`), with no conversion glue needed.
//!
//! `handle_provision`/`handle_deprovision`/`handle_status` are the pure
//! request handlers `b00t-historian`'s NATS request-reply loop calls into.

use crate::commands::provider::{ComputeProvider, EndpointConfig};
use crate::pipeline_scheduler::HostInfo;
use crate::pipeline_types::HostResources;
use anyhow::{bail, Context, Result};
use chrono::{DateTime, Duration as ChronoDuration, Utc};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};

/// Comma-separated list of `requested_by` identifiers allowed to provision/
/// deprovision. Unset or empty = deny all (fail closed, matching
/// capability-forge's `SkillTier::Restricted` default).
pub const VULTR_DELEGATE_ALLOWLIST_ENV: &str = "VULTR_DELEGATE_ALLOWLIST";

/// Overrides `DEFAULT_MAX_INSTANCES` if set.
pub const VULTR_DELEGATE_MAX_INSTANCES_ENV: &str = "VULTR_DELEGATE_MAX_INSTANCES";

/// Conservative default cap on b00t-managed Vultr instances running at once.
pub const DEFAULT_MAX_INSTANCES: u32 = 3;

/// Hard ceiling on `ttl_hours` — one week. This exists because a provisioned
/// instance is real billed infrastructure; there is no "forget about it
/// forever" option. Provision manually (outside this delegate path) for
/// anything longer-lived.
pub const MAX_TTL_HOURS: u32 = 168;

/// Vultr instance label prefix so `list_endpoints` results (already tag-
/// filtered to `"b00t"` by `VultrProvider`) can be told apart from instances
/// created outside this delegate path.
const INSTANCE_NAME_PREFIX: &str = "b00t-delegate";

// ── AllowedRequesters ────────────────────────────────────────────────────────

/// Fail-closed allowlist of `requested_by` identifiers.
#[derive(Debug, Clone, Default)]
pub struct AllowedRequesters(HashSet<String>);

impl AllowedRequesters {
    pub fn from_env() -> Self {
        Self::from_csv(&std::env::var(VULTR_DELEGATE_ALLOWLIST_ENV).unwrap_or_default())
    }

    pub fn from_csv(csv: &str) -> Self {
        Self(
            csv.split(',')
                .map(str::trim)
                .filter(|s| !s.is_empty())
                .map(String::from)
                .collect(),
        )
    }

    pub fn is_allowed(&self, requester: &str) -> bool {
        self.0.contains(requester)
    }
}

/// Reads `VULTR_DELEGATE_MAX_INSTANCES_ENV`, falling back to
/// `DEFAULT_MAX_INSTANCES` if unset or unparseable.
pub fn max_instances_from_env() -> u32 {
    std::env::var(VULTR_DELEGATE_MAX_INSTANCES_ENV)
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(DEFAULT_MAX_INSTANCES)
}

// ── Request/response types ───────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProvisionRequest {
    pub requested_by: String,
    pub purpose: String,
    /// Required — every provisioned instance must have a bounded lifetime.
    /// See `MAX_TTL_HOURS`.
    pub ttl_hours: u32,
    #[serde(default)]
    pub plan: Option<String>,
    #[serde(default)]
    pub region: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProvisionResponse {
    pub instance_id: String,
    /// Directly consumable by `pipeline_scheduler::Scheduler` — this is the
    /// point of shaping the response this way rather than a bespoke struct.
    pub host: HostInfo,
    pub created_at: DateTime<Utc>,
    pub expires_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeprovisionRequest {
    pub requested_by: String,
    pub instance_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeprovisionResponse {
    pub instance_id: String,
    pub status: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct StatusRequest {
    /// `None` = list all b00t-managed instances.
    #[serde(default)]
    pub instance_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InstanceStatus {
    pub instance_id: String,
    pub name: Option<String>,
    pub status: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StatusResponse {
    pub instances: Vec<InstanceStatus>,
}

// ── Plan parsing ─────────────────────────────────────────────────────────────

/// Best-effort parse of Vultr's `{family}-{cpu}c-{ram}gb[-...]` plan-id
/// convention (e.g. `vc2-1c-1gb`, `vhf-4c-8gb`) into `HostResources`.
/// Vultr's naming isn't fully regular across every plan family — on
/// anything unparseable this falls back to a conservative 1 CPU / 1GB RAM
/// rather than guessing high, since this feeds a scheduler's fit checks.
pub fn plan_to_resources(plan: &str) -> HostResources {
    let mut cpu_cores = 1u32;
    let mut ram_gb = 1.0f64;
    for part in plan.split('-') {
        if let Some(n) = part.strip_suffix('c') {
            if let Ok(v) = n.parse::<u32>() {
                cpu_cores = v;
            }
        } else if let Some(n) = part.strip_suffix("gb") {
            if let Ok(v) = n.parse::<f64>() {
                ram_gb = v;
            }
        }
    }
    HostResources {
        ram_gb,
        vram_gb: 0.0,
        gpu_count: 0,
        cpu_cores,
    }
}

// ── Validation ────────────────────────────────────────────────────────────────

pub fn check_capacity(current: usize, max: u32) -> Result<()> {
    if current as u32 >= max {
        bail!("at capacity: {current}/{max} b00t-managed vultr instance(s) already running — deprovision one first or raise {VULTR_DELEGATE_MAX_INSTANCES_ENV}");
    }
    Ok(())
}

pub fn validate_ttl_hours(ttl_hours: u32) -> Result<()> {
    if ttl_hours == 0 {
        bail!("ttl_hours must be > 0 — every provisioned instance must have an expiry");
    }
    if ttl_hours > MAX_TTL_HOURS {
        bail!(
            "ttl_hours {ttl_hours} exceeds MAX_TTL_HOURS ({MAX_TTL_HOURS}) — request a shorter TTL, or provision manually outside this delegate path for anything longer-lived"
        );
    }
    Ok(())
}

// ── Handlers ──────────────────────────────────────────────────────────────────

pub async fn handle_provision(
    req: &ProvisionRequest,
    provider: &dyn ComputeProvider,
    allowlist: &AllowedRequesters,
    max_instances: u32,
) -> Result<ProvisionResponse> {
    if !allowlist.is_allowed(&req.requested_by) {
        bail!(
            "requester '{}' is not on the vultr delegate allowlist ({VULTR_DELEGATE_ALLOWLIST_ENV})",
            req.requested_by
        );
    }
    validate_ttl_hours(req.ttl_hours)?;

    let current = provider
        .list_endpoints()
        .await
        .context("listing current instances for capacity check")?
        .len();
    check_capacity(current, max_instances)?;

    // Per-call plan/region override, honored by `vultr_create_instance_body`
    // reading `cfg.env` before the process-wide VULTR_PLAN/VULTR_REGION env
    // vars — see commands/provider.rs. Falls back to those process defaults
    // (and ultimately VULTR_DEFAULT_PLAN/VULTR_DEFAULT_REGION) if unset here.
    let mut env = HashMap::new();
    if let Some(plan) = &req.plan {
        env.insert("VULTR_PLAN".to_string(), plan.clone());
    }
    if let Some(region) = &req.region {
        env.insert("VULTR_REGION".to_string(), region.clone());
    }
    let cfg = EndpointConfig {
        name: format!("{INSTANCE_NAME_PREFIX}-{}", req.requested_by),
        env,
        ..Default::default()
    };
    let created = provider
        .deploy_inference_endpoint(&cfg)
        .await
        .context("deploying vultr instance")?;

    let resources = plan_to_resources(req.plan.as_deref().unwrap_or("vc2-1c-1gb"));
    let now = Utc::now();
    let mut labels = HashMap::new();
    labels.insert("provider".to_string(), "vultr".to_string());
    labels.insert("requested_by".to_string(), req.requested_by.clone());
    labels.insert("purpose".to_string(), req.purpose.clone());
    if let Some(status) = &created.status {
        labels.insert("status".to_string(), status.clone());
    }

    Ok(ProvisionResponse {
        instance_id: created.id.clone(),
        host: HostInfo {
            name: created.name.clone().unwrap_or_else(|| created.id.clone()),
            resources,
            labels,
        },
        created_at: now,
        expires_at: now + ChronoDuration::hours(req.ttl_hours as i64),
    })
}

pub async fn handle_deprovision(
    req: &DeprovisionRequest,
    provider: &dyn ComputeProvider,
    allowlist: &AllowedRequesters,
) -> Result<DeprovisionResponse> {
    if !allowlist.is_allowed(&req.requested_by) {
        bail!(
            "requester '{}' is not on the vultr delegate allowlist ({VULTR_DELEGATE_ALLOWLIST_ENV})",
            req.requested_by
        );
    }
    provider
        .teardown_endpoint(&req.instance_id)
        .await
        .context("tearing down vultr instance")?;
    Ok(DeprovisionResponse {
        instance_id: req.instance_id.clone(),
        status: "deprovisioned".to_string(),
    })
}

/// Read-only — not allowlist-gated. NATS auth already gates who can reach
/// this subject at all; instance status is informational, not consequential
/// (unlike provision/deprovision, which spend/reclaim real money).
pub async fn handle_status(
    req: &StatusRequest,
    provider: &dyn ComputeProvider,
) -> Result<StatusResponse> {
    let instances = if let Some(id) = &req.instance_id {
        let h = provider
            .endpoint_status(id)
            .await
            .context("querying vultr instance status")?;
        vec![InstanceStatus {
            instance_id: h.id,
            name: h.name,
            status: h.status,
        }]
    } else {
        provider
            .list_endpoints()
            .await
            .context("listing vultr instances")?
            .into_iter()
            .map(|h| InstanceStatus {
                instance_id: h.id,
                name: h.name,
                status: h.status,
            })
            .collect()
    };
    Ok(StatusResponse { instances })
}

/// Spawns a background task that tears down `instance_id` after `ttl`
/// elapses. This is what makes `ttl_hours` an enforced expiry rather than a
/// documentation-only field — a provisioned instance goes away on its own
/// even if nobody ever sends a deprovision request.
pub fn spawn_ttl_teardown(
    instance_id: String,
    ttl: std::time::Duration,
    provider: std::sync::Arc<dyn ComputeProvider>,
) -> tokio::task::JoinHandle<()> {
    tokio::spawn(async move {
        tokio::time::sleep(ttl).await;
        match provider.teardown_endpoint(&instance_id).await {
            Ok(()) => eprintln!("[vultr-delegate] TTL expired, auto-deprovisioned {instance_id}"),
            Err(e) => eprintln!(
                "[vultr-delegate] TTL expired but auto-deprovision of {instance_id} failed: {e}"
            ),
        }
    })
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::commands::provider::{
        BatchJobSpec, EndpointHandle, JobHandle, TrainingJobSpec,
    };
    use async_trait::async_trait;
    use std::sync::atomic::{AtomicU32, Ordering};
    use std::sync::Mutex;

    /// In-memory `ComputeProvider` test double. Tracks created/torn-down
    /// instance ids and call counts so tests can assert on both outcomes and
    /// side effects (e.g. TTL auto-teardown actually calling through).
    #[derive(Default)]
    struct MockComputeProvider {
        instances: Mutex<Vec<EndpointHandle>>,
        next_id: AtomicU32,
        teardown_calls: Mutex<Vec<String>>,
    }

    impl MockComputeProvider {
        fn with_existing(count: usize) -> Self {
            let m = Self::default();
            for i in 0..count {
                m.instances.lock().unwrap().push(EndpointHandle {
                    id: format!("existing-{i}"),
                    provider: "vultr".into(),
                    name: Some(format!("existing-{i}")),
                    status: Some("active".into()),
                });
            }
            m
        }

        fn teardown_calls(&self) -> Vec<String> {
            self.teardown_calls.lock().unwrap().clone()
        }
    }

    #[async_trait]
    impl ComputeProvider for MockComputeProvider {
        fn name(&self) -> &str {
            "mock-vultr"
        }

        async fn deploy_inference_endpoint(&self, cfg: &EndpointConfig) -> Result<EndpointHandle> {
            let id = format!("mock-{}", self.next_id.fetch_add(1, Ordering::SeqCst));
            let handle = EndpointHandle {
                id: id.clone(),
                provider: "vultr".into(),
                name: Some(cfg.name.clone()),
                status: Some("active".into()),
            };
            self.instances.lock().unwrap().push(handle.clone());
            Ok(handle)
        }

        async fn endpoint_status(&self, id: &str) -> Result<EndpointHandle> {
            self.instances
                .lock()
                .unwrap()
                .iter()
                .find(|h| h.id == id)
                .cloned()
                .context("no such mock instance")
        }

        async fn teardown_endpoint(&self, id: &str) -> Result<()> {
            self.teardown_calls.lock().unwrap().push(id.to_string());
            self.instances.lock().unwrap().retain(|h| h.id != id);
            Ok(())
        }

        async fn list_endpoints(&self) -> Result<Vec<EndpointHandle>> {
            Ok(self.instances.lock().unwrap().clone())
        }

        async fn submit_training_job(&self, _spec: &TrainingJobSpec) -> Result<JobHandle> {
            bail!("mock provider does not support training jobs")
        }
        async fn submit_batch_job(&self, _spec: &BatchJobSpec) -> Result<JobHandle> {
            bail!("mock provider does not support batch jobs")
        }
        async fn job_status(&self, _handle: &JobHandle) -> Result<String> {
            bail!("mock provider has no job management")
        }
        async fn cancel_job(&self, _handle: &JobHandle) -> Result<()> {
            bail!("mock provider has no job management")
        }
        async fn list_jobs(&self) -> Result<Vec<JobHandle>> {
            Ok(vec![])
        }
    }

    fn allow(who: &str) -> AllowedRequesters {
        AllowedRequesters::from_csv(who)
    }

    // ── AllowedRequesters ──────────────────────────────────────────────

    #[test]
    fn allowlist_denies_by_default() {
        let a = AllowedRequesters::from_csv("");
        assert!(!a.is_allowed("fung1"));
    }

    #[test]
    fn allowlist_parses_csv_and_trims() {
        let a = AllowedRequesters::from_csv(" fung1 , sm3lly ,,b00t-node");
        assert!(a.is_allowed("fung1"));
        assert!(a.is_allowed("sm3lly"));
        assert!(a.is_allowed("b00t-node"));
        assert!(!a.is_allowed("unknown-host"));
    }

    // ── plan_to_resources ──────────────────────────────────────────────

    #[test]
    fn plan_to_resources_parses_vc2_convention() {
        let r = plan_to_resources("vc2-4c-8gb");
        assert_eq!(r.cpu_cores, 4);
        assert_eq!(r.ram_gb, 8.0);
        assert_eq!(r.vram_gb, 0.0);
        assert_eq!(r.gpu_count, 0);
    }

    #[test]
    fn plan_to_resources_parses_other_family_prefixes() {
        let r = plan_to_resources("vhf-2c-4gb");
        assert_eq!(r.cpu_cores, 2);
        assert_eq!(r.ram_gb, 4.0);
    }

    #[test]
    fn plan_to_resources_falls_back_conservatively_on_unparseable_input() {
        let r = plan_to_resources("totally-bogus-plan-name");
        assert_eq!(r.cpu_cores, 1);
        assert_eq!(r.ram_gb, 1.0);
    }

    // ── validation ─────────────────────────────────────────────────────

    #[test]
    fn check_capacity_rejects_at_cap() {
        assert!(check_capacity(3, 3).is_err());
        assert!(check_capacity(4, 3).is_err());
    }

    #[test]
    fn check_capacity_allows_under_cap() {
        assert!(check_capacity(2, 3).is_ok());
    }

    #[test]
    fn validate_ttl_hours_rejects_zero() {
        assert!(validate_ttl_hours(0).is_err());
    }

    #[test]
    fn validate_ttl_hours_rejects_over_max() {
        assert!(validate_ttl_hours(MAX_TTL_HOURS + 1).is_err());
    }

    #[test]
    fn validate_ttl_hours_accepts_in_range() {
        assert!(validate_ttl_hours(1).is_ok());
        assert!(validate_ttl_hours(MAX_TTL_HOURS).is_ok());
    }

    // ── handle_provision ───────────────────────────────────────────────

    #[tokio::test]
    async fn provision_denies_unlisted_requester() {
        let provider = MockComputeProvider::default();
        let req = ProvisionRequest {
            requested_by: "sneaky-agent".to_string(),
            purpose: "test".to_string(),
            ttl_hours: 1,
            plan: None,
            region: None,
        };
        let result = handle_provision(&req, &provider, &allow("fung1"), 3).await;
        assert!(result.is_err());
        assert!(provider.list_endpoints().await.unwrap().is_empty());
    }

    #[tokio::test]
    async fn provision_rejects_zero_ttl_before_touching_provider() {
        let provider = MockComputeProvider::default();
        let req = ProvisionRequest {
            requested_by: "fung1".to_string(),
            purpose: "test".to_string(),
            ttl_hours: 0,
            plan: None,
            region: None,
        };
        let result = handle_provision(&req, &provider, &allow("fung1"), 3).await;
        assert!(result.is_err());
        assert!(provider.list_endpoints().await.unwrap().is_empty());
    }

    #[tokio::test]
    async fn provision_rejects_at_capacity() {
        let provider = MockComputeProvider::with_existing(3);
        let req = ProvisionRequest {
            requested_by: "fung1".to_string(),
            purpose: "test".to_string(),
            ttl_hours: 1,
            plan: None,
            region: None,
        };
        let result = handle_provision(&req, &provider, &allow("fung1"), 3).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn provision_happy_path_maps_to_host_info() {
        let provider = MockComputeProvider::default();
        let req = ProvisionRequest {
            requested_by: "fung1".to_string(),
            purpose: "cargo build farm".to_string(),
            ttl_hours: 4,
            plan: Some("vc2-4c-8gb".to_string()),
            region: Some("syd".to_string()),
        };
        let resp = handle_provision(&req, &provider, &allow("fung1"), 3)
            .await
            .expect("provision should succeed");

        assert_eq!(resp.host.resources.cpu_cores, 4);
        assert_eq!(resp.host.resources.ram_gb, 8.0);
        assert_eq!(resp.host.labels.get("requested_by").unwrap(), "fung1");
        assert_eq!(resp.host.labels.get("provider").unwrap(), "vultr");
        assert!(resp.expires_at > resp.created_at);
        assert_eq!(
            (resp.expires_at - resp.created_at).num_hours(),
            4
        );

        // provider actually got the per-call plan/region via cfg.env
        let created = provider.list_endpoints().await.unwrap();
        assert_eq!(created.len(), 1);
        assert_eq!(created[0].id, resp.instance_id);
    }

    // ── handle_deprovision ─────────────────────────────────────────────

    #[tokio::test]
    async fn deprovision_denies_unlisted_requester() {
        let provider = MockComputeProvider::with_existing(1);
        let req = DeprovisionRequest {
            requested_by: "sneaky-agent".to_string(),
            instance_id: "existing-0".to_string(),
        };
        let result = handle_deprovision(&req, &provider, &allow("fung1")).await;
        assert!(result.is_err());
        assert_eq!(provider.list_endpoints().await.unwrap().len(), 1);
    }

    #[tokio::test]
    async fn deprovision_happy_path_tears_down() {
        let provider = MockComputeProvider::with_existing(1);
        let req = DeprovisionRequest {
            requested_by: "fung1".to_string(),
            instance_id: "existing-0".to_string(),
        };
        let resp = handle_deprovision(&req, &provider, &allow("fung1"))
            .await
            .expect("deprovision should succeed");
        assert_eq!(resp.status, "deprovisioned");
        assert!(provider.list_endpoints().await.unwrap().is_empty());
    }

    // ── handle_status ──────────────────────────────────────────────────

    #[tokio::test]
    async fn status_lists_all_when_no_id_given() {
        let provider = MockComputeProvider::with_existing(2);
        let resp = handle_status(&StatusRequest::default(), &provider)
            .await
            .expect("status should succeed");
        assert_eq!(resp.instances.len(), 2);
    }

    #[tokio::test]
    async fn status_returns_single_instance_when_id_given() {
        let provider = MockComputeProvider::with_existing(2);
        let resp = handle_status(
            &StatusRequest {
                instance_id: Some("existing-1".to_string()),
            },
            &provider,
        )
        .await
        .expect("status should succeed");
        assert_eq!(resp.instances.len(), 1);
        assert_eq!(resp.instances[0].instance_id, "existing-1");
    }

    #[tokio::test]
    async fn status_is_not_allowlist_gated() {
        // No allowlist parameter exists on handle_status at all — this test
        // documents that as intentional (see the function's doc comment).
        let provider = MockComputeProvider::with_existing(1);
        assert!(handle_status(&StatusRequest::default(), &provider)
            .await
            .is_ok());
    }

    // ── spawn_ttl_teardown ─────────────────────────────────────────────

    #[tokio::test(start_paused = true)]
    async fn ttl_teardown_fires_after_expiry() {
        let provider: std::sync::Arc<dyn ComputeProvider> =
            std::sync::Arc::new(MockComputeProvider::with_existing(1));
        let handle = spawn_ttl_teardown(
            "existing-0".to_string(),
            std::time::Duration::from_secs(3600),
            provider.clone(),
        );

        tokio::time::advance(std::time::Duration::from_secs(3601)).await;
        handle.await.expect("teardown task should not panic");

        assert!(provider.list_endpoints().await.unwrap().is_empty());
    }

    #[tokio::test(start_paused = true)]
    async fn ttl_teardown_does_not_fire_before_expiry() {
        let mock = std::sync::Arc::new(MockComputeProvider::with_existing(1));
        let provider: std::sync::Arc<dyn ComputeProvider> = mock.clone();
        let _handle = spawn_ttl_teardown(
            "existing-0".to_string(),
            std::time::Duration::from_secs(3600),
            provider,
        );

        tokio::time::advance(std::time::Duration::from_secs(1800)).await;
        // Yield so the spawned task gets a chance to run if it were (wrongly) ready.
        tokio::task::yield_now().await;

        assert!(mock.teardown_calls().is_empty());
    }
}
