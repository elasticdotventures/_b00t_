// 🤓 b00t Knowledge Store — local-first, ontology-grounded object persistence.
//    Objects are stored with irontology StoragePlan naming (class + consumer + checksum).
//    Metadata is a JSONL manifest. Cloud sync uses credential datums for S3/R2 auth.
//    Queryable via: b00t store put|get|list|query|sync
//
//    Compound-engineering: thin wrappers over filesystem + manifest.
//    Advanced pragmatic-hacker: works NOW, cloud sync is a just recipe.

use anyhow::{Context, Result};
use chrono::Utc;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

// ── Types ──────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StoreEntry {
    pub key: String,
    pub ontology_class: String,
    pub consumer: String,
    pub shape: String,
    pub filename: String,
    pub checksum: String,
    pub size_bytes: u64,
    pub tags: BTreeMap<String, String>,
    pub created_at: String,
    pub source_file: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StoreManifest {
    pub entries: Vec<StoreEntry>,
    pub version: u32,
    pub updated_at: String,
}

// ── Store paths (configurable for testing) ────────────────────────────────

static STORE_ROOT: std::sync::OnceLock<PathBuf> = std::sync::OnceLock::new();

fn store_root() -> PathBuf {
    STORE_ROOT
        .get()
        .cloned()
        .unwrap_or_else(|| {
            dirs::home_dir()
                .unwrap_or_else(|| PathBuf::from("."))
                .join(".b00t")
                .join("store")
        })
}

#[doc(hidden)]
pub fn _set_store_root_for_test(path: PathBuf) {
    let _ = STORE_ROOT.set(path);
}

fn manifest_path() -> PathBuf {
    store_root().join("manifest.jsonl")
}

fn object_path(entry: &StoreEntry) -> PathBuf {
    store_root().join(&entry.ontology_class).join(&entry.key)
}

// ── Put ────────────────────────────────────────────────────────────────────

/// Store a file into the knowledge store. Computes SHA256 checksum, creates
/// a StoragePlan-keyed path, copies the file, and appends to the manifest.
pub fn put(
    source: &Path,
    ontology_class: &str,
    consumer: &str,
    tags: &BTreeMap<String, String>,
) -> Result<StoreEntry> {
    if !source.exists() {
        anyhow::bail!("source file not found: {}", source.display());
    }

    let data = std::fs::read(source).context("failed to read source file")?;
    let checksum = sha2(data.clone());
    let ext = source
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("bin");
    let shape: String = match ext {
        "jsonl" => "JSONL".into(),
        "json" => "JSON".into(),
        "toml" | "tomllm" => "TOML".into(),
        "gguf" => "GGUF".into(),
        "bin" => "BINARY".into(),
        other => other.to_uppercase(),
    };
    let short = &checksum[..12];
    let filename = source
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("unknown")
        .to_string();

    let entry = StoreEntry {
        key: format!("{}/{}/{}/{}", consumer, shape, short, filename),
        ontology_class: ontology_class.to_string(),
        consumer: consumer.to_string(),
        shape: shape.to_string(),
        filename: filename.clone(),
        checksum: checksum.clone(),
        size_bytes: data.len() as u64,
        tags: tags.clone(),
        created_at: Utc::now().to_rfc3339(),
        source_file: source.display().to_string(),
    };

    let target = object_path(&entry);
    if let Some(parent) = target.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(&target, &data)?;
    append_manifest(&entry)?;

    eprintln!(
        "📦 stored: {} ({} bytes, sha256:{})",
        entry.key,
        entry.size_bytes,
        short
    );
    Ok(entry)
}

// ── Get ────────────────────────────────────────────────────────────────────

/// Retrieve a stored object by key. Searches the manifest for metadata,
/// then reads from the object path.
pub fn get(key: &str, output: Option<&Path>) -> Result<Option<Vec<u8>>> {
    let manifest = load_manifest()?;
    let entry = manifest.entries.iter().find(|e| e.key == key);
    let target = match entry {
        Some(e) => object_path(e),
        None => store_root().join(key), // legacy: try direct path
    };
    if !target.exists() {
        return Ok(None);
    }
    let data = std::fs::read(&target).context("failed to read stored object")?;
    if let Some(out) = output {
        std::fs::write(out, &data).context("failed to write output file")?;
    }
    Ok(Some(data))
}

// ── List / Query ───────────────────────────────────────────────────────────

