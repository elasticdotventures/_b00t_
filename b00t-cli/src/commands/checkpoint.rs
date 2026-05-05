//! Checkpoint-gate commands for subagent work
//!
//! Write/resume/clean checkpoint artifacts for subagent work.
//! Checkpoints live in `.hermes/.checkpoint-<hash>.json`
//!
//! Subcommands:
//! - create: Create a new checkpoint
//! - status: List active checkpoints
//! - prune: Prune stale checkpoints
//! - clean: Remove a completed checkpoint

use anyhow::{Context, Result};
use clap::Parser;
use serde::{Deserialize, Serialize};
use std::fs;
use std::io::BufReader;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

/// The .hermes directory name
const HERMES_DIR: &str = ".hermes";
/// Checkpoint file prefix
const CHECKPOINT_PREFIX: &str = ".checkpoint";

/// Checkpoint data structure
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct CheckpointData {
    pub task: String,
    #[serde(default)]
    pub files: Vec<String>,
    pub intent: String,
    pub started: String,
}

#[derive(Parser, Clone)]
pub enum CheckpointGateCommands {
    /// Create a new checkpoint for a subagent task
    #[clap(about = "Create a new checkpoint for a subagent task")]
    Create {
        /// Task/goal hash
        #[clap(help = "Task/goal hash")]
        goal_hash: String,
        /// Intent description
        #[clap(help = "Intent/description of the task")]
        intent: String,
        /// Files associated with this checkpoint
        #[clap(help = "Files associated with this checkpoint", num_args = 0..)]
        files: Vec<String>,
    },
    /// List active checkpoints
    #[clap(about = "List active checkpoints")]
    Status,
    /// Prune stale checkpoints older than a duration
    #[clap(about = "Prune stale checkpoints")]
    Prune {
        /// Max age (e.g., 24h, 60m, 3600s)
        #[clap(long = "older-than", help = "Max age (e.g., 24h, 60m, 3600s)")]
        older_than: String,
    },
    /// Clean up a completed checkpoint
    #[clap(about = "Clean up a completed checkpoint")]
    Done {
        /// Task/goal hash to clean
        #[clap(help = "Task/goal hash to clean")]
        goal_hash: String,
    },
    /// Resume checkpoint data for a task hash
    #[clap(about = "Print checkpoint data for resumption")]
    Resume {
        /// Task/goal hash to resume
        #[clap(help = "Task/goal hash to resume")]
        goal_hash: String,
    },
}

/// Resolve the workspace root from the current directory or env
fn workspace_root() -> Result<PathBuf> {
    // Try env var first
    if let Ok(root) = std::env::var("WORKSPACE_PATH") {
        return Ok(PathBuf::from(root));
    }
    // Try current dir
    let cwd = std::env::current_dir().context("Failed to get current directory")?;
    Ok(cwd)
}

/// Get the .hermes directory path
fn hermes_dir() -> Result<PathBuf> {
    let root = workspace_root()?;
    Ok(root.join(HERMES_DIR))
}

/// Get checkpoint file path for a goal hash
fn checkpoint_path(goal_hash: &str) -> Result<PathBuf> {
    let dir = hermes_dir()?;
    Ok(dir.join(format!("{}-{}.json", CHECKPOINT_PREFIX, goal_hash)))
}

/// Parse a duration string like "24h", "60m", "3600s" into seconds
fn parse_duration(duration: &str) -> Result<u64> {
    if let Some(val) = duration.strip_suffix('h') {
        let hours: u64 = val.parse().context("Invalid duration: expected number before 'h'")?;
        Ok(hours * 3600)
    } else if let Some(val) = duration.strip_suffix('m') {
        let mins: u64 = val.parse().context("Invalid duration: expected number before 'm'")?;
        Ok(mins * 60)
    } else if let Some(val) = duration.strip_suffix('s') {
        let secs: u64 = val.parse().context("Invalid duration: expected number before 's'")?;
        Ok(secs)
    } else {
        // Try parsing as raw seconds
        let secs: u64 = duration.parse().context("Invalid duration format. Use e.g. 24h, 60m, 3600s")?;
        Ok(secs)
    }
}

