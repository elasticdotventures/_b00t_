//! `b00t finetune` — generic fine-tuning job manifest + runner.
//!
//! Thin layer over `_b00t_/ai-finetune.just`'s already-working Unsloth QLoRA
//! pipeline (real, existing infrastructure — see
//! `docs/superpowers/specs/2026-09-01-generic-finetune-job-oci-distribution-design.md`).
//! This module does NOT reimplement training: it reads a TOML job manifest,
//! generates the same kind of YAML config `ai-finetune.just`'s `train` recipe
//! already consumes (mirrors `fine-tune/config-smol.yaml` /
//! `fine-tune/config-cloud-coder.yaml`), and dispatches to whichever existing
//! mechanism `[target].kind` names:
//!   - `local`: `b00t hive activate finetune` (GPU gate + service stop) then
//!     `just -f _b00t_/ai-finetune.just ai-finetune::train <config>` — the
//!     exact same entrypoint the sm3lly RTX3090 path already uses.
//!   - `cloud`: `hf jobs run ghcr.io/elasticdotventures/b00t-training-image`
//!     with the same flags `ai-finetune.just`'s `cloud-train` recipe already
//!     hardcodes, parameterized by the manifest instead.
//!
//! After training produces a LoRA adapter directory, this module packages it
//! as a single OCI layer (custom packer — see `pack_adapter_as_oci_layer`'s
//! doc comment for why `oras` wasn't used), pushes it to S3-compatible
//! storage via a SPIRE-federated `spire-agent` AWS profile (infrastructure
//! repo PR #189's pattern — never root/static keys), and writes/updates the
//! resulting `<datum>.ai.toml` AI datum file.
//!
//! b00t finetune validate <manifest.toml>
//! b00t finetune pack --adapter-dir <dir> --out <oci-dir> --job-id <id> --base-model <model>
//! b00t finetune run <manifest.toml> [--dry-run] [--live-push]

use anyhow::{Context, Result, bail};
use backon::{BlockingRetryable, ExponentialBuilder};
use clap::Parser;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

// ── Manifest schema (per the approved design spec) ─────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FinetuneManifest {
    pub job: JobSection,
    pub dataset: DatasetSection,
    pub target: TargetSection,
    pub output: OutputSection,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JobSection {
    /// datum-name-shaped, not a UUID — e.g. "qwen38-peer-2026-09-01"
    pub id: String,
    pub base_model: String,
    #[serde(default = "default_framework")]
    pub framework: String,
}

