//! Tail-map block — fast executive agent scanning
//!
//! The tail-map is an optional structured block in the LAST ≤10 lines of any
//! `.tomllm` or `.md` file. It enables sm0l agents to scan many files quickly
//! (read only the tail, get the map) without loading full context.
//!
//! ## Format
//!
//! In `.tomllm` files (TOML comment syntax):
//! ```toml
//! # b00t:map v1
//! # summary: one-line human+LLM description
//! # tags: comma, separated, keywords
//! # tier: sm0l|ch0nky|frontier
//! # cmds: b00t hive activate inference-qwen3, b00t hive status
//! # complexity: 1-10
//! ```
//!
//! In `.md` files (HTML comment, invisible in rendered markdown):
//! ```markdown
//! <!-- b00t:map v1
//! summary: one-line description
//! tags: tag1, tag2
//! tier: ch0nky
//! cmds: b00t cmd1
//! -->
//! ```
//!
//! ## Magic bytes
//! The `b00t:map v1` marker is the magic sequence. Any agent can `tail -10 <file>`
//! and check for this marker to determine if a map exists without reading the file.

use serde::{Deserialize, Serialize};

/// Cognitive tier required to productively process a datum
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum CognitiveTier {
    /// Any sm0l model: classify, route, grep, format, lint, simple transforms
    Sm0l,
    /// Code-generation capable: implement, refactor, debug, explain code
    Ch0nky,
    /// Frontier reasoning: architecture, security review, compliance, novel design
    Frontier,
}

impl CognitiveTier {
    pub fn from_str(s: &str) -> Option<Self> {
        match s.trim().to_lowercase().as_str() {
            "sm0l" | "small" => Some(CognitiveTier::Sm0l),
            "ch0nky" | "chunky" | "medium" => Some(CognitiveTier::Ch0nky),
            "frontier" | "large" | "big" => Some(CognitiveTier::Frontier),
            _ => None,
        }
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            CognitiveTier::Sm0l => "sm0l",
            CognitiveTier::Ch0nky => "ch0nky",
            CognitiveTier::Frontier => "frontier",
        }
    }
}

impl std::fmt::Display for CognitiveTier {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

/// Structured tail-map block parsed from a .tomllm or .md file
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct MapBlock {
    pub summary: Option<String>,
    pub tags: Vec<String>,
    pub tier: Option<CognitiveTier>,
    /// Key commands relevant to this datum (for agent quick-reference)
    pub cmds: Vec<String>,
    /// Subjective complexity 1-10 (used for routing/batching decisions)
    pub complexity: Option<u8>,
    /// Free-form extra fields
    pub extra: std::collections::BTreeMap<String, String>,
}

impl MapBlock {
    pub const MAGIC: &'static str = "b00t:map v1";

    /// Scan the tail of a string (last `max_lines` lines) for a map block.
    /// O(n) where n = chars in tail — designed for fast scanning.
    pub fn scan_tail(content: &str, max_lines: usize) -> Option<Self> {
        let lines: Vec<&str> = content.lines().collect();
        let start = lines.len().saturating_sub(max_lines);
        let tail = &lines[start..];

        // Find magic marker
        let marker_idx = tail.iter().position(|l| {
            // strip TOML comment prefix (#) or HTML comment prefix (<!--)
            let stripped = l.trim_start_matches(|c: char| "#!<- ".contains(c)).trim();
            stripped.starts_with(Self::MAGIC)
        })?;

        let mut block = MapBlock::default();

        for line in &tail[marker_idx + 1..] {
            // Strip comment prefixes: # key: value  or  key: value (inside <!-- -->)
            let cleaned = line
                .trim()
                .trim_start_matches('#')
                .trim_start_matches("<!--")
                .trim_end_matches("-->")
                .trim();

            if cleaned.is_empty() || cleaned == "-->" {
                continue;
            }

            if let Some((key, val)) = cleaned.split_once(':') {
                let key = key.trim();
                let val = val.trim().to_string();
                match key {
                    "summary" => block.summary = Some(val),
                    "tags" => {
                        block.tags = val.split(',').map(|t| t.trim().to_string()).collect()
                    }
                    "tier" => block.tier = CognitiveTier::from_str(&val),
                    "cmds" => {
                        block.cmds = val.split(',').map(|c| c.trim().to_string()).collect()
                    }
                    "complexity" => block.complexity = val.parse().ok(),
                    _ => {
                        block.extra.insert(key.to_string(), val);
                    }
                }
            }
        }

        Some(block)
    }

