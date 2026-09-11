//! `b00t graph` — emit the datum + identity/authz graph as SPO triples (SP5-06).
//!
//! The bridge between the triple *compilers* (`datum_triples`, `identity_triples`,
//! this crate) and the `store-oxigraph` substrate in `b00t-c0re-lib`, which
//! only builds `--no-default-features --features store-oxigraph`. This
//! subcommand runs in the normal `b00t-cli` build and writes JSONL that the
//! `b00t-c0re-lib` `b00t-graph` example bin consumes.

use anyhow::{Context, Result};
use clap::Subcommand;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::io::Write;
use std::path::Path;

use crate::commands::finetune_job::{
    DEFAULT_S3_REGION, SPIRE_AGENT_PROFILE, push_to_s3,
};
use crate::datum_agent_profile::{GraphArtifactManifest, Signable, datum_signing_key_pem};

#[derive(Subcommand, Debug)]
pub enum GraphCommands {
    /// Emit the composed datum + identity/authz graph as JSONL `[s,p,o]`.
    EmitTriples {
        /// Tenant overlay to include (default: base tree only).
        #[arg(long)]
        tenant: Option<String>,
        /// Output file (default: stdout).
        #[arg(long)]
        out: Option<String>,
    },
    /// Sign a KerML view and publish it (+ a manifest) to S3, with a
    /// best-effort NATS reindex hint. See SP5 continuation:
    /// docs/superpowers/specs/2026-09-11-sp5-graph-artifact-publish-design.md.
    Publish {
        /// Path to the KerML view file to publish.
        #[arg(long)]
        kerml_view: String,
        /// Git tag this artifact corresponds to.
        #[arg(long)]
        tag: String,
        /// Git commit SHA this artifact corresponds to.
        #[arg(long)]
        commit_sha: String,
        /// Signing kid (key id) — same namespace as SP1's JWT key.
        #[arg(long)]
        kid: String,
        /// S3 bucket to publish to.
        #[arg(long)]
        bucket: String,
        /// S3 region (default: same as finetune_job's DEFAULT_S3_REGION).
        #[arg(long)]
        region: Option<String>,
        /// aws CLI profile (default: same as finetune_job's SPIRE_AGENT_PROFILE).
        #[arg(long)]
        profile: Option<String>,
        /// NATS server URL for the best-effort reindex hint.
        #[arg(long, env = "NATS_URL")]
        nats_url: Option<String>,
        /// Skip the actual S3 push and NATS publish (dry run).
        #[arg(long)]
        mock: bool,
    },
}

pub fn execute(cmd: &GraphCommands, b00t_path: &str) -> Result<()> {
    match cmd {
        GraphCommands::EmitTriples { tenant, out } => {
            let mut triples = crate::datum_triples::compile_datum_triples(b00t_path)?;
            triples.extend(crate::identity_triples::compile_identity_triples(
                b00t_path,
                tenant.as_deref(),
            )?);
            triples.sort();
            triples.dedup();

            let mut sink: Box<dyn Write> = match out {
                Some(p) => Box::new(std::fs::File::create(p)?),
                None => Box::new(std::io::stdout()),
            };
            for (s, p, o) in &triples {
                writeln!(sink, "{}", serde_json::to_string(&[s, p, o])?)?;
            }
            Ok(())
        }
        GraphCommands::Publish { .. } => {
            anyhow::bail!(
                "graph publish is async — call publish() directly (see main.rs's Graph dispatch, execute_async)"
            )
        }
    }
}

fn sha256_hex_file(path: &Path) -> Result<String> {
    let bytes = std::fs::read(path).with_context(|| format!("read {}", path.display()))?;
    let digest = Sha256::digest(&bytes);
    let mut s = String::with_capacity(64);
    for b in digest {
        s.push_str(&format!("{b:02x}"));
    }
    Ok(s)
}

