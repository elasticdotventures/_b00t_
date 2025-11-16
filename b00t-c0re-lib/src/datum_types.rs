//! Datum types for enhanced b00t schema
//! These types extend BootDatum with learn and usage metadata

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
