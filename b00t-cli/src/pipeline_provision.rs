//! 🤓 Dynamic provisioning for the pipeline scheduler — phase 2 of the vultr
//! delegation design (phase 1: `vultr_delegate.rs`, historian's NATS
//! request-reply surface). This is the "later milestone" `pipeline_executor.rs`
//! flags for `ComputeProvider`: when the scheduler can't fit a stage on any
//! known host, ask `b00t-historian` (over the `vultr.provision` subject
//! `vultr_delegate` exposes) for a new one instead of just failing.
//!
//! Deliberately scoped to the *scheduler* layer only (`Scheduler::schedule`
//! / `HostInfo` pool), not `pipeline_executor.rs`'s `run_stage_fn` — that
//! function is a documented MVP stub ("simulating work"), and actually
//! executing a stage on a freshly-provisioned remote host is a separate,
//! much larger real-execution-backend project. This module answers "can I
//! get a host that fits," not "how do I run on it."
//!
//! Disabled by default (`DynamicProvisionOptions::enabled = false`):
//! provisioning creates real billed infrastructure, so a pipeline run must
//! opt in explicitly rather than silently spending money the first time
//! its static host pool runs out.

use crate::pipeline_scheduler::{HostInfo, PipelineSchedule, ScheduleDecision, Scheduler};
use crate::pipeline_types::StageSpec;
use crate::vultr_delegate::{ProvisionRequest, ProvisionResponse};
use anyhow::Result;
use async_trait::async_trait;

/// Abstraction over "ask something to provision a host" — production
/// implementations send a real NATS request to `vultr.provision`; tests use
/// an in-memory double. Kept separate from `pipeline_executor::NatsClient`
/// (fire-and-forget pub/sub) since provisioning is inherently request-reply.
#[async_trait]
pub trait ProvisionClient: Send + Sync {
    async fn request_provision(&self, req: &ProvisionRequest) -> Result<ProvisionResponse>;
}

/// A real `ProvisionClient` wired to a live NATS connection.
///
/// Mirrors `b00t-historian`'s own `query_cmd` request-reply pattern
/// (`client.request(subject, payload)` with a timeout) — same subject
/// family (`vultr.provision`), same JSON envelope `vultr_delegate` already
/// defines.
pub struct NatsProvisionClient {
    client: async_nats::Client,
    subject: String,
    timeout: std::time::Duration,
}

impl NatsProvisionClient {
    pub fn new(client: async_nats::Client) -> Self {
        Self {
            client,
            subject: "vultr.provision".to_string(),
            timeout: std::time::Duration::from_secs(30),
        }
    }
}

#[async_trait]
impl ProvisionClient for NatsProvisionClient {
    async fn request_provision(&self, req: &ProvisionRequest) -> Result<ProvisionResponse> {
        let payload = serde_json::to_vec(req)?;
        let response = tokio::time::timeout(
            self.timeout,
            self.client.request(self.subject.clone(), payload.into()),
        )
        .await
        .map_err(|_| {
            anyhow::anyhow!(
                "{} timed out after {:?} (is `b00t-historian run` active?)",
                self.subject,
                self.timeout
            )
        })??;
        Ok(serde_json::from_slice(&response.payload)?)
    }
}

/// Governs whether/how the scheduler is allowed to request new hosts.
#[derive(Debug, Clone)]
pub struct DynamicProvisionOptions {
    /// Must be explicitly opted into — provisioning spends real money.
    pub enabled: bool,
    pub requested_by: String,
    pub purpose: String,
    pub ttl_hours: u32,
    /// Upper bound on provision requests within one `schedule_with_dynamic_provisioning`
    /// call, independent of `vultr_delegate`'s own server-side capacity cap —
    /// belt-and-suspenders against a pathological stage list driving
    /// unbounded provisioning in a single scheduling pass.
    pub max_provisions_per_run: u32,
}

impl Default for DynamicProvisionOptions {
    fn default() -> Self {
        Self {
            enabled: false,
            requested_by: "pipeline-scheduler".to_string(),
            purpose: "pipeline stage host".to_string(),
            ttl_hours: 4,
            max_provisions_per_run: 1,
        }
    }
}

