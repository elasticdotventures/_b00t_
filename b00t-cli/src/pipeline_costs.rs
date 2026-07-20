use crate::ansi;
use crate::pipeline_types::CapsuleProfile;
use serde::{Deserialize, Serialize};

// ── GH #745: Cost attribution and GPU-time accounting ──

/// Raw resource consumption for a pipeline run.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ResourceUsage {
    pub gpu_seconds: f64,
    pub cpu_seconds: f64,
    pub bytes_ingested: u64,
    pub bytes_egressed: u64,
}

/// Default cost configuration.
/// GPU: $2.50/hr, CPU: $0.10/hr — typical cloud spot instance ballpark.
impl ResourceUsage {
    /// Convert GPU seconds to GPU hours.
    pub fn gpu_hours(&self) -> f64 {
        self.gpu_seconds / 3600.0
    }

    /// Convert CPU seconds to CPU hours.
    pub fn cpu_hours(&self) -> f64 {
        self.cpu_seconds / 3600.0
    }

    /// Total data moved in GiB.
    pub fn data_gib(&self) -> f64 {
        (self.bytes_ingested + self.bytes_egressed) as f64 / (1024.0 * 1024.0 * 1024.0)
    }
}

/// A cost estimate for a pipeline run.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CostEstimate {
    pub usage: ResourceUsage,
    pub estimated_cost_usd: Option<f64>,
    pub cost_per_gpu_hour: f64,
    pub cost_per_cpu_hour: f64,
}

/// Configuration for cost calculation rates.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CostConfig {
    pub gpu_per_hour: f64,
    pub cpu_per_hour: f64,
}

impl Default for CostConfig {
    fn default() -> Self {
        Self {
            gpu_per_hour: 2.50,
            cpu_per_hour: 0.10,
        }
    }
}

impl CostEstimate {
    /// Compute a cost estimate from a usage snapshot and rate config.
    pub fn calculate(usage: &ResourceUsage, config: &CostConfig) -> Self {
        let gpu_cost = usage.gpu_hours() * config.gpu_per_hour;
        let cpu_cost = usage.cpu_hours() * config.cpu_per_hour;
        Self {
            usage: usage.clone(),
            estimated_cost_usd: Some(gpu_cost + cpu_cost),
            cost_per_gpu_hour: config.gpu_per_hour,
            cost_per_cpu_hour: config.cpu_per_hour,
        }
    }
}

/// Estimate resource usage from a capsule profile and run metrics.
///
/// GPU seconds are credited only when the stage's `ResourceRequirements`
/// declares `requires_gpu: true`. CPU seconds always equal the wall-clock
/// duration. Data egress is assumed equal to ingress as a first approximation.
pub fn estimate_from_stage_profile(
    profile: &CapsuleProfile,
    duration_seconds: f64,
    data_bytes: u64,
) -> ResourceUsage {
    ResourceUsage {
        gpu_seconds: if profile.resources.requires_gpu {
            duration_seconds
        } else {
            0.0
        },
        cpu_seconds: duration_seconds,
        bytes_ingested: data_bytes,
        bytes_egressed: data_bytes,
    }
}

/// One row in the per-stage cost breakdown.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StageCostRow {
    pub stage_name: String,
    pub gpu_hr: f64,
    pub cpu_hr: f64,
    pub data_gib: f64,
    pub cost_usd: f64,
}

/// Full cost report for an entire pipeline run.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PipelineCostReport {
    pub pipeline_id: String,
    pub stages: Vec<StageCostRow>,
    pub total_cost: CostEstimate,
}

