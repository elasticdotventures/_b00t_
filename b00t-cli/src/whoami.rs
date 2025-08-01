use anyhow::{Context, Result};
use duct::cmd;
use std::fs;
use crate::utils::{is_git_repo, get_workspace_root};
use crate::get_expanded_path;

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

    // Prepare template variables
    let timestamp = chrono::Utc::now()
        .format("%Y-%m-%d %H:%M:%S UTC")
        .to_string();
    let user = std::env::var("USER").unwrap_or_else(|_| "unknown".to_string());
    let branch = cmd!("git", "branch", "--show-current")
        .read()
        .unwrap_or_else(|_| "no-git".to_string())
        .trim()
        .to_string();
    let agent = detect_agent(false);
    let model_size = std::env::var("MODEL_SIZE").unwrap_or_else(|_| "unknown".to_string());
    let privacy = std::env::var("PRIVACY").unwrap_or_else(|_| "standard".to_string());
    let workspace_root = get_workspace_root();
    let is_git = is_git_repo();

    // Simple string replacement approach instead of Tera due to complex template syntax
    let mut rendered = template_content;

    // Replace variables manually
    rendered = rendered.replace("{{PID}}", &std::process::id().to_string());
    rendered = rendered.replace("{{TIMESTAMP}}", &timestamp);
    rendered = rendered.replace("{{USER}}", &user);
    rendered = rendered.replace("{{BRANCH}}", &branch);
    rendered = rendered.replace("{{_B00T_Agent}}", &agent);
    rendered = rendered.replace("{{_B00T_AGENT}}", &agent);
    rendered = rendered.replace("{{MODEL_SIZE}}", &model_size);
    rendered = rendered.replace("{{PRIVACY}}", &privacy);
    rendered = rendered.replace("{{WORKSPACE_ROOT}}", &workspace_root);
    rendered = rendered.replace("{{IS_GIT_REPO}}", &is_git.to_string());
    rendered = rendered.replace("{{GIT_REPO}}", &is_git.to_string());

    println!("{}", rendered);

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_detect_agent_claude() {
        // Clear existing env vars first
        unsafe { std::env::remove_var("_B00T_Agent"); }
        unsafe { std::env::set_var("CLAUDECODE", "1"); }
        assert_eq!(detect_agent(false), "claude");
        unsafe { std::env::remove_var("CLAUDECODE"); }
    }

    #[test]
    fn test_detect_agent_env_variable() {
        unsafe { std::env::remove_var("CLAUDECODE"); }
        unsafe { std::env::set_var("_B00T_Agent", "test-agent"); }
        assert_eq!(detect_agent(false), "test-agent");
        unsafe { std::env::remove_var("_B00T_Agent"); }
    }

    #[test]
    fn test_detect_agent_ignore_env() {
        unsafe { std::env::remove_var("CLAUDECODE"); }
        unsafe { std::env::set_var("_B00T_Agent", "test-agent"); }
        assert_eq!(detect_agent(true), "");
        unsafe { std::env::remove_var("_B00T_Agent"); }
    }
}