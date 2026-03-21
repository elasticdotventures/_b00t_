//! TomllmDoc — full parse of a .tomllm file with annotation extraction

use std::collections::BTreeMap;
use serde::{Deserialize, Serialize};

use crate::{Result, TomllmError};
use crate::map_block::MapBlock;
use crate::stripper::{extract_comments, strip};

/// Annotation prefixes that associate comments with the NEXT key
const ANNOTATION_PREFIXES: &[&str] = &[
    "🤓",       // melvin — tribal knowledge / non-obvious fact
    "@tribal:", // equivalent to 🤓
    "@example:",
    "@requires:",
    "@warn:",
    "@see:",
    "⚠️",
    "🚩",
    "🦨",       // skunk — renamed/changed, needs attention
];

/// A fully parsed .tomllm document
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TomllmDoc {
    /// Parsed TOML value (comments stripped)
    pub value: toml::Value,
    /// Annotations per key path (dot-separated, e.g. "b00t.hive.resources.ram_gb")
    /// Maps key → list of annotation strings from preceding comments
    pub annotations: BTreeMap<String, Vec<String>>,
    /// File-level comments (before any key-value pair)
    pub file_comments: Vec<String>,
    /// Tail-map block if present
    pub map_block: Option<MapBlock>,
}

impl TomllmDoc {
    /// Parse a .tomllm string into a TomllmDoc
    pub fn parse(input: &str) -> Result<Self> {
        let stripped = strip(input);
        let value: toml::Value = toml::from_str(&stripped)?;
        let (file_comments, annotations) = Self::extract_annotations(input);
        let map_block = MapBlock::scan_tail(input, 10);

        Ok(TomllmDoc { value, annotations, file_comments, map_block })
    }

    /// Parse from a file path
    pub fn from_file(path: &std::path::Path) -> Result<Self> {
        let content = std::fs::read_to_string(path)?;
        Self::parse(&content)
    }

    /// Return only the TOML string (no comments) — for downstream pipelines
    pub fn strip_for_pipeline(&self) -> String {
        serde_json::to_string(&self.value).unwrap_or_default()
    }

    /// Return the annotations for a given dot-path key (e.g. "b00t.hint")
    pub fn get_annotations(&self, key_path: &str) -> Vec<&str> {
        self.annotations
            .get(key_path)
            .map(|v| v.iter().map(|s| s.as_str()).collect())
            .unwrap_or_default()
    }

    /// Cognitive tier from tail-map (default: Sm0l if not specified)
    pub fn cognitive_tier(&self) -> crate::map_block::CognitiveTier {
        self.map_block
            .as_ref()
            .and_then(|m| m.tier.clone())
            .unwrap_or(crate::map_block::CognitiveTier::Sm0l)
    }

    /// Summary from tail-map, or fallback to `hint` field in b00t section
    pub fn summary(&self) -> Option<String> {
        if let Some(m) = &self.map_block {
            if let Some(s) = &m.summary {
                return Some(s.clone());
            }
        }
        // fallback to [b00t].hint
        self.value
            .get("b00t")
            .and_then(|b| b.get("hint"))
            .and_then(|h| h.as_str())
            .map(|s| s.to_string())
    }

    // ─── Private ─────────────────────────────────────────────────────────────

    fn extract_annotations(input: &str) -> (Vec<String>, BTreeMap<String, Vec<String>>) {
        let all_comments = extract_comments(input);
        let lines: Vec<&str> = input.lines().collect();

        let mut file_comments = Vec::new();
        let mut annotations: BTreeMap<String, Vec<String>> = BTreeMap::new();
        let mut pending_comments: Vec<String> = Vec::new();

        for (line_idx, comment_text) in &all_comments {
            // Check if this is an annotation-style comment
            let is_annotation = ANNOTATION_PREFIXES
                .iter()
                .any(|prefix| comment_text.starts_with(prefix));

            // Skip map block lines
            if comment_text.starts_with(MapBlock::MAGIC)
                || comment_text.starts_with("summary:")
                || comment_text.starts_with("tags:")
                || comment_text.starts_with("tier:")
                || comment_text.starts_with("cmds:")
                || comment_text.starts_with("complexity:")
            {
                continue;
            }

            pending_comments.push(comment_text.clone());

            // Look ahead to find the next non-comment, non-empty line
            let next_key_line = lines[line_idx + 1..]
                .iter()
                .find(|l| {
                    let t = l.trim();
                    !t.is_empty() && !t.starts_with('#')
                });

            if let Some(key_line) = next_key_line {
                let key_path = extract_key_path(key_line);
                if let Some(path) = key_path {
                    annotations
                        .entry(path)
                        .or_default()
                        .extend(pending_comments.drain(..));
                }
            }
        }

        // Remaining pending comments with no associated key = file-level
        file_comments.extend(pending_comments);

        (file_comments, annotations)
    }
}

/// Extract a key path string from a TOML line
/// e.g. `name = "foo"` → `"name"`, `[b00t.hive]` → `"b00t.hive"`
fn extract_key_path(line: &str) -> Option<String> {
    let trimmed = line.trim();
    if trimmed.starts_with('[') {
        // section header
        Some(
            trimmed
                .trim_start_matches('[')
                .trim_end_matches(']')
                .trim()
                .to_string(),
        )
    } else if let Some((key, _)) = trimmed.split_once('=') {
        Some(key.trim().to_string())
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = r#"
# @tribal: always use uv pip, never pip install
# @example: uv pip install requests
[b00t]
name = "python-toolchain"
type = "hive_profile"
hint = "Python toolchain config"

[b00t.hive.resources]
# ⚠️ ram_gb includes Python runtime overhead (2GB baseline)
ram_gb = 4.0

# b00t:map v1
# summary: Python toolchain hive profile
# tags: python, uv, toolchain
# tier: sm0l
# complexity: 2
"#;

    #[test]
    fn test_parse_basic() {
        let doc = TomllmDoc::parse(SAMPLE).expect("should parse");
        assert_eq!(
            doc.value["b00t"]["name"].as_str(),
            Some("python-toolchain")
        );
    }

    #[test]
    fn test_map_block_extracted() {
        let doc = TomllmDoc::parse(SAMPLE).expect("should parse");
        let map = doc.map_block.expect("should have map block");
        assert_eq!(map.summary.as_deref(), Some("Python toolchain hive profile"));
        assert_eq!(map.tier, Some(crate::map_block::CognitiveTier::Sm0l));
        assert_eq!(map.complexity, Some(2));
    }

    #[test]
    fn test_summary_fallback_to_hint() {
        let simple = r#"[b00t]
name = "test"
hint = "test hint"
"#;
        let doc = TomllmDoc::parse(simple).expect("should parse");
        assert_eq!(doc.summary().as_deref(), Some("test hint"));
    }

    #[test]
    fn test_cognitive_tier_default() {
        let simple = r#"[b00t]
name = "test"
hint = "no tier set"
"#;
        let doc = TomllmDoc::parse(simple).expect("should parse");
        assert_eq!(
            doc.cognitive_tier(),
            crate::map_block::CognitiveTier::Sm0l
        );
    }
}
