use crate::pipeline_costs::{
    estimate_from_stage_profile, CostConfig, CostEstimate, PipelineCostReport, ResourceUsage,
    StageCostRow,
};
use crate::pipeline_types::CapsuleProfile;
use anyhow::Result;
use clap::Parser;

// ── GH #745: CLI commands for pipeline cost attribution ──

#[derive(Parser, Debug)]
pub struct PipelineCostArgs {
    /// Pipeline datum name to show cost breakdown for.
    #[clap(help = "Pipeline datum name")]
    pub pipeline_id: Option<String>,

    /// Aggregate by operator (human-readable group).
    #[clap(long, help = "Aggregate costs by operator (requires --since)")]
    pub by_operator: bool,

    /// Show costs for runs within the last N days (e.g. 7d, 30d).
    #[clap(long, help = "Show costs since N days ago (e.g. 7d, 30d)")]
    pub since: Option<String>,

    /// Forecast cost for a pipeline name based on historical average.
    #[clap(long, help = "Estimate cost for a pipeline based on historical average")]
    pub forecast: Option<String>,
}

/// Handle the `pipeline cost` subcommand.
pub fn handle_pipeline_cost(args: &PipelineCostArgs) -> Result<()> {
    let config = CostConfig::default();

    // ── Forecast mode ──
    if let Some(ref pipeline_name) = args.forecast {
        return handle_forecast(pipeline_name, &config);
    }

    // ── Per-pipeline or by-operator mode ──
    if let Some(ref pipeline_id) = args.pipeline_id {
        let report = build_report_for_pipeline(pipeline_id, &config)?;
        report.print();
        return Ok(());
    }

    // Print usage when no arguments
    println!("Usage:");
    println!("  b00t pipeline cost <pipeline-id>              Show cost breakdown per stage");
    println!("  b00t pipeline cost --since 7d --by-operator   Aggregate by operator");
    println!("  b00t pipeline cost --forecast <pipeline-name>  Estimate based on historical avg");
    Ok(())
}

/// Build a sample cost report for a given pipeline ID.
///
/// In a production integration this would read actual stage execution telemetry
/// from the pipeline log store. For now we synthesise usage from dummy profiles
/// keyed by the pipeline ID.
fn build_report_for_pipeline(pipeline_id: &str, config: &CostConfig) -> Result<PipelineCostReport> {
    let stages = sample_stages_for(pipeline_id);

    let mut stage_rows = Vec::new();
    let mut total_usage = ResourceUsage::default();

    for stage in &stages {
        // Simulate: each stage runs for a variable duration with ~100 MiB data
        let simulated_duration = match stage.name.as_str() {
            s if s.contains("encode") || s.contains("transcode") => 300.0,
            s if s.contains("ingest") || s.contains("upload") => 120.0,
            s if s.contains("infer") || s.contains("classify") => 600.0,
            s if s.contains("decode") || s.contains("extract") => 180.0,
            _ => 60.0,
        };
        let simulated_data: u64 = match stage.name.as_str() {
            s if s.contains("video") || s.contains("image") => 500_000_000, // ~500 MB
            s if s.contains("audio") => 50_000_000,                         // ~50 MB
            _ => 10_000_000,                                                // ~10 MB
        };

        let stage_usage = estimate_from_stage_profile(&stage, simulated_duration, simulated_data);

        // Accumulate totals
        total_usage.gpu_seconds += stage_usage.gpu_seconds;
        total_usage.cpu_seconds += stage_usage.cpu_seconds;
        total_usage.bytes_ingested += stage_usage.bytes_ingested;
        total_usage.bytes_egressed += stage_usage.bytes_egressed;

        let stage_estimate = CostEstimate::calculate(&stage_usage, config);

        stage_rows.push(StageCostRow {
            stage_name: stage.name.clone(),
            gpu_hr: stage_usage.gpu_hours(),
            cpu_hr: stage_usage.cpu_hours(),
            data_gib: stage_usage.data_gib(),
            cost_usd: stage_estimate.estimated_cost_usd.unwrap_or(0.0),
        });
    }

    let total_estimate = CostEstimate::calculate(&total_usage, config);

    Ok(PipelineCostReport {
        pipeline_id: pipeline_id.to_string(),
        stages: stage_rows,
        total_cost: total_estimate,
    })
}

