//! `b00t finetune race` / `b00t finetune race-finalize` — competing
//! dual-agent fine-tuning race orchestrator.
//!
//! Sub-project **C** of the fine-tuning epic (task #50), built on top of
//! sub-project B's `finetune_job` module (`FinetuneManifest`, `run_job`,
//! `pack_adapter_as_oci_layer`, `push_to_s3`, `write_ai_datum` — all reused,
//! none reimplemented). Brian's framing: "have two parallel agents compete
//! to see who can train a better model" — one on `sm3lly` (local RTX 3090),
//! one in the cloud (HF Jobs), both fine-tuning the same base model on the
//! same dataset, then compared.
//!
//! This module is split into two phases because the two competitors don't
//! finish at the same wall-clock time (and, in this sandbox, can't finish
//! at all — no GPU, no HF/cloud credentials):
//!
//!   1. **Dispatch** (`dispatch_race` / `b00t-cli finetune race`) — starts
//!      both jobs concurrently via `finetune_job::run_job`, exactly the
//!      dispatch mechanism sub-project B already built. Nothing new is
//!      reimplemented here; this is bookkeeping (which manifest got which
//!      label) plus running both `run_job` futures concurrently instead of
//!      sequentially, which is the actual "race" part.
//!   2. **Finalize** (`finalize_race` / `b00t-cli finetune race-finalize`)
//!      — once both runs have produced a training log + adapter directory
//!      (for real: copied off sm3lly and pulled from the HF Jobs output),
//!      an operator feeds both in. This phase parses each run's final
//!      training loss, compares them with a pure function, packages the
//!      *winner's* adapter as an OCI layer, pushes it (mocked by default,
//!      identical to B's `finetune run` behavior), registers it as the
//!      canonical AI datum via `finetune_job::write_ai_datum`, and writes a
//!      race report recording both runs' metadata and which one won.
//!
//! ## Scoring: why final training loss, and not the alternatives
//!
//! Three options were on the table for "what does 'better' mean":
//!
//! 1. **Eval loss on a held-out split of the training dataset.** The more
//!    rigorous choice — it at least separates "fit the training data" from
//!    "fit data it didn't see." Rejected *for now*: it requires a
//!    deterministic train/eval split baked into the dataset-generation step
//!    (`fine-tune/generate_dataset.py`) and an eval pass wired into the
//!    training config, neither of which exists yet in B's generated YAML
//!    (`generate_training_config_yaml` has no `eval_dataset`/`eval_steps`
//!    fields — the manifest schema itself has no `[job.hyperparams]`
//!    section to carry them). Building that plumbing *and* trusting it
//!    would require actually running it against a real model — which this
//!    sandbox cannot do. Adding an eval-split contract no one can validate
//!    here would be exactly the "over-engineer an eval framework you can't
//!    test against a real model" trap this task explicitly warns against.
//! 2. **A small fixed eval-prompt set scored by an LLM judge.** There is
//!    real precedent for a *harness*-scoring pattern in this repo —
//!    `_b00t_/meta-harness/meta_harness.py`'s `blended_score`/
//!    `should_promote` (pooled/all-pass rates minus a token-cost penalty,
//!    gated by `build_passed`) — but that scores **agent harness
//!    mechanisms** (a proposer/evaluator loop over prompt/tool-use
//!    strategies), not **fine-tuned model weights**. Conflating the two
//!    would be a category error: `blended_score` has no notion of "did
//!    this LoRA adapter produce better completions," and retrofitting an
//!    LLM-judge rubric for model outputs here would be inventing a new,
//!    untested evaluation surface for the same reason (1) was rejected —
//!    no GPU to actually run either adapter and judge its outputs.
//! 3. **Final training loss from each run's log.** Chosen. It is the
//!    cheapest possible signal — already emitted by `transformers.Trainer`
//!    (and Unsloth, which wraps it) without any additional instrumentation,
//!    log format, or dataset-split contract — and it requires nothing this
//!    sandbox can't provide: `parse_final_train_loss` below is a pure
//!    function over log *text*, unit-tested against synthetic log fixtures
//!    shaped exactly like what `ai-finetune.just`'s `train` recipe already
//!    produces on stdout (`transformers.Trainer`'s default dict-repr log
//!    lines) or writes to `trainer_state.json`.
//!
//! **Caveat, stated plainly and carried into every `RaceReport`
//! (`LOSS_CAVEAT`):** final training loss does not by itself prove one
//! adapter generalizes better than the other — it only measures fit to the
//! training distribution, and is sensitive to run-to-run variance (dataset
//! shuffling, a differently-seeded run, or simply one run getting slightly
//! more/fewer effective steps due to batch/grad-accum differences on
//! different hardware). It is a legitimate first-pass signal for a race
//! between two runs on the *identical* dataset and manifest-declared
//! hyperparameters (which `generate_training_config_yaml` fixes identically
//! for both competitors), not a substitute for held-out or human eval once
//! real GPU time is available to build and trust one.