/// Sign a KerML view file and publish it (+ its manifest) to S3, with a
/// best-effort NATS reindex hint on `b00t.graph.reindex`.
///
/// Publish order is strict and matches the design doc: per-tag objects
/// first (kerml-view + manifest), then the `latest` pointer overwritten
/// only on success — so `latest` never references a half-published tag.
/// The NATS publish is a direct, one-shot `async_nats` connect+publish —
/// deliberately NOT routed through `NatsMeshNode`/`b00t-comms` (still
/// unbuilt, gh#1299/task#210) — and its failure is logged, not fatal: the
/// S3 objects are already durably stored by that point.
#[allow(clippy::too_many_arguments)]
pub async fn publish(
    kerml_view: &Path,
    tag: &str,
    commit_sha: &str,
    kid: &str,
    bucket: &str,
    region: Option<&str>,
    profile: Option<&str>,
    nats_url: Option<&str>,
    mock: bool,
) -> Result<()> {
    let region = region.unwrap_or(DEFAULT_S3_REGION);
    let profile = profile.unwrap_or(SPIRE_AGENT_PROFILE);
    let nats_url = nats_url.unwrap_or("nats://localhost:4222");

    let kerml_digest = sha256_hex_file(kerml_view)?;
    let mut manifest = GraphArtifactManifest {
        tag: tag.to_string(),
        commit_sha: commit_sha.to_string(),
        kerml_digest,
        iso_ir_digest: String::new(), // no iso_ir export exists yet (see workflow note)
        signature: None,
    };
    let signing_key = datum_signing_key_pem()?;
    manifest.sign(kid, &signing_key)?;

    let manifest_path = std::env::temp_dir().join(format!("graph-manifest-{tag}.json"));
    std::fs::write(
        &manifest_path,
        serde_json::to_vec_pretty(&manifest).context("serialize manifest")?,
    )
    .with_context(|| format!("write {}", manifest_path.display()))?;

    let kerml_key = format!("graph/tags/{tag}/kerml-view.kerml");
    let manifest_key = format!("graph/tags/{tag}/manifest.json");
    let latest_key = "graph/latest/manifest.json".to_string();

    push_to_s3(kerml_view, bucket, &kerml_key, profile, region, mock)?;
    push_to_s3(&manifest_path, bucket, &manifest_key, profile, region, mock)?;
    // Only after both per-tag objects succeed: update the latest pointer.
    push_to_s3(&manifest_path, bucket, &latest_key, profile, region, mock)?;

    if mock {
        eprintln!("[mock] would publish NATS reindex hint on b00t.graph.reindex");
        return Ok(());
    }

    let event = serde_json::json!({
        "tag": tag,
        "commit_sha": commit_sha,
        "content_hash": manifest.kerml_digest,
        "s3_prefix": format!("s3://{bucket}/graph/tags/{tag}/"),
    });
    match async_nats::connect(nats_url).await {
        Ok(client) => {
            if let Err(e) = client
                .publish(
                    "b00t.graph.reindex",
                    serde_json::to_vec(&event)
                        .context("serialize reindex event")?
                        .into(),
                )
                .await
            {
                eprintln!("⚠️  NATS reindex publish failed (non-fatal, S3 objects already durable): {e}");
            }
        }
        Err(e) => {
            eprintln!("⚠️  NATS connect failed (non-fatal, S3 objects already durable): {e}");
        }
    }

    Ok(())
}

/// Async dispatch entry point — `Publish` needs real async I/O (S3 push via
/// a blocking subprocess is fine sync, but the NATS publish is async); every
/// other variant delegates to the sync `execute()` unchanged.
pub async fn execute_async(cmd: GraphCommands, b00t_path: &str) -> Result<()> {
    match cmd {
        GraphCommands::Publish {
            kerml_view,
            tag,
            commit_sha,
            kid,
            bucket,
            region,
            profile,
            nats_url,
            mock,
        } => {
            let _ = b00t_path; // publish() doesn't need the datum tree
            publish(
                Path::new(&kerml_view),
                &tag,
                &commit_sha,
                &kid,
                &bucket,
                region.as_deref(),
                profile.as_deref(),
                nats_url.as_deref(),
                mock,
            )
            .await
        }
        other => execute(&other, b00t_path),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn emit_triples_writes_jsonl_arrays() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(
            dir.path().join("rust.cli.toml"),
            "[b00t]\nname = \"rust.cli\"\ntype = \"cli\"\ndepends_on = [\"cargo\"]\n",
        )
        .unwrap();
        let out = dir.path().join("g.jsonl");
        execute(
            &GraphCommands::EmitTriples {
                tenant: None,
                out: Some(out.to_string_lossy().into()),
            },
            dir.path().to_str().unwrap(),
        )
        .unwrap();
        let body = std::fs::read_to_string(&out).unwrap();
        assert!(!body.is_empty());
        assert!(body.lines().all(|l| {
            let v: Vec<String> = serde_json::from_str(l).unwrap();
            v.len() == 3
        }));
        assert!(body.contains("b00t:dependsOn"));
    }
}