/// Generate sample stage profiles for known pipeline IDs.
///
/// These simulate real-world pipeline topologies. When available, replace with
/// actual pipeline DAG definitions from the datum store.
fn sample_stages_for(pipeline_id: &str) -> Vec<CapsuleProfile> {
    match pipeline_id {
        "video-pipeline" | "video-transcode" => vec![
            CapsuleProfile {
                name: "ffmpeg-decode".into(),
                ports: vec![],
                resources: crate::pipeline_types::ResourceRequirements {
                    min_ram_gb: 2.0, min_vram_gb: 0.0, requires_gpu: false,
                    cpu_cores: Some(2), scratch_disk_gb: None,
                },
                image: None, timeout_seconds: None,
            },
            CapsuleProfile {
                name: "gpu-encode-h264".into(),
                ports: vec![],
                resources: crate::pipeline_types::ResourceRequirements {
                    min_ram_gb: 4.0, min_vram_gb: 8.0, requires_gpu: true,
                    cpu_cores: Some(4), scratch_disk_gb: None,
                },
                image: None, timeout_seconds: None,
            },
            CapsuleProfile {
                name: "mux-upload".into(),
                ports: vec![],
                resources: crate::pipeline_types::ResourceRequirements {
                    min_ram_gb: 1.0, min_vram_gb: 0.0, requires_gpu: false,
                    cpu_cores: None, scratch_disk_gb: None,
                },
                image: None, timeout_seconds: None,
            },
        ],
        "inference-pipeline" | "llm-infer" => vec![
            CapsuleProfile {
                name: "tokenize".into(),
                ports: vec![],
                resources: crate::pipeline_types::ResourceRequirements {
                    min_ram_gb: 0.5, min_vram_gb: 0.0, requires_gpu: false,
                    cpu_cores: None, scratch_disk_gb: None,
                },
                image: None, timeout_seconds: None,
            },
            CapsuleProfile {
                name: "gpu-infer-llama".into(),
                ports: vec![],
                resources: crate::pipeline_types::ResourceRequirements {
                    min_ram_gb: 8.0, min_vram_gb: 24.0, requires_gpu: true,
                    cpu_cores: Some(8), scratch_disk_gb: None,
                },
                image: None, timeout_seconds: None,
            },
            CapsuleProfile {
                name: "detokenize-output".into(),
                ports: vec![],
                resources: crate::pipeline_types::ResourceRequirements {
                    min_ram_gb: 0.5, min_vram_gb: 0.0, requires_gpu: false,
                    cpu_cores: None, scratch_disk_gb: None,
                },
                image: None, timeout_seconds: None,
            },
        ],
        "audio-pipeline" | "stt" => vec![
            CapsuleProfile {
                name: "audio-ingest".into(),
                ports: vec![],
                resources: crate::pipeline_types::ResourceRequirements {
                    min_ram_gb: 1.0, min_vram_gb: 0.0, requires_gpu: false,
                    cpu_cores: None, scratch_disk_gb: None,
                },
                image: None, timeout_seconds: None,
            },
            CapsuleProfile {
                name: "gpu-whisper-transcribe".into(),
                ports: vec![],
                resources: crate::pipeline_types::ResourceRequirements {
                    min_ram_gb: 4.0, min_vram_gb: 6.0, requires_gpu: true,
                    cpu_cores: Some(4), scratch_disk_gb: None,
                },
                image: None, timeout_seconds: None,
            },
            CapsuleProfile {
                name: "text-postprocess".into(),
                ports: vec![],
                resources: crate::pipeline_types::ResourceRequirements {
                    min_ram_gb: 0.5, min_vram_gb: 0.0, requires_gpu: false,
                    cpu_cores: None, scratch_disk_gb: None,
                },
                image: None, timeout_seconds: None,
            },
        ],
        _ => vec![
            CapsuleProfile {
                name: format!("{}-stage-default", pipeline_id),
                ports: vec![],
                resources: crate::pipeline_types::ResourceRequirements {
                    min_ram_gb: 1.0, min_vram_gb: 0.0, requires_gpu: false,
                    cpu_cores: None, scratch_disk_gb: None,
                },
                image: None, timeout_seconds: None,
            },
        ],
    }
}

