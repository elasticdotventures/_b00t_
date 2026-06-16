use anyhow::{Context, Result};
use serde::Deserialize;
use std::collections::HashMap;
use std::path::{Path, PathBuf};

/// File extension determines parse priority: tomllmd > tomllm > toml.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TomllmdExt {
    Toml,
    Tomllm,
    Tomllmd,
}

impl TomllmdExt {
    pub fn from_path(path: &Path) -> Option<Self> {
        match path.extension()?.to_str()? {
            "tomllmd" => Some(Self::Tomllmd),
            "tomllm" => Some(Self::Tomllm),
            "toml" => Some(Self::Toml),
            _ => None,
        }
    }
}

/// Parsed b00t.schema section common to all TOMLLM datums.
#[derive(Debug, Clone, Deserialize, Default)]
pub struct B00tSchema {
    pub version: Option<String>,
    #[serde(rename = "type")]
    pub datum_type: Option<String>,
    #[serde(default)]
    pub type_tags: Vec<String>,
}

/// Container for b00t.schema in file.
#[derive(Debug, Clone, Deserialize, Default)]
struct B00tWrapper {
    pub schema: Option<B00tSchema>,
}

/// Outer TOML envelope.
#[derive(Debug, Clone, Deserialize, Default)]
struct RawDoc {
    #[serde(default)]
    pub b00t: B00tWrapper,
}

/// A parsed `.tomllmd` / `.tomllm` / `.toml` datum document.
///
/// `TomllmDoc` extracts the `[b00t.schema]` header, strips enriched comments
/// (`# @tribal:`, `# 🤓`, `# summary:`, `# tier:`, etc.) into `map_tags`,
/// and makes extra TOML sections available for callers.
#[derive(Debug, Clone)]
pub struct TomllmDoc {
    pub source_path: PathBuf,
    pub ext: TomllmdExt,
    /// Cleaned TOML (comments stripped).
    pub raw_toml: String,
    /// b00t.schema section.
    pub schema: B00tSchema,
    /// Key/value pairs from the `# b00t:map v1` tail block.
    pub map_tags: HashMap<String, String>,
    /// All extra top-level TOML keys as raw JSON values (excludes `b00t`).
    pub sections: HashMap<String, serde_json::Value>,
}

impl TomllmDoc {
    pub fn from_path(path: &Path) -> Result<Self> {
        let ext = TomllmdExt::from_path(path)
            .with_context(|| format!("unsupported extension: {}", path.display()))?;
        let content = std::fs::read_to_string(path)
            .with_context(|| format!("cannot read {}", path.display()))?;
        Self::from_str(&content, ext, path.to_path_buf())
    }

    pub fn from_str(src: &str, ext: TomllmdExt, source_path: PathBuf) -> Result<Self> {
        let map_tags = extract_map_block(src);
        let clean = strip_comments(src);

        let raw: RawDoc = toml::from_str(&clean)
            .with_context(|| format!("TOML parse error in {}", source_path.display()))?;
        let schema = raw.b00t.schema.unwrap_or_default();

        let sections = extra_sections(&clean)?;

        Ok(Self {
            source_path,
            ext,
            raw_toml: clean,
            schema,
            map_tags,
            sections,
        })
    }

    /// Returns `type_tags` from `[b00t.schema]`.
    pub fn type_tags(&self) -> &[String] {
        &self.schema.type_tags
    }

    /// Returns `# summary:` value from the `# b00t:map v1` tail block.
    pub fn summary(&self) -> Option<&str> {
        self.map_tags.get("summary").map(String::as_str).filter(|s| !s.is_empty())
    }

    /// Returns `# tier:` value from the tail block.
    pub fn tier(&self) -> Option<&str> {
        self.map_tags.get("tier").map(String::as_str).filter(|s| !s.is_empty())
    }

    /// Returns `# complexity:` parsed as u8.
    pub fn complexity(&self) -> Option<u8> {
        self.map_tags.get("complexity")?.trim().parse().ok()
    }