fn default_framework() -> String {
    "unsloth".to_string()
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct DatasetSection {
    pub source: Option<String>,
    #[serde(default)]
    pub generate: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum TargetKind {
    Local,
    Cloud,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TargetSection {
    pub kind: TargetKind,
    /// local only — defaults to "finetune" (the existing
    /// `_b00t_/finetune.hive.toml` gate: gpu_free_mb >= 15000)
    pub hive_profile: Option<String>,
    /// cloud only — e.g. "ghcr.io/elasticdotventures/b00t-training-image:latest"
    pub image: Option<String>,
    /// cloud only — e.g. "a100-large", "h200", "a10g-large"
    pub flavor: Option<String>,
    #[serde(default = "default_timeout_hours")]
    pub timeout_hours: f32,
}

fn default_timeout_hours() -> f32 {
    10.0
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum StorageKind {
    S3,
    Zerofs,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OutputSection {
    /// → `<datum>.ai.toml` gets written/updated
    pub datum: String,
    pub storage: StorageKind,
    /// required when storage = "s3" (ap-southeast-4)
    pub s3_bucket: Option<String>,
    #[serde(default = "default_true")]
    pub mirror_to_hf: bool,
}

fn default_true() -> bool {
    true
}

impl FinetuneManifest {
    pub fn from_toml_str(s: &str) -> Result<Self> {
        let manifest: Self = toml::from_str(s).context("parsing finetune job manifest TOML")?;
        manifest.validate()?;
        Ok(manifest)
    }

    pub fn from_file(path: &Path) -> Result<Self> {
        let s = fs::read_to_string(path)
            .with_context(|| format!("reading finetune manifest {path:?}"))?;
        Self::from_toml_str(&s)
    }

    /// Cross-field validation the TOML schema alone can't express. Split out
    /// from `from_toml_str` so tests can exercise it against hand-built
    /// structs without round-tripping through TOML text.
    pub fn validate(&self) -> Result<()> {
        if self.job.id.trim().is_empty() {
            bail!("[job].id must not be empty");
        }
        if self.job.base_model.trim().is_empty() {
            bail!("[job].base_model must not be empty");
        }
        if self.job.framework != "unsloth" {
            bail!(
                "[job].framework = \"{}\" not implemented — only \"unsloth\" is wired to a runner today (axolotl reserved, see axolotl.ai.toml)",
                self.job.framework
            );
        }
        if self.dataset.source.is_none() && !self.dataset.generate {
            bail!("[dataset] must set either `source = \"hf://...\"` or `generate = true`");
        }
        match self.target.kind {
            TargetKind::Local => {}
            TargetKind::Cloud => {
                if self.target.image.as_deref().unwrap_or("").trim().is_empty() {
                    bail!("[target].image is required when kind = \"cloud\"");
                }
                if self.target.flavor.as_deref().unwrap_or("").trim().is_empty() {
                    bail!("[target].flavor is required when kind = \"cloud\"");
                }
            }
        }
        if self.output.datum.trim().is_empty() {
            bail!("[output].datum must not be empty");
        }
        if self.output.storage == StorageKind::S3
            && self.output.s3_bucket.as_deref().unwrap_or("").trim().is_empty()
        {
            bail!("[output].s3_bucket is required when storage = \"s3\"");
        }
        Ok(())
    }

    pub fn hive_profile_name(&self) -> &str {
        self.target.hive_profile.as_deref().unwrap_or("finetune")
    }
}

// ── Training config YAML generation (parameterizes ai-finetune.just's `train`) ──

/// Generates the YAML config `ai-finetune.just`'s generic `train` recipe
/// already reads (mirrors `fine-tune/config-smol.yaml` /
/// `fine-tune/config-cloud-coder.yaml`'s shape exactly — same field names,
/// same recipe). Hyperparameters are deliberately fixed: the approved
/// manifest schema has no `[job.hyperparams]` section, so this is not the
/// place to invent per-run overrides.
pub fn generate_training_config_yaml(
    manifest: &FinetuneManifest,
    dataset_path: &str,
    output_dir: &str,
) -> Result<String> {
    #[derive(Serialize)]
    struct TrainConfig<'a> {
        base_model: &'a str,
        adapter_name: &'a str,
        dataset: &'a str,
        output_dir: &'a str,
        lora_r: u32,
        lora_alpha: u32,
        lora_dropout: f32,
        target_modules: Vec<&'static str>,
        num_epochs: u32,
        batch_size: u32,
        grad_accum: u32,
        learning_rate: f64,
        max_seq_length: u32,
        optim: &'static str,
        weight_decay: f64,
        lr_scheduler: &'static str,
        warmup_ratio: f64,
        load_in_4bit: bool,
        use_gradient_checkpointing: bool,
        report_to: &'static str,
        logging_steps: u32,
        #[serde(skip_serializing_if = "Option::is_none")]
        hub_model_id: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        push_to_hub: Option<bool>,
        #[serde(skip_serializing_if = "Option::is_none")]
        private: Option<bool>,
    }

    let mirror = manifest.output.mirror_to_hf;
    let cfg = TrainConfig {
        base_model: &manifest.job.base_model,
        adapter_name: &manifest.job.id,
        dataset: dataset_path,
        output_dir,
        lora_r: 16,
        lora_alpha: 32,
        lora_dropout: 0.0,
        target_modules: vec![
            "q_proj",
            "v_proj",
            "k_proj",
            "o_proj",
            "gate_proj",
            "up_proj",
            "down_proj",
        ],
        num_epochs: 1,
        batch_size: 1,
        grad_accum: 16,
        learning_rate: 1e-4,
        max_seq_length: 1024,
        optim: "adamw_8bit",
        weight_decay: 0.01,
        lr_scheduler: "cosine",
        warmup_ratio: 0.05,
        load_in_4bit: true,
        use_gradient_checkpointing: true,
        report_to: "none",
        logging_steps: 10,
        hub_model_id: mirror.then(|| format!("elasticdotventures/{}", manifest.output.datum)),
        push_to_hub: mirror.then_some(true),
        private: mirror.then_some(true),
    };
    serde_yaml::to_string(&cfg).context("serializing generated training config to YAML")
}

// ── Local dispatch: pure argv builder ───────────────────────────────────────

/// Pure argv builder for the local path: `just -f <justfile> ai-finetune::train <config>`.
/// Unit-testable without a live GPU/hive — mirrors the recipe invocation
/// exactly (see `_b00t_/ai-finetune.just`'s `train` recipe).
pub fn local_train_args(justfile_path: &str, config_path: &str) -> Vec<String> {
    vec![
        "-f".to_string(),
        justfile_path.to_string(),
        "ai-finetune::train".to_string(),
        config_path.to_string(),
    ]
}

// ── Cloud dispatch: pure argv builder ───────────────────────────────────────

/// Pure argv builder mirroring `ai-finetune.just`'s `cloud-train` recipe
/// exactly (same image/flavor/timeout/env/volume shape, same `/bin/sh -c`
/// bootstrap for `just`) — parameterized by the manifest instead of
/// hardcoded. Unit-testable without the `hf` CLI installed.
pub fn hf_jobs_run_args(manifest: &FinetuneManifest, config_hf_name: &str) -> Result<Vec<String>> {
    let image = manifest
        .target
        .image
        .as_deref()
        .context("[target].image required for cloud jobs")?;
    let flavor = manifest
        .target
        .flavor
        .as_deref()
        .context("[target].flavor required for cloud jobs")?;
    let timeout = format!("{}h", manifest.target.timeout_hours.ceil() as u32);
    let sh_cmd = format!(
        "command -v just 2>/dev/null || {{ curl -sSL -o /tmp/just.tar.gz https://github.com/casey/just/releases/download/1.54.0/just-1.54.0-x86_64-unknown-linux-musl.tar.gz && tar -xzf /tmp/just.tar.gz -C /tmp just; }} && \
$(command -v just 2>/dev/null || echo /tmp/just) -f /data/ai-finetune.just train /data/{config_hf_name}"
    );
    Ok(vec![
        "jobs".to_string(),
        "run".to_string(),
        image.to_string(),
        "--flavor".to_string(),
        flavor.to_string(),
        "--timeout".to_string(),
        timeout,
        "-e".to_string(),
        "HF_HOME=/tmp/hf-cache".to_string(),
        "-e".to_string(),
        "PYTORCH_ALLOC_CONF=expandable_segments:True".to_string(),
        "-e".to_string(),
        "TORCHDYNAMO_DISABLE=1".to_string(),
        "-e".to_string(),
        "UNSLOTH_COMPILE_LOCATION=/tmp/unsloth_compiled_cache".to_string(),
        "--secrets".to_string(),
        "HF_TOKEN".to_string(),
        "-v".to_string(),
        "hf://datasets/elasticdotventures/b00t-training:/data:ro".to_string(),
        "-v".to_string(),
        "hf://buckets/elasticdotventures/b00t-adapters:/adapters:rw".to_string(),
        "--detach".to_string(),
        "/bin/sh".to_string(),
        "-c".to_string(),
        sh_cmd,
    ])
}

// ── OCI-layer packaging ──────────────────────────────────────────────────
//
// 🤓 oras-vs-custom-packer (spec's open question, resolved here): `oras`
//    targets the OCI Distribution Spec `/v2/` HTTP registry protocol — it
//    pushes blobs to a registry, not to arbitrary S3 object storage. This
//    design's storage target IS S3 (or ZeroFS's POSIX mount), with no OCI
//    registry sitting in front of it, so `oras push <registry-ref>` doesn't
//    apply to the actual transport step at all. What we still want from the
//    "OCI-layer" idea is the *shape*: a content-addressed, digest-named blob
//    plus a small manifest describing it, so "one OCI layer per AI datum
//    version" is a real, verifiable contract rather than a metaphor. A
//    minimal custom packer (this module) produces exactly that shape as a
//    local OCI Image Layout directory (blobs/sha256/*, index.json,
//    oci-layout) — no new tool dependency, no registry required, and it's
//    fully unit-testable in a GPU-less sandbox. `oras` was checked and is
//    NOT installed in this environment (`which oras` → not found); if a
//    later phase wants blobs mirrored into an actual OCI registry too (e.g.
//    ghcr.io, for interop with tools that expect `oras pull`), that's an
//    additive `oras push` step layered on top of these same blob files —
//    not a replacement for this packer.

pub const B00T_FINETUNE_ARTIFACT_TYPE: &str = "application/vnd.b00t.finetune.lora-adapter.v1";
pub const OCI_EMPTY_CONFIG_MEDIA_TYPE: &str = "application/vnd.oci.empty.v1+json";
pub const OCI_LAYER_MEDIA_TYPE: &str = "application/vnd.oci.image.layer.v1.tar+gzip";
pub const OCI_MANIFEST_MEDIA_TYPE: &str = "application/vnd.oci.image.manifest.v1+json";

#[derive(Debug, Clone)]
pub struct OciLayerResult {
    pub layer_digest: String,
    pub layer_size: u64,
    pub layer_path: PathBuf,
    pub manifest_digest: String,
    pub manifest_path: PathBuf,
    pub oci_layout_dir: PathBuf,
}

/// Returns true if the `oras` binary is on PATH. Checked rather than assumed
/// — the design spec explicitly calls out oras as "no existing dependency on
/// it yet, would be a new tool addition." Not currently used for the S3
/// transport (see module doc comment) — kept as a documented capability
/// probe for a future registry-mirroring step.
pub fn oras_available() -> bool {
    which::which("oras").is_ok()
}

/// Packages `adapter_dir` (expects `adapter_model.safetensors`,
/// `adapter_config.json`, tokenizer files — whatever
/// `model.save_pretrained()`/`tokenizer.save_pretrained()` wrote in
/// `ai-finetune.just`'s `train` recipe) as a single OCI layer inside a local
/// OCI Image Layout directory at `out_dir`. Returns the layer's digest,
/// which becomes the version identifier written into the AI datum.
pub fn pack_adapter_as_oci_layer(
    adapter_dir: &Path,
    out_dir: &Path,
    job_id: &str,
    base_model: &str,
    framework: &str,
) -> Result<OciLayerResult> {
    if !adapter_dir.is_dir() {
        bail!("adapter directory not found: {adapter_dir:?}");
    }
    let blobs_dir = out_dir.join("blobs").join("sha256");
    fs::create_dir_all(&blobs_dir).context("creating OCI blobs dir")?;

    // 1. Deterministic tar+gzip of the adapter directory — the single layer.
    let layer_bytes = build_deterministic_tar_gz(adapter_dir)?;
    let layer_digest_hex = sha256_hex(&layer_bytes);
    let layer_size = layer_bytes.len() as u64;
    let layer_path = blobs_dir.join(&layer_digest_hex);
    fs::write(&layer_path, &layer_bytes).context("writing OCI layer blob")?;

    // 2. Empty OCI config blob — this is an "artifact" per the OCI 1.1
    //    Image Spec's artifact-without-config convention: a LoRA adapter has
    //    no meaningful "image config," so config is the canonical `{}`.
    let config_bytes: &[u8] = b"{}";
    let config_digest_hex = sha256_hex(config_bytes);
    fs::write(blobs_dir.join(&config_digest_hex), config_bytes)
        .context("writing OCI config blob")?;

    // 3. Manifest referencing config + layer, with b00t provenance annotations.
    let manifest = serde_json::json!({
        "schemaVersion": 2,
        "mediaType": OCI_MANIFEST_MEDIA_TYPE,
        "artifactType": B00T_FINETUNE_ARTIFACT_TYPE,
        "config": {
            "mediaType": OCI_EMPTY_CONFIG_MEDIA_TYPE,
            "digest": format!("sha256:{config_digest_hex}"),
            "size": config_bytes.len(),
        },
        "layers": [{
            "mediaType": OCI_LAYER_MEDIA_TYPE,
            "digest": format!("sha256:{layer_digest_hex}"),
            "size": layer_size,
            "annotations": { "org.opencontainers.image.title": "lora-adapter.tar.gz" },
        }],
        "annotations": {
            "io.b00t.finetune.job_id": job_id,
            "io.b00t.finetune.base_model": base_model,
            "io.b00t.finetune.framework": framework,
            "org.opencontainers.image.created": chrono::Utc::now().to_rfc3339(),
        },
    });
    let manifest_bytes =
        serde_json::to_vec_pretty(&manifest).context("serializing OCI manifest")?;
    let manifest_digest_hex = sha256_hex(&manifest_bytes);
    let manifest_path = blobs_dir.join(&manifest_digest_hex);
    fs::write(&manifest_path, &manifest_bytes).context("writing OCI manifest blob")?;

    // 4. oci-layout marker + index.json (OCI Image Layout spec) so the
    //    directory is a valid, tool-inspectable OCI layout on its own.
    fs::write(out_dir.join("oci-layout"), br#"{"imageLayoutVersion":"1.0.0"}"#)
        .context("writing oci-layout")?;
    let index = serde_json::json!({
        "schemaVersion": 2,
        "manifests": [{
            "mediaType": OCI_MANIFEST_MEDIA_TYPE,
            "digest": format!("sha256:{manifest_digest_hex}"),
            "size": manifest_bytes.len(),
            "annotations": { "io.b00t.finetune.datum_id": job_id },
        }],
    });
    fs::write(
        out_dir.join("index.json"),
        serde_json::to_vec_pretty(&index).context("serializing index.json")?,
    )
    .context("writing index.json")?;

    Ok(OciLayerResult {
        layer_digest: format!("sha256:{layer_digest_hex}"),
        layer_size,
        layer_path,
        manifest_digest: format!("sha256:{manifest_digest_hex}"),
        manifest_path,
        oci_layout_dir: out_dir.to_path_buf(),
    })
}

fn sha256_hex(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    hex::encode(hasher.finalize())
}

/// Builds a deterministic tar+gzip byte stream from a directory: sorted file
/// order, zeroed mtime/uid/gid, fixed mode. Determinism matters here: this
/// is the "one OCI layer per AI datum version" contract — re-packing
/// byte-identical adapter files must always yield the identical layer
/// digest, or every re-run of this packer (with unchanged weights) would
/// mint a spurious new "version."
fn build_deterministic_tar_gz(dir: &Path) -> Result<Vec<u8>> {
    let mut entries: Vec<PathBuf> = walkdir::WalkDir::new(dir)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().is_file())
        .map(|e| e.path().to_path_buf())
        .collect();
    entries.sort();
    if entries.is_empty() {
        bail!("adapter directory {dir:?} contains no files to package");
    }

    let mut tar_bytes = Vec::new();
    {
        let mut builder = tar::Builder::new(&mut tar_bytes);
        for path in &entries {
            let rel = path
                .strip_prefix(dir)
                .context("computing relative path for tar entry")?;
            let metadata = fs::metadata(path)
                .with_context(|| format!("stat-ing {path:?} for tar entry"))?;
            let mut header = tar::Header::new_gnu();
            header.set_size(metadata.len());
            header.set_mode(0o644);
            header.set_mtime(0);
            header.set_uid(0);
            header.set_gid(0);
            let mut f =
                fs::File::open(path).with_context(|| format!("opening {path:?} for tar entry"))?;
            builder
                .append_data(&mut header, rel, &mut f)
                .with_context(|| format!("appending {path:?} to tar archive"))?;
        }
        builder.finish().context("finishing tar archive")?;
    }

    // GzBuilder's default mtime is 0 (derived `Default` on a `u32` field) —
    // no explicit `.mtime(0)` needed, but the plain `GzEncoder::new`
    // constructor delegates to `GzBuilder::new()` internally, so this is
    // deterministic by construction, not by accident left unverified.
    let mut encoder = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::default());
    std::io::Write::write_all(&mut encoder, &tar_bytes).context("gzip-compressing tar layer")?;
    encoder.finish().context("finalizing gzip stream")
}

// ── Storage push — SPIRE-federated, not root creds ──────────────────────────
//
// Follows infrastructure repo PR #189's just-landed pattern
// (`terraform/{azure,google}/backend.tf`): `profile = "spire-agent"` — an
// AWS CLI/SDK profile backed by a `credential_process` wrapper that exchanges
// a SPIRE JWT-SVID for short-lived STS credentials (infra#179/#181), never a
// root/static access key. That wrapper and the `~/.aws/config` profile entry
// live host-side (deliberately not committed to any repo, per infra#179) —
// this code only ever *names* the profile, exactly like the Terraform side
// does; it never reads/mints/caches a credential itself. Shelling out to the
// `aws` CLI (rather than pulling in `aws-sdk-s3`/`aws-config`) keeps this a
// thin wrapper consistent with how `provider.rs` already treats `hf`/
// `dstack` — a CLI already implements the full credential_process contract,
// so there's nothing this module could correctly do by re-implementing it in
// Rust.

pub const SPIRE_AGENT_PROFILE: &str = "spire-agent";
pub const DEFAULT_S3_REGION: &str = "ap-southeast-4";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PushReceipt {
    pub storage: String,
    pub location: String,
    pub mocked: bool,
    pub pushed_at: chrono::DateTime<chrono::Utc>,
}

/// Pure argv builder for `aws s3 cp --profile spire-agent`. Unit-testable
/// without the `aws` CLI installed or real credentials — same pattern as
/// `hf_jobs_run_args`/`local_train_args` above.
pub fn aws_s3_cp_args(
    local_path: &Path,
    bucket: &str,
    key: &str,
    profile: &str,
    region: &str,
) -> Vec<String> {
    vec![
        "s3".to_string(),
        "cp".to_string(),
        local_path.display().to_string(),
        format!("s3://{bucket}/{key}"),
        "--profile".to_string(),
        profile.to_string(),
        "--region".to_string(),
        region.to_string(),
    ]
}

/// Pushes `local_path` to S3-compatible storage.
///
/// `mock: true` (the default from `finetune run` unless `--live-push` is
/// passed) skips the `aws` CLI invocation entirely and returns a receipt
/// marked `mocked: true` — this sandbox has neither the `aws` CLI nor real
/// `spire-agent` credentials, and the task this function was built for
/// explicitly says not to attempt a real push here. The non-mock branch is
/// real, working code: it shells to `aws s3 cp`, wrapped in an exponential
/// backoff retry (per the design spec's "Storage push failure: retry with
/// backoff" guidance — reusing `backon`, already resolved transitively in
/// this workspace's `Cargo.lock`, rather than hand-rolling retry logic).
/// Each retry re-invokes the `aws` CLI from scratch, so a `spire-agent`
/// credential that expires mid-retry is simply re-resolved by the CLI's own
/// `credential_process` on the next attempt — nothing here caches a
/// credential across the retry loop, matching the spec's "re-fetch rather
/// than cache" guidance for short-lived creds.
pub fn push_to_s3(
    local_path: &Path,
    bucket: &str,
    key: &str,
    profile: &str,
    region: &str,
    mock: bool,
) -> Result<PushReceipt> {
    let location = format!("s3://{bucket}/{key}");
    if mock {
        return Ok(PushReceipt {
            storage: "s3".to_string(),
            location,
            mocked: true,
            pushed_at: chrono::Utc::now(),
        });
    }

    let args = aws_s3_cp_args(local_path, bucket, key, profile, region);
    (|| run_aws_cli(&args))
        .retry(ExponentialBuilder::default().with_max_times(3))
        .call()?;

    Ok(PushReceipt {
        storage: "s3".to_string(),
        location,
        mocked: false,
        pushed_at: chrono::Utc::now(),
    })
}

fn run_aws_cli(args: &[String]) -> Result<()> {
    let out = Command::new("aws").args(args).output().context(
        "aws CLI not found — install awscli and configure the 'spire-agent' profile \
         (credential_process wrapper; see infrastructure repo issues #179/#181) before using --live-push",
    )?;
    if !out.status.success() {
        bail!(
            "aws {} failed: {}",
            args.join(" "),
            String::from_utf8_lossy(&out.stderr)
        );
    }
    Ok(())
}

// ── AI datum registration ────────────────────────────────────────────────

/// Writes/updates `<output.datum>.ai.toml` in `datum_dir`, mirroring
/// `qwen36-peer.ai.toml`'s shape (`[b00t]` / `[b00t.ai]` header + a
/// `b00t:map v1` trailer comment block) plus a `[b00t.finetune]`/`[b00t.oci]`
/// section carrying this run's provenance and the new OCI digest — "one OCI
/// layer per AI datum version" is only a real contract if the datum file
/// actually points at the digest that was just pushed.
pub fn write_ai_datum(
    datum_dir: &Path,
    manifest: &FinetuneManifest,
    oci: &OciLayerResult,
    push: &PushReceipt,
) -> Result<PathBuf> {
    #[derive(Serialize)]
    struct B00t<'a> {
        name: &'a str,
        #[serde(rename = "type")]
        type_: &'a str,
        hint: String,
        ai: Ai<'a>,
        finetune: Finetune<'a>,
        oci: Oci<'a>,
    }
    #[derive(Serialize)]
    struct Ai<'a> {
        provider: &'a str,
        api_type: &'a str,
        models: Vec<&'a str>,
        base_model: &'a str,
    }
    #[derive(Serialize)]
    struct Finetune<'a> {
        job_id: &'a str,
        base_model: &'a str,
        framework: &'a str,
        dataset_source: Option<&'a str>,
        dataset_generated: bool,
        trained_at: String,
    }
    #[derive(Serialize)]
    struct Oci<'a> {
        layer_digest: &'a str,
        layer_size: u64,
        manifest_digest: &'a str,
        storage: &'a str,
        location: &'a str,
        mocked: bool,
        mirror_to_hf: bool,
    }
    #[derive(Serialize)]
    struct Root<'a> {
        b00t: B00t<'a>,
    }

    let storage_str = match manifest.output.storage {
        StorageKind::S3 => "s3",
        StorageKind::Zerofs => "zerofs",
    };
    let root = Root {
        b00t: B00t {
            name: &manifest.output.datum,
            type_: "ai",
            hint: format!(
                "Fine-tuned {} LoRA adapter for {} (job {}) — {} storage{}",
                manifest.job.framework,
                manifest.job.base_model,
                manifest.job.id,
                storage_str,
                if push.mocked { ", MOCKED push (no real credentials in this environment)" } else { "" }
            ),
            ai: Ai {
                provider: "oci-artifact",
                api_type: "lora-adapter",
                models: vec![manifest.job.id.as_str()],
                base_model: &manifest.job.base_model,
            },
            finetune: Finetune {
                job_id: &manifest.job.id,
                base_model: &manifest.job.base_model,
                framework: &manifest.job.framework,
                dataset_source: manifest.dataset.source.as_deref(),
                dataset_generated: manifest.dataset.generate,
                trained_at: chrono::Utc::now().to_rfc3339(),
            },
            oci: Oci {
                layer_digest: &oci.layer_digest,
                layer_size: oci.layer_size,
                manifest_digest: &oci.manifest_digest,
                storage: storage_str,
                location: &push.location,
                mocked: push.mocked,
                mirror_to_hf: manifest.output.mirror_to_hf,
            },
        },
    };

    let mut body = toml::to_string_pretty(&root).context("serializing AI datum TOML")?;
    body.push_str(&format!(
        "\n# b00t:map v1\n\
         # summary: Fine-tuned {framework} LoRA adapter for {base_model} — job {job_id}\n\
         # tags: ai, finetune, lora, {framework}, oci-artifact\n\
         # tier: frontier\n\
         # cmds: b00t-cli finetune run <manifest>, b00t-cli finetune pack --adapter-dir <dir>\n\
         # complexity: 5\n",
        framework = manifest.job.framework,
        base_model = manifest.job.base_model,
        job_id = manifest.job.id,
    ));

    fs::create_dir_all(datum_dir).context("creating datum dir")?;
    let path = datum_dir.join(format!("{}.ai.toml", manifest.output.datum));
    fs::write(&path, body).with_context(|| format!("writing AI datum {path:?}"))?;
    Ok(path)
}