    /// Generate a tail-map block as TOML comment lines (for .tomllm files)
    pub fn to_toml_comments(&self) -> String {
        let mut lines = vec![format!("# {}", Self::MAGIC)];
        if let Some(s) = &self.summary {
            lines.push(format!("# summary: {}", s));
        }
        if !self.tags.is_empty() {
            lines.push(format!("# tags: {}", self.tags.join(", ")));
        }
        if let Some(t) = &self.tier {
            lines.push(format!("# tier: {}", t));
        }
        if !self.cmds.is_empty() {
            lines.push(format!("# cmds: {}", self.cmds.join(", ")));
        }
        if let Some(c) = self.complexity {
            lines.push(format!("# complexity: {}", c));
        }
        for (k, v) in &self.extra {
            lines.push(format!("# {}: {}", k, v));
        }
        lines.join("\n")
    }

    /// Generate a tail-map block as HTML comment (for .md files)
    pub fn to_md_comment(&self) -> String {
        let mut lines = vec![format!("<!-- {} ", Self::MAGIC)];
        if let Some(s) = &self.summary {
            lines.push(format!("summary: {}", s));
        }
        if !self.tags.is_empty() {
            lines.push(format!("tags: {}", self.tags.join(", ")));
        }
        if let Some(t) = &self.tier {
            lines.push(format!("tier: {}", t));
        }
        if !self.cmds.is_empty() {
            lines.push(format!("cmds: {}", self.cmds.join(", ")));
        }
        if let Some(c) = self.complexity {
            lines.push(format!("complexity: {}", c));
        }
        for (k, v) in &self.extra {
            lines.push(format!("{}: {}", k, v));
        }
        lines.push("-->".to_string());
        lines.join("\n")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_scan_tail_toml() {
        let content = r#"
key = "value"
other = 123

# b00t:map v1
# summary: test datum for inference
# tags: vllm, qwen3, inference
# tier: ch0nky
# cmds: b00t hive activate inference-qwen3
# complexity: 6
"#;
        let map = MapBlock::scan_tail(content, 10).expect("should find map block");
        assert_eq!(map.summary.as_deref(), Some("test datum for inference"));
        assert_eq!(map.tags, vec!["vllm", "qwen3", "inference"]);
        assert_eq!(map.tier, Some(CognitiveTier::Ch0nky));
        assert_eq!(map.cmds, vec!["b00t hive activate inference-qwen3"]);
        assert_eq!(map.complexity, Some(6));
    }

    #[test]
    fn test_scan_tail_md() {
        let content = r#"# Some Doc

content here

<!-- b00t:map v1
summary: architecture overview
tags: hive, cmdb
tier: frontier
cmds: b00t hive status
-->
"#;
        let map = MapBlock::scan_tail(content, 10).expect("should find map block");
        assert_eq!(map.summary.as_deref(), Some("architecture overview"));
        assert_eq!(map.tier, Some(CognitiveTier::Frontier));
    }

    #[test]
    fn test_no_map_block() {
        let content = "key = \"value\"\nother = 123\n";
        assert!(MapBlock::scan_tail(content, 10).is_none());
    }

    #[test]
    fn test_round_trip_toml_comments() {
        let block = MapBlock {
            summary: Some("hive inference profile".to_string()),
            tags: vec!["vllm".to_string(), "qwen3".to_string()],
            tier: Some(CognitiveTier::Ch0nky),
            cmds: vec!["b00t hive activate inference-qwen3".to_string()],
            complexity: Some(7),
            extra: Default::default(),
        };
        let comments = block.to_toml_comments();
        assert!(comments.contains("b00t:map v1"));
        assert!(comments.contains("ch0nky"));
        // round-trip: scan the generated comments
        let recovered = MapBlock::scan_tail(&comments, 10).expect("round-trip failed");
        assert_eq!(recovered.summary, block.summary);
        assert_eq!(recovered.tier, block.tier);
    }

    #[test]
    fn test_cognitive_tier_from_str() {
        assert_eq!(CognitiveTier::from_str("sm0l"), Some(CognitiveTier::Sm0l));
        assert_eq!(CognitiveTier::from_str("small"), Some(CognitiveTier::Sm0l));
        assert_eq!(CognitiveTier::from_str("ch0nky"), Some(CognitiveTier::Ch0nky));
        assert_eq!(CognitiveTier::from_str("frontier"), Some(CognitiveTier::Frontier));
        assert_eq!(CognitiveTier::from_str("unknown"), None);
    }
}