/// Runs `scheduler.schedule`; for every stage that comes back `NoFit`,
/// requests one new host via `provision_client` (up to
/// `opts.max_provisions_per_run` times total) and re-schedules, until
/// either nothing is unfit or the provision budget is exhausted.
///
/// `hosts` is extended in place with any newly-provisioned hosts, so the
/// caller's pool reflects what's now actually available.
pub async fn schedule_with_dynamic_provisioning(
    scheduler: &dyn Scheduler,
    stages: &[StageSpec],
    hosts: &mut Vec<HostInfo>,
    provision_client: &dyn ProvisionClient,
    opts: &DynamicProvisionOptions,
) -> Result<PipelineSchedule> {
    let mut schedule = scheduler.schedule(stages, hosts);
    if !opts.enabled {
        return Ok(schedule);
    }

    let mut provisions = 0u32;
    loop {
        let nofit_stage = schedule.mapping.iter().find_map(|r| match &r.decision {
            ScheduleDecision::NoFit { .. } => Some(r.stage_name.clone()),
            _ => None,
        });

        let Some(stage_name) = nofit_stage else {
            break;
        };
        if provisions >= opts.max_provisions_per_run {
            break;
        }

        let Some(stage) = stages.iter().find(|s| s.name == stage_name) else {
            break;
        };

        let req = ProvisionRequest {
            requested_by: opts.requested_by.clone(),
            purpose: format!("{} (stage: {})", opts.purpose, stage.name),
            ttl_hours: opts.ttl_hours,
            plan: None,
            region: None,
        };
        let resp = provision_client.request_provision(&req).await?;
        hosts.push(resp.host);
        provisions += 1;

        schedule = scheduler.schedule(stages, hosts);
    }

    Ok(schedule)
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pipeline_scheduler::GreedyScheduler;
    use crate::pipeline_types::ResourceRequirements;
    use std::collections::HashMap;
    use std::sync::Mutex;

    fn stage_needing(name: &str, ram_gb: f64, cpu_cores: u32) -> StageSpec {
        let mut s = StageSpec::from_name(name);
        s.profile.resources = ResourceRequirements {
            min_ram_gb: ram_gb,
            min_vram_gb: 0.0,
            requires_gpu: false,
            cpu_cores: Some(cpu_cores),
            scratch_disk_gb: None,
        };
        s
    }

    fn tiny_host(name: &str) -> HostInfo {
        HostInfo {
            name: name.to_string(),
            resources: crate::pipeline_types::HostResources {
                ram_gb: 1.0,
                vram_gb: 0.0,
                gpu_count: 0,
                cpu_cores: 1,
            },
            labels: HashMap::new(),
        }
    }

    /// Records every request it receives and returns a canned host large
    /// enough to satisfy any stage this test module creates.
    struct MockProvisionClient {
        calls: Mutex<Vec<ProvisionRequest>>,
        big_enough: bool,
    }

    impl MockProvisionClient {
        fn new(big_enough: bool) -> Self {
            Self {
                calls: Mutex::new(Vec::new()),
                big_enough,
            }
        }

        fn call_count(&self) -> usize {
            self.calls.lock().unwrap().len()
        }
    }

    #[async_trait]
    impl ProvisionClient for MockProvisionClient {
        async fn request_provision(&self, req: &ProvisionRequest) -> Result<ProvisionResponse> {
            self.calls.lock().unwrap().push(req.clone());
            let (ram_gb, cpu_cores) = if self.big_enough { (16.0, 8) } else { (1.0, 1) };
            Ok(ProvisionResponse {
                instance_id: format!("mock-{}", self.calls.lock().unwrap().len()),
                host: HostInfo {
                    name: "provisioned-host".to_string(),
                    resources: crate::pipeline_types::HostResources {
                        ram_gb,
                        vram_gb: 0.0,
                        gpu_count: 0,
                        cpu_cores,
                    },
                    labels: HashMap::new(),
                },
                created_at: chrono::Utc::now(),
                expires_at: chrono::Utc::now() + chrono::Duration::hours(req.ttl_hours as i64),
            })
        }
    }

    #[tokio::test]
    async fn disabled_by_default_never_calls_provision_client() {
        let stages = vec![stage_needing("big-stage", 64.0, 32)];
        let mut hosts = vec![tiny_host("only-host")];
        let client = MockProvisionClient::new(true);
        let opts = DynamicProvisionOptions::default(); // enabled: false

        let schedule = schedule_with_dynamic_provisioning(
            &GreedyScheduler,
            &stages,
            &mut hosts,
            &client,
            &opts,
        )
        .await
        .unwrap();

        assert_eq!(client.call_count(), 0);
        assert_eq!(hosts.len(), 1); // untouched
        assert!(matches!(
            schedule.mapping[0].decision,
            ScheduleDecision::NoFit { .. }
        ));
    }

    #[tokio::test]
    async fn enabled_provisions_and_resolves_nofit() {
        let stages = vec![stage_needing("big-stage", 8.0, 4)];
        let mut hosts = vec![tiny_host("only-host")];
        let client = MockProvisionClient::new(true);
        let opts = DynamicProvisionOptions {
            enabled: true,
            ..Default::default()
        };

        let schedule = schedule_with_dynamic_provisioning(
            &GreedyScheduler,
            &stages,
            &mut hosts,
            &client,
            &opts,
        )
        .await
        .unwrap();

        assert_eq!(client.call_count(), 1);
        assert_eq!(hosts.len(), 2); // the provisioned host got added
        assert!(matches!(
            schedule.mapping[0].decision,
            ScheduleDecision::Allocate { .. }
        ));
    }

    #[tokio::test]
    async fn respects_max_provisions_per_run() {
        // Two stages, neither of which the (deliberately too-small)
        // provisioned host can satisfy — each retry still comes back NoFit,
        // so this also proves the loop terminates on budget rather than
        // spinning forever.
        let stages = vec![
            stage_needing("stage-a", 64.0, 32),
            stage_needing("stage-b", 64.0, 32),
        ];
        let mut hosts = vec![tiny_host("only-host")];
        let client = MockProvisionClient::new(false); // never big enough
        let opts = DynamicProvisionOptions {
            enabled: true,
            max_provisions_per_run: 2,
            ..Default::default()
        };

        let _schedule = schedule_with_dynamic_provisioning(
            &GreedyScheduler,
            &stages,
            &mut hosts,
            &client,
            &opts,
        )
        .await
        .unwrap();

        assert_eq!(client.call_count(), 2);
        assert_eq!(hosts.len(), 3); // original + 2 provisioned
    }

    #[tokio::test]
    async fn already_fitting_stage_never_triggers_provisioning() {
        let stages = vec![stage_needing("small-stage", 0.5, 1)];
        let mut hosts = vec![tiny_host("only-host")];
        let client = MockProvisionClient::new(true);
        let opts = DynamicProvisionOptions {
            enabled: true,
            ..Default::default()
        };

        let schedule = schedule_with_dynamic_provisioning(
            &GreedyScheduler,
            &stages,
            &mut hosts,
            &client,
            &opts,
        )
        .await
        .unwrap();

        assert_eq!(client.call_count(), 0);
        assert_eq!(hosts.len(), 1);
        assert!(matches!(
            schedule.mapping[0].decision,
            ScheduleDecision::Allocate { .. }
        ));
    }
}