// ── Orchestration ────────────────────────────────────────────────────────

fn run_cmd_in(dir: &Path, bin: &str, args: &[&str]) -> Result<()> {
    let status = Command::new(bin)
        .args(args)
        .current_dir(dir)
        .status()
        .with_context(|| format!("failed to spawn `{bin} {}` in {dir:?}", args.join(" ")))?;
    if !status.success() {
        bail!("`{bin} {}` exited with {status}", args.join(" "));
    }
    Ok(())
}

/// End-to-end: dispatch training per `[target].kind`, package the resulting
/// adapter as an OCI layer, push it, and register the AI datum.
///
/// Not exercised by this crate's automated tests — it shells out to `just`,
/// `hf`, and (for the local path) `crate::hive::activate_profile`, none of
/// which are available/safe to run in a GPU-less, credential-less sandbox.
/// The pure pieces it calls (`generate_training_config_yaml`,
/// `local_train_args`, `hf_jobs_run_args`, `pack_adapter_as_oci_layer`,
/// `aws_s3_cp_args`, `write_ai_datum`) are unit-tested directly instead —
/// the same dependency-injection-free testing style `provider.rs` already
/// uses for `hf_batch_args`/`local_batch_args`/`dstack_task_yaml`.
pub async fn run_job(
    manifest_path: &Path,
    datum_dir: &Path,
    repo_root: &Path,
    dry_run: bool,
    live_push: bool,
) -> Result<()> {
    let manifest = FinetuneManifest::from_file(manifest_path)?;
    println!(
        "→ finetune job '{}' ({:?}) base_model={}",
        manifest.job.id, manifest.target.kind, manifest.job.base_model
    );

    if manifest.dataset.generate {
        println!("  dataset: generating via fine-tune/generate_dataset.py");
        if !dry_run {
            run_cmd_in(
                repo_root,
                "uv",
                &["run", "python3", "fine-tune/generate_dataset.py"],
            )?;
        }
    }

    let (dataset_path, output_dir) = match manifest.target.kind {
        TargetKind::Local => (
            "fine-tune/train.jsonl".to_string(),
            format!("fine-tune/output-{}", manifest.job.id),
        ),
        TargetKind::Cloud => ("/data/train.jsonl".to_string(), "/tmp/output".to_string()),
    };
    let yaml = generate_training_config_yaml(&manifest, &dataset_path, &output_dir)?;
    let config_filename = format!("config-{}.yaml", manifest.job.id);
    let config_path = repo_root.join("fine-tune").join(&config_filename);
    if let Some(parent) = config_path.parent() {
        fs::create_dir_all(parent).context("creating fine-tune/ dir")?;
    }
    fs::write(&config_path, &yaml).context("writing generated training config")?;
    println!("  config: {}", config_path.display());

    match manifest.target.kind {
        TargetKind::Local => {
            let profile_name = manifest.hive_profile_name();
            let profile = crate::hive::load_profile(profile_name, datum_dir)
                .with_context(|| format!("loading hive profile '{profile_name}'"))?;
            let snapshot = crate::hive::SystemSnapshot::capture()?;
            println!("  {}", snapshot.summary_line());
            let gate_issues = snapshot.satisfies_gate(&profile);
            if !gate_issues.is_empty() {
                bail!(
                    "GPU/RAM gate failed before activation:\n  {}",
                    gate_issues.join("\n  ")
                );
            }
            if !dry_run {
                let log = crate::hive::activate_profile(&profile, &snapshot, false, false)?;
                for l in &log {
                    println!("  {l}");
                }
                // Re-check right before launch — mitigates the gate-then-race
                // condition the design spec calls out explicitly: something
                // else could have claimed the GPU between the gate check
                // above and activation finishing.
                let post_snapshot = crate::hive::SystemSnapshot::capture()?;
                let race_issues = post_snapshot.satisfies_gate(&profile);
                if !race_issues.is_empty() {
                    bail!(
                        "GPU/RAM gate failed right before launch (raced with another process):\n  {}",
                        race_issues.join("\n  ")
                    );
                }
            }
            let justfile_path = repo_root
                .join("_b00t_")
                .join("ai-finetune.just")
                .display()
                .to_string();
            let config_arg = Path::new("fine-tune").join(&config_filename);
            let args = local_train_args(&justfile_path, &config_arg.display().to_string());
            println!("  $ just {}", args.join(" "));
            if !dry_run {
                let arg_refs: Vec<&str> = args.iter().map(String::as_str).collect();
                run_cmd_in(repo_root, "just", &arg_refs)?;
            } else {
                println!("  [dry-run] stopping before OCI packaging/push");
                return Ok(());
            }
        }
        TargetKind::Cloud => {
            println!(
                "  $ hf upload elasticdotventures/b00t-training fine-tune/{config_filename} {config_filename} --type dataset --private"
            );
            if !dry_run {
                run_cmd_in(
                    repo_root,
                    "hf",
                    &[
                        "upload",
                        "elasticdotventures/b00t-training",
                        &format!("fine-tune/{config_filename}"),
                        &config_filename,
                        "--type",
                        "dataset",
                        "--private",
                    ],
                )?;
            }
            let args = hf_jobs_run_args(&manifest, &config_filename)?;
            println!("  $ hf {}", args.join(" "));
            if dry_run {
                println!("  [dry-run] stopping before job submission");
                return Ok(());
            }
            let arg_refs: Vec<&str> = args.iter().map(String::as_str).collect();
            run_cmd_in(repo_root, "hf", &arg_refs)?;
            println!(
                "  ⏳ cloud job submitted (detached) — poll via: just ai-finetune::hf-job-status <id>"
            );
            println!(
                "     cloud jobs are async: OCI packaging + storage push happen in a follow-up pass"
            );
            println!(
                "     once the adapter is pulled locally (just ai-finetune::hf-adapter-pull <hub_model_id> <dir>),"
            );
            println!("     then: b00t-cli finetune pack --adapter-dir <dir> --out .b00t-oci/{} --job-id {} --base-model {} --framework {}", manifest.output.datum, manifest.job.id, manifest.job.base_model, manifest.job.framework);
            return Ok(());
        }
    }

    // Reachable only for the synchronous local path.
    let adapter_dir = repo_root.join(&output_dir).join("lora-adapter");
    if !adapter_dir.is_dir() {
        bail!("expected adapter directory not found after training: {adapter_dir:?}");
    }
    let oci_out = repo_root.join(".b00t-oci").join(&manifest.output.datum);
    let oci_result = pack_adapter_as_oci_layer(
        &adapter_dir,
        &oci_out,
        &manifest.job.id,
        &manifest.job.base_model,
        &manifest.job.framework,
    )?;
    println!("  OCI layer digest: {}", oci_result.layer_digest);

    let push = match manifest.output.storage {
        StorageKind::S3 => {
            let bucket = manifest
                .output
                .s3_bucket
                .as_deref()
                .context("[output].s3_bucket required for storage = \"s3\"")?;
            let key = format!(
                "{}/{}.tar.gz",
                manifest.output.datum,
                oci_result.layer_digest.trim_start_matches("sha256:")
            );
            push_to_s3(
                &oci_result.layer_path,
                bucket,
                &key,
                SPIRE_AGENT_PROFILE,
                DEFAULT_S3_REGION,
                !live_push,
            )?
        }
        StorageKind::Zerofs => {
            bail!(
                "storage = \"zerofs\" requires the infrastructure#192 ZeroFS container mount \
                 (not yet build-verified) — use storage = \"s3\" for now"
            );
        }
    };
    println!(
        "  pushed ({}): {}",
        if push.mocked { "MOCKED" } else { "live" },
        push.location
    );

    let datum_path = write_ai_datum(datum_dir, &manifest, &oci_result, &push)?;
    println!("  ai datum: {}", datum_path.display());

    Ok(())
}

