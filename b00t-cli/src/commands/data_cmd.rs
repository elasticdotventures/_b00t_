//! `b00t data` — inspect AbDataFrame JSONL files.
//!
//! Reads JSONL files via `FocusJsonlSequence` and displays
//! header metadata, row counts, and row samples.
//!
//! # Usage
//! ```bash
//! b00t-cli data inspect records.jsonl
//! b00t-cli data inspect records.jsonl --headers
//! b00t-cli data inspect records.jsonl --sample 10
//! ```

use crate::commands::fabric_cmd::{FabricCommands, handle_fabric_command};
use crate::datum_schema::{CellValue, FocusJsonlSequence};
use anyhow::{Result, anyhow};
use clap::Parser;
use std::path::PathBuf;

#[derive(Parser, Clone)]
pub enum DataCommands {
    /// Interact with the data fabric (triple store + graph + vector)
    #[clap(subcommand)]
    Fabric(FabricCommands),

    #[clap(about = "Inspect an AbDataFrame — list headers, count rows, sample")]
    Inspect {
        #[clap(help = "Path to JSONL file")]
        path: PathBuf,
        #[clap(long, help = "List headers only")]
        headers: bool,
        #[clap(long, help = "Sample N rows", default_value_t = 5)]
        sample: usize,
    },
}

pub fn handle_data_command(args: &DataCommands) -> Result<()> {
    match args {
        DataCommands::Fabric(fabric_args) => handle_fabric_command(fabric_args),
        DataCommands::Inspect {
            path,
            headers: show_headers,
            sample,
        } => {
            let path_str = path.to_string_lossy().to_string();

            let mut seq = FocusJsonlSequence::open(&path_str)
                .map_err(|e| anyhow!("failed to open '{}': {}", path.display(), e))?;

            // Collect all frames into rows for inspection
            let mut total_rows = 0usize;
            let mut frame_headers: Option<Vec<crate::datum_schema::AbDataHeader>> = None;
            let mut sample_rows: Vec<crate::datum_schema::AbDataFrame> = Vec::new();

            for result in &mut seq {
                let frame = result.map_err(|e| anyhow!("read error: {}", e.0))?;

                if frame_headers.is_none() {
                    frame_headers = Some(frame.headers.clone());
                }

                let count = frame.row_count();
                total_rows += count;

                // Collect sample rows
                if sample_rows.len() < *sample {
                    sample_rows.push(frame);
                }
            }

            let headers = frame_headers
                .as_ref()
                .ok_or_else(|| anyhow!("no records found in '{}'", path.display()))?;

            println!("📋 DataFrame Inspection: {}", path.display());
            println!("   Total rows: {}", total_rows);
            println!("   Headers:    {}", headers.len());
            println!();

            if *show_headers {
                println!("── Headers ──────────────────────────────────────────");
                for h in headers {
                    let null_flag = if h.nullable { "nullable" } else { "required" };
                    println!(
                        "   [{:3}] {:<30} {:8} ({})",
                        h.ordinal,
                        h.name,
                        format!("{:?}", h.data_type),
                        null_flag
                    );
                }
                println!();
                return Ok(());
            }

            println!("── Schema ────────────────────────────────────────────");
            for h in headers {
                let null_flag = if h.nullable { "nullable" } else { "required" };
                println!(
                    "   [{:3}] {:<30} {:8} ({})",
                    h.ordinal,
                    h.name,
                    format!("{:?}", h.data_type),
                    null_flag
                );
            }
            println!();

            println!(
                "── Sample ({} rows) ──────────────────────────────",
                (*sample).min(total_rows)
            );
            for (i, frame) in sample_rows.iter().enumerate() {
                if let Some(row) = frame.rows.first() {
                    println!("   Row {}:", i + 1);
                    for (j, hdr) in frame.headers.iter().enumerate() {
                        if j >= 5 {
                            // Show first 5 columns per row to avoid clutter
                            if j == 5 {
                                println!("     ... ({} more columns)", frame.headers.len() - 5);
                            }
                            continue;
                        }
                        let val_str = match row.get(j) {
                            Some(CellValue::String(s)) => s.clone(),
                            Some(CellValue::Float64(f)) => format!("{:.4}", f),
                            Some(CellValue::Int64(n)) => n.to_string(),
                            Some(CellValue::Bool(b)) => b.to_string(),
                            Some(CellValue::Null) => "null".to_string(),
                            None => "?".to_string(),
                        };
                        println!("     {}: {}", hdr.name, val_str);
                    }
                    println!();
                }
            }

            Ok(())
        }
    }
}