impl PipelineCostReport {
    /// Print a colored table of per-stage costs with a total row.
    pub fn print(&self) {
        let header = format!(
            "{:<24} {:>10} {:>10} {:>10} {:>12}",
            "Stage", "GPU·hr", "CPU·hr", "Data·GiB", "Cost·$"
        );
        println!("\n  {}", ansi::bold(&header));
        println!("  {}", ansi::dim(&"─".repeat(68)));

        for row in &self.stages {
            let line = format!(
                "{:<24} {:>10.4} {:>10.4} {:>10.4} {:>12.6}",
                row.stage_name, row.gpu_hr, row.cpu_hr, row.data_gib, row.cost_usd
            );
            println!("  {}", line);
        }

        println!("  {}", ansi::dim(&"─".repeat(68)));

        let total = self.total_cost.estimated_cost_usd.unwrap_or(0.0);
        let total_line = format!(
            "{:<24} {:>10.4} {:>10.4} {:>10.4} {:>12.6}",
            ansi::bold("Total"),
            self.total_cost.usage.gpu_hours(),
            self.total_cost.usage.cpu_hours(),
            self.total_cost.usage.data_gib(),
            total,
        );
        println!("  {}", total_line);
        println!();

        // Show rate info
        println!(
            "  {}  GPU: ${:.2}/hr  |  CPU: ${:.2}/hr",
            ansi::dim("Rates:"),
            self.total_cost.cost_per_gpu_hour,
            self.total_cost.cost_per_cpu_hour
        );

        // Highlight expensive runs
        if total > 10.0 {
            println!(
                "  {}  ${:.2} — consider optimising GPU-intensive stages",
                ansi::red("⚠"),
                total
            );
        } else if total > 1.0 {
            println!("  {}  ${:.2} — moderate cost", ansi::yellow("ℹ"), total);
        } else {
            println!("  {}  ${:.2} — low cost", ansi::green("✓"), total);
        }
        println!();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_usage() -> ResourceUsage {
        ResourceUsage {
            gpu_seconds: 3600.0,           // 1 GPU·hr
            cpu_seconds: 7200.0,           // 2 CPU·hr
            bytes_ingested: 1_073_741_824, // 1 GiB
            bytes_egressed: 2_147_483_648, // 2 GiB
        }
    }

    // ── CostEstimate::calculate ──

    #[test]
    fn calculate_default_rates() {
        let usage = sample_usage();
        let config = CostConfig::default();
        let est = CostEstimate::calculate(&usage, &config);
        // 1 GPU·hr @ $2.50 = $2.50, 2 CPU·hr @ $0.10 = $0.20 → $2.70
        assert!((est.estimated_cost_usd.unwrap() - 2.70).abs() < 1e-9);
        assert_eq!(est.cost_per_gpu_hour, 2.50);
        assert_eq!(est.cost_per_cpu_hour, 0.10);
    }

    #[test]
    fn calculate_custom_rates() {
        let usage = ResourceUsage {
            gpu_seconds: 1800.0, // 0.5 GPU·hr
            cpu_seconds: 3600.0, // 1 CPU·hr
            ..Default::default()
        };
        let config = CostConfig {
            gpu_per_hour: 5.00,
            cpu_per_hour: 0.50,
        };
        let est = CostEstimate::calculate(&usage, &config);
        // 0.5 GPU·hr @ $5.00 = $2.50, 1 CPU·hr @ $0.50 = $0.50 → $3.00
        assert!((est.estimated_cost_usd.unwrap() - 3.00).abs() < 1e-9);
    }

    #[test]
    fn calculate_zero_usage() {
        let usage = ResourceUsage::default();
        let config = CostConfig::default();
        let est = CostEstimate::calculate(&usage, &config);
        assert!((est.estimated_cost_usd.unwrap()).abs() < 1e-9);
    }

    #[test]
    fn calculate_gpu_seconds_correct() {
        let usage = ResourceUsage {
            gpu_seconds: 180.0, // 0.05 GPU·hr
            cpu_seconds: 180.0, // 0.05 CPU·hr
            ..Default::default()
        };
        let config = CostConfig::default();
        let est = CostEstimate::calculate(&usage, &config);
        // 0.05 * 2.50 + 0.05 * 0.10 = 0.125 + 0.005 = 0.13
        let expected = (180.0 / 3600.0) * 2.50 + (180.0 / 3600.0) * 0.10;
        assert!((est.estimated_cost_usd.unwrap() - expected).abs() < 1e-9);
    }

    // ── ResourceUsage helpers ──

    #[test]
    fn resource_usage_gpu_hours() {
        let usage = ResourceUsage {
            gpu_seconds: 7200.0,
            ..Default::default()
        };
        assert!((usage.gpu_hours() - 2.0).abs() < 1e-9);
    }

    #[test]
    fn resource_usage_cpu_hours() {
        let usage = ResourceUsage {
            cpu_seconds: 3600.0,
            ..Default::default()
        };
        assert!((usage.cpu_hours() - 1.0).abs() < 1e-9);
    }

    #[test]
    fn resource_usage_data_gib() {
        let usage = ResourceUsage {
            bytes_ingested: 1_073_741_824, // 1 GiB
            bytes_egressed: 1_073_741_824, // 1 GiB
            ..Default::default()
        };
        assert!((usage.data_gib() - 2.0).abs() < 1e-6);
    }

    #[test]
    fn resource_usage_default_zero() {
        let usage = ResourceUsage::default();
        assert_eq!(usage.gpu_seconds, 0.0);
        assert_eq!(usage.cpu_seconds, 0.0);
        assert_eq!(usage.bytes_ingested, 0);
        assert_eq!(usage.bytes_egressed, 0);
    }

    // ── estimate_from_stage_profile ──

    #[test]
    fn estimate_from_gpu_profile() {
        let profile = CapsuleProfile {
            name: "gpu-encode".into(),
            ports: vec![],
            resources: crate::pipeline_types::ResourceRequirements {
                min_ram_gb: 4.0,
                min_vram_gb: 8.0,
                requires_gpu: true,
                cpu_cores: Some(4),
                scratch_disk_gb: None,
            },
            image: None,
            timeout_seconds: None,
        };
        let usage = estimate_from_stage_profile(&profile, 120.0, 524_288_000);
        assert!((usage.gpu_seconds - 120.0).abs() < 1e-9);
        assert!((usage.cpu_seconds - 120.0).abs() < 1e-9);
        assert_eq!(usage.bytes_ingested, 524_288_000);
        assert_eq!(usage.bytes_egressed, 524_288_000);
    }

    #[test]
    fn estimate_from_cpu_profile() {
        let profile = CapsuleProfile {
            name: "cpu-parse".into(),
            ports: vec![],
            resources: crate::pipeline_types::ResourceRequirements {
                min_ram_gb: 2.0,
                min_vram_gb: 0.0,
                requires_gpu: false,
                cpu_cores: None,
                scratch_disk_gb: None,
            },
            image: None,
            timeout_seconds: None,
        };
        let usage = estimate_from_stage_profile(&profile, 60.0, 100_000);
        assert_eq!(usage.gpu_seconds, 0.0); // no GPU
        assert!((usage.cpu_seconds - 60.0).abs() < 1e-9);
    }

    #[test]
    fn estimate_zero_duration() {
        let profile = CapsuleProfile {
            name: "noop".into(),
            ports: vec![],
            resources: crate::pipeline_types::ResourceRequirements {
                min_ram_gb: 0.0,
                min_vram_gb: 0.0,
                requires_gpu: false,
                cpu_cores: None,
                scratch_disk_gb: None,
            },
            image: None,
            timeout_seconds: None,
        };
        let usage = estimate_from_stage_profile(&profile, 0.0, 0);
        assert_eq!(usage.gpu_seconds, 0.0);
        assert_eq!(usage.cpu_seconds, 0.0);
        assert_eq!(usage.bytes_ingested, 0);
        assert_eq!(usage.bytes_egressed, 0);
    }

    // ── PipelineCostReport ──

    #[test]
    fn report_print_does_not_panic() {
        let report = PipelineCostReport {
            pipeline_id: "test-pipe".into(),
            stages: vec![
                StageCostRow {
                    stage_name: "encode".into(),
                    gpu_hr: 1.0,
                    cpu_hr: 0.5,
                    data_gib: 2.0,
                    cost_usd: 2.55,
                },
                StageCostRow {
                    stage_name: "decode".into(),
                    gpu_hr: 0.0,
                    cpu_hr: 1.0,
                    data_gib: 0.5,
                    cost_usd: 0.10,
                },
            ],
            total_cost: CostEstimate {
                usage: ResourceUsage {
                    gpu_seconds: 3600.0,
                    cpu_seconds: 5400.0,
                    bytes_ingested: 1_073_741_824,
                    bytes_egressed: 2_147_483_648,
                },
                estimated_cost_usd: Some(2.65),
                cost_per_gpu_hour: 2.50,
                cost_per_cpu_hour: 0.10,
            },
        };
        // Should not panic when printing
        report.print();
    }

    // ── Serialization ──

    #[test]
    fn resource_usage_round_trip() {
        let u = ResourceUsage {
            gpu_seconds: 42.5,
            cpu_seconds: 100.0,
            bytes_ingested: 512,
            bytes_egressed: 256,
        };
        let json = serde_json::to_string(&u).unwrap();
        let back: ResourceUsage = serde_json::from_str(&json).unwrap();
        assert!((back.gpu_seconds - 42.5).abs() < 1e-9);
        assert_eq!(back.bytes_ingested, 512);
    }

    #[test]
    fn cost_estimate_round_trip() {
        let ce = CostEstimate {
            usage: ResourceUsage::default(),
            estimated_cost_usd: Some(1.23),
            cost_per_gpu_hour: 3.00,
            cost_per_cpu_hour: 0.15,
        };
        let json = serde_json::to_string(&ce).unwrap();
        let back: CostEstimate = serde_json::from_str(&json).unwrap();
        assert!((back.estimated_cost_usd.unwrap() - 1.23).abs() < 1e-9);
        assert!((back.cost_per_gpu_hour - 3.00).abs() < 1e-9);
    }

    #[test]
    fn cost_config_default_rates() {
        let cfg = CostConfig::default();
        assert!((cfg.gpu_per_hour - 2.50).abs() < 1e-9);
        assert!((cfg.cpu_per_hour - 0.10).abs() < 1e-9);
    }
}
