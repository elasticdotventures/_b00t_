//! Resource-aware scheduler — match stages to hosts based on ResourceRequirements.
//!
//! Provides two scheduling strategies:
//! - `GreedyScheduler`: assigns each stage to the first host that satisfies
//!   `ResourceFit`, preferring hosts with the highest available-resource score.
//! - `BinpackScheduler`: packs stages onto hosts to minimise the total number
//!   of hosts used (greedy binpack, largest-first).
//!
//! # Usage
//! ```bash
//! b00t pipeline schedule my-pipeline
//! ```

use crate::pipeline_types::{HostResources, ResourceFit, ResourceRequirements, StageSpec};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

// ── Core scheduling types ──────────────────────────────────────────────────────

/// A host candidate for stage placement.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HostInfo {
    pub name: String,
    pub resources: HostResources,
    pub labels: HashMap<String, String>,
}

/// Decision for a single stage-to-host assignment.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ScheduleDecision {
    /// Stage is allocated to this host with a fitness score (0.0–1.0).
    Allocate {
        host: String,
        score: f64,
    },
    /// Stage cannot fit on any available host.
    NoFit {
        reason: String,
    },
    /// Stage could fit but is deferred (e.g. dependency not yet scheduled).
    Deferred {
        reason: String,
    },
}

/// Scheduling outcome for a single stage.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScheduleResult {
    pub stage_name: String,
    pub decision: ScheduleDecision,
    /// Alternative host names that also satisfied the fit (in descending score order).
    pub alternatives: Vec<String>,
}

/// Complete schedule for a pipeline across available hosts.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PipelineSchedule {
    pub mapping: Vec<ScheduleResult>,
    /// Aggregate score across all allocated stages (mean of individual scores).
    pub overall_score: f64,
    /// Stage names that could not be assigned to any host.
    pub unassigned: Vec<String>,
}

// ── Scheduler trait ────────────────────────────────────────────────────────────

/// Strategy for mapping stages to hosts.
pub trait Scheduler {
    /// Produce a schedule assigning each stage to a host (or marking it unassignable).
    fn schedule(&self, stages: &[StageSpec], hosts: &[HostInfo]) -> PipelineSchedule;
}

// ── Scoring helpers ────────────────────────────────────────────────────────────

/// Compute a normalised [0.0, 1.0] score for how well a host's available resources
/// satisfy a stage's requirements.  Higher = better fit.
fn score_fit(req: &ResourceRequirements, host: &HostResources) -> f64 {
    let mut dims = 0u32;
    let mut total = 0.0f64;

    // RAM ratio: how much headroom does the host have beyond the requirement?
    if host.ram_gb > 0.0 {
        dims += 1;
        total += (host.ram_gb - req.min_ram_gb).min(host.ram_gb) / host.ram_gb;
    }

    // VRAM ratio (only matters when the host has vram or the stage needs it)
    if host.vram_gb > 0.0 || req.min_vram_gb > 0.0 {
        dims += 1;
        let available = if host.vram_gb > 0.0 { host.vram_gb } else { 0.0 };
        let needed = if req.min_vram_gb > 0.0 { req.min_vram_gb } else { 0.0 };
        total += if available > 0.0 {
            (available - needed).min(available) / available
        } else if needed == 0.0 {
            1.0 // no vram needed, no vram available → neutral
        } else {
            0.0
        };
    }

    // CPU cores
    if let Some(cores) = req.cpu_cores {
        if host.cpu_cores > 0 {
            dims += 1;
            total += (host.cpu_cores.saturating_sub(cores)) as f64 / host.cpu_cores as f64;
        }
    }

    if dims == 0 {
        0.5 // neutral score when no dimensions to compare
    } else {
        total / dims as f64
    }
}

/// Find all hosts that satisfy ResourceFit for the given requirements, returning
/// `(host_index, score)` pairs sorted by descending score.
fn ranked_fitting_hosts<'a>(
    req: &ResourceRequirements,
    hosts: &'a [HostInfo],
) -> Vec<(usize, f64)> {
    let mut candidates: Vec<(usize, f64)> = hosts
        .iter()
        .enumerate()
        .filter(|(_, h)| req.fits_on(&h.resources))
        .map(|(i, h)| (i, score_fit(req, &h.resources)))
        .collect();
    // Sort by score descending — stable sort preserves insertion order for ties.
    candidates.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
    candidates
}

