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

/// External reference metadata such as docs, repos, blogs, or tutorials
///
/// Supports two TOML formats:
/// 1. Concise: `references = ["https://example.com#description", ...]`
/// 2. Verbose: `[[b00t.references]]` with explicit fields
#[derive(Deserialize, Serialize, Debug, Clone, PartialEq)]
pub struct Reference {
    pub url: String,
    pub description: String,
    #[serde(rename = "type")]
    pub ref_type: ReferenceType,
}

impl Reference {
    /// Parse a concise string reference by inferring metadata from URL fragments/domains
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

        Self { url, description, ref_type }
    }

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

    fn generate_description(url: &str, ref_type: &ReferenceType) -> String {
        match ref_type {
            ReferenceType::GitHub => url
                .split('/')
                .rev()
                .find(|segment| !segment.is_empty())
                .unwrap_or("repository")
                .to_string(),
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

/// Custom deserializer for references supporting concise and verbose TOML formats
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
        Some(ReferenceFormat::Strings(entries)) => Ok(Some(
            entries.into_iter().map(|entry| Reference::from_url(&entry)).collect(),
        )),
        Some(ReferenceFormat::Structured(refs)) => Ok(Some(refs)),
    }
}

/// Custom deserializer for usage examples that supports both concise and verbose TOML formats
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
        Some(UsageFormat::Strings(cmds)) => Ok(Some(
            cmds.into_iter().map(|command| UsageExample::from_str(&command)).collect(),
        )),
        Some(UsageFormat::Structured(examples)) => Ok(Some(examples)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_reference_from_url_with_fragment() {
        let reference = Reference::from_url("https://github.com/casey/just#upstream-repo");
        assert_eq!(reference.url, "https://github.com/casey/just");
        assert_eq!(reference.description, "upstream repo");
        assert_eq!(reference.ref_type, ReferenceType::GitHub);
    }

    #[test]
    fn test_reference_from_url_infers_docs() {
        let docs_ref = Reference::from_url("https://just.systems/man/en/");
        assert_eq!(docs_ref.ref_type, ReferenceType::Docs);
        assert_eq!(docs_ref.description, "documentation");
    }

    #[test]
    fn test_reference_fragment_hyphen_conversion() {
        let reference = Reference::from_url("https://github.com/user/repo#my-cool-feature");
        assert_eq!(reference.description, "my cool feature");
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
}