use anyhow::{Context, Result, bail};
use serde::Serialize;
use std::fs;
use std::path::{Path, PathBuf};

use crate::commands::finetune_job::{
    self, DEFAULT_S3_REGION, FinetuneManifest, SPIRE_AGENT_PROFILE, StorageKind, TargetKind,
};

// ── Phase 1: dispatch (reuses finetune_job::run_job verbatim) ──────────────

/// One competitor in a race: a human-readable label plus the manifest that
/// defines its job. Labels default to "competitor-a"/"competitor-b" when not
/// given explicitly by the CLI (`--label-a`/`--label-b`).
#[derive(Debug, Clone)]
pub struct RaceEntry {
    pub label: String,
    pub manifest_path: PathBuf,
}

/// Outcome of dispatching one competitor. `result` mirrors exactly what
/// `finetune_job::run_job` returned — this struct adds no new dispatch
/// semantics, only race bookkeeping (which label/manifest produced it).
pub struct DispatchOutcome {
    pub label: String,
    pub manifest: FinetuneManifest,
    pub result: Result<()>,
}

/// Dispatches every entry's job **concurrently** via
/// `finetune_job::run_job` — the actual "race" (both competitors start at
/// the same time, on their own target) rather than a reimplementation of
/// how either target is launched. One competitor failing (e.g. the local
/// GPU gate rejecting a run, or a cloud CLI being unavailable) does not
/// abort the other; both outcomes are collected and returned.
///
/// Not exercised end-to-end by this crate's automated tests for the local
/// path (`run_job`'s GPU gate will reject any host without a free
/// `>=15000` MB GPU, which this sandbox — and most CI runners — doesn't
/// have) — that is the same, already-documented limitation of
/// `finetune_job::run_job` itself, not something new introduced here. The
/// cloud path's `--dry-run` branch requires no live tool and is exercised
/// directly.
pub async fn dispatch_race(
    entries: Vec<RaceEntry>,
    datum_dir: PathBuf,
    repo_root: PathBuf,
    dry_run: bool,
    live_push: bool,
) -> Result<Vec<DispatchOutcome>> {
    let mut handles = Vec::with_capacity(entries.len());
    for entry in entries {
        let manifest = FinetuneManifest::from_file(&entry.manifest_path)
            .with_context(|| format!("loading manifest for competitor '{}'", entry.label))?;
        let label = entry.label.clone();
        let manifest_path = entry.manifest_path.clone();
        let datum_dir = datum_dir.clone();
        let repo_root = repo_root.clone();
        let manifest_for_task = manifest.clone();
        handles.push(tokio::spawn(async move {
            let result =
                finetune_job::run_job(&manifest_path, &datum_dir, &repo_root, dry_run, live_push)
                    .await;
            DispatchOutcome {
                label,
                manifest: manifest_for_task,
                result,
            }
        }));
    }

    let mut outcomes = Vec::with_capacity(handles.len());
    for handle in handles {
        outcomes.push(
            handle
                .await
                .context("race dispatch task panicked before completing")?,
        );
    }
    Ok(outcomes)
}

// ── Log parsing: pure function, the boundary between "real GPU output" and ──
// ── "what this sandbox can unit-test" ───────────────────────────────────────

/// Parses the final training loss out of a training run's log text. Real,
/// pure, and unit-tested against two shapes `transformers.Trainer` (which
/// Unsloth wraps, and which `ai-finetune.just`'s `train` recipe already
/// drives) actually produces — this function does not invent a new log
/// format:
///
///   1. `trainer_state.json` — HF Trainer's own checkpoint state. Its
///      `log_history` array holds one entry per logged step; the final
///      post-training entry conventionally carries a `train_loss` key
///      (the run's overall average loss), which takes priority when
///      present. Falls back to the last per-step `loss` entry otherwise.
///   2. Plain stdout/log-file text containing `transformers.Trainer`'s
///      default Python-dict-repr log lines, e.g.
///      `{'loss': 0.8421, 'epoch': 1.0}` for per-step logs and
///      `{'train_runtime': 123.4, 'train_loss': 0.7532, 'epoch': 1.0}` for
///      the final summary line. `train_loss` is again preferred over the
///      last `loss` when both are present.
pub fn parse_final_train_loss(log_text: &str) -> Result<f64> {
    let trimmed = log_text.trim();

    if let Ok(v) = serde_json::from_str::<serde_json::Value>(trimmed) {
        let history = v
            .get("log_history")
            .and_then(|h| h.as_array())
            .with_context(|| "trainer_state.json has no usable \"log_history\" array")?;
        for entry in history.iter().rev() {
            if let Some(loss) = entry.get("train_loss").and_then(|x| x.as_f64()) {
                return Ok(loss);
            }
        }
        for entry in history.iter().rev() {
            if let Some(loss) = entry.get("loss").and_then(|x| x.as_f64()) {
                return Ok(loss);
            }
        }
        bail!("trainer_state.json's log_history has no \"train_loss\" or \"loss\" entry");
    }

    let mut last_loss: Option<f64> = None;
    let mut last_train_loss: Option<f64> = None;
    for line in log_text.lines() {
        if let Some(v) = extract_dict_key_f64(line, "train_loss") {
            last_train_loss = Some(v);
        } else if let Some(v) = extract_dict_key_f64(line, "loss") {
            last_loss = Some(v);
        }
    }
    last_train_loss
        .or(last_loss)
        .context("no 'loss' or 'train_loss' value found in training log text")
}