/// Parse an ISO-8601 timestamp to epoch seconds
fn iso_to_epoch(iso: &str) -> Result<u64> {
    // Simple ISO-8601 parser: handles "2025-01-01T00:00:00+00:00" and "2025-01-01T00:00:00Z"
    // Use `date` command as fallback, or basic parsing
    let cleaned = iso
        .replace('Z', "+00:00")
        .trim_end_matches('Z')
        .to_string();

    // Try basic parsing: YYYY-MM-DDTHH:MM:SS
    if cleaned.len() >= 19 {
        let year: i32 = cleaned[0..4].parse().unwrap_or(0);
        let month: u32 = cleaned[5..7].parse().unwrap_or(1);
        let day: u32 = cleaned[8..10].parse().unwrap_or(1);
        let hour: u32 = cleaned[11..13].parse().unwrap_or(0);
        let min: u32 = cleaned[14..16].parse().unwrap_or(0);
        let sec: u32 = cleaned[17..19].parse().unwrap_or(0);

        use chrono::NaiveDate;
        if let Some(d) = NaiveDate::from_ymd_opt(year, month, day) {
            if let Some(dt) = d.and_hms_opt(hour, min, sec) {
                return Ok(dt.and_utc().timestamp() as u64);
            }
        }
    }

    // Fallback: try `date` command
    let output = std::process::Command::new("date")
        .arg("-d")
        .arg(iso)
        .arg("+%s")
        .output()
        .map_err(|_| anyhow::anyhow!("Cannot parse timestamp: {}", iso))?;

    let epoch_str = String::from_utf8_lossy(&output.stdout).trim().to_string();
    let epoch: u64 = epoch_str.parse().context("Failed to parse date output")?;
    Ok(epoch)
}

/// Get current unix epoch
fn now_epoch() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

fn cmd_create(goal_hash: &str, intent: &str, files: &[String]) -> Result<()> {
    let cpath = checkpoint_path(goal_hash)?;

    // If checkpoint already exists, resume
    if cpath.exists() {
        let content = fs::read_to_string(&cpath)
            .context("Failed to read existing checkpoint")?;
        let data: CheckpointData = serde_json::from_str(&content)
            .context("Failed to parse existing checkpoint")?;
        println!("RESUME: checkpoint already exists for hash={}", goal_hash);
        println!("  Previous intent: {}", data.intent);
        println!("  File: {}", cpath.display());
        return Ok(());
    }

    // Ensure .hermes dir exists
    if let Some(parent) = cpath.parent() {
        fs::create_dir_all(parent).context("Failed to create .hermes directory")?;
    }

    let started = chrono::Utc::now().to_rfc3339();

    let data = CheckpointData {
        task: goal_hash.to_string(),
        files: files.to_vec(),
        intent: intent.to_string(),
        started,
    };

    let json = serde_json::to_string_pretty(&data)
        .context("Failed to serialize checkpoint")?;
    fs::write(&cpath, json).context("Failed to write checkpoint file")?;

    println!("CHECKPOINT CREATED: hash={} intent={}", goal_hash, intent);
    println!("  File: {}", cpath.display());

    Ok(())
}

fn cmd_status() -> Result<()> {
    let dir = hermes_dir()?;

    if !dir.exists() {
        println!("No checkpoints ({} does not exist)", dir.display());
        return Ok(());
    }

    let pattern = format!("{}-*.json", CHECKPOINT_PREFIX);
    let entries: Vec<_> = fs::read_dir(&dir)
        .context("Failed to read .hermes directory")?
        .filter_map(|e| e.ok())
        .filter(|e| {
            e.file_name()
                .to_str()
                .map(|n| n.starts_with(CHECKPOINT_PREFIX) && n.ends_with(".json"))
                .unwrap_or(false)
        })
        .collect();

    if entries.is_empty() {
        println!("No active checkpoints in {}", dir.display());
        return Ok(());
    }

    println!("Active checkpoints ({}):", entries.len());

    let now = now_epoch();

    for entry in &entries {
        let fname = entry.file_name();
        let name = fname.to_string_lossy();
        // Extract hash from filename: .checkpoint-<hash>.json
        let hash = name
            .strip_prefix(CHECKPOINT_PREFIX)
            .and_then(|s| s.strip_suffix(".json"))
            .unwrap_or("unknown");

        // Read metadata
        let path = entry.path();
        match fs::File::open(&path) {
            Ok(file) => {
                let reader = BufReader::new(file);
                if let Ok(data) = serde_json::from_reader::<_, CheckpointData>(reader) {
                    let age = iso_to_epoch(&data.started)
                        .map(|started_epoch| {
                            if now > started_epoch {
                                now - started_epoch
                            } else {
                                0
                            }
                        })
                        .unwrap_or(0);
                    println!(
                        "  {:16}  started={}  age={}s  intent={}",
                        hash, data.started, age, data.intent
                    );
                } else {
                    println!("  {:16}  (unparseable)", hash);
                }
            }
            Err(e) => {
                println!("  {:16}  (error: {})", hash, e);
            }
        }
    }

    Ok(())
}

