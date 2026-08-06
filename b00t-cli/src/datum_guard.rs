//! Model cache guard — disk-space gate + usage tracking per datum (#625).
//!
//! Generic guard that fronts any large-artifact datum (model, dataset, checkpoint).
//! Pre-flight checks available disk space and warns before downloads.
//!
//! # Functions accept an optional `base_dir` for test isolation:
//! - `usage_log_with_base(datum, event, base_dir)` — append to base_dir/.b00t/model-usage.jsonl
//! - `cache_evict_candidates_with_base(lru_days, base_dir)` — read from base_dir/.b00t/model-usage.jsonl
//! - `download_guard_with_disk(datum, cache_dir, disk_free_fn)` — inject disk_free for tests

use crate::BootDatum;
use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::io::Write;
use std::path::{Path, PathBuf};

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
/// Reads `model_size_gb` (or `model_size_4bit_gb` if quantized) from the datum,
/// checks available disk space, and returns a sm0l-tier contract: PASS or FAIL with details.
pub fn download_guard(datum: &BootDatum, hf_cache_dir: &Path) -> Result<GuardResult> {
    download_guard_with_disk(datum, hf_cache_dir, disk_free)
}

/// Variant accepting a disk-free function for test injection.
pub fn download_guard_with_disk(
    datum: &BootDatum,
    hf_cache_dir: &Path,
    disk_free_fn: impl Fn(&Path) -> Result<f64>,
) -> Result<GuardResult> {
    let needed = datum
        .model_size_gb
        .or(datum.model_size_4bit_gb)
        .unwrap_or(0.0);
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

    let already = cached_size(hf_cache_dir, &hf_id);
    let remaining = (needed - already).max(0.0);
    let available = disk_free_fn(hf_cache_dir)?;

    let pass = available >= remaining;
    let message = if pass {
        format!(
            "PASS: {:.1}GB needed, {:.1}GB available ({} cached)",
            remaining,
            available,
            if already > 0.0 {
                format!("{:.1}GB", already)
            } else {
                "none".into()
            }
        )
    } else {
        format!(
            "FAIL: need {:.1}GB, have {:.1}GB (short by {:.1}GB)",
            remaining,
            available,
            remaining - available
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
/// Scans the HuggingFace cache directory for snapshot directories matching
/// the model ID. Returns approximate cached size in GB.
fn cached_size(hf_cache_dir: &Path, model_id: &str) -> f64 {
    if hf_cache_dir.as_os_str().is_empty() || model_id.is_empty() {
        return 0.0;
    }
    let model_dir = hf_cache_dir
        .join("models--")
        .join(model_id.replace('/', "--"));
    if !model_dir.exists() {
        return 0.0;
    }
    dir_size_bytes(&model_dir) as f64 / 1_000_000_000.0
}

/// Get available disk space on a path's filesystem in GB.
/// Uses GNU df (Linux). TODO: cross-platform via libc::statvfs.
fn disk_free(path: &Path) -> Result<f64> {
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

/// Recursively compute directory size in bytes.
fn dir_size_bytes(path: &Path) -> u64 {
    let mut total: u64 = 0;
    if let Ok(entries) = std::fs::read_dir(path) {
        for entry in entries.flatten() {
            if let Ok(meta) = entry.metadata() {
                if meta.is_file() {
                    total += meta.len();
                } else if meta.is_dir() {
                    total += dir_size_bytes(&entry.path());
                }
            }
        }
    }
    total
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
    let home = home_dir();
    usage_log_with_base(datum, event, &home)
}

/// Variant accepting a base directory for test isolation.
pub fn usage_log_with_base(datum: &str, event: &str, base_dir: &Path) -> Result<()> {
    let log_path = base_dir.join(".b00t").join("model-usage.jsonl");
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
pub fn cache_evict_candidates(lru_days: u32) -> Result<Vec<String>> {
    let home = home_dir();
    cache_evict_candidates_with_base(lru_days, &home)
}

/// Variant accepting a base directory for test isolation.
pub fn cache_evict_candidates_with_base(lru_days: u32, base_dir: &Path) -> Result<Vec<String>> {
    let log_path = base_dir.join(".b00t").join("model-usage.jsonl");

    if !log_path.exists() {
        return Ok(Vec::new());
    }

    let now = chrono::Utc::now();
    let threshold = now - chrono::Duration::days(lru_days as i64);

    let mut last_used: std::collections::BTreeMap<String, chrono::DateTime<chrono::Utc>> =
        std::collections::BTreeMap::new();

    // TODO: stream-read for large files (currently O(n) memory)
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

    let mut candidates: Vec<String> = last_used
        .into_iter()
        .filter(|(_, ts)| *ts < threshold)
        .map(|(datum, _)| datum)
        .collect();
    candidates.sort();

    Ok(candidates)
}

fn home_dir() -> PathBuf {
    std::env::var("HOME")
        .ok()
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("/tmp"))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_datum(name: &str, size_gb: f64) -> BootDatum {
        BootDatum {
            name: name.into(),
            model_size_gb: Some(size_gb),
            ..Default::default()
        }
    }

    #[test]
    fn guard_passes_when_no_size_declared() {
        let datum = BootDatum {
            name: "test".into(),
            ..Default::default()
        };
        let result = download_guard(&datum, Path::new("/tmp")).unwrap();
        assert!(result.pass);
        assert!(result.message.contains("skipping guard"));
    }

    #[test]
    fn guard_detects_insufficient_space() {
        let datum = test_datum("big-model", f64::MAX);
        let always_zero = |_: &Path| Ok(0.0);
        let result = download_guard_with_disk(&datum, Path::new("/tmp"), always_zero).unwrap();
        assert!(!result.pass);
        assert!(result.message.contains("FAIL"));
    }

    #[test]
    fn guard_passes_when_plenty_of_space() {
        let datum = BootDatum {
            name: "small".into(),
            model_size_4bit_gb: Some(5.0), // use 4bit variant
            ..Default::default()
        };
        let always_terabyte = |_: &Path| Ok(1000.0);
        let result = download_guard_with_disk(&datum, Path::new("/tmp"), always_terabyte).unwrap();
        assert!(result.pass);
        assert_eq!(result.needed_gb, 5.0);
    }

    #[test]
    fn usage_log_writes_and_reads_back() {
        let tmp = std::env::temp_dir().join("b00t-test-guard");
        std::fs::create_dir_all(&tmp).unwrap();

        usage_log_with_base("test/model", "load", &tmp).unwrap();
        usage_log_with_base("test/model", "unload", &tmp).unwrap();

        let log_path = tmp.join(".b00t").join("model-usage.jsonl");
        let content = std::fs::read_to_string(&log_path).unwrap();
        assert!(content.contains("test/model"));
        assert!(content.contains("load"));
        assert!(content.contains("unload"));
        assert_eq!(content.lines().count(), 2);

        let _ = std::fs::remove_dir_all(&tmp);
    }

    #[test]
    fn cache_evict_returns_empty_when_no_log() {
        let tmp = std::env::temp_dir().join("b00t-test-evict-none");
        std::fs::create_dir_all(&tmp).unwrap();
        let candidates = cache_evict_candidates_with_base(30, &tmp).unwrap();
        assert!(candidates.is_empty());
        let _ = std::fs::remove_dir_all(&tmp);
    }

    #[test]
    fn cache_evict_finds_stale_candidates() {
        let tmp = std::env::temp_dir().join("b00t-test-evict");
        std::fs::create_dir_all(&tmp).unwrap();

        // Write an old event
        let log_dir = tmp.join(".b00t");
        std::fs::create_dir_all(&log_dir).unwrap();
        let log_path = log_dir.join("model-usage.jsonl");
        let old =
            r#"{"ts":"2020-01-01T00:00:00Z","datum":"old-model","event":"load","host":"test"}"#;
        std::fs::write(&log_path, format!("{old}\n")).unwrap();

        let candidates = cache_evict_candidates_with_base(30, &tmp).unwrap();
        assert_eq!(candidates, vec!["old-model".to_string()]);

        let _ = std::fs::remove_dir_all(&tmp);
    }
}
