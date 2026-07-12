// 🤓 b00t stage — stage registry commands (list, search, info)
//    Mirrors the PipelineCommands pattern in pipeline.rs.

use crate::ansi;
use crate::pipeline_types::{PortDirection, PortMediaType};
use crate::stage_registry::StageRegistry;
use anyhow::Result;
use clap::Subcommand;

#[derive(Debug, Subcommand)]
pub enum StageCommands {
    #[clap(about = "List all available stages with optional media-type filter")]
    List {
        #[clap(
            long = "filter",
            help = "Filter by port media type (Video, Audio, Image, Json, Parquet, Bytes)"
        )]
        filter: Option<String>,
    },
    #[clap(about = "Search stages by name or port media type (case-insensitive substring)")]
    Search {
        #[clap(help = "Search query")]
        query: String,
    },
    #[clap(about = "Show detailed information for a specific stage by name")]
    Info {
        #[clap(help = "Stage name (exact match)")]
        name: String,
    },
}

pub fn handle_stage_command(cmd: &StageCommands, b00t_path: &str) -> Result<()> {
    let registry = StageRegistry::discover(b00t_path);

    match cmd {
        StageCommands::List { filter } => {
            let stages = registry.list(filter.as_deref());
            if stages.is_empty() {
                let msg = match filter {
                    Some(f) => format!("No stages found for media type '{}'.", f),
                    None => "No stages found. Create _b00t_/*.stage.tomllm files.".to_string(),
                };
                println!("{}", ansi::yellow(&msg));
                return Ok(());
            }

            println!(
                "{}",
                ansi::bold(&format!("Stages ({}):", stages.len()))
            );
            println!();
            // Table: Name │ Ports │ GPU │ RAM
            println!(
                "{}",
                ansi::dim("  Name                    Ports          GPU    RAM     Image")
            );
            println!(
                "{}",
                ansi::dim("  ─────────────────────── ────────────── ────── ─────── ──────────────────")
            );
            for s in &stages {
                let ports_desc = if s.ports.is_empty() {
                    "—".to_string()
                } else {
                    s.ports
                        .iter()
                        .map(|p| {
                            let dir = match p.direction {
                                PortDirection::Input => "←",
                                PortDirection::Output => "→",
                            };
                            format!("{}{:?}", dir, p.media_type)
                        })
                        .collect::<Vec<_>>()
                        .join(", ")
                };
                let gpu_flag = if s.resources.requires_gpu {
                    ansi::green(" YES ")
                } else {
                    ansi::dim("  —  ")
                };
                let ram_str = format!("{:.1}GB", s.resources.min_ram_gb);
                let image_str = s.image.as_deref().unwrap_or("—");
                println!(
                    "  {:25} {:16} {} {:>5}  {}",
                    s.name,
                    ports_desc,
                    gpu_flag,
                    ram_str,
                    image_str,
                );
            }
        }
        StageCommands::Search { query } => {
            let results = registry.search(query);
            if results.is_empty() {
                println!(
                    "{}",
                    ansi::yellow(&format!("No stages matching '{}'.", query))
                );
                return Ok(());
            }

            println!(
                "{}",
                ansi::bold(&format!(
                    "Found {} stage(s) matching '{}':",
                    results.len(),
                    query
                ))
            );
            println!();
            for s in &results {
                let port_count = s.ports.len();
                let gpu = if s.resources.requires_gpu { "GPU" } else { "CPU" };
                println!(
                    "  {}  ({} port{}, {}, {:.1}GB RAM){}",
                    ansi::cyan(&s.name),
                    port_count,
                    if port_count == 1 { "" } else { "s" },
                    gpu,
                    s.resources.min_ram_gb,
                    s.image
                        .as_ref()
                        .map(|i| format!("  ── {}", i))
                        .unwrap_or_default()
                );
            }
        }
        StageCommands::Info { name } => {
            match registry.get(name) {
                None => {
                    println!(
                        "{}",
                        ansi::red(&format!("Stage '{}' not found.", name))
                    );
                    // Suggest similar names via case-insensitive search
                    let similar = registry.search(name);
                    if !similar.is_empty() {
                        println!(
                            "{}",
                            ansi::dim("Did you mean:")
                        );
                        for s in &similar {
                            println!("  {}", ansi::cyan(&s.name));
                        }
                    }
                }
                Some(profile) => {
                    println!("{}", ansi::bold(&format!("Stage: {}", profile.name)));
                    println!("{}", ansi::dim(&"─".repeat(48)));
                    println!("  Name:           {}", ansi::cyan(&profile.name));
                    println!("  Image:          {}", profile.image.as_deref().unwrap_or("—"));
                    println!(
                        "  Timeout:        {}",
                        profile
                            .timeout_seconds
                            .map(|t| format!("{}s", t))
                            .unwrap_or_else(|| "none".to_string())
                    );
                    println!();
                    println!("  Resources:");
                    println!(
                        "    RAM:          {:.1} GB",
                        profile.resources.min_ram_gb
                    );
                    println!(
                        "    VRAM:         {:.1} GB",
                        profile.resources.min_vram_gb
                    );
                    println!(
                        "    GPU required: {}",
                        if profile.resources.requires_gpu {
                            ansi::green("yes")
                        } else {
                            "no".to_string()
                        }
                    );
                    if let Some(cores) = profile.resources.cpu_cores {
                        println!("    CPU cores:    {}", cores);
                    }
                    if let Some(disk) = profile.resources.scratch_disk_gb {
                        println!("    Scratch disk: {:.1} GB", disk);
                    }
                    println!();
                    if profile.ports.is_empty() {
                        println!("  Ports: none");
                    } else {
                        println!(
                            "  Ports ({}):",
                            profile.ports.len()
                        );
                        for (i, port) in profile.ports.iter().enumerate() {
                            let dir_icon = match port.direction {
                                PortDirection::Input => "← IN ",
                                PortDirection::Output => "→ OUT",
                            };
                            let desc = port
                                .description
                                .as_deref()
                                .unwrap_or("");
                            println!(
                                "    {}. {}  {:?}  {}",
                                i + 1,
                                dir_icon,
                                port.media_type,
                                ansi::dim(desc),
                            );
                        }
                    }
                }
            }
        }
    }
    Ok(())
}