fn cmd_prune(older_than: &str) -> Result<()> {
    let max_age_sec = parse_duration(older_than)?;
    let dir = hermes_dir()?;

    if !dir.exists() {
        println!("No checkpoints to prune");
        return Ok(());
    }

    let now = now_epoch();
    let mut pruned = 0u32;

    let pattern = format!("{}-*.json", CHECKPOINT_PREFIX);
    let dir_entries = fs::read_dir(&dir)
        .context("Failed to read .hermes directory")?;

    for entry in dir_entries {
        let entry = entry?;
        let fname = entry.file_name();
        let name = fname.to_string_lossy();
        if !name.starts_with(CHECKPOINT_PREFIX) || !name.ends_with(".json") {
            continue;
        }

        let path = entry.path();
        match fs::File::open(&path) {
            Ok(file) => {
                let reader = BufReader::new(file);
                if let Ok(data) = serde_json::from_reader::<_, CheckpointData>(reader) {
                    if let Ok(started_epoch) = iso_to_epoch(&data.started) {
                        if now > started_epoch {
                            let age = now - started_epoch;
                            if age > max_age_sec {
                                let hash = name
                                    .strip_prefix(CHECKPOINT_PREFIX)
                                    .and_then(|s| s.strip_suffix(".json"))
                                    .unwrap_or("unknown");
                                fs::remove_file(&path)
                                    .with_context(|| format!("Failed to remove {}", path.display()))?;
                                println!("PRUNED: {} (age={}s > {}s)", hash, age, max_age_sec);
                                pruned += 1;
                            }
                        }
                    }
                }
            }
            Err(_) => continue,
        }
    }

    println!("Pruned {} checkpoint(s)", pruned);
    Ok(())
}

fn cmd_done(goal_hash: &str) -> Result<()> {
    let cpath = checkpoint_path(goal_hash)?;

    if !cpath.exists() {
        println!("No checkpoint found for hash={}", goal_hash);
        return Ok(());
    }

    fs::remove_file(&cpath)
        .with_context(|| format!("Failed to remove checkpoint {}", cpath.display()))?;
    println!("CHECKPOINT CLEANED: hash={}", goal_hash);
    Ok(())
}

fn cmd_resume(goal_hash: &str) -> Result<()> {
    let cpath = checkpoint_path(goal_hash)?;

    if !cpath.exists() {
        eprintln!("No checkpoint found for hash={}", goal_hash);
        return Err(anyhow::anyhow!("No checkpoint found for hash={}", goal_hash));
    }

    let content = fs::read_to_string(&cpath)
        .context("Failed to read checkpoint")?;
    println!("RESUMING checkpoint:");
    println!("{}", content);
    Ok(())
}

/// Handle checkpoint-gate commands
pub fn handle_checkpoint_gate_command(command: CheckpointGateCommands) -> Result<()> {
    match command {
        CheckpointGateCommands::Create {
            goal_hash,
            intent,
            files,
        } => cmd_create(&goal_hash, &intent, &files),
        CheckpointGateCommands::Status => cmd_status(),
        CheckpointGateCommands::Prune { older_than } => cmd_prune(&older_than),
        CheckpointGateCommands::Done { goal_hash } => cmd_done(&goal_hash),
        CheckpointGateCommands::Resume { goal_hash } => cmd_resume(&goal_hash),
    }
}