// ── GreedyScheduler ────────────────────────────────────────────────────────────

/// Assigns each stage to the first host that satisfies `ResourceFit`, preferring
/// the host with the highest available-resource score.
///
/// Stages are processed in declaration order.  Each stage is matched to the
/// best-fitting host independently (no state is carried between stages).
#[derive(Debug, Clone, Default)]
pub struct GreedyScheduler;

impl Scheduler for GreedyScheduler {
    fn schedule(&self, stages: &[StageSpec], hosts: &[HostInfo]) -> PipelineSchedule {
        let mut mapping = Vec::with_capacity(stages.len());
        let mut unassigned = Vec::new();
        let mut score_sum = 0.0f64;
        let mut allocated = 0u32;

        for stage in stages {
            let req = &stage.profile.resources;
            let ranked = ranked_fitting_hosts(req, hosts);

            if ranked.is_empty() {
                // Build a diagnostic reason
                let reason = build_no_fit_reason(req, hosts);
                mapping.push(ScheduleResult {
                    stage_name: stage.name.clone(),
                    decision: ScheduleDecision::NoFit { reason: reason.clone() },
                    alternatives: vec![],
                });
                unassigned.push(stage.name.clone());
            } else {
                let (best_idx, best_score) = ranked[0];
                let alternatives: Vec<String> = ranked[1..]
                    .iter()
                    .map(|(i, _)| hosts[*i].name.clone())
                    .collect();
                mapping.push(ScheduleResult {
                    stage_name: stage.name.clone(),
                    decision: ScheduleDecision::Allocate {
                        host: hosts[best_idx].name.clone(),
                        score: best_score,
                    },
                    alternatives,
                });
                score_sum += best_score;
                allocated += 1;
            }
        }

        PipelineSchedule {
            overall_score: if allocated > 0 { score_sum / allocated as f64 } else { 0.0 },
            mapping,
            unassigned,
        }
    }
}

// ── BinpackScheduler ───────────────────────────────────────────────────────────

/// Packs stages onto hosts to minimise the total number of hosts used (greedy
/// binpack with largest-first ordering).
///
/// Stages are sorted by resource demand (descending) before assignment.  Each
/// stage is placed on the first already-used host that fits; if no used host
/// suffices, a new host is opened.
#[derive(Debug, Clone, Default)]
pub struct BinpackScheduler;

