use anyhow::{Context, Result};
use serde::Deserialize;
use std::collections::HashMap;
use std::fs;

use crate::get_expanded_path;
use b00t_c0re_lib::TemplateRenderer;

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
            // No topic specified, show the available topics in JSON format
            // Render template using b00t-c0re-lib
            let renderer = TemplateRenderer::with_defaults()
                .context("Failed to create template renderer")?;

            // Serialize topics to JSON
            let topics_json = serde_json::to_string_pretty(&learn_config.topics)
                .context("Failed to serialize topics to JSON")?;

            // Apply template variables to the JSON output
            let rendered = renderer.render(&topics_json)
                .context("Failed to render topics template")?;

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

                // Render topic file using template engine  
                let renderer = TemplateRenderer::with_defaults()
                    .context("Failed to create template renderer")?;
                
                let rendered = renderer.render(&template_content)
                    .context("Failed to render topic template")?;

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