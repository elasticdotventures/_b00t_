use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

use crate::{edl::EdlQuery, TomllmDoc, TomllmdExt};

/// Single entry in the datum index — slim summary for fast query without loading full doc.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DatumIndexEntry {
    pub key: String,
    pub path: String,
    pub datum_type: Option<String>,
    pub tier: Option<String>,
    pub complexity: Option<u8>,
    #[serde(default)]
    pub type_tags: Vec<String>,
    pub summary: Option<String>,
}

/// Fast datum index; serialised to `~/.b00t/datum-index.json`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DatumIndex {
    pub entries: Vec<DatumIndexEntry>,
    pub built_at: DateTime<Utc>,
}

impl DatumIndex {
    /// Load from `datum-index.json`; returns empty index if file missing.
    pub fn load(path: &Path) -> Result<Self> {
        if !path.exists() {
            return Ok(Self { entries: vec![], built_at: Utc::now() });
        }
        let content = std::fs::read_to_string(path)
            .with_context(|| format!("cannot read {}", path.display()))?;
        serde_json::from_str(&content)
            .with_context(|| format!("cannot parse {}", path.display()))
    }

    /// Scan `datums_dir` recursively, parse each `.tomllmd`/`.tomllm`/`.toml`, build index.
    pub fn rebuild(datums_dir: &Path) -> Result<Self> {
        let mut entries = Vec::new();
        scan_dir(datums_dir, &mut entries)?;
        entries.sort_by(|a, b| a.key.cmp(&b.key));
        Ok(Self { entries, built_at: Utc::now() })
    }

    /// Save to `datum-index.json`.
    pub fn save(&self, path: &Path) -> Result<()> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let json = serde_json::to_string_pretty(self)?;
        std::fs::write(path, json)?;
        Ok(())
    }

    /// Filter entries by an `EdlQuery`. Returns references to matching entries.
    pub fn query<'a>(&'a self, filter: &EdlQuery) -> Vec<&'a DatumIndexEntry> {
        self.entries.iter().filter(|e| filter.matches(e)).collect()
    }
}

fn scan_dir(dir: &Path, entries: &mut Vec<DatumIndexEntry>) -> Result<()> {
    let rd = match std::fs::read_dir(dir) {
        Ok(r) => r,
        Err(_) => return Ok(()),
    };
    for entry in rd.flatten() {
        let path = entry.path();
        if path.is_dir() {
            scan_dir(&path, entries)?;
        } else if let Some(ext) = TomllmdExt::from_path(&path) {
            if let Ok(doc) = TomllmDoc::from_path(&path) {
                entries.push(doc_to_entry(doc, path, ext));
            }
        }
    }
    Ok(())
}

fn doc_to_entry(doc: TomllmDoc, path: PathBuf, _ext: TomllmdExt) -> DatumIndexEntry {
    let key = path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("")
        .to_string();
    DatumIndexEntry {
        key,
        path: path.display().to_string(),
        datum_type: doc.schema.datum_type.clone(),
        tier: doc.tier().map(String::from),
        complexity: doc.complexity(),
        type_tags: doc.type_tags().to_vec(),
        summary: doc.summary().map(String::from),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::edl::{EdlQuery, EdlTagFilter};
    use tempfile::TempDir;

    fn write_datum(dir: &Path, name: &str, content: &str) {
        std::fs::write(dir.join(format!("{name}.tomllmd")), content).unwrap();
    }

    #[test]
    fn rebuild_and_query() {
        let tmp = TempDir::new().unwrap();

        write_datum(tmp.path(), "PRD-A", r#"
[b00t.schema]
version = "1"
type = "prd"
type_tags = ["prd", "ooda"]
# b00t:map v1
# tier: frontier
# complexity: 5
"#);

        write_datum(tmp.path(), "PRD-B", r#"
[b00t.schema]
version = "1"
type = "prd"
type_tags = ["prd", "agent"]
# b00t:map v1
# tier: sm0l
# complexity: 2
"#);

        let idx = DatumIndex::rebuild(tmp.path()).unwrap();
        assert_eq!(idx.entries.len(), 2);

        let q = EdlQuery {
            type_tags: Some(EdlTagFilter::All(vec!["ooda".into()])),
            datum_type: None,
            tier: Some("frontier".into()),
            complexity_max: None,
            z3_constraint: None,
        };
        let results = idx.query(&q);
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].key, "PRD-A");
    }

    #[test]
    fn save_and_load_roundtrip() {
        let tmp = TempDir::new().unwrap();
        let index_path = tmp.path().join("datum-index.json");

        write_datum(tmp.path(), "X", "[b00t.schema]\ntype = \"prd\"\ntype_tags = [\"prd\"]\n");
        let idx = DatumIndex::rebuild(tmp.path()).unwrap();
        idx.save(&index_path).unwrap();

        let loaded = DatumIndex::load(&index_path).unwrap();
        assert_eq!(loaded.entries.len(), idx.entries.len());
    }
}