/// List all stored entries from the manifest, optionally filtered.
pub fn list(class: Option<&str>, consumer: Option<&str>) -> Result<Vec<StoreEntry>> {
    let manifest = load_manifest()?;
    let mut entries: Vec<StoreEntry> = manifest
        .entries
        .into_iter()
        .filter(|e| {
            class.map_or(true, |c| e.ontology_class == c)
                && consumer.map_or(true, |c| e.consumer == c)
        })
        .collect();
    entries.sort_by(|a, b| b.created_at.cmp(&a.created_at));
    Ok(entries)
}

/// Query by tags (all tags must match).
pub fn query(tags: &BTreeMap<String, String>) -> Result<Vec<StoreEntry>> {
    let manifest = load_manifest()?;
    Ok(manifest
        .entries
        .into_iter()
        .filter(|e| tags.iter().all(|(k, v)| e.tags.get(k) == Some(v)))
        .collect())
}

// ── Sync (local → remote placeholder) ─────────────────────────────────────

/// Sync stored objects to a remote S3/R2 bucket using credential datums.
/// This is the integration point with datum_credential and rustfs-mcp.
pub fn sync(provider: &str) -> Result<()> {
    let cred =
        crate::datum_credential::find_credential_by_name(provider)?
            .ok_or_else(|| anyhow::anyhow!(
                "No credential found for '{}'. Set: b00t server key set --provider {} --key <KEY>",
                provider, provider
            ))?;

    let (access_key, _secret_key) = cred;
    let endpoint = match provider {
        "cloudflare-r2" => format!(
            "https://{}.r2.cloudflarestorage.com",
            std::env::var("R2_ACCOUNT_ID").unwrap_or_else(|_| "unknown".into())
        ),
        "aws-s3" | "aws" => "https://s3.amazonaws.com".into(),
        _ => anyhow::bail!("unsupported sync provider: {}", provider),
    };

    eprintln!("☁️  syncing to {} ({}...)", provider, &access_key[..12.min(access_key.len())]);
    eprintln!("   endpoint: {}", endpoint);
    eprintln!("   (full S3 sync via rustfs-mcp or aws-cli — just recipe available)");

    let manifest = load_manifest()?;
    for entry in &manifest.entries {
        eprintln!("   would sync: {}", entry.key);
    }

    eprintln!("📋 {} objects ready. Run: just store-cloud-sync provider={}", manifest.entries.len(), provider);
    Ok(())
}

// ── Internal: manifest + crypto ────────────────────────────────────────────

fn load_manifest() -> Result<StoreManifest> {
    let path = manifest_path();
    if !path.exists() {
        return Ok(StoreManifest {
            entries: Vec::new(),
            version: 1,
            updated_at: Utc::now().to_rfc3339(),
        });
    }
    let mut entries = Vec::new();
    let content = std::fs::read_to_string(&path).unwrap_or_default();
    for line in content.lines() {
        if let Ok(entry) = serde_json::from_str::<StoreEntry>(line) {
            entries.push(entry);
        }
    }
    Ok(StoreManifest {
        entries,
        version: 1,
        updated_at: Utc::now().to_rfc3339(),
    })
}

fn append_manifest(entry: &StoreEntry) -> Result<()> {
    let path = manifest_path();
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let line = serde_json::to_string(entry)? + "\n";
    std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .write(true)
        .open(&path)
        .context("failed to open manifest")?;
    // Use fs::write for simplicity after reading
    let mut content = std::fs::read_to_string(&path).unwrap_or_default();
    content.push_str(&line);
    std::fs::write(&path, &content)?;
    Ok(())
}

fn sha2(data: Vec<u8>) -> String {
    use sha2::{Digest, Sha256};
    format!("{:x}", Sha256::digest(&data))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_put_get_list_query() {
        let tmp = std::env::temp_dir().join("b00t-store-test");
        let _ = std::fs::remove_dir_all(&tmp);
        std::fs::create_dir_all(&tmp).unwrap();
        _set_store_root_for_test(tmp.clone());

        let source = tmp.join("test.jsonl");
        std::fs::write(&source, r#"{"msg":"hello"}"#).unwrap();
        let mut tags = BTreeMap::new();
        tags.insert("model".into(), "qwen3.5".into());

        let entry = put(&source, "b00t:TrainingCorpus", "rust-doc", &tags).expect("put");
        assert_eq!(entry.ontology_class, "b00t:TrainingCorpus");
        assert_eq!(entry.consumer, "rust-doc");
        assert_eq!(entry.tags.get("model").unwrap(), "qwen3.5");

        let data = get(&entry.key, None).expect("get");
        assert!(data.is_some());

        let list = list(Some("b00t:TrainingCorpus"), None).expect("list");
        assert!(!list.is_empty());

        let query = query(&tags).expect("query");
        assert!(!query.is_empty());
    }
}