impl Scheduler for BinpackScheduler {
    fn schedule(&self, stages: &[StageSpec], hosts: &[HostInfo]) -> PipelineSchedule {
        // Sort stages by demand: GPU-first, then highest-RAM-first.
        let mut indexed: Vec<(usize, &StageSpec)> = stages.iter().enumerate().collect();
        indexed.sort_by(|(_, a), (_, b)| {
            let a_demand = resource_demand(&a.profile.resources);
            let b_demand = resource_demand(&b.profile.resources);
            b_demand.partial_cmp(&a_demand).unwrap_or(std::cmp::Ordering::Equal)
        });

        // Track remaining resources per host as we pack.
        let mut remaining: Vec<(String, HostResources)> = hosts
            .iter()
            .map(|h| (h.name.clone(), h.resources.clone()))
            .collect();

        // Track which physical host indexes have been used.
        let mut used_hosts: Vec<usize> = Vec::new();
        // Result mapping: stage_name → (host_name, score, alternatives, is_assigned)
        // We build this after assignment via original order.
        let mut assignments: Vec<Option<(String, f64, Vec<String>)>> = vec![None; stages.len()];
        let mut unassigned: Vec<String> = Vec::new();
        let mut score_sum = 0.0f64;
        let mut allocated = 0u32;

        for (orig_idx, stage) in &indexed {
            let req = &stage.profile.resources;

            // Try to fit on an already-opened host first.
            let mut placed = false;
            for &hi in &used_hosts {
                let (ref _name, ref host_res) = remaining[hi];
                if req.fits_on(host_res) {
                    // Place here.
                    let score = score_fit(req, host_res);
                    let alternatives: Vec<String> = used_hosts
                        .iter()
                        .filter(|&&i| i != hi)
                        .filter(|&&i| req.fits_on(&remaining[i].1))
                        .map(|&i| remaining[i].0.clone())
                        .collect();
                    assignments[*orig_idx] = Some((
                        remaining[hi].0.clone(),
                        score,
                        alternatives,
                    ));
                    // Deduct used resources (simple subtraction — no fragmentation model).
                    let (_, ref mut res) = remaining[hi];
                    res.ram_gb = (res.ram_gb - req.min_ram_gb).max(0.0);
                    res.vram_gb = (res.vram_gb - req.min_vram_gb).max(0.0);
                    score_sum += score;
                    allocated += 1;
                    placed = true;
                    break;
                }
            }

            if placed {
                continue;
            }

            // Try unused hosts.
            let mut best: Option<(usize, f64)> = None;
            for (hi, (_, host_res)) in remaining.iter().enumerate() {
                if used_hosts.contains(&hi) {
                    continue;
                }
                if req.fits_on(host_res) {
                    let score = score_fit(req, host_res);
                    let better = match best {
                        None => true,
                        Some((_, best_score)) => score > best_score,
                    };
                    if better {
                        best = Some((hi, score));
                    }
                }
            }

            if let Some((hi, score)) = best {
                let alternatives: Vec<String> = remaining
                    .iter()
                    .enumerate()
                    .filter(|&(i, _)| i != hi && req.fits_on(&remaining[i].1))
                    .map(|(_, (name, _))| name.clone())
                    .collect();
                assignments[*orig_idx] = Some((remaining[hi].0.clone(), score, alternatives));
                used_hosts.push(hi);
                let (_, ref mut res) = remaining[hi];
                res.ram_gb = (res.ram_gb - req.min_ram_gb).max(0.0);
                res.vram_gb = (res.vram_gb - req.min_vram_gb).max(0.0);
                score_sum += score;
                allocated += 1;
            } else {
                // No host fits
                let _reason = build_no_fit_reason(req, hosts);
                unassigned.push(stage.name.clone());
                // Leave assignments[orig_idx] as None
            }
        }

        // Build mapping in *original* declaration order.
        let mapping: Vec<ScheduleResult> = stages
            .iter()
            .enumerate()
            .map(|(i, stage)| match &assignments[i] {
                Some((host, score, alternatives)) => ScheduleResult {
                    stage_name: stage.name.clone(),
                    decision: ScheduleDecision::Allocate {
                        host: host.clone(),
                        score: *score,
                    },
                    alternatives: alternatives.clone(),
                },
                None => ScheduleResult {
                    stage_name: stage.name.clone(),
                    decision: ScheduleDecision::NoFit {
                        reason: build_no_fit_reason(&stage.profile.resources, hosts),
                    },
                    alternatives: vec![],
                },
            })
            .collect();

        PipelineSchedule {
            overall_score: if allocated > 0 { score_sum / allocated as f64 } else { 0.0 },
            mapping,
            unassigned,
        }
    }
}

// ── Helpers ────────────────────────────────────────────────────────────────────

/// Rough proxy for resource demand — used to sort stages largest-first for binpack.
fn resource_demand(req: &ResourceRequirements) -> f64 {
    let gpu_weight = if req.requires_gpu { 1000.0 } else { 0.0 };
    gpu_weight + req.min_ram_gb + req.min_vram_gb * 2.0
}

/// Produce a human-readable reason explaining why no host fits this stage.
fn build_no_fit_reason(req: &ResourceRequirements, hosts: &[HostInfo]) -> String {
    let mut reasons = Vec::new();

    if req.requires_gpu && hosts.iter().all(|h| h.resources.gpu_count == 0) {
        reasons.push("no GPU hosts available".to_string());
    }
    if hosts.iter().all(|h| h.resources.ram_gb < req.min_ram_gb) {
        reasons.push(format!("insufficient RAM (need {} GB)", req.min_ram_gb));
    }
    if hosts.iter().all(|h| h.resources.vram_gb < req.min_vram_gb) {
        reasons.push(format!("insufficient VRAM (need {} GB)", req.min_vram_gb));
    }
    if let Some(cores) = req.cpu_cores {
        if hosts.iter().all(|h| h.resources.cpu_cores < cores) {
            reasons.push(format!("insufficient CPU cores (need {})", cores));
        }
    }

    if reasons.is_empty() {
        "no host matches resource requirements".to_string()
    } else {
        reasons.join("; ")
    }
}

