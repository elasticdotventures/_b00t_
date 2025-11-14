//! Datum types for enhanced b00t schema
//! These types extend BootDatum with learn, usage, and reference metadata

use serde::{Deserialize, Serialize};

/// Learn metadata - links datum to learning materials and auto-digest to grok
#[derive(Deserialize, Serialize, Debug, Clone, PartialEq)]
pub struct LearnMetadata {
    /// Reference to learn topic (maps to learn.toml)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub topic: Option<String>,
    /// Inline markdown content (alternative to topic reference)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub inline: Option<String>,
    /// Automatically digest learn content to grok on install/update
    #[serde(skip_serializing_if = "Option::is_none")]
    pub auto_digest: Option<bool>,
}

/// Usage example for CLI/API usage patterns
#[derive(Deserialize, Serialize, Debug, Clone, PartialEq)]
pub struct UsageExample {
    pub description: String,
    pub command: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub output: Option<String>,
}

/// External reference - docs, repos, blogs, tutorials
#[derive(Deserialize, Serialize, Debug, Clone, PartialEq)]
pub struct Reference {
    pub url: String,
    pub description: String,
    #[serde(rename = "type")]
    pub ref_type: ReferenceType,
}

/// Reference types for external links
#[derive(Deserialize, Serialize, Debug, Clone, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum ReferenceType {
    Docs,
    GitHub,
    Blog,
    Tutorial,
    StackOverflow,
    Community,
}