/// Extracts a numeric value following `'key':` or `"key":` in one log line.
/// Deliberately a crude, dependency-free scanner rather than a full Python
/// literal parser — `transformers.Trainer`'s dict-repr log lines are
/// consistent enough (`'key': <number>[,}]`) that this is sufficient, and
/// adding a real Python-literal-eval dependency for this one call site
/// would be disproportionate to what it buys.
fn extract_dict_key_f64(line: &str, key: &str) -> Option<f64> {
    for quote in ['\'', '"'] {
        let needle = format!("{quote}{key}{quote}:");
        if let Some(idx) = line.find(&needle) {
            let rest = line[idx + needle.len()..].trim_start();
            let end = rest.find([',', '}']).unwrap_or(rest.len());
            if let Ok(v) = rest[..end].trim().parse::<f64>() {
                return Some(v);
            }
        }
    }
    None
}

// ── Phase 2: comparison — pure, independently unit-testable from dispatch ──

/// One competitor's completed-run result: everything the comparison logic
/// needs, and nothing it doesn't. Deliberately holds no `Command`/process
/// handle or file-descriptor state — this is what makes `compare_results`
/// testable purely in-memory with mocked/hand-built instances, independent
/// of how (or whether) the job was actually dispatched.
#[derive(Debug, Clone, Serialize)]
pub struct RaceResult {
    pub label: String,
    pub job_id: String,
    pub base_model: String,
    pub target_kind: TargetKind,
    pub final_train_loss: f64,
    pub completed_at: chrono::DateTime<chrono::Utc>,
}