// ── CLI surface ──────────────────────────────────────────────────────────

#[derive(Parser)]
pub enum FinetuneCommands {
    #[clap(about = "Validate a job manifest TOML without running anything")]
    Validate {
        #[clap(help = "Path to the job manifest TOML file")]
        manifest: PathBuf,
    },
    #[clap(
        about = "Package an existing LoRA adapter directory as a single OCI layer (no training)"
    )]
    Pack {
        #[clap(long, help = "Directory containing adapter_model.safetensors etc.")]
        adapter_dir: PathBuf,
        #[clap(long, help = "Directory to write the OCI Image Layout into")]
        out: PathBuf,
        #[clap(long)]
        job_id: String,
        #[clap(long)]
        base_model: String,
        #[clap(long, default_value = "unsloth")]
        framework: String,
    },
    #[clap(
        about = "Run a fine-tuning job manifest end-to-end: dispatch training, pack OCI layer, push to storage, register AI datum"
    )]
    Run {
        #[clap(help = "Path to the job manifest TOML file")]
        manifest: PathBuf,
        #[clap(long, help = "Print the commands that would run without executing them")]
        dry_run: bool,
        #[clap(
            long,
            help = "Actually push to S3 instead of mocking (requires the aws CLI + a configured 'spire-agent' profile)"
        )]
        live_push: bool,
    },
    #[clap(
        about = "Sub-project C: dispatch two competing job manifests (e.g. local vs cloud) concurrently — see `race-finalize` for comparing results once both finish"
    )]
    Race {
        #[clap(help = "Path to competitor A's job manifest TOML")]
        manifest_a: PathBuf,
        #[clap(help = "Path to competitor B's job manifest TOML")]
        manifest_b: PathBuf,
        #[clap(long, help = "Human-readable label for competitor A (default: competitor-a)")]
        label_a: Option<String>,
        #[clap(long, help = "Human-readable label for competitor B (default: competitor-b)")]
        label_b: Option<String>,
        #[clap(long, help = "Print the commands that would run without executing them")]
        dry_run: bool,
        #[clap(long, help = "Actually push to S3 instead of mocking")]
        live_push: bool,
    },
    #[clap(
        about = "Sub-project C: compare two completed race competitors by final training loss, register the winner as the canonical AI datum"
    )]
    RaceFinalize {
        #[clap(long, help = "Path to competitor A's job manifest TOML")]
        manifest_a: PathBuf,
        #[clap(long, help = "Human-readable label for competitor A (default: competitor-a)")]
        label_a: Option<String>,
        #[clap(long, help = "Path to competitor A's training log (stdout capture or trainer_state.json)")]
        log_a: PathBuf,
        #[clap(long, help = "Path to competitor A's trained LoRA adapter directory")]
        adapter_dir_a: PathBuf,
        #[clap(long, help = "Path to competitor B's job manifest TOML")]
        manifest_b: PathBuf,
        #[clap(long, help = "Human-readable label for competitor B (default: competitor-b)")]
        label_b: Option<String>,
        #[clap(long, help = "Path to competitor B's training log (stdout capture or trainer_state.json)")]
        log_b: PathBuf,
        #[clap(long, help = "Path to competitor B's trained LoRA adapter directory")]
        adapter_dir_b: PathBuf,
        #[clap(long, help = "Directory to write the OCI layer + race report into (default: .b00t-race)")]
        race_out_dir: Option<PathBuf>,
        #[clap(long, help = "Actually push the winner to S3 instead of mocking")]
        live_push: bool,
    },
}

