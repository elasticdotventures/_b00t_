//! Man page parser and formatter
//!
//! Provides utilities to parse, paginate, and format man pages for display
//! with table of contents and auto-datum creation

use anyhow::{Context, Result};
use std::process::Command;

#[derive(Debug, Clone)]
pub struct ManPage {
    pub name: String,
    pub version: Option<String>,
    pub description: Option<String>,
    pub sections: Vec<ManSection>,
    pub raw_content: String,
}

#[derive(Debug, Clone)]
pub struct ManSection {
    pub title: String,
    pub content: String,
    pub line_start: usize,
    pub line_end: usize,
}

impl ManPage {
    /// Parse man page from command
    pub fn from_command(cmd: &str) -> Result<Self> {
        // Try to get man page content
        let output = Command::new("man")
            .arg(cmd)
            .output()
            .context("Failed to execute man command")?;

        if !output.status.success() {
            anyhow::bail!("No man page found for '{}'", cmd);
        }

        let content = String::from_utf8_lossy(&output.stdout).to_string();
        Self::parse(&content, cmd)
    }

    /// Parse man page content
    fn parse(content: &str, name: &str) -> Result<Self> {
        let mut sections = Vec::new();
        let lines: Vec<&str> = content.lines().collect();

        let mut current_section: Option<(String, usize)> = None;
        let mut description = None;

        for (idx, line) in lines.iter().enumerate() {
            // Detect section headers (all caps, often at start of line)
            if {
                let trimmed = line.trim();
                // Require minimal indentation (no more than 2 leading spaces)
                let leading_spaces = line.chars().take_while(|c| c.is_whitespace()).count();
                // Require line to be reasonably short (<= 40 chars)
                let max_length = 40;
                // Check next line is not another header or empty
                let next_line = lines.get(idx + 1).map(|l| l.trim()).unwrap_or("");
                trimmed.chars().all(|c| {
                    c.is_uppercase() || c.is_whitespace() || c == '-' || c == '(' || c == ')'
                }) && !trimmed.is_empty()
                    && trimmed.len() > 2
                    && leading_spaces <= 2
                    && trimmed.len() <= max_length
                    && !next_line.is_empty()
                    && !next_line.chars().all(|c| {
                        c.is_uppercase() || c.is_whitespace() || c == '-' || c == '(' || c == ')'
                    })
            } {
                // Save previous section
                if let Some((title, start)) = current_section.take() {
                    let section_content: String = lines[start..idx]
                        .iter()
                        .map(|l| l.to_string())
                        .collect::<Vec<_>>()
                        .join("\n");

                    sections.push(ManSection {
                        title: title.clone(),
                        content: section_content.trim().to_string(),
                        line_start: start,
                        line_end: idx,
                    });

                    // Extract description from DESCRIPTION section
                    if title.contains("DESCRIPTION") && description.is_none() {
                        description = Some(
                            section_content
                                .lines()
                                .take(3)
                                .collect::<Vec<_>>()
                                .join(" ")
                                .trim()
                                .to_string(),
                        );
                    }
                }

                current_section = Some((line.trim().to_string(), idx + 1));
            }
        }

        // Save last section
        if let Some((title, start)) = current_section {
            let section_content: String = lines[start..]
                .iter()
                .map(|l| l.to_string())
                .collect::<Vec<_>>()
                .join("\n");

            sections.push(ManSection {
                title,
                content: section_content.trim().to_string(),
                line_start: start,
                line_end: lines.len(),
            });
        }

        Ok(ManPage {
            name: name.to_string(),
            version: Self::detect_version(name),
            description,
            sections,
            raw_content: content.to_string(),
        })
    }

    /// Detect version from --version or man page
    pub fn detect_version(cmd: &str) -> Option<String> {
        // Try --version flag
        let output = Command::new(cmd).arg("--version").output().ok()?;

        if output.status.success() {
            let version_str = String::from_utf8_lossy(&output.stdout);
            // Extract first line, often contains version
            let first_line = version_str.lines().next()?;

            // Look for version patterns like "v1.2.3", "1.2.3", "version 1.2.3"
            if let Some(version) = first_line.split_whitespace().find(|s| {
                s.chars().next().map(|c| c.is_numeric()).unwrap_or(false) || s.starts_with('v')
            }) {
                return Some(version.trim_start_matches('v').to_string());
            }
        }

        None
    }

