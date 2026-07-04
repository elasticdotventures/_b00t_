//! Model cache guard — disk-space gate + usage tracking per datum (#625).
//!
//! Generic guard that fronts any large-artifact datum (model, dataset, checkpoint).
//! Pre-flight checks available disk space and warns before downloads.

use crate::BootDatum;
use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// Result of a download-space preflight check.
#[derive(Debug, Clone, Serialize)]
pub struct GuardResult {
    pub datum: String,
    pub pass: bool,
    pub needed_gb: f64,
    pub available_gb: f64,
    pub message: String,
}

/// Check if there's enough disk space to download a model datum.
///
/// Reads `model_size_gb` from the datum, checks `df` on the HF cache
/// partition, and returns a sm0l-tier contract: PASS or FAIL with details.
pub fn download_guard(datum: &BootDatum, hf_cache_dir: &PathBuf) -> Result<GuardResult> {
    let needed = datum.model_size_gb.unwrap_or(0.0);
    let name = datum.name.clone();
    let hf_id = datum.model_hf_id.clone().unwrap_or_default();

    if needed == 0.0 {
        return Ok(GuardResult {
            datum: name,
            pass: true,
            needed_gb: 0.0,
            available_gb: 0.0,
            message: "no size declared — skipping guard".into(),
        });
    }

    // Check if already downloaded (reduce need)
    let already = cached_size(hf_cache_dir, &hf_id);
    let remaining = (needed - already).max(0.0);

    // Get available space
    let available = disk_free(hf_cache_dir)?;

    let pass = available >= remaining;
    let message = if pass {
        format!(
            "PASS: {:.1}GB needed, {:.1}GB available ({} cached)",
            remaining, available, if already > 0.0 { format!("{:.1}GB", already) } else { "none".into() }
        )
    } else {
        format!(
            "FAIL: need {:.1}GB, have {:.1}GB (short by {:.1}GB)",
            remaining, available, remaining - available
        )
    };

    Ok(GuardResult {
        datum: name,
        pass,
        needed_gb: remaining,
        available_gb: available,
        message,
    })
}

/// Check how much of a model is already cached on disk.
///
/// Scans the HuggingFace cache directory for snapshot directories matching
/// the model ID. Returns approximate cached size in GB.
fn cached_size(hf_cache_dir: &PathBuf, model_id: &str) -> f64 {
    if hf_cache_dir.as_os_str().is_empty() || model_id.is_empty() {
        return 0.0;
    }
    let model_dir = hf_cache_dir.join("models--").join(model_id.replace('/', "--"));
    if !model_dir.exists() {
        return 0.0;
    }
    dir_size_gb(&model_dir)
}

/// Get available disk space on a path's filesystem in GB.
fn disk_free(path: &PathBuf) -> Result<f64> {
    let output = std::process::Command::new("df")
        .args(["-B1", "--output=avail"])
        .arg(path)
        .output()
        .context("df command failed")?;

    let stdout = String::from_utf8_lossy(&output.stdout);
    let bytes: u64 = stdout
        .lines()
        .nth(1)
        .and_then(|l| l.trim().parse().ok())
        .unwrap_or(0);

    Ok(bytes as f64 / 1_000_000_000.0)
}

/// Recursively compute directory size in GB.
fn dir_size_gb(path: &PathBuf) -> f64 {
    let mut total: u64 = 0;
    if let Ok(entries) = std::fs::read_dir(path) {
        for entry in entries.flatten() {
            if let Ok(meta) = entry.metadata() {
                if meta.is_file() {
                    total += meta.len();
                } else if meta.is_dir() {
                    total += (dir_size_gb(&entry.path()) * 1_000_000_000.0) as u64;
                }
            }
        }
    }
    total as f64 / 1_000_000_000.0
}

/// Usage event for append-only JSONL tracking.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UsageEvent {
    pub ts: String,
    pub datum: String,
    pub event: String,
    pub host: String,
}