pub async fn handle_finetune_command(cmd: &FinetuneCommands, path: &str) -> Result<()> {
    let datum_dir = PathBuf::from(shellexpand::tilde(path).to_string());
    let repo_root = datum_dir.parent().unwrap_or(&datum_dir).to_path_buf();

    match cmd {
        FinetuneCommands::Validate { manifest } => {
            let m = FinetuneManifest::from_file(manifest)?;
            println!(
                "✓ manifest valid: job.id={} framework={} target.kind={:?} output.datum={} storage={:?}",
                m.job.id, m.job.framework, m.target.kind, m.output.datum, m.output.storage
            );
            Ok(())
        }
        FinetuneCommands::Pack {
            adapter_dir,
            out,
            job_id,
            base_model,
            framework,
        } => {
            if !oras_available() {
                println!(
                    "ℹ oras not found on PATH — using the custom OCI packer (see module docs for why oras doesn't apply to S3 transport anyway)"
                );
            }
            let result = pack_adapter_as_oci_layer(adapter_dir, out, job_id, base_model, framework)?;
            println!("layer digest:    {}", result.layer_digest);
            println!("layer size:      {} bytes", result.layer_size);
            println!("manifest digest: {}", result.manifest_digest);
            println!("oci layout:      {}", result.oci_layout_dir.display());
            Ok(())
        }
        FinetuneCommands::Run {
            manifest,
            dry_run,
            live_push,
        } => run_job(manifest, &datum_dir, &repo_root, *dry_run, *live_push).await,
        FinetuneCommands::Race {
            manifest_a,
            manifest_b,
            label_a,
            label_b,
            dry_run,
            live_push,
        } => {
            crate::commands::finetune_race::handle_race_dispatch(
                manifest_a,
                manifest_b,
                label_a.clone(),
                label_b.clone(),
                &datum_dir,
                &repo_root,
                *dry_run,
                *live_push,
            )
            .await
        }
        FinetuneCommands::RaceFinalize {
            manifest_a,
            label_a,
            log_a,
            adapter_dir_a,
            manifest_b,
            label_b,
            log_b,
            adapter_dir_b,
            race_out_dir,
            live_push,
        } => {
            let race_out = race_out_dir
                .clone()
                .unwrap_or_else(|| repo_root.join(".b00t-race"));
            crate::commands::finetune_race::handle_race_finalize(
                manifest_a,
                label_a.clone(),
                log_a,
                adapter_dir_a,
                manifest_b,
                label_b.clone(),
                log_b,
                adapter_dir_b,
                &datum_dir,
                &race_out,
                *live_push,
            )
        }
    }
}