    /// Returns `# tags:` as a comma-split vec (from the map block, not type_tags).
    pub fn map_tag_list(&self) -> Vec<&str> {
        self.map_tags
            .get("tags")
            .map(|s| s.split(',').map(str::trim).collect())
            .unwrap_or_default()
    }
}

/// Strip `#`-prefixed comment lines; preserves structure for TOML parser.
fn strip_comments(src: &str) -> String {
    src.lines()
        .map(|line| {
            let trimmed = line.trim_start();
            if trimmed.starts_with('#') {
                ""
            } else {
                line
            }
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// Extract key/value pairs from the `# b00t:map v1` tail block.
///
/// Block format (last ≤10 lines with `#` prefix after `# b00t:map v1`):
/// ```text
/// # b00t:map v1
/// # summary: one-line description
/// # tags: keyword, list
/// # tier: sm0l|ch0nky|frontier
/// # complexity: 6
/// ```
fn extract_map_block(src: &str) -> HashMap<String, String> {
    let mut map = HashMap::new();
    let mut in_map = false;
    for line in src.lines() {
        let t = line.trim();
        if t == "# b00t:map v1" {
            in_map = true;
            continue;
        }
        if in_map {
            if let Some(rest) = t.strip_prefix('#') {
                let rest = rest.trim();
                if let Some((k, v)) = rest.split_once(':') {
                    map.insert(k.trim().to_string(), v.trim().to_string());
                }
            } else {
                break;
            }
        }
    }
    map
}

/// Parse cleaned TOML and return extra top-level sections (excluding `b00t`) as JSON.
fn extra_sections(clean: &str) -> Result<HashMap<String, serde_json::Value>> {
    let value: toml::Value = toml::from_str(clean).unwrap_or(toml::Value::Table(Default::default()));
    let mut out = HashMap::new();
    if let toml::Value::Table(table) = value {
        for (k, v) in table {
            if k == "b00t" {
                continue;
            }
            out.insert(k, toml_value_to_json(v));
        }
    }
    Ok(out)
}

fn toml_value_to_json(v: toml::Value) -> serde_json::Value {
    match v {
        toml::Value::String(s) => serde_json::Value::String(s),
        toml::Value::Integer(i) => serde_json::Value::Number(i.into()),
        toml::Value::Float(f) => serde_json::json!(f),
        toml::Value::Boolean(b) => serde_json::Value::Bool(b),
        toml::Value::Array(arr) => {
            serde_json::Value::Array(arr.into_iter().map(toml_value_to_json).collect())
        }
        toml::Value::Table(tbl) => {
            serde_json::Value::Object(tbl.into_iter().map(|(k, v)| (k, toml_value_to_json(v))).collect())
        }
        toml::Value::Datetime(dt) => serde_json::Value::String(dt.to_string()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = r#"
# PRD: test datum
[b00t.schema]
version = "1"
type = "prd"
type_tags = ["prd", "ooda"]

[prd]
id = "PRD-TEST"
title = "Test datum"
status = "proposed"
tier = "frontier"

# b00t:map v1
# summary: test datum summary
# tags: prd, test
# tier: frontier
# complexity: 3
"#;

    #[test]
    fn parses_schema() {
        let doc = TomllmDoc::from_str(SAMPLE, TomllmdExt::Tomllmd, "test.tomllmd".into()).unwrap();
        assert_eq!(doc.schema.datum_type.as_deref(), Some("prd"));
        assert!(doc.type_tags().contains(&"ooda".to_string()));
    }

    #[test]
    fn extracts_map_block() {
        let doc = TomllmDoc::from_str(SAMPLE, TomllmdExt::Tomllmd, "test.tomllmd".into()).unwrap();
        assert_eq!(doc.summary(), Some("test datum summary"));
        assert_eq!(doc.tier(), Some("frontier"));
        assert_eq!(doc.complexity(), Some(3));
    }

    #[test]
    fn extra_sections_has_prd() {
        let doc = TomllmDoc::from_str(SAMPLE, TomllmdExt::Tomllmd, "test.tomllmd".into()).unwrap();
        assert!(doc.sections.contains_key("prd"));
    }
}