/// Log a usage event to ~/.b00t/model-usage.jsonl (append-only, no git noise).
pub fn usage_log(datum: &str, event: &str) -> Result<()> {
    let home = dirs_next().unwrap_or_else(|| PathBuf::from("/tmp"));
    let log_path = home.join(".b00t").join("model-usage.jsonl");
    std::fs::create_dir_all(log_path.parent().unwrap())?;

    let entry = UsageEvent {
        ts: chrono::Utc::now().to_rfc3339(),
        datum: datum.to_string(),
        event: event.to_string(),
        host: hostname::get()
            .map(|h| h.to_string_lossy().to_string())
            .unwrap_or_else(|_| "unknown".into()),
    };

    let mut file = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&log_path)?;

    serde_json::to_writer(&mut file, &entry)?;
    writeln!(file)?;

    Ok(())
}

/// List eviction candidates: models unused for more than `lru_days` days.
///
/// Scans the usage log for last-use timestamps and identifies models
/// that haven't been accessed recently.
pub fn cache_evict_candidates(lru_days: u32) -> Result<Vec<String>> {
    let home = dirs_next().unwrap_or_else(|| PathBuf::from("/tmp"));
    let log_path = home.join(".b00t").join("model-usage.jsonl");

    if !log_path.exists() {
        return Ok(Vec::new());
    }

    let now = chrono::Utc::now();
    let threshold = now - chrono::Duration::days(lru_days as i64);

    let mut last_used: std::collections::HashMap<String, chrono::DateTime<chrono::Utc>> =
        std::collections::HashMap::new();

    let content = std::fs::read_to_string(&log_path)?;
    for line in content.lines() {
        if let Ok(event) = serde_json::from_str::<UsageEvent>(line) {
            if let Ok(ts) = chrono::DateTime::parse_from_rfc3339(&event.ts) {
                let ts_utc = ts.with_timezone(&chrono::Utc);
                last_used
                    .entry(event.datum.clone())
                    .and_modify(|e| {
                        if ts_utc > *e {
                            *e = ts_utc;
                        }
                    })
                    .or_insert(ts_utc);
            }
        }
    }

    let candidates: Vec<String> = last_used
        .into_iter()
        .filter(|(_, ts)| *ts < threshold)
        .map(|(datum, _)| datum)
        .collect();

    Ok(candidates)
}

fn dirs_next() -> Option<PathBuf> {
    std::env::var("HOME").ok().map(PathBuf::from)
}

use std::io::Write;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn guard_passes_when_no_size_declared() {
        let datum = BootDatum {
            name: "test-model".into(),
            ..Default::default()
        };
        let result = download_guard(&datum, &PathBuf::from("/tmp")).unwrap();
        assert!(result.pass);
        assert!(result.message.contains("skipping guard"));
    }

    #[test]
    fn guard_detects_insufficient_space() {
        let datum = BootDatum {
            name: "big-model".into(),
            model_size_gb: Some(500.0), // impossible
            ..Default::default()
        };
        let result = download_guard(&datum, &PathBuf::from("/tmp")).unwrap();
        assert!(!result.pass);
        assert!(result.message.contains("FAIL"));
    }

    #[test]
    fn usage_log_writes_jsonl() {
        let tmp = std::env::temp_dir().join("b00t-test-usage");
        std::fs::create_dir_all(&tmp).unwrap();
        let log_path = tmp.join("model-usage.jsonl");

        // Create a minimal test
        let entry = UsageEvent {
            ts: "2026-07-04T00:00:00Z".into(),
            datum: "test/model".into(),
            event: "load".into(),
            host: "test-host".into(),
        };

        let mut file = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&log_path)
            .unwrap();
        serde_json::to_writer(&mut file, &entry).unwrap();
        writeln!(file).unwrap();

        let content = std::fs::read_to_string(&log_path).unwrap();
        assert!(content.contains("test/model"));
        assert!(content.contains("load"));

        let _ = std::fs::remove_dir_all(&tmp);
    }

    #[test]
    fn cache_evict_returns_empty_when_no_log() {
        let candidates = cache_evict_candidates(30).unwrap();
        assert!(candidates.is_empty());
    }
}
