//! `b00t focus` — query persisted FOCUS records from ledgrrr-mcp JSONL files.
//!
//! FOCUS records are written to `focus_records.jsonl` by the ledgrrr-mcp server.
//! This command reads them back via `FocusJsonlSequence`, which yields
//! `AbDataFrame`s conforming to `FocusSchema`.
//!
//! # Usage
//! ```bash
//! b00t-cli focus query                          # default: ./focus_records.jsonl
//! b00t-cli focus query --path /tmp/records.jsonl
//! b00t-cli focus query --experiment exp-42
//! b00t-cli focus query --json                   # emit JSON lines
//! b00t-cli focus query --t00n                   # emit t00n format (same JSONL)
//! ```

use crate::datum_schema::{AbDataFrame, CellValue, FocusJsonlSequence};
use anyhow::{anyhow, Result};
use clap::Parser;
use std::collections::HashMap;
use std::io::Write;
use std::path::PathBuf;

#[derive(Parser, Clone)]
pub enum FocusCommands {
    #[clap(about = "Query FOCUS records from focus_records.jsonl")]
    Query {
        #[clap(long, help = "Path to FOCUS records JSONL file", default_value = "focus_records.jsonl")]
        path: PathBuf,
        #[clap(long, help = "Filter by experiment ID")]
        experiment: Option<String>,
        #[clap(long, help = "Emit as JSON")]
        json: bool,
        #[clap(long, help = "Emit as t00n format")]
        t00n: bool,
    },
    #[clap(about = "List past experiments from FOCUS records")]
    History {
        #[clap(long, help = "Number of recent experiments", default_value_t = 10)]
        limit: usize,
        #[clap(long, help = "Emit as JSON")]
        json: bool,
    },
    #[clap(about = "Aggregate FOCUS records by dimension")]
    Aggregate {
        #[clap(long, help = "Path to FOCUS records JSONL file", default_value = "focus_records.jsonl")]
        path: PathBuf,
        #[clap(long, help = "Group by dimension (e.g. ServiceName)")]
        group_by: Option<String>,
        #[clap(long, help = "Metric to aggregate (default: BilledCost)")]
        metric: Option<String>,
    },
    #[clap(about = "Export FOCUS records in various formats")]
    Export {
        #[clap(long, help = "Path to FOCUS records JSONL file", default_value = "focus_records.jsonl")]
        path: PathBuf,
        #[clap(long, help = "Output format: json, t00n, csv")]
        format: String,
        #[clap(long, help = "Output file (default: stdout)")]
        output: Option<PathBuf>,
    },
    #[clap(about = "Follow focus_records.jsonl like tail -f")]
    Tail {
        #[clap(long, help = "Path to FOCUS records JSONL file", default_value = "focus_records.jsonl")]
        path: PathBuf,
        #[clap(long, help = "Show last N records first", default_value_t = 5)]
        lines: usize,
    },
    #[clap(about = "Merge two FOCUS JSONL files, dedup by experiment_id")]
    Merge {
        #[clap(help = "Primary JSONL file")]
        primary: PathBuf,
        #[clap(help = "Secondary JSONL file to merge in")]
        secondary: PathBuf,
        #[clap(long, help = "Output file (default: stdout)")]
        output: Option<PathBuf>,
    },
}

// ─── Helpers ──────────────────────────────────────────────────────────────────

/// Convert a single-row AbDataFrame into a JSON object.
fn frame_to_json(frame: &AbDataFrame) -> serde_json::Value {
    let mut obj = serde_json::Map::new();
    if let Some(row) = frame.rows.first() {
        for (i, hdr) in frame.headers.iter().enumerate() {
            let val = row.get(i).map(|cell| match cell {
                CellValue::String(s) => serde_json::Value::String(s.clone()),
                CellValue::Float64(f) => serde_json::Number::from_f64(*f)
                    .map(serde_json::Value::Number)
                    .unwrap_or(serde_json::Value::Null),
                CellValue::Int64(n) => serde_json::Value::Number((*n).into()),
                CellValue::Bool(b) => serde_json::Value::Bool(*b),
                CellValue::Null => serde_json::Value::Null,
            }).unwrap_or(serde_json::Value::Null);
            obj.insert(hdr.name.clone(), val);
        }
    }
    serde_json::Value::Object(obj)
}