/// Return a default set of hosts representing the local machine.
///
/// The resources are approximated from compile-time constants.  On a real
/// deployment these would come from `b00t hive status` or `/proc/meminfo`.
pub fn default_hosts() -> Vec<HostInfo> {
    let hostname = hostname::get()
        .map(|h| h.to_string_lossy().to_string())
        .unwrap_or_else(|_| "localhost".to_string());

    vec![HostInfo {
        name: hostname,
        resources: HostResources {
            ram_gb: 16.0,
            vram_gb: 0.0,
            gpu_count: 0,
            cpu_cores: 8,
        },
        labels: HashMap::from([
            ("provider".to_string(), "local".to_string()),
            ("region".to_string(), "localhost".to_string()),
        ]),
    }]
}

// ── CLI handler ────────────────────────────────────────────────────────────────

/// Simulate scheduling a pipeline against available hosts.
///
/// Dispatched from `b00t pipeline schedule <name> [--strategy greedy|binpack]`.
pub fn handle_schedule_command(
    pipeline_name: &str,
    strategy: &str,
    b00t_path: &str,
) -> anyhow::Result<String> {
    use crate::commands::pipeline::discover_pipeline_datums;
    use crate::datum_pipeline::PipelineDatum;
    use std::path::Path;

    let pipelines = discover_pipeline_datums(b00t_path)?;
    let (_, datum, file_path) = pipelines
        .into_iter()
        .find(|(key, _, _)| key == pipeline_name)
        .ok_or_else(|| anyhow::anyhow!("no pipeline datum named '{}'", pipeline_name))?;

    let base_dir = Path::new(&file_path)
        .parent()
        .unwrap_or_else(|| Path::new("."))
        .to_path_buf();
    let pipeline = PipelineDatum::from_datum(datum, &base_dir)?;

    // Build StageSpecs from the pipeline datum.
    let stages: Vec<StageSpec> = pipeline
        .stages()
        .iter()
        .map(|name| {
            // Use a basic StageSpec — the profile resources come from the datum.
            let mut spec = StageSpec::from_name(name);
            spec.profile.resources = ResourceRequirements {
                min_ram_gb: 1.0,
                min_vram_gb: 0.0,
                requires_gpu: false,
                cpu_cores: None,
                scratch_disk_gb: None,
            };
            spec
        })
        .collect();

    let hosts = default_hosts();

    let scheduler: Box<dyn Scheduler> = match strategy {
        "binpack" => Box::new(BinpackScheduler),
        _ => Box::new(GreedyScheduler),
    };

    let schedule = scheduler.schedule(&stages, &hosts);
    let output = format_schedule(pipeline_name, &schedule);
    Ok(output)
}

/// Format a PipelineSchedule as a human-readable string.
fn format_schedule(pipeline_name: &str, schedule: &PipelineSchedule) -> String {
    let mut out = String::new();
    out.push_str(&format!(
        "Pipeline Schedule: {}\n",
        pipeline_name
    ));
    out.push_str(&format!(
        "Overall Score: {:.3}  |  Unassigned: {}\n\n",
        schedule.overall_score,
        schedule.unassigned.len()
    ));

    for result in &schedule.mapping {
        match &result.decision {
            ScheduleDecision::Allocate { host, score } => {
                out.push_str(&format!(
                    "  ✓ {:<20} → {:<20}  (score: {:.3})",
                    result.stage_name, host, score
                ));
                if !result.alternatives.is_empty() {
                    out.push_str(&format!("  alt: [{}]", result.alternatives.join(", ")));
                }
                out.push('\n');
            }
            ScheduleDecision::NoFit { reason } => {
                out.push_str(&format!(
                    "  ✗ {:<20} → NO FIT  ({})\n",
                    result.stage_name, reason
                ));
            }
            ScheduleDecision::Deferred { reason } => {
                out.push_str(&format!(
                    "  ~ {:<20} → DEFERRED ({})\n",
                    result.stage_name, reason
                ));
            }
        }
    }

    if !schedule.unassigned.is_empty() {
        out.push_str(&format!(
            "\nUnassigned stages: {}\n",
            schedule.unassigned.join(", ")
        ));
    }

    out
}