    /// Generate table of contents
    pub fn get_toc(&self) -> String {
        let mut toc = String::from("Table of Contents:\n");
        for (idx, section) in self.sections.iter().enumerate() {
            toc.push_str(&format!("[{}] {}\n", idx + 1, section.title));
        }
        toc
    }

    /// Paginate by sections or line chunks
    pub fn paginate(&self, lines_per_page: usize) -> Vec<String> {
        // Prioritize section-based pagination
        if !self.sections.is_empty() {
            return self
                .sections
                .iter()
                .map(|s| format!("## {}\n\n{}", s.title, s.content))
                .collect();
        }

        // Fall back to line-based pagination
        let lines: Vec<&str> = self.raw_content.lines().collect();
        lines
            .chunks(lines_per_page)
            .map(|chunk| chunk.join("\n"))
            .collect()
    }

    /// Convert to markdown (concise mode for agents)
    pub fn to_markdown(&self, concise: bool) -> String {
        if concise {
            self.to_concise_markdown()
        } else {
            self.to_full_markdown()
        }
    }

    fn to_concise_markdown(&self) -> String {
        let mut md = format!(
            "# {} ({})\n\n",
            self.name,
            self.version.as_deref().unwrap_or("unknown")
        );

        if let Some(desc) = &self.description {
            md.push_str(&format!("{}\n\n", desc));
        }

        // Show only key sections: NAME, SYNOPSIS, DESCRIPTION
        for section in &self.sections {
            if section.title.contains("NAME")
                || section.title.contains("SYNOPSIS")
                || section.title.contains("DESCRIPTION")
            {
                md.push_str(&format!("## {}\n{}\n\n", section.title, section.content));
            }
        }

        md.push_str(&format!("\nFull manual: `man {}`\n", self.name));
        md
    }

    fn to_full_markdown(&self) -> String {
        let mut md = format!("# {} Manual\n\n", self.name);

        if let Some(version) = &self.version {
            md.push_str(&format!("Version: {}\n\n", version));
        }

        for section in &self.sections {
            md.push_str(&format!("## {}\n\n{}\n\n", section.title, section.content));
        }

        md
    }

    /// Generate datum TOML content for auto-creation
    pub fn to_datum_toml(&self) -> String {
        let description = self
            .description
            .as_deref()
            .unwrap_or("Command line utility")
            .lines()
            .next()
            .unwrap_or("Command line utility");

        format!(
            r#"[b00t]
name = "{}"
type = "cli"
hint = "{}"
desires = "{}"
lfmf_category = "{}"

[b00t.learn]
inline = "See: man {}"

# Auto-generated from man page
# Edit to add usage examples, dependencies, etc.
"#,
            self.name,
            description,
            self.version.as_deref().unwrap_or("latest"),
            self.name,
            self.name
        )
    }

    /// Get specific section by number (1-indexed)
    pub fn get_section(&self, section_num: usize) -> Option<&ManSection> {
        self.sections.get(section_num.saturating_sub(1))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_man_page_parsing() {
        let content = r#"
NAME
       ls - list directory contents

SYNOPSIS
       ls [OPTION]... [FILE]...

DESCRIPTION
       List information about the FILEs (the current directory by default).
       Sort entries alphabetically if none of -cftuvSUX nor --sort is specified.
"#;

        let man_page = ManPage::parse(content, "ls").unwrap();
        assert_eq!(man_page.name, "ls");
        assert!(man_page.sections.len() >= 2);
        assert!(man_page.description.is_some());
    }

    #[test]
    fn test_toc_generation() {
        let content = "NAME\nls\nSYNOPSIS\nls [OPTION]\nDESCRIPTION\nList files";
        let man_page = ManPage::parse(content, "ls").unwrap();
        let toc = man_page.get_toc();
        assert!(toc.contains("[1]"));
    }
}
