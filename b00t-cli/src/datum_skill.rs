//! Skill datum type - Progressive disclosure knowledge for Claude agents
//!
//! Skills provide composable, lightweight instructions and examples for agents
//! following the Claude Skills pattern for token-efficient knowledge transfer.
//!
//! # Progressive Disclosure Pattern
//!
//! Skills implement a three-tier loading strategy:
//! 1. **Metadata** (~100 tokens): applies_to, output_types, dependencies
//! 2. **Instructions** (<5k tokens): Loaded on-demand from instructions_file
//! 3. **Files/Scripts**: Full content loaded only when needed
//!
//! # Example
//!
//! ```toml
//! [b00t]
//! name = "prd-to-job"
//! type = "skill"
//! hint = "Generate job workflows from PRD documents"
//!
//! [b00t.skill]
//! description = "Converts Product Requirements Documents to executable job workflows"
//! instructions_file = "prd-to-job.md"
//! examples = ["example.prd.txt", "example-output.job.toml"]
//! tags = ["workflow", "planning"]
//!
//! [b00t.skill.metadata]
//! applies_to = ["workflow planning", "task generation", "prd conversion"]
//! output_types = [".job.toml"]
//! dependencies = []
//! ```

use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

/// Skill datum - Defines progressive disclosure knowledge pattern
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkillDatum {
    #[serde(flatten)]
    pub datum: crate::BootDatum,
}

/// YAML frontmatter extracted from agentskills.io SKILL.md format
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct SkillMdFrontmatter {
    pub name: String,
    pub description: String,
    #[serde(default)]
    pub tags: Vec<String>,
    #[serde(default)]
    pub applies_to: Vec<String>,
    #[serde(default)]
    pub output_types: Vec<String>,
    #[serde(default)]
    pub dependencies: Vec<String>,
    pub author: Option<String>,
    pub version: Option<String>,
}

/// Skill configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkillConfig {
    /// Brief description of what the skill provides
    pub description: String,

    /// Path to instructions markdown file (relative to skill directory)
    /// Empty when skill was parsed from SKILL.md (use instructions_inline instead)
    pub instructions_file: String,

    /// Inline instructions body — set when parsed from SKILL.md format
    /// Takes precedence over instructions_file in load_instructions()
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub instructions_inline: Option<String>,

    /// Example files demonstrating the skill (relative to skill directory)
    #[serde(default)]
    pub examples: Vec<String>,

    /// Tags for skill categorization and discovery
    #[serde(default)]
    pub tags: Vec<String>,

    /// Metadata for progressive disclosure (loaded first, ~100 tokens)
    pub metadata: SkillMetadata,

    /// Optional template files for output generation
    #[serde(default)]
    pub templates: Vec<String>,
}

/// Lightweight metadata for skill discovery and filtering
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkillMetadata {
    /// What contexts this skill applies to
    pub applies_to: Vec<String>,

    /// What file types this skill can generate
    pub output_types: Vec<String>,

    /// Other skills this skill depends on
    #[serde(default)]
    pub dependencies: Vec<String>,

    /// Optional author attribution
    #[serde(default)]
    pub author: Option<String>,

    /// Skill version
    #[serde(default)]
    pub version: Option<String>,
}

impl SkillDatum {
    /// Parse agentskills.io SKILL.md format into a SkillDatum.
    ///
    /// SKILL.md format:
    /// ```markdown
    /// ---
    /// name: my-skill
    /// description: When to use this skill.
    /// tags: [rust, cli]
    /// applies_to: [code generation, cli tooling]
    /// output_types: [.rs]
    /// ---
    /// # My Skill
    /// Full instructions body...
    /// ```
    ///
    /// Instructions are stored inline (`instructions_inline`); no file I/O needed.
    pub fn from_skill_md(path: &Path) -> Result<Self> {
        let content = std::fs::read_to_string(path)
            .map_err(|e| anyhow::anyhow!("Failed to read SKILL.md at {}: {}", path.display(), e))?;

        let (frontmatter, body) = parse_skill_md_frontmatter(&content)?;

        let skill_config = SkillConfig {
            description: frontmatter.description.clone(),
            // Use the SKILL.md path as instructions_file so validation passes,
            // while still providing the parsed body via instructions_inline.
            instructions_file: path.to_string_lossy().into_owned(),
            instructions_inline: Some(body),
            examples: vec![],
            tags: frontmatter.tags.clone(),
            metadata: SkillMetadata {
                applies_to: if frontmatter.applies_to.is_empty() {
                    // fall back: derive from description if no applies_to
                    vec![frontmatter.description.clone()]
                } else {
                    frontmatter.applies_to.clone()
                },
                output_types: frontmatter.output_types.clone(),
                dependencies: frontmatter.dependencies.clone(),
                author: frontmatter.author.clone(),
                version: frontmatter.version.clone(),
            },
            templates: vec![],
        };

        let skill_json = serde_json::to_value(&skill_config)
            .map_err(|e| anyhow::anyhow!("Skill serialization failed: {}", e))?;

        let datum = crate::BootDatum {
            name: frontmatter.name.clone(),
            datum_type: Some(crate::DatumType::Skill),
            hint: frontmatter.description.clone(),
            skill: Some(skill_json),
            ..Default::default()
        };

        Ok(SkillDatum { datum })
    }

