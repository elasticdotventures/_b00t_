//! Datum types for enhanced b00t schema
//! These types extend BootDatum with learn, usage, and reference metadata

use serde::{Deserialize, Deserializer, Serialize};

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
///
/// Supports two TOML formats:
/// 1. Concise: `usage = ["command  # description", ...]`
/// 2. Verbose: `[[b00t.usage]]` with fields
#[derive(Deserialize, Serialize, Debug, Clone, PartialEq)]
pub struct UsageExample {
    pub description: String,
    pub command: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub output: Option<String>,
}

impl UsageExample {
    /// Parse from concise string format: "command  # description"
    pub fn from_str(s: &str) -> Self {
        if let Some((cmd, desc)) = s.split_once('#') {
            Self {
                command: cmd.trim().to_string(),
                description: desc.trim().to_string(),
                output: None,
            }
        } else {
            Self {
                command: s.trim().to_string(),
                description: String::new(),
                output: None,
            }
        }
    }
}

/// External reference - docs, repos, blogs, tutorials
///
/// Supports two TOML formats:
/// 1. Concise: `references = ["https://github.com/user/repo#description", ...]`
/// 2. Verbose: `[[b00t.references]]` with explicit fields
///
/// Type auto-detected from URL domain:
/// - github.com → GitHub
/// - *.readthedocs.io, */docs → Docs
/// - *.blog, medium.com → Blog
/// - stackoverflow.com → StackOverflow
#[derive(Deserialize, Serialize, Debug, Clone, PartialEq)]
pub struct Reference {
    pub url: String,
    pub description: String,
    #[serde(rename = "type")]
    pub ref_type: ReferenceType,
}

impl Reference {
    /// Parse from concise URL format: "https://example.com/path#description"
    ///
    /// Description extracted from URL fragment (#text)
    /// Type auto-detected from URL domain
    pub fn from_url(url_with_fragment: &str) -> Self {
        let (url, fragment) = if let Some((u, f)) = url_with_fragment.split_once('#') {
            (u.to_string(), f.trim().replace('-', " "))
        } else {
            (url_with_fragment.to_string(), String::new())
        };

        let ref_type = Self::detect_type(&url);
        let description = if fragment.is_empty() {
            Self::generate_description(&url, &ref_type)
        } else {
            fragment
        };

        Self {
            url,
            description,
            ref_type,
        }
    }

    /// Auto-detect reference type from URL
    fn detect_type(url: &str) -> ReferenceType {
        let lower = url.to_lowercase();
        if lower.contains("github.com") || lower.contains("gitlab.com") {
            ReferenceType::GitHub
        } else if lower.contains("stackoverflow.com") || lower.contains("stackexchange.com") {
            ReferenceType::StackOverflow
        } else if lower.contains(".blog") || lower.contains("medium.com") || lower.contains("/blog/") {
            ReferenceType::Blog
        } else if lower.contains("tutorial") || lower.contains("/learn/") {
            ReferenceType::Tutorial
        } else if lower.contains("/docs")
            || lower.contains("/doc/")
            || lower.contains("readthedocs")
            || lower.contains(".systems")
            || lower.contains("doc.")
            || lower.contains("docs.") {
            ReferenceType::Docs
        } else {
            ReferenceType::Community
        }
    }

    /// Generate default description from URL and type
    fn generate_description(url: &str, ref_type: &ReferenceType) -> String {
        match ref_type {
            ReferenceType::GitHub => {
                // Extract repo name from github.com/user/repo
                url.split('/').rev().take(1).next()
                    .unwrap_or("repository")
                    .to_string()
            }
            ReferenceType::Docs => "documentation".to_string(),
            ReferenceType::Blog => "blog post".to_string(),
            ReferenceType::Tutorial => "tutorial".to_string(),
            ReferenceType::StackOverflow => "Q&A".to_string(),
            ReferenceType::Community => "resource".to_string(),
        }
    }
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

/// Custom deserializer that accepts both string arrays and structured tables
pub fn deserialize_references<'de, D>(deserializer: D) -> Result<Option<Vec<Reference>>, D::Error>
where
    D: Deserializer<'de>,
{
    #[derive(Deserialize)]
    #[serde(untagged)]
    enum ReferenceFormat {
        Strings(Vec<String>),
        Structured(Vec<Reference>),
    }

    match Option::<ReferenceFormat>::deserialize(deserializer)? {
        None => Ok(None),
        Some(ReferenceFormat::Strings(urls)) => {
            Ok(Some(urls.into_iter().map(|u| Reference::from_url(&u)).collect()))
        }
        Some(ReferenceFormat::Structured(refs)) => Ok(Some(refs)),
    }
}

/// Custom deserializer for usage examples
pub fn deserialize_usage<'de, D>(deserializer: D) -> Result<Option<Vec<UsageExample>>, D::Error>
where
    D: Deserializer<'de>,
{
    #[derive(Deserialize)]
    #[serde(untagged)]
    enum UsageFormat {
        Strings(Vec<String>),
        Structured(Vec<UsageExample>),
    }

    match Option::<UsageFormat>::deserialize(deserializer)? {
        None => Ok(None),
        Some(UsageFormat::Strings(cmds)) => {
            Ok(Some(cmds.into_iter().map(|c| UsageExample::from_str(&c)).collect()))
        }
        Some(UsageFormat::Structured(examples)) => Ok(Some(examples)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_reference_from_url_with_fragment() {
        let ref1 = Reference::from_url("https://github.com/casey/just#upstream-repo");
        assert_eq!(ref1.url, "https://github.com/casey/just");
        assert_eq!(ref1.description, "upstream repo");
        assert_eq!(ref1.ref_type, ReferenceType::GitHub);
    }

    #[test]
    fn test_reference_from_url_auto_detect_github() {
        let ref1 = Reference::from_url("https://github.com/elasticdotventures/just");
        assert_eq!(ref1.ref_type, ReferenceType::GitHub);
        assert_eq!(ref1.description, "just");
    }

    #[test]
    fn test_reference_from_url_auto_detect_docs() {
        let ref1 = Reference::from_url("https://just.systems/man/en/");
        assert_eq!(ref1.ref_type, ReferenceType::Docs);
        
        let ref2 = Reference::from_url("https://docs.python.org/");
        assert_eq!(ref2.ref_type, ReferenceType::Docs);
    }

    #[test]
    fn test_reference_from_url_auto_detect_stackoverflow() {
        let ref1 = Reference::from_url("https://stackoverflow.com/questions/12345");
        assert_eq!(ref1.ref_type, ReferenceType::StackOverflow);
    }

    #[test]
    fn test_usage_from_str() {
        let usage = UsageExample::from_str("just -l  # List recipes");
        assert_eq!(usage.command, "just -l");
        assert_eq!(usage.description, "List recipes");
        assert_eq!(usage.output, None);
    }

    #[test]
    fn test_usage_from_str_no_description() {
        let usage = UsageExample::from_str("cargo build");
        assert_eq!(usage.command, "cargo build");
        assert_eq!(usage.description, "");
    }

    #[test]
    fn test_fragment_hyphen_conversion() {
        let ref1 = Reference::from_url("https://github.com/user/repo#my-cool-feature");
        assert_eq!(ref1.description, "my cool feature");
    }
}
