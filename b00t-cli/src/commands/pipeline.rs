// 🤓 b00t pipeline — typed, discoverable multi-stage pipeline datums (list, run)
//    First CLI dispatch surface for any CliExecutor implementor (CliDatum,
//    JustfileDatum, PipelineDatum previously had none). Shape mirrors
//    StoreCommands (commands/store.rs) — the closest proven multi-verb
//    datum-type subcommand pattern in this codebase.
use crate::commands::pipeline_cost::{handle_pipeline_cost, PipelineCostArgs};
use crate::commands::pipeline_validate::{print_validate_report, validate_pipeline};
use crate::datum_pipeline::PipelineDatum;
use crate::datum_utils::get_all_datums_with_paths;
use crate::pipeline_dataframe::handle_pipeline_data;
use crate::pipeline_logs::{handle_pipeline_logs, PipelineLogsArgs, PIPELINE_LOG_STORE};
use crate::pipeline_scheduler::handle_schedule_command;
use crate::traits::CliExecutor;
use crate::{BootDatum, DatumType};
use anyhow::Result;
use clap::Subcommand;
use std::path::Path;

#[derive(Debug, Subcommand)]
pub enum PipelineCommands {
    #[clap(about = "List discovered pipeline datums")]
    List,
    #[clap(about = "Run a pipeline's declared stages (all, or a selected subset)")]
    Run {
        #[clap(help = "Pipeline datum name")]
        name: String,
        #[clap(
            long = "stage",
            help = "Stage to run (repeatable; default: all stages, in declared order)"
        )]
        stages: Vec<String>,
    },
    #[clap(about = "Query or stream stage execution logs and telemetry")]
    Logs {
        #[clap(flatten)]
        args: PipelineLogsArgs,
    },
    #[clap(about = "Static validation of pipeline DAG before execution")]
    Validate {
        #[clap(help = "Pipeline datum name")]
        name: String,
    },
    #[clap(about = "Cost attribution and GPU-time accounting per pipeline run")]
    Cost {
        #[clap(flatten)]
        args: PipelineCostArgs,
    },
    #[clap(about = "Query stage outputs as dataframe rows")]
    Data {
        #[clap(help = "Pipeline run ID")]
        id: String,
        #[clap(long, help = "Filter by stage name")]
        stage: Option<String>,
        #[clap(long, help = "Comma-separated column names to include")]
        columns: Option<String>,
    },
    #[clap(about = "Simulate resource-aware scheduling against available hosts")]
    Schedule {
        #[clap(help = "Pipeline datum name")]
        name: String,
        #[clap(
            long,
            default_value = "greedy",
            help = "Scheduling strategy: greedy (default) or binpack"
        )]
        strategy: String,
    },
}

pub fn discover_pipeline_datums(b00t_path: &str) -> Result<Vec<(String, BootDatum, String)>> {
    let all = get_all_datums_with_paths(b00t_path, None)?;
    let mut pipelines: Vec<(String, BootDatum, String)> = all
        .into_iter()
        .filter(|(_, (datum, _))| datum.datum_type == Some(DatumType::Pipeline))
        .map(|(key, (datum, file_path))| (key, datum, file_path))
        .collect();
    pipelines.sort_by(|a, b| a.0.cmp(&b.0));
    Ok(pipelines)
}

pub fn handle_pipeline_command(cmd: &PipelineCommands, b00t_path: &str) -> Result<()> {
    match cmd {
        PipelineCommands::List => {
            let pipelines = discover_pipeline_datums(b00t_path)?;
            if pipelines.is_empty() {
                println!("No pipeline datums found.");
            } else {
                println!("Pipeline datums:");
                for (key, datum, file_path) in &pipelines {
                    let base_dir = Path::new(file_path).parent().unwrap_or_else(|| Path::new("."));
                    let stage_count = PipelineDatum::from_datum(datum.clone(), base_dir)
                        .map(|p| p.stages().len())
                        .unwrap_or(0);
                    println!("  {}  ({} stages)  {}", key, stage_count, datum.hint);
                }
            }
        }
        PipelineCommands::Run { name, stages } => {
            let pipelines = discover_pipeline_datums(b00t_path)?;
            let (_, datum, file_path) = pipelines
                .into_iter()
                .find(|(key, _, _)| key == name)
                .ok_or_else(|| anyhow::anyhow!("no pipeline datum named '{}'", name))?;
            let base_dir = Path::new(&file_path)
                .parent()
                .unwrap_or_else(|| Path::new("."))
                .to_path_buf();
            let pipeline = PipelineDatum::from_datum(datum, &base_dir)?;
            let out = pipeline.execute(stages)?;
            println!("{}", out.value);
        }
        PipelineCommands::Logs { args } => {
            handle_pipeline_logs(&*PIPELINE_LOG_STORE, args)?;
        }
        PipelineCommands::Validate { name } => {
            let report = validate_pipeline(name, b00t_path)?;
            print_validate_report(&report);
            if !report.summary.passed {
                std::process::exit(1);
            }
        }
        PipelineCommands::Cost { args } => {
            handle_pipeline_cost(args)?;
        }
        PipelineCommands::Data { id, stage, columns } => {
            handle_pipeline_data(b00t_path, id, stage.as_deref(), columns.as_deref())?;
        }
        PipelineCommands::Schedule { name, strategy } => {
            let output = handle_schedule_command(name, strategy, b00t_path)?;
            println!("{}", output);
        }
    }
    Ok(())
}
