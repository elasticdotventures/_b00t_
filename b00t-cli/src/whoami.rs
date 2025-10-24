use crate::get_expanded_path;
use anyhow::{Context, Result};
use b00t_c0re_lib::TemplateRenderer;
use std::fs;

/// Detect current AI agent based on environment variables
pub fn detect_agent(ignore_env: bool) -> String {
    // Check if _B00T_Agent is already set and we're not ignoring env
    if !ignore_env {
        if let Ok(agent) = std::env::var("_B00T_Agent") {
            if !agent.is_empty() {
                return agent;
            }
        }
    }

    // Check for Claude Code
    if std::env::var("CLAUDECODE").unwrap_or_default() == "1" {
        return "claude".to_string();
    }

    // TODO: Add detection for other agents based on their shell environment:
    // - gemini: specific environment vars set by gemini-cli shell
    // - codex: specific environment vars set by codex shell
    // - other agents: their respective shell environment indicators

    // Return empty string if no agent detected
    "".to_string()
}

/// Display agent identity information from AGENT.md template
pub fn whoami(path: &str) -> Result<()> {
    let expanded_path = get_expanded_path(path)?;
    let agent_md_path = expanded_path.join("AGENT.md");

    if !agent_md_path.exists() {
        anyhow::bail!(
            "AGENT.md not found in {}. This file contains agent identity information.",
            expanded_path.display()
        );
    }

    let template_content = fs::read_to_string(&agent_md_path).context(format!(
        "Failed to read AGENT.md from {}",
        agent_md_path.display()
    ))?;

    // Use b00t-c0re-lib template renderer
    let renderer =
        TemplateRenderer::with_defaults().context("Failed to create template renderer")?;

    let rendered = renderer
        .render(&template_content)
        .context("Failed to render template")?;

    println!("{}", rendered);

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_detect_agent_claude() {
        // Clear existing env vars first
        unsafe {
            std::env::remove_var("_B00T_Agent");
        }
        unsafe {
            std::env::set_var("CLAUDECODE", "1");
        }
        assert_eq!(detect_agent(false), "claude");
        unsafe {
            std::env::remove_var("CLAUDECODE");
        }
    }

    #[test]
    fn test_detect_agent_env_variable() {
        unsafe {
            std::env::remove_var("CLAUDECODE");
        }
        unsafe {
            std::env::set_var("_B00T_Agent", "test-agent");
        }
        assert_eq!(detect_agent(false), "test-agent");
        unsafe {
            std::env::remove_var("_B00T_Agent");
        }
    }

    #[test]
    fn test_detect_agent_ignore_env() {
        unsafe {
            std::env::remove_var("CLAUDECODE");
        }
        unsafe {
            std::env::set_var("_B00T_Agent", "test-agent");
        }
        assert_eq!(detect_agent(true), "");
        unsafe {
            std::env::remove_var("_B00T_Agent");
        }
    }
}
