use anyhow::{Context, Result};
use duct::cmd;
use serde::Deserialize;
use std::collections::HashMap;
use std::fs;

use crate::{get_expanded_path, get_workspace_root, detect_agent};
use crate::utils::{is_git_repo};

#[derive(Debug, Deserialize)]
struct LearnConfig {
    topics: HashMap<String, String>,
}

pub fn handle_learn(path: &str, topic: Option<&str>) -> Result<()> {
    let expanded_path = get_expanded_path(path)?;
    let learn_config_path = expanded_path.parent()
        .ok_or_else(|| anyhow::anyhow!("Cannot get parent directory"))?
        .join("learn.toml");

    if !learn_config_path.exists() {
        anyhow::bail!(
            "learn.toml not found in {}. This file maps topics to their documentation files.",
            learn_config_path.parent().unwrap_or(&learn_config_path).display()
        );
    }

    let config_content = fs::read_to_string(&learn_config_path).context(format!(
        "Failed to read learn.toml from {}",
        learn_config_path.display()
    ))?;

    let learn_config: LearnConfig = toml::from_str(&config_content).context(format!(
        "Failed to parse learn.toml from {}",
        learn_config_path.display()
    ))?;

    match topic {
        None => {
            // No topic specified, show the available topics in JSON format like whoami
            // Apply the same template variable substitution as whoami
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

            // Serialize topics to JSON
            let topics_json = serde_json::to_string_pretty(&learn_config.topics)
                .context("Failed to serialize topics to JSON")?;

            // Apply template variables to the JSON output
            let mut rendered = topics_json;
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
        }
        Some(topic_name) => {
            // Topic specified, load and render the markdown file
            if let Some(relative_path) = learn_config.topics.get(topic_name) {
                let topic_file_path = expanded_path.parent()
                    .ok_or_else(|| anyhow::anyhow!("Cannot get parent directory"))?
                    .join(relative_path);

                if !topic_file_path.exists() {
                    anyhow::bail!(
                        "Topic file not found: {}. Check the path in learn.toml.",
                        topic_file_path.display()
                    );
                }

                let template_content = fs::read_to_string(&topic_file_path).context(format!(
                    "Failed to read topic file from {}",
                    topic_file_path.display()
                ))?;

                // Apply the same template variable substitution as whoami
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

                // Simple string replacement approach (same as whoami)
                let mut rendered = template_content;
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
            } else {
                let available_topics: Vec<String> = learn_config.topics.keys().map(|k| k.clone()).collect();
                anyhow::bail!(
                    "Topic '{}' not found. Available topics: {}",
                    topic_name,
                    available_topics.join(", ")
                );
            }
        }
    }

    Ok(())
}