/// Extract an f64 cost value from a frame cell, handling both Float64 and String repr.
fn cost_value(frame: &AbDataFrame, header: &str) -> Option<f64> {
    match frame.cell(0, header)? {
        CellValue::Float64(v) => Some(*v),
        CellValue::String(s) => s.parse::<f64>().ok(),
        _ => None,
    }
}

// ─── Handler ──────────────────────────────────────────────────────────────────

pub fn handle_focus_command(args: &FocusCommands) -> Result<()> {
    match args {
        FocusCommands::Query { path, experiment, json, t00n } => {
            let path_str = path.to_string_lossy().to_string();

            let mut seq = FocusJsonlSequence::open(&path_str)
                .map_err(|e| anyhow::anyhow!("failed to open '{}': {}", path.display(), e))?;

            let mut frames: Vec<AbDataFrame> = Vec::new();
            let mut total_billed = 0.0_f64;
            let mut total_effective = 0.0_f64;

            for result in &mut seq {
                let frame = result.map_err(|e| anyhow::anyhow!("read FOCUS record: {}", e.0))?;

                // Apply experiment filter
                if let Some(exp_id) = experiment {
                    let matches = match frame.cell(0, "x_ExperimentId") {
                        Some(CellValue::String(v)) => v.as_str() == exp_id.as_str(),
                        _ => false,
                    };
                    if !matches {
                        continue;
                    }
                }

                // Accumulate costs
                if let Some(v) = cost_value(&frame, "BilledCost") {
                    total_billed += v;
                }
                if let Some(v) = cost_value(&frame, "EffectiveCost") {
                    total_effective += v;
                }

                frames.push(frame);
            }

            // ── Summary ────────────────────────────────────────────────────────
            println!("FOCUS records: {}", path.display());
            println!("  Frames: {}", frames.len());
            println!("  Total billed: ${:.2}", total_billed);
            println!("  Total effective: ${:.2}", total_effective);
            if let Some(exp_id) = experiment {
                println!("  (filtered by experiment: {})", exp_id);
            }
            println!();

            // ── Record output ─────────────────────────────────────────────────
            if *json || *t00n {
                for frame in &frames {
                    println!("{}", serde_json::to_string(&frame_to_json(frame))?);
                }
            } else {
                for (i, frame) in frames.iter().enumerate() {
                    let billed = cost_value(frame, "BilledCost")
                        .map(|v| format!("${:.2}", v))
                        .unwrap_or_else(|| "N/A".to_string());
                    let effective = cost_value(frame, "EffectiveCost")
                        .map(|v| format!("${:.2}", v))
                        .unwrap_or_else(|| "N/A".to_string());
                    let service = match frame.cell(0, "ServiceName") {
                        Some(CellValue::String(v)) => v.clone(),
                        _ => "?".to_string(),
                    };
                    let exp = match frame.cell(0, "x_ExperimentId") {
                        Some(CellValue::String(v)) => format!(" [exp: {}]", v),
                        _ => String::new(),
                    };
                    println!("  [{:3}] {} | billed: {} | effective: {}{}",
                        i + 1, service, billed, effective, exp);
                }
            }

            Ok(())
        }
        FocusCommands::History { limit, json } => {
            let path = PathBuf::from("focus_records.jsonl");
            let path_str = path.to_string_lossy().to_string();

            let mut seq = FocusJsonlSequence::open(&path_str)
                .map_err(|e| anyhow!("failed to open '{}': {}", path.display(), e))?;

            // Collect unique experiment IDs with frame counts
            let mut experiment_counts: HashMap<String, usize> = HashMap::new();
            let mut total_frames = 0usize;

            for result in &mut seq {
                let frame = result.map_err(|e| anyhow!("read FOCUS record: {}", e.0))?;
                total_frames += 1;
                if let Some(CellValue::String(exp_id)) = frame.cell(0, "x_ExperimentId") {
                    *experiment_counts.entry(exp_id.clone()).or_insert(0) += 1;
                }
            }

            // Sort by experiment ID for deterministic output, apply limit
            let mut sorted: Vec<(String, usize)> = experiment_counts.into_iter().collect();
            sorted.sort_by(|a, b| a.0.cmp(&b.0));
            let len = sorted.len();
            let shown: Vec<_> = sorted.into_iter().skip(len.saturating_sub(*limit)).collect();

            if *json {
                let output: Vec<serde_json::Value> = shown
                    .iter()
                    .map(|(id, count)| {
                        serde_json::json!({
                            "experiment_id": id,
                            "frame_count": count,
                        })
                    })
                    .collect();
                println!("{}", serde_json::to_string_pretty(&output)?);
            } else {
                println!("Past experiments (from {}):", path.display());
                println!("  Total FOCUS frames: {}", total_frames);
                println!();
                for (id, count) in &shown {
                    println!("  {:<48} {} frames", id, count);
                }
            }

            Ok(())
        }
        FocusCommands::Aggregate { path, group_by, metric } => {
            let path_str = path.to_string_lossy().to_string();
            let metric = metric.as_deref().unwrap_or("BilledCost");

            let mut seq = FocusJsonlSequence::open(&path_str)
                .map_err(|e| anyhow!("failed to open '{}': {}", path.display(), e))?;

            let mut total_count = 0usize;
            let mut total_sum = 0.0_f64;
            let mut groups: HashMap<String, (usize, f64)> = HashMap::new();

            for result in &mut seq {
                let frame = result.map_err(|e| anyhow!("read FOCUS record: {}", e.0))?;
                total_count += 1;
                if let Some(v) = cost_value(&frame, metric) {
                    total_sum += v;
                }
                let key = if let Some(gb) = group_by {
                    match frame.cell(0, gb) {
                        Some(CellValue::String(v)) => v.clone(),
                        _ => "unknown".to_string(),
                    }
                } else {
                    "*".to_string()
                };
                let entry = groups.entry(key).or_insert((0, 0.0));
                entry.0 += 1;
                if let Some(v) = cost_value(&frame, metric) {
                    entry.1 += v;
                }
            }

            println!("Aggregate (metric: {}):", metric);
            println!("  Total records: {}", total_count);
            println!("  Total sum:     ${:.2}", total_sum);
            if total_count > 0 {
                println!("  Average:       ${:.2}", total_sum / total_count as f64);
            }
            println!();

            if groups.len() > 1 || group_by.is_some() {
                println!("  By {}:", group_by.as_deref().unwrap_or("(all)"));
                let mut sorted: Vec<_> = groups.into_iter().collect();
                sorted.sort_by(|a, b| b.1.1.partial_cmp(&a.1.1).unwrap_or(std::cmp::Ordering::Less));
                for (key, (count, sum)) in &sorted {
                    let avg = if *count > 0 { sum / *count as f64 } else { 0.0 };
                    println!("  {:<32} {:4} records  ${:>8.2} avg ${:>6.2}", key, count, sum, avg);
                }
            }

            Ok(())
        }
        FocusCommands::Export { path, format, output } => {
            let path_str = path.to_string_lossy().to_string();

            let mut seq = FocusJsonlSequence::open(&path_str)
                .map_err(|e| anyhow!("failed to open '{}': {}", path.display(), e))?;

            let mut frames: Vec<AbDataFrame> = Vec::new();
            for result in &mut seq {
                let frame = result.map_err(|e| anyhow!("read FOCUS record: {}", e.0))?;
                frames.push(frame);
            }

            let export_data: String = match format.as_str() {
                "json" | "t00n" => frames
                    .iter()
                    .filter_map(|f| serde_json::to_string(&frame_to_json(f)).ok())
                    .collect::<Vec<_>>()
                    .join("\n")
                    + "\n",
                "csv" => {
                    let mut csv = String::new();
                    if let Some(first) = frames.first() {
                        // CSV header
                        csv.push_str(&first.headers.iter().map(|h| h.name.as_str()).collect::<Vec<_>>().join(","));
                        csv.push('\n');
                        // CSV rows
                        for frame in &frames {
                            if let Some(row) = frame.rows.first() {
                                let vals: Vec<String> = row.iter().map(|cell| match cell {
                                    CellValue::String(s) => format!("\"{}\"", s),
                                    CellValue::Float64(f) => format!("{:.2}", f),
                                    CellValue::Int64(n) => n.to_string(),
                                    CellValue::Bool(b) => b.to_string(),
                                    CellValue::Null => String::new(),
                                }).collect();
                                csv.push_str(&vals.join(","));
                                csv.push('\n');
                            }
                        }
                    }
                    csv
                }
                other => return Err(anyhow!("unsupported format '{other}'. Use: json, t00n, csv")),
            };

            match output {
                Some(out_path) => std::fs::write(out_path, &export_data)?,
                None => print!("{}", export_data),
            }

            Ok(())
        }
        FocusCommands::Tail { path, lines } => {
            let path_str = path.to_string_lossy().to_string();

            // Read entire file, show last N lines
            let content = std::fs::read_to_string(&path_str)
                .map_err(|e| anyhow!("failed to read '{}': {}", path.display(), e))?;

            let all_lines: Vec<&str> = content.lines().collect();
            let start = if all_lines.len() > *lines {
                all_lines.len() - *lines
            } else {
                0
            };

            for line in &all_lines[start..] {
                println!("{}", line);
            }

            let mut last_len = content.len();

            // Poll loop — like tail -f
            loop {
                std::thread::sleep(std::time::Duration::from_secs(1));
                match std::fs::read_to_string(&path_str) {
                    Ok(new_content) => {
                        let new_len = new_content.len();
                        if new_len > last_len {
                            print!("{}", &new_content[last_len..]);
                            std::io::stdout().flush().ok();
                            last_len = new_len;
                        }
                    }
                    Err(e) => {
                        eprintln!("Error reading '{}': {}", path.display(), e);
                        break Ok(());
                    }
                }
            }
        }
        FocusCommands::Merge {
            primary,
            secondary,
            output,
        } => {
            let primary_str = primary.to_string_lossy().to_string();
            let secondary_str = secondary.to_string_lossy().to_string();

            let mut primary_seq = FocusJsonlSequence::open(&primary_str)
                .map_err(|e| anyhow!("failed to open primary '{}': {}", primary.display(), e))?;
            let mut secondary_seq = FocusJsonlSequence::open(&secondary_str)
                .map_err(|e| anyhow!("failed to open secondary '{}': {}", secondary.display(), e))?;

            // Collect primary records keyed by x_ExperimentId
            let mut merged: Vec<serde_json::Value> = Vec::new();
            let mut seen: std::collections::HashSet<String> = std::collections::HashSet::new();

            for result in &mut primary_seq {
                let frame = result.map_err(|e| anyhow!("read primary record: {}", e.0))?;
                let exp_id = match frame.cell(0, "x_ExperimentId") {
                    Some(CellValue::String(v)) => v.clone(),
                    _ => format!("row-{}", merged.len()),
                };
                seen.insert(exp_id.clone());
                merged.push(frame_to_json(&frame));
            }

            // Collect secondary records, skipping duplicates by experiment_id
            for result in &mut secondary_seq {
                let frame = result.map_err(|e| anyhow!("read secondary record: {}", e.0))?;
                let exp_id = match frame.cell(0, "x_ExperimentId") {
                    Some(CellValue::String(v)) => v.clone(),
                    _ => format!("row-{}", merged.len()),
                };
                if seen.contains(&exp_id) {
                    continue; // primary takes priority
                }
                seen.insert(exp_id);
                merged.push(frame_to_json(&frame));
            }

            let output_str = merged
                .iter()
                .filter_map(|v| serde_json::to_string(v).ok())
                .collect::<Vec<_>>()
                .join("\n")
                + "\n";

            match output {
                Some(out_path) => std::fs::write(out_path, &output_str)?,
                None => print!("{}", output_str),
            }

            eprintln!(
                "📦 Merged {} records (primary: {}, merged: {})",
                merged.len(),
                primary.display(),
                secondary.display()
            );

            Ok(())
        }
    }
}