/// Forecast cost for a pipeline based on historical averages.
///
/// Uses the same stage profiles as `sample_stages_for` but multiplies
/// by an assumed number of runs per day for a forecast window.
fn handle_forecast(pipeline_name: &str, config: &CostConfig) -> Result<()> {
    let stages = sample_stages_for(pipeline_name);
    if stages.is_empty() {
        anyhow::bail!("Unknown pipeline: {}", pipeline_name);
    }

    // Simulate: average run takes 5 minutes processing 200 MB
    let avg_duration = 300.0;
    let avg_data: u64 = 200_000_000;

    let mut total_usage = ResourceUsage::default();
    for stage in &stages {
        let stage_usage = estimate_from_stage_profile(stage, avg_duration, avg_data);
        total_usage.gpu_seconds += stage_usage.gpu_seconds;
        total_usage.cpu_seconds += stage_usage.cpu_seconds;
        total_usage.bytes_ingested += stage_usage.bytes_ingested;
        total_usage.bytes_egressed += stage_usage.bytes_egressed;
    }

    let per_run = CostEstimate::calculate(&total_usage, config);
    let cost_per_run = per_run.estimated_cost_usd.unwrap_or(0.0);

    let daily_runs = 24.0; // ~1 run/hour
    let daily_cost = cost_per_run * daily_runs;
    let monthly_cost = daily_cost * 30.0;

    println!();
    println!("  {} Forecast for: {}", crate::ansi::bold("Pipeline:"), pipeline_name);
    println!(
        "  {}  1 run  — ${:.4}  (est. {:.1}s, {:.1} MiB)",
        crate::ansi::dim("Estimate"),
        cost_per_run,
        avg_duration,
        avg_data as f64 / (1024.0 * 1024.0)
    );
    println!(
        "  {}  Daily  — ${:.2}  ({} runs/day)",
        crate::ansi::dim("Projection"),
        daily_cost,
        daily_runs
    );
    println!(
        "  {}  Monthly — ${:.2}  (30-day estimate)",
        crate::ansi::bold("Projection"),
        monthly_cost
    );
    println!();

    if monthly_cost > 500.0 {
        println!("  {}  Monthly cost exceeds $500 — consider GPU reservation or spot instances", crate::ansi::red("⚠"));
    }
    println!();

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn handle_forecast_known_pipeline() {
        let config = CostConfig::default();
        let result = handle_forecast("video-pipeline", &config);
        assert!(result.is_ok());
    }

    #[test]
    fn handle_forecast_unknown_pipeline() {
        let config = CostConfig::default();
        let result = handle_forecast("nonexistent-pipeline-xyz", &config);
        assert!(result.is_err());
    }

    #[test]
    fn sample_stages_for_known_ids() {
        assert_eq!(sample_stages_for("video-pipeline").len(), 3);
        assert_eq!(sample_stages_for("inference-pipeline").len(), 3);
        assert_eq!(sample_stages_for("audio-pipeline").len(), 3);
    }

    #[test]
    fn sample_stages_for_unknown_id() {
        let stages = sample_stages_for("my-custom-pipe");
        assert_eq!(stages.len(), 1);
        assert!(stages[0].name.starts_with("my-custom-pipe"));
    }

    #[test]
    fn build_report_known_pipeline() {
        let config = CostConfig::default();
        let report = build_report_for_pipeline("video-pipeline", &config).unwrap();
        assert_eq!(report.pipeline_id, "video-pipeline");
        assert!(!report.stages.is_empty());
        assert!(report.total_cost.estimated_cost_usd.unwrap() > 0.0);
    }

    #[test]
    fn build_report_unknown_pipeline() {
        let config = CostConfig::default();
        let report = build_report_for_pipeline("custom", &config).unwrap();
        assert_eq!(report.stages.len(), 1);
    }
}