// ── Tests ──────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_local_toml() -> &'static str {
        r#"
[job]
id = "qwen38-peer-2026-09-01"
base_model = "unsloth/Qwen3.8-27B-unsloth-bnb-4bit"
framework = "unsloth"

[dataset]
source = "hf://datasets/elasticdotventures/b00t-training"

[target]
kind = "local"
hive_profile = "finetune"

[output]
datum = "qwen38-peer"
storage = "s3"
s3_bucket = "b00t-finetune-artifacts"
mirror_to_hf = true
"#
    }

    fn sample_cloud_toml() -> &'static str {
        r#"
[job]
id = "qwen38-coder-2026-09-01"
base_model = "Qwen/Qwen3-Coder-30B-A3B-Instruct"

[dataset]
generate = true

[target]
kind = "cloud"
image = "ghcr.io/elasticdotventures/b00t-training-image:latest"
flavor = "a100-large"
timeout_hours = 10.0

[output]
datum = "qwen38-coder"
storage = "s3"
s3_bucket = "b00t-finetune-artifacts"
"#
    }

    // ── manifest parsing / validation ───────────────────────────────────

    #[test]
    fn parses_local_manifest_with_defaults() {
        let m = FinetuneManifest::from_toml_str(sample_local_toml()).expect("valid manifest");
        assert_eq!(m.job.id, "qwen38-peer-2026-09-01");
        assert_eq!(m.job.framework, "unsloth");
        assert_eq!(m.target.kind, TargetKind::Local);
        assert_eq!(m.hive_profile_name(), "finetune");
        assert!(m.output.mirror_to_hf);
        assert_eq!(m.target.timeout_hours, 10.0); // default
    }

    #[test]
    fn parses_cloud_manifest_generate_dataset() {
        let m = FinetuneManifest::from_toml_str(sample_cloud_toml()).expect("valid manifest");
        assert_eq!(m.target.kind, TargetKind::Cloud);
        assert!(m.dataset.generate);
        assert_eq!(m.target.image.as_deref(), Some("ghcr.io/elasticdotventures/b00t-training-image:latest"));
        // mirror_to_hf omitted → defaults true
        assert!(m.output.mirror_to_hf);
    }

    #[test]
    fn rejects_empty_job_id() {
        let mut m = FinetuneManifest::from_toml_str(sample_local_toml()).unwrap();
        m.job.id = "  ".to_string();
        let err = m.validate().unwrap_err().to_string();
        assert!(err.contains("[job].id"));
    }

    #[test]
    fn rejects_unimplemented_framework() {
        let mut m = FinetuneManifest::from_toml_str(sample_local_toml()).unwrap();
        m.job.framework = "axolotl".to_string();
        let err = m.validate().unwrap_err().to_string();
        assert!(err.contains("axolotl") || err.contains("unsloth"));
    }

    #[test]
    fn rejects_missing_dataset_source_and_generate() {
        let mut m = FinetuneManifest::from_toml_str(sample_local_toml()).unwrap();
        m.dataset.source = None;
        m.dataset.generate = false;
        let err = m.validate().unwrap_err().to_string();
        assert!(err.contains("[dataset]"));
    }

    #[test]
    fn rejects_cloud_target_missing_image_or_flavor() {
        let mut m = FinetuneManifest::from_toml_str(sample_cloud_toml()).unwrap();
        m.target.image = None;
        let err = m.validate().unwrap_err().to_string();
        assert!(err.contains("image"));

        let mut m2 = FinetuneManifest::from_toml_str(sample_cloud_toml()).unwrap();
        m2.target.flavor = None;
        let err2 = m2.validate().unwrap_err().to_string();
        assert!(err2.contains("flavor"));
    }

    #[test]
    fn rejects_s3_storage_without_bucket() {
        let mut m = FinetuneManifest::from_toml_str(sample_local_toml()).unwrap();
        m.output.s3_bucket = None;
        let err = m.validate().unwrap_err().to_string();
        assert!(err.contains("s3_bucket"));
    }

    #[test]
    fn zerofs_storage_does_not_require_bucket() {
        let mut m = FinetuneManifest::from_toml_str(sample_local_toml()).unwrap();
        m.output.storage = StorageKind::Zerofs;
        m.output.s3_bucket = None;
        assert!(m.validate().is_ok());
    }

    // ── training config YAML generation ─────────────────────────────────

    #[test]
    fn generates_training_config_with_expected_fields() {
        let m = FinetuneManifest::from_toml_str(sample_local_toml()).unwrap();
        let yaml = generate_training_config_yaml(&m, "fine-tune/train.jsonl", "fine-tune/output-x").unwrap();
        let parsed: serde_yaml::Value = serde_yaml::from_str(&yaml).unwrap();
        assert_eq!(
            parsed["base_model"].as_str().unwrap(),
            "unsloth/Qwen3.8-27B-unsloth-bnb-4bit"
        );
        assert_eq!(parsed["adapter_name"].as_str().unwrap(), "qwen38-peer-2026-09-01");
        assert_eq!(parsed["dataset"].as_str().unwrap(), "fine-tune/train.jsonl");
        assert_eq!(parsed["hub_model_id"].as_str().unwrap(), "elasticdotventures/qwen38-peer");
        assert!(parsed["push_to_hub"].as_bool().unwrap());
        assert_eq!(parsed["lora_r"].as_u64().unwrap(), 16);
    }

    #[test]
    fn generated_config_omits_hub_fields_when_mirror_disabled() {
        let mut m = FinetuneManifest::from_toml_str(sample_local_toml()).unwrap();
        m.output.mirror_to_hf = false;
        let yaml = generate_training_config_yaml(&m, "d.jsonl", "out").unwrap();
        let parsed: serde_yaml::Value = serde_yaml::from_str(&yaml).unwrap();
        assert!(parsed.get("hub_model_id").is_none());
        assert!(parsed.get("push_to_hub").is_none());
    }

    // ── argv builders (pure, no live tools required) ────────────────────

    #[test]
    fn local_train_args_matches_recipe_shape() {
        let args = local_train_args("_b00t_/ai-finetune.just", "fine-tune/config-x.yaml");
        assert_eq!(
            args,
            vec!["-f", "_b00t_/ai-finetune.just", "ai-finetune::train", "fine-tune/config-x.yaml"]
        );
    }

    #[test]
    fn hf_jobs_run_args_matches_cloud_train_recipe_shape() {
        let m = FinetuneManifest::from_toml_str(sample_cloud_toml()).unwrap();
        let args = hf_jobs_run_args(&m, "config-qwen38-coder-2026-09-01.yaml").unwrap();
        assert_eq!(args[0], "jobs");
        assert_eq!(args[1], "run");
        assert_eq!(args[2], "ghcr.io/elasticdotventures/b00t-training-image:latest");
        assert!(args.contains(&"--flavor".to_string()));
        assert!(args.contains(&"a100-large".to_string()));
        assert!(args.contains(&"--timeout".to_string()));
        assert!(args.contains(&"10h".to_string()));
        assert!(args.contains(&"--secrets".to_string()));
        assert!(args.contains(&"HF_TOKEN".to_string()));
        assert!(args.iter().any(|a| a.contains("hf://datasets/elasticdotventures/b00t-training:/data:ro")));
        assert!(args.iter().any(|a| a.contains("hf://buckets/elasticdotventures/b00t-adapters:/adapters:rw")));
        let sh_cmd = args.last().unwrap();
        assert!(sh_cmd.contains("ai-finetune.just train /data/config-qwen38-coder-2026-09-01.yaml"));
    }

    #[test]
    fn hf_jobs_run_args_requires_image_and_flavor() {
        let mut m = FinetuneManifest::from_toml_str(sample_cloud_toml()).unwrap();
        m.target.image = None;
        assert!(hf_jobs_run_args(&m, "c.yaml").is_err());
    }

    #[test]
    fn aws_s3_cp_args_uses_spire_agent_profile_by_default() {
        let args = aws_s3_cp_args(
            Path::new("/tmp/layer.tar.gz"),
            "b00t-finetune-artifacts",
            "qwen38-peer/abc123.tar.gz",
            SPIRE_AGENT_PROFILE,
            DEFAULT_S3_REGION,
        );
        assert_eq!(args[0], "s3");
        assert_eq!(args[1], "cp");
        assert_eq!(args[3], "s3://b00t-finetune-artifacts/qwen38-peer/abc123.tar.gz");
        assert_eq!(args[4], "--profile");
        assert_eq!(args[5], "spire-agent");
        assert_eq!(args[6], "--region");
        assert_eq!(args[7], "ap-southeast-4");
    }

    #[test]
    fn push_to_s3_mock_never_shells_out() {
        // If this accidentally invoked the real `aws` CLI (not installed in
        // this sandbox) it would return Err, not a mocked receipt.
        let receipt = push_to_s3(
            Path::new("/tmp/does-not-exist.tar.gz"),
            "some-bucket",
            "some/key.tar.gz",
            SPIRE_AGENT_PROFILE,
            DEFAULT_S3_REGION,
            true,
        )
        .expect("mock push must not touch the aws CLI");
        assert!(receipt.mocked);
        assert_eq!(receipt.location, "s3://some-bucket/some/key.tar.gz");
    }

    #[test]
    fn push_to_s3_live_without_aws_cli_fails_clearly() {
        // Sandbox has no `aws` CLI — confirms the "wired but not faked" claim:
        // a live push attempt fails loudly rather than silently succeeding.
        let result = push_to_s3(
            Path::new("/tmp/does-not-exist.tar.gz"),
            "some-bucket",
            "some/key.tar.gz",
            SPIRE_AGENT_PROFILE,
            DEFAULT_S3_REGION,
            false,
        );
        assert!(result.is_err());
    }

    // ── OCI layer packaging + digest determinism ────────────────────────

    fn make_fake_adapter_dir() -> tempfile::TempDir {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("adapter_model.safetensors"), b"fake-safetensors-bytes").unwrap();
        fs::write(
            dir.path().join("adapter_config.json"),
            br#"{"r":16,"lora_alpha":32}"#,
        )
        .unwrap();
        fs::write(dir.path().join("tokenizer.json"), b"{}").unwrap();
        dir
    }

    #[test]
    fn packs_adapter_and_produces_valid_oci_layout() {
        let adapter = make_fake_adapter_dir();
        let out = tempfile::tempdir().unwrap();
        let result = pack_adapter_as_oci_layer(
            adapter.path(),
            out.path(),
            "qwen38-peer-2026-09-01",
            "unsloth/Qwen3.8-27B-unsloth-bnb-4bit",
            "unsloth",
        )
        .expect("packing should succeed");

        assert!(result.layer_digest.starts_with("sha256:"));
        assert!(result.manifest_digest.starts_with("sha256:"));
        assert!(result.layer_path.is_file());
        assert!(result.manifest_path.is_file());
        assert!(out.path().join("oci-layout").is_file());
        assert!(out.path().join("index.json").is_file());

        let manifest_bytes = fs::read(&result.manifest_path).unwrap();
        let manifest: serde_json::Value = serde_json::from_slice(&manifest_bytes).unwrap();
        assert_eq!(manifest["schemaVersion"], 2);
        assert_eq!(manifest["artifactType"], B00T_FINETUNE_ARTIFACT_TYPE);
        assert_eq!(
            manifest["layers"][0]["digest"].as_str().unwrap(),
            result.layer_digest
        );
        assert_eq!(
            manifest["annotations"]["io.b00t.finetune.job_id"],
            "qwen38-peer-2026-09-01"
        );
    }

    #[test]
    fn oci_layer_digest_is_deterministic_across_repacks() {
        let adapter = make_fake_adapter_dir();
        let out1 = tempfile::tempdir().unwrap();
        let out2 = tempfile::tempdir().unwrap();

        let r1 = pack_adapter_as_oci_layer(adapter.path(), out1.path(), "job-a", "base-a", "unsloth")
            .unwrap();
        // Sleep is unnecessary — determinism must not depend on wall-clock
        // gaps between the two packing calls.
        let r2 = pack_adapter_as_oci_layer(adapter.path(), out2.path(), "job-a", "base-a", "unsloth")
            .unwrap();

        assert_eq!(
            r1.layer_digest, r2.layer_digest,
            "identical adapter files must yield identical layer digests"
        );
    }

    #[test]
    fn oci_layer_digest_changes_when_adapter_content_changes() {
        let adapter = make_fake_adapter_dir();
        let out1 = tempfile::tempdir().unwrap();
        let r1 = pack_adapter_as_oci_layer(adapter.path(), out1.path(), "job-a", "base-a", "unsloth")
            .unwrap();

        fs::write(
            adapter.path().join("adapter_model.safetensors"),
            b"different-bytes-now",
        )
        .unwrap();
        let out2 = tempfile::tempdir().unwrap();
        let r2 = pack_adapter_as_oci_layer(adapter.path(), out2.path(), "job-a", "base-a", "unsloth")
            .unwrap();

        assert_ne!(r1.layer_digest, r2.layer_digest);
    }

    #[test]
    fn pack_rejects_missing_adapter_dir() {
        let out = tempfile::tempdir().unwrap();
        let result = pack_adapter_as_oci_layer(
            Path::new("/definitely/does/not/exist"),
            out.path(),
            "job-a",
            "base-a",
            "unsloth",
        );
        assert!(result.is_err());
    }

    #[test]
    fn pack_rejects_empty_adapter_dir() {
        let adapter = tempfile::tempdir().unwrap();
        let out = tempfile::tempdir().unwrap();
        let result = pack_adapter_as_oci_layer(adapter.path(), out.path(), "job-a", "base-a", "unsloth");
        assert!(result.is_err());
    }

    // ── AI datum registration ────────────────────────────────────────────

    #[test]
    fn writes_ai_datum_matching_expected_shape() {
        let manifest = FinetuneManifest::from_toml_str(sample_local_toml()).unwrap();
        let oci = OciLayerResult {
            layer_digest: "sha256:deadbeef".to_string(),
            layer_size: 1234,
            layer_path: PathBuf::from("/tmp/layer.tar.gz"),
            manifest_digest: "sha256:cafef00d".to_string(),
            manifest_path: PathBuf::from("/tmp/manifest.json"),
            oci_layout_dir: PathBuf::from("/tmp/oci"),
        };
        let push = PushReceipt {
            storage: "s3".to_string(),
            location: "s3://b00t-finetune-artifacts/qwen38-peer/deadbeef.tar.gz".to_string(),
            mocked: true,
            pushed_at: chrono::Utc::now(),
        };

        let datum_dir = tempfile::tempdir().unwrap();
        let path = write_ai_datum(datum_dir.path(), &manifest, &oci, &push).unwrap();
        assert_eq!(path.file_name().unwrap(), "qwen38-peer.ai.toml");

        let content = fs::read_to_string(&path).unwrap();
        let parsed: toml::Value = toml::from_str(&content).unwrap();
        assert_eq!(parsed["b00t"]["name"].as_str().unwrap(), "qwen38-peer");
        assert_eq!(parsed["b00t"]["type"].as_str().unwrap(), "ai");
        assert_eq!(
            parsed["b00t"]["oci"]["layer_digest"].as_str().unwrap(),
            "sha256:deadbeef"
        );
        assert!(parsed["b00t"]["oci"]["mocked"].as_bool().unwrap());
        assert!(content.contains("# b00t:map v1"));
        assert!(content.contains("# tags:"));
    }

    #[test]
    fn write_ai_datum_creates_missing_datum_dir() {
        let manifest = FinetuneManifest::from_toml_str(sample_local_toml()).unwrap();
        let oci = OciLayerResult {
            layer_digest: "sha256:aa".to_string(),
            layer_size: 1,
            layer_path: PathBuf::from("/tmp/l"),
            manifest_digest: "sha256:bb".to_string(),
            manifest_path: PathBuf::from("/tmp/m"),
            oci_layout_dir: PathBuf::from("/tmp/o"),
        };
        let push = PushReceipt {
            storage: "s3".to_string(),
            location: "s3://x/y".to_string(),
            mocked: true,
            pushed_at: chrono::Utc::now(),
        };
        let base = tempfile::tempdir().unwrap();
        let nested = base.path().join("nested").join("datums");
        let path = write_ai_datum(&nested, &manifest, &oci, &push).unwrap();
        assert!(path.is_file());
    }
}
