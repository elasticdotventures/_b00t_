use crate::{DatumType, get_config, get_expanded_path};
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

/// Display agent identity information from AGENT.md template and role datum (if available)
pub fn whoami(path: &str, role_override: Option<String>) -> Result<()> {
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

    // Append role summary if we can resolve a role datum
    if let Some(role) = resolve_role(role_override) {
        if let Some(role_details) = load_role_datum(&role, path) {
            print_role_summary(&role_details);
        } else {
            println!(
                "⚠️ Role datum '{}' not found or missing required fields",
                role
            );
        }
    }

    Ok(())
}

#[derive(Clone, Debug, PartialEq)]
struct RoleDetails {
    name: String,
    hint: String,
    skills: Vec<String>,
    compliance: Vec<String>,
}

fn resolve_role(role_override: Option<String>) -> Option<String> {
    role_override
        .filter(|r| !r.trim().is_empty())
        .or_else(|| std::env::var("_B00T_ROLE").ok())
        .map(|r| r.to_lowercase())
}

fn load_role_datum(role: &str, path: &str) -> Option<RoleDetails> {
    let (config, _) = get_config(role, path).ok()?;
    let datum = config.b00t;

    if let Some(datum_type) = &datum.datum_type {
        if datum_type != &DatumType::Role {
            return None;
        }
    } else {
        // Without an explicit type we assume a generic datum is intended for this role
    }

    let skills = datum.skills.unwrap_or_default();
    let compliance = datum.compliance.unwrap_or_default();

    Some(RoleDetails {
        name: datum.name,
        hint: datum.hint,
        skills,
        compliance,
    })
}

fn summarize_list(items: &[String], max_items: usize) -> Option<String> {
    if items.is_empty() {
        return None;
    }

    let shown: Vec<String> = items.iter().take(max_items).cloned().collect();
    let remaining = items.len().saturating_sub(shown.len());

    let mut summary = shown.join(", ");
    if remaining > 0 {
        summary.push_str(&format!(" (+{} more)", remaining));
    }

    Some(summary)
}

fn print_role_summary(role: &RoleDetails) {
    println!("🎭 Role: {}", role.name);
    println!("💡 {}", role.hint);

    if let Some(skills_summary) = summarize_list(&role.skills, 5) {
        println!("🧠 Skills: {}", skills_summary);
    }

    if let Some(compliance_summary) = summarize_list(&role.compliance, 3) {
        println!("⚖️ Compliance: {}", compliance_summary);
    }
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

    #[test]
    fn test_resolve_role_prefers_override() {
        unsafe {
            std::env::set_var("_B00T_ROLE", "captain");
        }
        let resolved = resolve_role(Some("executive".to_string()));
        assert_eq!(resolved, Some("executive".to_string()));
        unsafe {
            std::env::remove_var("_B00T_ROLE");
        }
    }

    #[test]
    fn test_summarize_list_limits_output() {
        let items = vec![
            "a".to_string(),
            "b".to_string(),
            "c".to_string(),
            "d".to_string(),
        ];
        let summary = summarize_list(&items, 2).unwrap();
        assert_eq!(summary, "a, b (+2 more)");
    }
}
