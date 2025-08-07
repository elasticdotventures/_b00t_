use anyhow::{Context, Result};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

/// Aggregates available learn topics from TOML and markdown files
pub fn get_learn_topics(path: &str) -> Result<Vec<String>> {
    let expanded_path = expand_path(path);
    let learn_dir = expanded_path.join("learn");
    let learn_config_path = expanded_path.parent()
        .ok_or_else(|| anyhow::anyhow!("Cannot get parent directory"))?
        .join("learn.toml");

    let mut toml_topics: HashMap<String, String> = HashMap::new();
    if learn_config_path.exists() {
        let config_content = fs::read_to_string(&learn_config_path).context(format!(
            "Failed to read learn.toml from {}",
            learn_config_path.display()
        ))?;
        let learn_config: LearnConfig = toml::from_str(&config_content).context(format!(
            "Failed to parse learn.toml from {}",
            learn_config_path.display()
        ))?;
        toml_topics = learn_config.topics;
    }

    let mut md_topics: HashMap<String, PathBuf> = HashMap::new();
    if learn_dir.exists() {
        for entry in fs::read_dir(&learn_dir)? {
            let entry = entry?;
            let path = entry.path();
            if let Some(fname) = path.file_name().and_then(|s| s.to_str()) {
                if fname.ends_with(".md") {
                    let topic_name = fname.trim_end_matches(".md");
                    md_topics.insert(topic_name.to_string(), path.clone());
                }
            }
        }
    }

    let mut all_topics: HashMap<String, PathBuf> = HashMap::new();
    for (topic, rel_path) in &toml_topics {
        let topic_file_path = expanded_path.parent()
            .ok_or_else(|| anyhow::anyhow!("Cannot get parent directory"))?
            .join(rel_path);
        all_topics.insert(topic.clone(), topic_file_path);
    }
    for (topic, md_path) in &md_topics {
        all_topics.entry(topic.clone()).or_insert(md_path.clone());
    }

    Ok(all_topics.keys().cloned().collect())
}

/// Returns the lesson/documentation for a topic
pub fn get_learn_lesson(path: &str, topic: &str) -> Result<String> {
    let expanded_path = expand_path(path);
    let learn_dir = expanded_path.join("learn");
    let learn_config_path = expanded_path.parent()
        .ok_or_else(|| anyhow::anyhow!("Cannot get parent directory"))?
        .join("learn.toml");

    let mut toml_topics: HashMap<String, String> = HashMap::new();
    if learn_config_path.exists() {
        let config_content = fs::read_to_string(&learn_config_path).context(format!(
            "Failed to read learn.toml from {}",
            learn_config_path.display()
        ))?;
        let learn_config: LearnConfig = toml::from_str(&config_content).context(format!(
            "Failed to parse learn.toml from {}",
            learn_config_path.display()
        ))?;
        toml_topics = learn_config.topics;
    }

    let mut md_topics: HashMap<String, PathBuf> = HashMap::new();
    if learn_dir.exists() {
        for entry in fs::read_dir(&learn_dir)? {
            let entry = entry?;
            let path = entry.path();
            if let Some(fname) = path.file_name().and_then(|s| s.to_str()) {
                if fname.ends_with(".md") {
                    let topic_name = fname.trim_end_matches(".md");
                    md_topics.insert(topic_name.to_string(), path.clone());
                }
            }
        }
    }

    let mut all_topics: HashMap<String, PathBuf> = HashMap::new();
    for (topic, rel_path) in &toml_topics {
        let topic_file_path = expanded_path.parent()
            .ok_or_else(|| anyhow::anyhow!("Cannot get parent directory"))?
            .join(rel_path);
        all_topics.insert(topic.clone(), topic_file_path);
    }
    for (topic, md_path) in &md_topics {
        all_topics.entry(topic.clone()).or_insert(md_path.clone());
    }

    if let Some(topic_file_path) = all_topics.get(topic) {
        if !topic_file_path.exists() {
            anyhow::bail!("Topic file not found: {}.", topic_file_path.display());
        }
        let content = fs::read_to_string(&topic_file_path).context(format!(
            "Failed to read topic file from {}",
            topic_file_path.display()
        ))?;
        Ok(content)
    } else {
        let available_topics: Vec<String> = all_topics.keys().cloned().collect();
        anyhow::bail!(
            "Topic '{}' not found. Available topics: {}",
            topic,
            available_topics.join(", ")
        );
    }
}

/// Record a lesson for a tool/topic
pub fn record_lesson(path: &str, tool: &str, lesson: &str) -> Result<()> {
    let expanded_path = expand_path(path);
    let learn_dir = expanded_path.join("learn");
    std::fs::create_dir_all(&learn_dir).context("Failed to create learn directory")?;
    let tool_file = learn_dir.join(format!("{}.md", tool));
    let mut file = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&tool_file)
        .context(format!("Failed to open or create {}", tool_file.display()))?;
    use std::io::Write;
    writeln!(file, "---\n{}\n", lesson).context("Failed to write lesson")?;
    Ok(())
}

fn expand_path(path: &str) -> PathBuf {
    // Simple home expansion for now
    if path.starts_with("~") {
        if let Some(home) = dirs::home_dir() {
            return home.join(path.trim_start_matches("~/"));
        }
    }
    PathBuf::from(path)
}

#[derive(Debug, serde::Deserialize)]
struct LearnConfig {
    topics: HashMap<String, String>,
}