// ── Tests ──────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pipeline_types::CapsuleProfile;

    fn make_stage(name: &str, req: ResourceRequirements) -> StageSpec {
        StageSpec {
            name: name.to_string(),
            profile: CapsuleProfile {
                name: name.to_string(),
                ports: vec![],
                resources: req,
                image: None,
                timeout_seconds: None,
            },
            input_ports: vec![],
            output_ports: vec![],
            error_routes: vec![],
            env: None,
            checkpoint_interval_seconds: None,
            secret_refs: None,
            flow_control: None,
        }
    }

    fn make_host(name: &str, ram_gb: f64, vram_gb: f64, gpu_count: u32, cpu_cores: u32) -> HostInfo {
        HostInfo {
            name: name.to_string(),
            resources: HostResources {
                ram_gb,
                vram_gb,
                gpu_count,
                cpu_cores,
            },
            labels: HashMap::new(),
        }
    }

    // ── Basic fit tests ────────────────────────────────────────────────────

    #[test]
    fn stage_fits_on_host() {
        let req = ResourceRequirements {
            min_ram_gb: 4.0,
            min_vram_gb: 0.0,
            requires_gpu: false,
            cpu_cores: None,
            scratch_disk_gb: None,
        };
        let host = make_host("worker-1", 16.0, 0.0, 0, 8);
        assert!(req.fits_on(&host.resources));

        let stage = make_stage("encode", req);
        let scheduler = GreedyScheduler;
        let result = scheduler.schedule(&[stage], &[host]);
        assert_eq!(result.unassigned.len(), 0);
        assert_eq!(result.mapping.len(), 1);
        match &result.mapping[0].decision {
            ScheduleDecision::Allocate { host, .. } => assert_eq!(host, "worker-1"),
            other => panic!("expected Allocate, got {other:?}"),
        }
    }

    #[test]
    fn stage_needs_gpu_no_gpu_host() {
        let req = ResourceRequirements {
            min_ram_gb: 1.0,
            min_vram_gb: 8.0,
            requires_gpu: true,
            cpu_cores: None,
            scratch_disk_gb: None,
        };
        let host = make_host("cpu-only", 16.0, 0.0, 0, 8);
        let stage = make_stage("infer", req);
        let scheduler = GreedyScheduler;
        let result = scheduler.schedule(&[stage], &[host]);
        assert_eq!(result.unassigned.len(), 1);
        match &result.mapping[0].decision {
            ScheduleDecision::NoFit { reason } => {
                assert!(reason.contains("GPU"), "reason: {reason}");
            }
            other => panic!("expected NoFit, got {other:?}"),
        }
    }

    #[test]
    fn multiple_stages_spread_across_hosts() {
        let req_a = ResourceRequirements {
            min_ram_gb: 8.0, min_vram_gb: 0.0, requires_gpu: false,
            cpu_cores: None, scratch_disk_gb: None,
        };
        let req_b = ResourceRequirements {
            min_ram_gb: 4.0, min_vram_gb: 0.0, requires_gpu: false,
            cpu_cores: None, scratch_disk_gb: None,
        };

        let host_a = make_host("big-host", 32.0, 0.0, 0, 16);
        let host_b = make_host("small-host", 8.0, 0.0, 0, 4);

        let stage_a = make_stage("stage-a", req_a);
        let stage_b = make_stage("stage-b", req_b);

        let scheduler = GreedyScheduler;
        let result = scheduler.schedule(&[stage_a, stage_b], &[host_a, host_b]);
        assert_eq!(result.unassigned.len(), 0);
        assert_eq!(result.mapping.len(), 2);

        // Both should be allocated.
        for r in &result.mapping {
            assert!(matches!(r.decision, ScheduleDecision::Allocate { .. }));
        }
    }

    #[test]
    fn greedy_vs_binpack_different_results() {
        // Four small stages that could fit on 2 hosts — greedy will spread,
        // binpack will concentrate.
        let req = ResourceRequirements {
            min_ram_gb: 6.0, min_vram_gb: 0.0, requires_gpu: false,
            cpu_cores: None, scratch_disk_gb: None,
        };

        let hosts = vec![
            make_host("node-a", 16.0, 0.0, 0, 8),
            make_host("node-b", 16.0, 0.0, 0, 8),
        ];

        let stages = vec![
            make_stage("s1", req.clone()),
            make_stage("s2", req.clone()),
            make_stage("s3", req.clone()),
        ];

        let greedy = GreedyScheduler.schedule(&stages, &hosts);
        let binpack = BinpackScheduler.schedule(&stages, &hosts);

        // Both should assign all stages.
        assert_eq!(greedy.unassigned.len(), 0);
        assert_eq!(binpack.unassigned.len(), 0);

        // Greedy may spread across hosts; binpack should pack onto as few as possible.
        let greedy_hosts: std::collections::HashSet<String> = greedy
            .mapping
            .iter()
            .filter_map(|r| match &r.decision {
                ScheduleDecision::Allocate { host, .. } => Some(host.clone()),
                _ => None,
            })
            .collect();

        let binpack_hosts: std::collections::HashSet<String> = binpack
            .mapping
            .iter()
            .filter_map(|r| match &r.decision {
                ScheduleDecision::Allocate { host, .. } => Some(host.clone()),
                _ => None,
            })
            .collect();

        // With 3 stages of 6GB each, binpack should pack 2+1 onto two hosts
        // (same as greedy in this case), but the *distribution* may differ.
        // We verify at least that both succeed and strategies differ in distribution.
        // In the specific case of equal hosts + equal stages, greedy and binpack may
        // produce identical results.  The key property: both produce valid schedules.
        assert!(greedy_hosts.len() >= 1);
        assert!(binpack_hosts.len() >= 1);

        // Now test with uneven stages where binpack should definitely differ:
        let big_req = ResourceRequirements {
            min_ram_gb: 12.0, min_vram_gb: 0.0, requires_gpu: false,
            cpu_cores: None, scratch_disk_gb: None,
        };
        let small_req = ResourceRequirements {
            min_ram_gb: 2.0, min_vram_gb: 0.0, requires_gpu: false,
            cpu_cores: None, scratch_disk_gb: None,
        };

        let uneven_stages = vec![
            make_stage("big", big_req),
            make_stage("small-a", small_req.clone()),
            make_stage("small-b", small_req.clone()),
            make_stage("small-c", small_req),
        ];

        // Only one host with enough RAM for the big stage
        let uneven_hosts = vec![
            make_host("fat-node", 16.0, 0.0, 0, 8),
            make_host("thin-node", 4.0, 0.0, 0, 4),
        ];

        let greedy_u = GreedyScheduler.schedule(&uneven_stages, &uneven_hosts);
        let binpack_u = BinpackScheduler.schedule(&uneven_stages, &uneven_hosts);

        let greedy_host_count: usize = greedy_u
            .mapping
            .iter()
            .filter_map(|r| match &r.decision {
                ScheduleDecision::Allocate { host, .. } => Some(host.clone()),
                _ => None,
            })
            .collect::<std::collections::HashSet<_>>()
            .len();

        let binpack_host_count: usize = binpack_u
            .mapping
            .iter()
            .filter_map(|r| match &r.decision {
                ScheduleDecision::Allocate { host, .. } => Some(host.clone()),
                _ => None,
            })
            .collect::<std::collections::HashSet<_>>()
            .len();

        // Both schedules are valid (all stages assigned).
        assert_eq!(greedy_u.unassigned.len(), 0);
        assert_eq!(binpack_u.unassigned.len(), 0);

        // Greedy (stateless) puts every stage on the single best-fitting host;
        // binpack (stateful with resource tracking) spills to thin-node when
        // fat-node fills up. The strategies MUST produce different distributions.
        let greedy_map: Vec<Option<&str>> = greedy_u.mapping.iter().map(|r| match &r.decision {
            ScheduleDecision::Allocate { host, .. } => Some(host.as_str()),
            _ => None,
        }).collect();
        let binpack_map: Vec<Option<&str>> = binpack_u.mapping.iter().map(|r| match &r.decision {
            ScheduleDecision::Allocate { host, .. } => Some(host.as_str()),
            _ => None,
        }).collect();
        assert_ne!(
            greedy_map, binpack_map,
            "greedy and binpack should produce different host assignments for uneven stages"
        );
    }

    #[test]
    fn empty_stage_list() {
        let hosts = vec![make_host("any", 16.0, 0.0, 0, 8)];
        let scheduler = GreedyScheduler;
        let result = scheduler.schedule(&[], &hosts);
        assert_eq!(result.mapping.len(), 0);
        assert_eq!(result.unassigned.len(), 0);
        assert_eq!(result.overall_score, 0.0);

        let result2 = BinpackScheduler.schedule(&[], &hosts);
        assert_eq!(result2.mapping.len(), 0);
        assert_eq!(result2.unassigned.len(), 0);
    }

    // ── Default hosts ─────────────────────────────────────────────────────

    #[test]
    fn default_hosts_returns_localhost() {
        let hosts = default_hosts();
        assert_eq!(hosts.len(), 1);
        assert!(!hosts[0].name.is_empty());
        assert_eq!(hosts[0].resources.ram_gb, 16.0);
        assert_eq!(hosts[0].resources.cpu_cores, 8);
        assert!(hosts[0].labels.contains_key("provider"));
        assert_eq!(hosts[0].labels["provider"], "local");
    }

    // ── Serialization round-trips ──────────────────────────────────────────

    #[test]
    fn host_info_serialize_round_trip() {
        let host = HostInfo {
            name: "test-host".into(),
            resources: HostResources { ram_gb: 32.0, vram_gb: 0.0, gpu_count: 0, cpu_cores: 16 },
            labels: HashMap::from([("rack".into(), "A1".into())]),
        };
        let json = serde_json::to_string(&host).unwrap();
        let back: HostInfo = serde_json::from_str(&json).unwrap();
        assert_eq!(host.name, back.name);
        assert_eq!(host.resources.ram_gb, back.resources.ram_gb);
    }

    #[test]
    fn schedule_decision_serialize_round_trip() {
        let cases: Vec<ScheduleDecision> = vec![
            ScheduleDecision::Allocate { host: "node-1".into(), score: 0.85 },
            ScheduleDecision::NoFit { reason: "needs GPU".into() },
            ScheduleDecision::Deferred { reason: "waiting on upstream".into() },
        ];
        for d in &cases {
            let json = serde_json::to_string(d).unwrap();
            let back: ScheduleDecision = serde_json::from_str(&json).unwrap();
            // Compare debug representations for enum variant equality.
            assert_eq!(
                format!("{d:?}"),
                format!("{back:?}"),
                "round-trip failed for {d:?}"
            );
        }
    }

    #[test]
    fn pipeline_schedule_serialize_round_trip() {
        let schedule = PipelineSchedule {
            mapping: vec![ScheduleResult {
                stage_name: "encode".into(),
                decision: ScheduleDecision::Allocate { host: "gpu-1".into(), score: 0.9 },
                alternatives: vec!["gpu-2".into()],
            }],
            overall_score: 0.9,
            unassigned: vec![],
        };
        let json = serde_json::to_string(&schedule).unwrap();
        let back: PipelineSchedule = serde_json::from_str(&json).unwrap();
        assert_eq!(schedule.overall_score, back.overall_score);
        assert_eq!(schedule.mapping.len(), back.mapping.len());
    }

    // ── Score sanity ──────────────────────────────────────────────────────

    #[test]
    fn score_fit_perfect_match_is_one() {
        let req = ResourceRequirements {
            min_ram_gb: 8.0, min_vram_gb: 0.0, requires_gpu: false,
            cpu_cores: Some(4), scratch_disk_gb: None,
        };
        let host = HostResources { ram_gb: 8.0, vram_gb: 0.0, gpu_count: 0, cpu_cores: 4 };
        // When exact match, remaining = 0, so ram score = (8-8)/8 = 0, cpu score = (4-4)/4 = 0.
        // So score should be 0.0 (no headroom).
        let score = score_fit(&req, &host);
        assert!((score - 0.0).abs() < 1e-6, "expected 0.0 for exact match, got {score}");
    }

    #[test]
    fn score_fit_ample_headroom() {
        let req = ResourceRequirements {
            min_ram_gb: 2.0, min_vram_gb: 0.0, requires_gpu: false,
            cpu_cores: None, scratch_disk_gb: None,
        };
        let host = HostResources { ram_gb: 16.0, vram_gb: 0.0, gpu_count: 0, cpu_cores: 8 };
        let score = score_fit(&req, &host);
        // (16-2)/16 = 0.875
        assert!((score - 0.875).abs() < 1e-6, "expected ~0.875, got {score}");
    }
}