/// Builds a `RaceResult` from a manifest + that run's training log text.
/// The only "impure" thing here is `Utc::now()` for `completed_at`;
/// everything else is `parse_final_train_loss` plus copying manifest
/// fields.
pub fn build_race_result(label: &str, manifest: &FinetuneManifest, log_text: &str) -> Result<RaceResult> {
    let final_train_loss = parse_final_train_loss(log_text)
        .with_context(|| format!("parsing training log for competitor '{label}'"))?;
    Ok(RaceResult {
        label: label.to_string(),
        job_id: manifest.job.id.clone(),
        base_model: manifest.job.base_model.clone(),
        target_kind: manifest.target.kind,
        final_train_loss,
        completed_at: chrono::Utc::now(),
    })
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum RaceWinner {
    A,
    B,
    /// Exact float equality between two independent training runs is not
    /// expected in practice; A wins ties by convention so the comparison
    /// stays a total order rather than requiring a real caller to special-
    /// case a result neither `compare_results` nor `build_race_report`
    /// leaves ambiguous.
    Tie,
}

#[derive(Debug, Clone, Serialize)]
pub struct RaceComparison {
    pub winner: RaceWinner,
    pub margin: f64,
    pub metric: &'static str,
    pub reasoning: String,
}

pub const RACE_METRIC: &str = "final_training_loss";

/// Pure comparison over two completed results: lower `final_train_loss`
/// wins. See the module doc comment for why this metric was chosen.
/// Independently unit-testable from dispatch — takes two plain structs, no
/// I/O, no async.
pub fn compare_results(a: &RaceResult, b: &RaceResult) -> RaceComparison {
    let diff = a.final_train_loss - b.final_train_loss;
    if diff.abs() < 1e-9 {
        return RaceComparison {
            winner: RaceWinner::Tie,
            margin: 0.0,
            metric: RACE_METRIC,
            reasoning: format!(
                "tie: '{}' and '{}' both ended at train loss {:.6}",
                a.label, b.label, a.final_train_loss
            ),
        };
    }
    if diff < 0.0 {
        RaceComparison {
            winner: RaceWinner::A,
            margin: -diff,
            metric: RACE_METRIC,
            reasoning: format!(
                "'{}' wins: final training loss {:.6} < '{}'s {:.6}",
                a.label, a.final_train_loss, b.label, b.final_train_loss
            ),
        }
    } else {
        RaceComparison {
            winner: RaceWinner::B,
            margin: diff,
            metric: RACE_METRIC,
            reasoning: format!(
                "'{}' wins: final training loss {:.6} < '{}'s {:.6}",
                b.label, b.final_train_loss, a.label, a.final_train_loss
            ),
        }
    }
}

// ── Race report ─────────────────────────────────────────────────────────────

pub const LOSS_CAVEAT: &str = "Final training loss is the cheapest available signal and does not \
by itself prove one adapter generalizes better than the other — it measures fit to the training \
distribution only, and is sensitive to run-to-run variance (shuffling, seeding, effective step \
count differences across hardware). It is a legitimate first-pass signal because both competitors \
train on the identical dataset and manifest-declared hyperparameters, not a substitute for \
held-out or human eval once real GPU time is available to build and trust one.";

#[derive(Debug, Clone, Serialize)]
pub struct RaceReport {
    pub race_id: String,
    pub generated_at: chrono::DateTime<chrono::Utc>,
    pub metric: &'static str,
    pub caveat: &'static str,
    pub entry_a: RaceResult,
    pub entry_b: RaceResult,
    pub winner_label: String,
    pub margin: f64,
    pub reasoning: String,
}

/// Builds the full race report from two completed results — pure, wraps
/// `compare_results`.
pub fn build_race_report(a: &RaceResult, b: &RaceResult) -> RaceReport {
    let cmp = compare_results(a, b);
    let winner_label = match cmp.winner {
        RaceWinner::A | RaceWinner::Tie => a.label.clone(),
        RaceWinner::B => b.label.clone(),
    };
    RaceReport {
        race_id: format!("{}-vs-{}", a.job_id, b.job_id),
        generated_at: chrono::Utc::now(),
        metric: cmp.metric,
        caveat: LOSS_CAVEAT,
        entry_a: a.clone(),
        entry_b: b.clone(),
        winner_label,
        margin: cmp.margin,
        reasoning: cmp.reasoning,
    }
}

/// Writes the race report as TOML (consistent with every other datum/config
/// file in this repo) to `<out_dir>/<race_id>.race-report.toml`.
pub fn write_race_report(report: &RaceReport, out_dir: &Path) -> Result<PathBuf> {
    fs::create_dir_all(out_dir).context("creating race report output dir")?;
    let path = out_dir.join(format!("{}.race-report.toml", report.race_id));
    let body = toml::to_string_pretty(report).context("serializing race report TOML")?;
    fs::write(&path, body).with_context(|| format!("writing race report {path:?}"))?;
    Ok(path)
}

// ── Finalize: comparison + winner OCI-pack/push/datum-registration ─────────
// All reused from finetune_job — this function adds no new packing/push/
// datum-write logic, only picks which manifest+adapter-dir the winner is.

/// Given both competitors' manifests, training logs, and adapter
/// directories, computes the comparison, packages + (mock-)pushes the
/// *winner's* adapter using `finetune_job::pack_adapter_as_oci_layer` /
/// `finetune_job::push_to_s3` (B's code, unmodified), registers it as the
/// canonical AI datum via `finetune_job::write_ai_datum` (also B's code,
/// unmodified), and writes the race report. Returns the report plus the
/// paths written.
///
/// Fully unit-testable without a GPU: `log_a`/`log_b` are training-log text
/// (mocked in tests, real log files in production), and `adapter_dir_a`/
/// `adapter_dir_b` need only contain files, not a real trained adapter —
/// exactly the same `make_fake_adapter_dir` pattern B's own tests use for
/// `pack_adapter_as_oci_layer`.
#[allow(clippy::too_many_arguments)]
pub fn finalize_race(
    manifest_a: &FinetuneManifest,
    label_a: &str,
    log_a: &str,
    adapter_dir_a: &Path,
    manifest_b: &FinetuneManifest,
    label_b: &str,
    log_b: &str,
    adapter_dir_b: &Path,
    datum_dir: &Path,
    race_out_dir: &Path,
    live_push: bool,
) -> Result<(RaceReport, PathBuf, PathBuf)> {
    let result_a = build_race_result(label_a, manifest_a, log_a)?;
    let result_b = build_race_result(label_b, manifest_b, log_b)?;
    let report = build_race_report(&result_a, &result_b);

    let (winner_manifest, winner_adapter_dir) = if report.winner_label == label_a {
        (manifest_a, adapter_dir_a)
    } else {
        (manifest_b, adapter_dir_b)
    };

    let oci_out = race_out_dir.join("oci").join(&winner_manifest.output.datum);
    let oci = finetune_job::pack_adapter_as_oci_layer(
        winner_adapter_dir,
        &oci_out,
        &winner_manifest.job.id,
        &winner_manifest.job.base_model,
        &winner_manifest.job.framework,
    )
    .context("packing winning adapter as OCI layer")?;

    let push = match winner_manifest.output.storage {
        StorageKind::S3 => {
            let bucket = winner_manifest
                .output
                .s3_bucket
                .as_deref()
                .context("[output].s3_bucket required for storage = \"s3\"")?;
            let key = format!(
                "{}/{}.tar.gz",
                winner_manifest.output.datum,
                oci.layer_digest.trim_start_matches("sha256:")
            );
            finetune_job::push_to_s3(
                &oci.layer_path,
                bucket,
                &key,
                SPIRE_AGENT_PROFILE,
                DEFAULT_S3_REGION,
                !live_push,
            )
            .context("pushing winning adapter's OCI layer to S3")?
        }
        StorageKind::Zerofs => bail!(
            "storage = \"zerofs\" requires the infrastructure#192 ZeroFS container mount \
             (not yet build-verified) — same limitation as finetune_job::run_job"
        ),
    };

    let datum_path = finetune_job::write_ai_datum(datum_dir, winner_manifest, &oci, &push)
        .context("registering winning adapter as the canonical AI datum")?;
    let report_path = write_race_report(&report, &race_out_dir.join("reports"))
        .context("writing race report")?;

    Ok((report, datum_path, report_path))
}

// ── CLI surface ──────────────────────────────────────────────────────────

/// `b00t-cli finetune race <manifest_a> <manifest_b>` — dispatches both
/// competitors concurrently and prints each outcome. See module doc
/// comment: real jobs cannot complete inside a GPU-less/credential-less
/// sandbox, so this only starts them; `race-finalize` is the follow-up step
/// once both runs have produced real logs + adapters.
#[allow(clippy::too_many_arguments)]
pub async fn handle_race_dispatch(
    manifest_a: &Path,
    manifest_b: &Path,
    label_a: Option<String>,
    label_b: Option<String>,
    datum_dir: &Path,
    repo_root: &Path,
    dry_run: bool,
    live_push: bool,
) -> Result<()> {
    let label_a = label_a.unwrap_or_else(|| "competitor-a".to_string());
    let label_b = label_b.unwrap_or_else(|| "competitor-b".to_string());
    println!("→ race: '{label_a}' ({}) vs '{label_b}' ({})", manifest_a.display(), manifest_b.display());

    let entries = vec![
        RaceEntry {
            label: label_a,
            manifest_path: manifest_a.to_path_buf(),
        },
        RaceEntry {
            label: label_b,
            manifest_path: manifest_b.to_path_buf(),
        },
    ];
    let outcomes = dispatch_race(
        entries,
        datum_dir.to_path_buf(),
        repo_root.to_path_buf(),
        dry_run,
        live_push,
    )
    .await?;

    for outcome in &outcomes {
        match &outcome.result {
            Ok(()) => println!(
                "  ✓ [{}] job.id={} dispatch step completed",
                outcome.label, outcome.manifest.job.id
            ),
            Err(e) => println!(
                "  ✗ [{}] job.id={} dispatch failed: {e:#}",
                outcome.label, outcome.manifest.job.id
            ),
        }
    }

    println!(
        "\nBoth competitors dispatched. Once each run finishes for real, gather each run's\n\
         training log + adapter directory and run:\n  \
         b00t-cli finetune race-finalize \\\n    \
         --manifest-a {} --log-a <log-a.txt> --adapter-dir-a <dir-a> \\\n    \
         --manifest-b {} --log-b <log-b.txt> --adapter-dir-b <dir-b>",
        manifest_a.display(),
        manifest_b.display()
    );
    Ok(())
}

/// `b00t-cli finetune race-finalize` — the completion path: compares both
/// runs' final training loss, packages + registers the winner. See
/// `finalize_race` for what's reused vs. new.
#[allow(clippy::too_many_arguments)]
pub fn handle_race_finalize(
    manifest_a_path: &Path,
    label_a: Option<String>,
    log_a_path: &Path,
    adapter_dir_a: &Path,
    manifest_b_path: &Path,
    label_b: Option<String>,
    log_b_path: &Path,
    adapter_dir_b: &Path,
    datum_dir: &Path,
    race_out_dir: &Path,
    live_push: bool,
) -> Result<()> {
    let manifest_a = FinetuneManifest::from_file(manifest_a_path)?;
    let manifest_b = FinetuneManifest::from_file(manifest_b_path)?;
    let label_a = label_a.unwrap_or_else(|| "competitor-a".to_string());
    let label_b = label_b.unwrap_or_else(|| "competitor-b".to_string());
    let log_a = fs::read_to_string(log_a_path)
        .with_context(|| format!("reading training log {log_a_path:?}"))?;
    let log_b = fs::read_to_string(log_b_path)
        .with_context(|| format!("reading training log {log_b_path:?}"))?;

    let (report, datum_path, report_path) = finalize_race(
        &manifest_a,
        &label_a,
        &log_a,
        adapter_dir_a,
        &manifest_b,
        &label_b,
        &log_b,
        adapter_dir_b,
        datum_dir,
        race_out_dir,
        live_push,
    )?;

    println!("race: {}", report.race_id);
    println!(
        "  {} (loss={:.6}) vs {} (loss={:.6})",
        report.entry_a.label,
        report.entry_a.final_train_loss,
        report.entry_b.label,
        report.entry_b.final_train_loss
    );
    println!("  winner: {} (margin {:.6})", report.winner_label, report.margin);
    println!("  reasoning: {}", report.reasoning);
    println!("  caveat: {}", report.caveat);
    println!("  ai datum written: {}", datum_path.display());
    println!("  race report written: {}", report_path.display());
    Ok(())
}

// ── Tests ──────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::commands::finetune_job::FinetuneManifest;

    fn local_manifest_toml(datum: &str) -> String {
        format!(
            r#"
[job]
id = "race-local-2026-09-01"
base_model = "unsloth/Qwen3.8-27B-unsloth-bnb-4bit"
framework = "unsloth"

[dataset]
source = "hf://datasets/elasticdotventures/b00t-training"

[target]
kind = "local"
hive_profile = "finetune"

[output]
datum = "{datum}"
storage = "s3"
s3_bucket = "b00t-finetune-artifacts"
mirror_to_hf = true
"#
        )
    }

    fn cloud_manifest_toml(datum: &str) -> String {
        format!(
            r#"
[job]
id = "race-cloud-2026-09-01"
base_model = "unsloth/Qwen3.8-27B-unsloth-bnb-4bit"
framework = "unsloth"

[dataset]
source = "hf://datasets/elasticdotventures/b00t-training"

[target]
kind = "cloud"
image = "ghcr.io/elasticdotventures/b00t-training-image:latest"
flavor = "a100-large"
timeout_hours = 10.0

[output]
datum = "{datum}"
storage = "s3"
s3_bucket = "b00t-finetune-artifacts"
mirror_to_hf = true
"#
        )
    }

    // ── parse_final_train_loss ───────────────────────────────────────────

    #[test]
    fn parses_final_loss_from_dict_repr_log_prefers_train_loss_summary() {
        let log = "\
{'loss': 1.2345, 'grad_norm': 0.9, 'learning_rate': 5e-05, 'epoch': 0.5}
{'loss': 0.9876, 'grad_norm': 0.8, 'learning_rate': 4e-05, 'epoch': 1.0}
{'train_runtime': 123.4, 'train_samples_per_second': 8.1, 'train_loss': 0.7532, 'epoch': 1.0}
";
        let loss = parse_final_train_loss(log).expect("should parse");
        assert!((loss - 0.7532).abs() < 1e-9);
    }

    #[test]
    fn parses_final_loss_from_dict_repr_log_falls_back_to_last_loss_when_no_summary() {
        let log = "\
{'loss': 1.2345, 'epoch': 0.5}
{'loss': 0.6543, 'epoch': 1.0}
";
        let loss = parse_final_train_loss(log).expect("should parse");
        assert!((loss - 0.6543).abs() < 1e-9);
    }

    #[test]
    fn parses_final_loss_from_trainer_state_json_prefers_train_loss() {
        let json = r#"{
            "log_history": [
                {"loss": 1.1, "epoch": 0.5, "step": 10},
                {"loss": 0.9, "epoch": 1.0, "step": 20},
                {"train_runtime": 50.0, "train_loss": 0.85, "epoch": 1.0, "step": 20}
            ]
        }"#;
        let loss = parse_final_train_loss(json).expect("should parse");
        assert!((loss - 0.85).abs() < 1e-9);
    }

    #[test]
    fn parses_final_loss_from_trainer_state_json_falls_back_to_last_loss() {
        let json = r#"{
            "log_history": [
                {"loss": 1.1, "epoch": 0.5, "step": 10},
                {"loss": 0.42, "epoch": 1.0, "step": 20}
            ]
        }"#;
        let loss = parse_final_train_loss(json).expect("should parse");
        assert!((loss - 0.42).abs() < 1e-9);
    }

    #[test]
    fn rejects_log_text_with_no_loss_values() {
        let log = "starting training...\nepoch 1 complete\nno numbers here to parse\n";
        assert!(parse_final_train_loss(log).is_err());
    }

    #[test]
    fn rejects_trainer_state_json_without_log_history() {
        let json = r#"{"best_metric": null}"#;
        assert!(parse_final_train_loss(json).is_err());
    }

    // ── compare_results / build_race_report ─────────────────────────────

    fn result_with_loss(label: &str, loss: f64) -> RaceResult {
        RaceResult {
            label: label.to_string(),
            job_id: format!("job-{label}"),
            base_model: "unsloth/Qwen3.8-27B-unsloth-bnb-4bit".to_string(),
            target_kind: TargetKind::Local,
            final_train_loss: loss,
            completed_at: chrono::Utc::now(),
        }
    }

    #[test]
    fn lower_loss_wins() {
        let a = result_with_loss("a", 0.5);
        let b = result_with_loss("b", 0.8);
        let cmp = compare_results(&a, &b);
        assert_eq!(cmp.winner, RaceWinner::A);
        assert!((cmp.margin - 0.3).abs() < 1e-9);
        assert_eq!(cmp.metric, RACE_METRIC);
    }

    #[test]
    fn higher_loss_side_loses_regardless_of_order() {
        let a = result_with_loss("a", 0.9);
        let b = result_with_loss("b", 0.4);
        let cmp = compare_results(&a, &b);
        assert_eq!(cmp.winner, RaceWinner::B);
        assert!((cmp.margin - 0.5).abs() < 1e-9);
    }

    #[test]
    fn exact_tie_resolves_to_a_by_convention() {
        let a = result_with_loss("a", 0.6);
        let b = result_with_loss("b", 0.6);
        let cmp = compare_results(&a, &b);
        assert_eq!(cmp.winner, RaceWinner::Tie);
        assert_eq!(cmp.margin, 0.0);

        let report = build_race_report(&a, &b);
        assert_eq!(report.winner_label, "a");
    }

    #[test]
    fn build_race_report_carries_loss_caveat_and_metric() {
        let a = result_with_loss("sm3lly-local", 0.5);
        let b = result_with_loss("hf-jobs-cloud", 0.6);
        let report = build_race_report(&a, &b);
        assert_eq!(report.winner_label, "sm3lly-local");
        assert_eq!(report.metric, "final_training_loss");
        assert!(report.caveat.contains("does not by itself prove"));
        assert!((report.margin - 0.1).abs() < 1e-9);
        assert_eq!(report.race_id, "job-sm3lly-local-vs-job-hf-jobs-cloud");
    }

    #[test]
    fn write_race_report_writes_valid_toml() {
        let a = result_with_loss("a", 0.5);
        let b = result_with_loss("b", 0.7);
        let report = build_race_report(&a, &b);
        let out = tempfile::tempdir().unwrap();
        let path = write_race_report(&report, out.path()).unwrap();
        assert!(path.is_file());
        let content = fs::read_to_string(&path).unwrap();
        let parsed: toml::Value = toml::from_str(&content).unwrap();
        assert_eq!(parsed["winner_label"].as_str().unwrap(), "a");
        assert_eq!(parsed["metric"].as_str().unwrap(), "final_training_loss");
    }

    // ── build_race_result ─────────────────────────────────────────────────

    #[test]
    fn build_race_result_extracts_loss_and_copies_manifest_fields() {
        let manifest = FinetuneManifest::from_toml_str(&local_manifest_toml("qwen38-peer-race")).unwrap();
        let log = "{'train_runtime': 1.0, 'train_loss': 0.321, 'epoch': 1.0}\n";
        let result = build_race_result("sm3lly-local", &manifest, log).unwrap();
        assert_eq!(result.label, "sm3lly-local");
        assert_eq!(result.job_id, "race-local-2026-09-01");
        assert_eq!(result.base_model, "unsloth/Qwen3.8-27B-unsloth-bnb-4bit");
        assert_eq!(result.target_kind, TargetKind::Local);
        assert!((result.final_train_loss - 0.321).abs() < 1e-9);
    }

    #[test]
    fn build_race_result_propagates_log_parse_error_with_label_context() {
        let manifest = FinetuneManifest::from_toml_str(&local_manifest_toml("qwen38-peer-race")).unwrap();
        let err = build_race_result("sm3lly-local", &manifest, "no loss here").unwrap_err();
        assert!(err.to_string().contains("sm3lly-local"));
    }

    // ── finalize_race: end-to-end with mocked logs + fake adapter dirs ────

    fn make_fake_adapter_dir(tag: &str) -> tempfile::TempDir {
        let dir = tempfile::tempdir().unwrap();
        fs::write(
            dir.path().join("adapter_model.safetensors"),
            format!("fake-safetensors-{tag}").as_bytes(),
        )
        .unwrap();
        fs::write(
            dir.path().join("adapter_config.json"),
            br#"{"r":16,"lora_alpha":32}"#,
        )
        .unwrap();
        dir
    }

    #[test]
    fn finalize_race_picks_lower_loss_winner_and_registers_its_datum() {
        let manifest_a =
            FinetuneManifest::from_toml_str(&local_manifest_toml("qwen38-peer-race")).unwrap();
        let manifest_b =
            FinetuneManifest::from_toml_str(&cloud_manifest_toml("qwen38-peer-race")).unwrap();

        let adapter_a = make_fake_adapter_dir("local");
        let adapter_b = make_fake_adapter_dir("cloud");
        let datum_dir = tempfile::tempdir().unwrap();
        let race_out = tempfile::tempdir().unwrap();

        // 'a' (local) wins: lower final loss.
        let log_a = "{'train_runtime': 1.0, 'train_loss': 0.30, 'epoch': 1.0}\n";
        let log_b = "{'train_runtime': 1.0, 'train_loss': 0.55, 'epoch': 1.0}\n";

        let (report, datum_path, report_path) = finalize_race(
            &manifest_a,
            "sm3lly-local",
            log_a,
            adapter_a.path(),
            &manifest_b,
            "hf-jobs-cloud",
            log_b,
            adapter_b.path(),
            datum_dir.path(),
            race_out.path(),
            false,
        )
        .expect("finalize_race should succeed with mocked push");

        assert_eq!(report.winner_label, "sm3lly-local");
        assert!(datum_path.is_file());
        assert!(report_path.is_file());

        // Registered datum must reflect the WINNER's manifest (job a, not b).
        let datum_content = fs::read_to_string(&datum_path).unwrap();
        let parsed: toml::Value = toml::from_str(&datum_content).unwrap();
        assert_eq!(
            parsed["b00t"]["finetune"]["job_id"].as_str().unwrap(),
            "race-local-2026-09-01"
        );
        assert!(parsed["b00t"]["oci"]["mocked"].as_bool().unwrap());
    }

    #[test]
    fn finalize_race_registers_b_when_b_has_lower_loss() {
        let manifest_a =
            FinetuneManifest::from_toml_str(&local_manifest_toml("qwen38-peer-race")).unwrap();
        let manifest_b =
            FinetuneManifest::from_toml_str(&cloud_manifest_toml("qwen38-peer-race")).unwrap();

        let adapter_a = make_fake_adapter_dir("local");
        let adapter_b = make_fake_adapter_dir("cloud");
        let datum_dir = tempfile::tempdir().unwrap();
        let race_out = tempfile::tempdir().unwrap();

        let log_a = "{'train_runtime': 1.0, 'train_loss': 0.90, 'epoch': 1.0}\n";
        let log_b = "{'train_runtime': 1.0, 'train_loss': 0.20, 'epoch': 1.0}\n";

        let (report, datum_path, _report_path) = finalize_race(
            &manifest_a,
            "sm3lly-local",
            log_a,
            adapter_a.path(),
            &manifest_b,
            "hf-jobs-cloud",
            log_b,
            adapter_b.path(),
            datum_dir.path(),
            race_out.path(),
            false,
        )
        .unwrap();

        assert_eq!(report.winner_label, "hf-jobs-cloud");
        let datum_content = fs::read_to_string(&datum_path).unwrap();
        let parsed: toml::Value = toml::from_str(&datum_content).unwrap();
        assert_eq!(
            parsed["b00t"]["finetune"]["job_id"].as_str().unwrap(),
            "race-cloud-2026-09-01"
        );
    }

    #[test]
    fn finalize_race_fails_clearly_on_unparseable_log() {
        let manifest_a =
            FinetuneManifest::from_toml_str(&local_manifest_toml("qwen38-peer-race")).unwrap();
        let manifest_b =
            FinetuneManifest::from_toml_str(&cloud_manifest_toml("qwen38-peer-race")).unwrap();
        let adapter_a = make_fake_adapter_dir("local");
        let adapter_b = make_fake_adapter_dir("cloud");
        let datum_dir = tempfile::tempdir().unwrap();
        let race_out = tempfile::tempdir().unwrap();

        let result = finalize_race(
            &manifest_a,
            "sm3lly-local",
            "not a log at all",
            adapter_a.path(),
            &manifest_b,
            "hf-jobs-cloud",
            "{'train_loss': 0.5}",
            adapter_b.path(),
            datum_dir.path(),
            race_out.path(),
            false,
        );
        assert!(result.is_err());
    }

    // ── dispatch_race: cloud dry-run path needs no live tool ─────────────

    #[tokio::test]
    async fn dispatch_race_runs_both_competitors_concurrently_and_collects_outcomes() {
        let dir = tempfile::tempdir().unwrap();
        let manifest_a_path = dir.path().join("a.finetune.toml");
        let manifest_b_path = dir.path().join("b.finetune.toml");
        fs::write(&manifest_a_path, cloud_manifest_toml("race-a")).unwrap();
        fs::write(&manifest_b_path, cloud_manifest_toml("race-b")).unwrap();

        let entries = vec![
            RaceEntry {
                label: "competitor-a".to_string(),
                manifest_path: manifest_a_path,
            },
            RaceEntry {
                label: "competitor-b".to_string(),
                manifest_path: manifest_b_path,
            },
        ];

        // Both manifests are `kind = "cloud"` with dry_run = true: run_job's
        // cloud branch returns Ok(()) before touching any live tool (see
        // finetune_job::run_job's dry-run early-return), so this exercises
        // real concurrent dispatch without needing the `hf` CLI installed.
        let outcomes = dispatch_race(entries, dir.path().to_path_buf(), dir.path().to_path_buf(), true, false)
            .await
            .expect("dispatch_race should not error at the orchestration level");

        assert_eq!(outcomes.len(), 2);
        let labels: Vec<&str> = outcomes.iter().map(|o| o.label.as_str()).collect();
        assert!(labels.contains(&"competitor-a"));
        assert!(labels.contains(&"competitor-b"));
        for outcome in &outcomes {
            assert!(
                outcome.result.is_ok(),
                "cloud dry-run dispatch for '{}' should succeed without a live `hf` CLI: {:?}",
                outcome.label,
                outcome.result.as_ref().err()
            );
        }
    }
}
