// 🤓 b00t pipeline — typed, discoverable multi-stage pipeline datums (list, run)
//    First CLI dispatch surface for any CliExecutor implementor (CliDatum,
//    JustfileDatum, PipelineDatum previously had none). Shape mirrors
//    StoreCommands (commands/store.rs) — the closest proven multi-verb
//    datum-type subcommand pattern in this codebase.
use crate::datum_pipeline::PipelineDatum;
use crate::datum_utils::get_all_datums_with_paths;
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
}

fn discover_pipeline_datums(b00t_path: &str) -> Result<Vec<(String, BootDatum, String)>> {
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
    }
    Ok(())
}