    /// Load skill from TOML file
    pub fn from_config(name: &str, path: &str) -> Result<Self> {
        // Strip .skill.toml extension if present
        let base_name = name.trim_end_matches(".skill.toml");
        let (config, _filename) =
            crate::get_config(base_name, path).map_err(|e| anyhow::anyhow!("{}", e))?;
        Ok(SkillDatum { datum: config.b00t })
    }

    /// Get skill configuration
    pub fn skill_config(&self) -> Result<SkillConfig> {
        let skill_value = self
            .datum
            .skill
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("No skill configuration found"))?;

        serde_json::from_value(skill_value.clone())
            .map_err(|e| anyhow::anyhow!("Failed to parse skill config: {}", e))
    }

    /// Validate skill definition
    pub fn validate(&self) -> Result<()> {
        let config = self.skill_config()?;

        // Ensure instructions file exists
        if config.instructions_file.is_empty() {
            anyhow::bail!("Skill must specify instructions_file");
        }

        // Ensure at least one applies_to context
        if config.metadata.applies_to.is_empty() {
            anyhow::bail!("Skill metadata must specify at least one applies_to context");
        }

        // Ensure at least one output type
        if config.metadata.output_types.is_empty() {
            anyhow::bail!("Skill metadata must specify at least one output_type");
        }

        Ok(())
    }

    /// Load instructions content — inline body (SKILL.md) takes precedence over file
    pub fn load_instructions(&self, skill_base_path: &PathBuf) -> Result<String> {
        let config = self.skill_config()?;

        // Inline instructions (from SKILL.md parsing) — no file I/O needed
        if let Some(inline) = &config.instructions_inline {
            return Ok(inline.clone());
        }

        let instructions_path = skill_base_path.join(&config.instructions_file);

        if !instructions_path.exists() {
            anyhow::bail!(
                "Instructions file not found: {}",
                instructions_path.display()
            );
        }

        std::fs::read_to_string(&instructions_path).map_err(|e| {
            anyhow::anyhow!(
                "Failed to read instructions from {}: {}",
                instructions_path.display(),
                e
            )
        })
    }

    /// Load example files (lazy, on-demand)
    pub fn load_examples(&self, skill_base_path: &PathBuf) -> Result<Vec<(String, String)>> {
        let config = self.skill_config()?;
        let mut examples = Vec::new();

        for example_file in &config.examples {
            let example_path = skill_base_path.join(example_file);

            if !example_path.exists() {
                anyhow::bail!("Example file not found: {}", example_path.display());
            }

            let content = std::fs::read_to_string(&example_path).map_err(|e| {
                anyhow::anyhow!(
                    "Failed to read example from {}: {}",
                    example_path.display(),
                    e
                )
            })?;

            examples.push((example_file.clone(), content));
        }

        Ok(examples)
    }

    /// Load template files (lazy, on-demand)
    pub fn load_templates(&self, skill_base_path: &PathBuf) -> Result<Vec<(String, String)>> {
        let config = self.skill_config()?;
        let mut templates = Vec::new();

        for template_file in &config.templates {
            let template_path = skill_base_path.join(template_file);

            if !template_path.exists() {
                anyhow::bail!("Template file not found: {}", template_path.display());
            }

            let content = std::fs::read_to_string(&template_path).map_err(|e| {
                anyhow::anyhow!(
                    "Failed to read template from {}: {}",
                    template_path.display(),
                    e
                )
            })?;

            templates.push((template_file.clone(), content));
        }

        Ok(templates)
    }

    /// Check if skill applies to given context
    pub fn applies_to(&self, context: &str) -> bool {
        if let Ok(config) = self.skill_config() {
            config
                .metadata
                .applies_to
                .iter()
                .any(|ctx| ctx.to_lowercase().contains(&context.to_lowercase()))
        } else {
            false
        }
    }

    /// Check if skill can generate given output type
    pub fn can_generate(&self, output_type: &str) -> bool {
        if let Ok(config) = self.skill_config() {
            config
                .metadata
                .output_types
                .iter()
                .any(|ot| ot == output_type)
        } else {
            false
        }
    }

    /// Get skill dependencies
    pub fn dependencies(&self) -> Vec<String> {
        if let Ok(config) = self.skill_config() {
            config.metadata.dependencies.clone()
        } else {
            vec![]
        }
    }

    /// Get lightweight metadata summary (~100 tokens)
    pub fn metadata_summary(&self) -> Result<String> {
        let config = self.skill_config()?;

        Ok(format!(
            "**{}** ({})\n- Applies to: {}\n- Generates: {}\n- Dependencies: {}",
            self.datum.name,
            config.description,
            config.metadata.applies_to.join(", "),
            config.metadata.output_types.join(", "),
            if config.metadata.dependencies.is_empty() {
                "none".to_string()
            } else {
                config.metadata.dependencies.join(", ")
            }
        ))
    }
}

/// Parse YAML frontmatter + body from SKILL.md content.
/// Returns (frontmatter, body) where body is everything after the closing `---`.
fn parse_skill_md_frontmatter(content: &str) -> Result<(SkillMdFrontmatter, String)> {
    // Must start with ---
    let rest = content
        .strip_prefix("---")
        .ok_or_else(|| anyhow::anyhow!("SKILL.md must start with YAML frontmatter (---)"))?;

    // Find closing ---
    let end = rest
        .find("\n---")
        .ok_or_else(|| anyhow::anyhow!("SKILL.md frontmatter not closed (missing closing ---)"))?;

    let yaml = &rest[..end];
    let body = rest[end + 4..].trim_start_matches('\n').to_string(); // skip "\n---\n"

    let frontmatter: SkillMdFrontmatter = serde_yaml::from_str(yaml)
        .map_err(|e| anyhow::anyhow!("Failed to parse SKILL.md frontmatter: {}", e))?;

    if frontmatter.name.is_empty() {
        anyhow::bail!("SKILL.md frontmatter must include 'name'");
    }
    if frontmatter.description.is_empty() {
        anyhow::bail!("SKILL.md frontmatter must include 'description'");
    }

    Ok((frontmatter, body))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_skill_datum_structure() {
        let skill_toml = r#"
[b00t]
name = "test-skill"
type = "skill"
hint = "Test skill"
usage = []

[b00t.skill]
description = "Test skill for unit testing"
instructions_file = "test.md"
examples = ["example.txt"]
tags = ["test"]

[b00t.skill.metadata]
applies_to = ["testing", "validation"]
output_types = [".txt", ".md"]
dependencies = []
"#;

        let config: crate::UnifiedConfig = toml::from_str(skill_toml).unwrap();
        let datum = SkillDatum { datum: config.b00t };

        assert_eq!(datum.datum.name, "test-skill");
        assert!(datum.validate().is_ok());

        let skill_config = datum.skill_config().unwrap();
        assert_eq!(skill_config.description, "Test skill for unit testing");
        assert_eq!(skill_config.instructions_file, "test.md");
        assert_eq!(skill_config.examples, vec!["example.txt"]);
        assert_eq!(
            skill_config.metadata.applies_to,
            vec!["testing", "validation"]
        );
        assert_eq!(skill_config.metadata.output_types, vec![".txt", ".md"]);
    }

    #[test]
    fn test_skill_applies_to() {
        let skill_toml = r#"
[b00t]
name = "workflow-skill"
type = "skill"
hint = "Workflow skill"
usage = []

[b00t.skill]
description = "Workflow planning skill"
instructions_file = "workflow.md"
tags = ["workflow"]

[b00t.skill.metadata]
applies_to = ["workflow planning", "task generation"]
output_types = [".job.toml"]
"#;

        let config: crate::UnifiedConfig = toml::from_str(skill_toml).unwrap();
        let datum = SkillDatum { datum: config.b00t };

        assert!(datum.applies_to("workflow"));
        assert!(datum.applies_to("task generation"));
        assert!(!datum.applies_to("database"));
    }

    #[test]
    fn test_skill_can_generate() {
        let skill_toml = r#"
[b00t]
name = "generator-skill"
type = "skill"
hint = "Generator skill"
usage = []

[b00t.skill]
description = "Multi-format generator"
instructions_file = "gen.md"

[b00t.skill.metadata]
applies_to = ["generation"]
output_types = [".job.toml", ".md", ".txt"]
"#;

        let config: crate::UnifiedConfig = toml::from_str(skill_toml).unwrap();
        let datum = SkillDatum { datum: config.b00t };

        assert!(datum.can_generate(".job.toml"));
        assert!(datum.can_generate(".md"));
        assert!(!datum.can_generate(".rs"));
    }

    #[test]
    fn test_parse_skill_md_frontmatter_minimal() {
        let content = "---\nname: fast-rust\ndescription: Fast, reliable Rust code. Use when writing Rust.\n---\n# Fast Rust\nWrite idiomatic Rust.";
        let (fm, body) = parse_skill_md_frontmatter(content).unwrap();
        assert_eq!(fm.name, "fast-rust");
        assert_eq!(
            fm.description,
            "Fast, reliable Rust code. Use when writing Rust."
        );
        assert!(body.contains("Write idiomatic Rust."));
    }

    #[test]
    fn test_parse_skill_md_frontmatter_full() {
        let content = r#"---
name: pdf-processing
description: Extract PDF text, fill forms, merge files. Use when handling PDFs.
tags: [pdf, document]
applies_to: [document processing, pdf handling]
output_types: [.pdf, .txt]
dependencies: [pdfplumber]
author: acme
version: "1.0"
---
# PDF Processing

## When to use
Use for PDF work.
"#;
        let (fm, body) = parse_skill_md_frontmatter(content).unwrap();
        assert_eq!(fm.name, "pdf-processing");
        assert_eq!(fm.tags, vec!["pdf", "document"]);
        assert_eq!(fm.applies_to, vec!["document processing", "pdf handling"]);
        assert_eq!(fm.output_types, vec![".pdf", ".txt"]);
        assert_eq!(fm.dependencies, vec!["pdfplumber"]);
        assert_eq!(fm.author.as_deref(), Some("acme"));
        assert!(body.contains("Use for PDF work."));
    }

    #[test]
    fn test_from_skill_md_builds_datum() {
        use std::io::Write;
        let content = r#"---
name: test-skill-md
description: Test skill from SKILL.md format.
tags: [test]
applies_to: [testing]
output_types: [.txt]
---
# Test
Instructions here.
"#;
        let mut tmp = tempfile::NamedTempFile::new().unwrap();
        tmp.write_all(content.as_bytes()).unwrap();
        let datum = SkillDatum::from_skill_md(tmp.path()).unwrap();
        assert_eq!(datum.datum.name, "test-skill-md");
        let cfg = datum.skill_config().unwrap();
        assert_eq!(cfg.description, "Test skill from SKILL.md format.");
        assert!(
            cfg.instructions_inline
                .as_deref()
                .unwrap()
                .contains("Instructions here.")
        );
        // load_instructions returns inline content without needing a real path
        let instructions = datum
            .load_instructions(&PathBuf::from("/nonexistent"))
            .unwrap();
        assert!(instructions.contains("Instructions here."));
    }

    #[test]
    fn test_parse_skill_md_missing_delimiter_errors() {
        let bad = "name: foo\ndescription: bar\n";
        assert!(parse_skill_md_frontmatter(bad).is_err());
    }

    #[test]
    fn test_parse_skill_md_missing_name_errors() {
        let bad = "---\ndescription: something\n---\nbody";
        assert!(parse_skill_md_frontmatter(bad).is_err());
    }

    #[test]
    fn test_metadata_summary() {
        let skill_toml = r#"
[b00t]
name = "prd-to-job"
type = "skill"
hint = "PRD to Job converter"
usage = []

[b00t.skill]
description = "Converts PRDs to job workflows"
instructions_file = "prd.md"

[b00t.skill.metadata]
applies_to = ["workflow planning", "prd conversion"]
output_types = [".job.toml"]
dependencies = ["git", "toml"]
"#;

        let config: crate::UnifiedConfig = toml::from_str(skill_toml).unwrap();
        let datum = SkillDatum { datum: config.b00t };

        let summary = datum.metadata_summary().unwrap();
        assert!(summary.contains("prd-to-job"));
        assert!(summary.contains("Converts PRDs to job workflows"));
        assert!(summary.contains("workflow planning, prd conversion"));
        assert!(summary.contains(".job.toml"));
        assert!(summary.contains("git, toml"));
    }
}
