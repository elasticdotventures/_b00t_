//! # LFMF (Lessons From My Failures) Module
//!
//! Shared implementation of the syntax therapist system that provides:
//! - Recording tribal knowledge from failures/lessons learned
//! - Vector database integration with b00t-grok for semantic search
//! - Filesystem fallback for immediate functionality
//! - Configurable advice retrieval with TOML/env configuration

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::{env, fs, path::Path};
use chrono::{DateTime, Utc};

use crate::grok::GrokClient;

/// Configuration for advice/LFMF system
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LfmfConfig {
    #[serde(default)]
    pub qdrant: QdrantConfig,
    #[serde(default)]
    pub filesystem: FilesystemConfig,
}

impl Default for LfmfConfig {
    fn default() -> Self {
        Self {
            qdrant: QdrantConfig::default(),
            filesystem: FilesystemConfig::default(),
        }
    }
}

/// Qdrant vector database configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QdrantConfig {
    #[serde(default = "default_qdrant_url")]
    pub url: String,
    #[serde(default)]
    pub api_key: Option<String>,
    #[serde(default)]
    pub collection: Option<String>,
}

impl Default for QdrantConfig {
    fn default() -> Self {
        Self {
            url: default_qdrant_url(),
            api_key: None,
            collection: None,
        }
    }
}

fn default_qdrant_url() -> String {
    "http://localhost:6334".to_string()
}

/// Filesystem storage configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FilesystemConfig {
    #[serde(default = "default_learn_dir")]
    pub learn_dir: String,
}

impl Default for FilesystemConfig {
    fn default() -> Self {
        Self {
            learn_dir: default_learn_dir(),
        }
    }
}

fn default_learn_dir() -> String {
    "learn".to_string()
}

/// A lesson learned from failures - stored in both vector DB and filesystem
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Lesson {
    pub tool: String,
    pub topic: String,
    pub content: String,
    pub timestamp: DateTime<Utc>,
    pub confidence: Option<f32>,
    pub error_pattern: Option<String>,
    pub solution: Option<String>,
    pub tags: Vec<String>,
}

/// LFMF system for recording and retrieving tribal knowledge
pub struct LfmfSystem {
    config: LfmfConfig,
    grok_client: Option<GrokClient>,
}

impl LfmfSystem {
    /// Create a new LFMF system with configuration
    pub fn new(config: LfmfConfig) -> Self {
        Self {
            config,
            grok_client: None,
        }
    }

    /// Load configuration from TOML file or environment variables
    pub fn load_config(path: &str) -> Result<LfmfConfig> {
        let config_path = Path::new(path).join("lfmf.toml");
        
        let mut config = if config_path.exists() {
            let content = fs::read_to_string(&config_path)
                .context(format!("Failed to read LFMF config from {}", config_path.display()))?;
            toml::from_str(&content)
                .context("Failed to parse lfmf.toml")?
        } else {
            LfmfConfig::default()
        };

        // Override with environment variables if present
        if let Ok(url) = env::var("QDRANT_URL") {
            config.qdrant.url = url;
        }
        
        if let Ok(api_key) = env::var("QDRANT_API_KEY") {
            config.qdrant.api_key = Some(api_key);
        }
        
        if let Ok(collection) = env::var("QDRANT_COLLECTION") {
            config.qdrant.collection = Some(collection);
        }

        if let Ok(learn_dir) = env::var("B00T_LEARN_DIR") {
            config.filesystem.learn_dir = learn_dir;
        }

        Ok(config)
    }

    /// Initialize the system with vector database connection
    pub async fn initialize(&mut self) -> Result<()> {
        let mut client = GrokClient::new();
        
        // Set up environment variables for the grok client
        if let Some(ref api_key) = self.config.qdrant.api_key {
            unsafe { std::env::set_var("QDRANT_API_KEY", api_key); }
        }
        unsafe { std::env::set_var("QDRANT_URL", &self.config.qdrant.url); }
        
        match client.initialize().await {
            Ok(_) => {
                self.grok_client = Some(client);
                Ok(())
            },
            Err(e) => {
                // Vector DB not available, will use filesystem fallback
                Err(e)
            }
        }
    }

    /// Record a lesson learned from failure
    pub async fn record_lesson(&mut self, tool: &str, lesson: &str) -> Result<()> {
        let lesson_obj = self.parse_lesson(tool, lesson)?;

        // Try to store in vector database first
        if let Some(ref client) = self.grok_client {
            if let Err(e) = self.store_in_vector_db(client, &lesson_obj).await {
                eprintln!("⚠️ Failed to store in vector database: {}", e);
            }
        }

        // Always store in filesystem as backup
        self.store_in_filesystem(&lesson_obj)?;
        
        Ok(())
    }

    /// Parse lesson string into structured format
    fn parse_lesson(&self, tool: &str, lesson: &str) -> Result<Lesson> {
        let timestamp = Utc::now();
        
        // Try to extract topic and content from "topic: content" format
        let (topic, content) = if let Some((topic_part, content_part)) = lesson.split_once(':') {
            (topic_part.trim().to_string(), content_part.trim().to_string())
        } else {
            ("General".to_string(), lesson.to_string())
        };

        Ok(Lesson {
            tool: tool.to_string(),
            topic: topic.clone(),
            content: content.clone(),
            timestamp,
            confidence: Some(1.0), // High confidence for manually recorded lessons
            error_pattern: Some(topic),
            solution: Some(content),
            tags: vec![tool.to_string(), "lfmf".to_string()],
        })
    }

    /// Store lesson in vector database
    async fn store_in_vector_db(&self, client: &GrokClient, lesson: &Lesson) -> Result<()> {
        // Create content for vector storage
        let vector_content = format!("Tool: {}\nTopic: {}\nSolution: {}", 
            lesson.tool, lesson.topic, lesson.content);

        // Store using grok digest functionality
        client.digest(&lesson.tool, &vector_content).await
            .context("Failed to store lesson in vector database")?;

        Ok(())
    }

    /// Store lesson in filesystem
    fn store_in_filesystem(&self, lesson: &Lesson) -> Result<()> {
        let learn_dir = Path::new(&self.config.filesystem.learn_dir);
        let tool_file = learn_dir.join(format!("{}.md", lesson.tool));

        // Ensure directory exists
        if let Some(parent) = tool_file.parent() {
            fs::create_dir_all(parent)
                .context("Failed to create learn directory")?;
        }

        // Format lesson entry
        let entry = format!("---\n{}: {}\n", lesson.topic, lesson.content);

        // Append to tool file
        fs::write(&tool_file, if tool_file.exists() {
            let existing = fs::read_to_string(&tool_file)?;
            format!("{}\n{}", existing, entry)
        } else {
            entry
        })
        .context(format!("Failed to write lesson to {}", tool_file.display()))?;

        Ok(())
    }

    /// Get advice for a specific error/query
    pub async fn get_advice(&mut self, tool: &str, query: &str, count: Option<usize>) -> Result<Vec<String>> {
        let max_results = count.unwrap_or(5);

        // Try vector database first if available
        if let Some(ref client) = self.grok_client {
            match self.get_vector_advice(client, tool, query, max_results).await {
                Ok(advice) => return Ok(advice),
                Err(e) => {
                    eprintln!("🔄 Vector database query failed: {}, using filesystem fallback", e);
                }
            }
        }

        // Fallback to filesystem
        self.get_filesystem_advice(tool, query, max_results)
    }

    /// Get advice from vector database
    async fn get_vector_advice(&self, client: &GrokClient, tool: &str, query: &str, max_results: usize) -> Result<Vec<String>> {
        let results = client.ask(query, Some(tool), Some(max_results)).await?;
        
        Ok(results.results.into_iter()
            .take(max_results)
            .map(|r| r.content)
            .collect())
    }

    /// Get advice from filesystem using pattern matching
    fn get_filesystem_advice(&self, tool: &str, query: &str, max_results: usize) -> Result<Vec<String>> {
        let learn_dir = Path::new(&self.config.filesystem.learn_dir);
        let tool_file = learn_dir.join(format!("{}.md", tool));

        if !tool_file.exists() {
            return Ok(vec![format!("💡 No lessons found for tool '{}'. Record lessons with: lfmf {} \"<topic>: <lesson>\"", tool, tool)]);
        }

        let content = fs::read_to_string(&tool_file)
            .context(format!("Failed to read lessons from {}", tool_file.display()))?;

        let lessons: Vec<&str> = content.split("---\n").filter(|s| !s.trim().is_empty()).collect();
        let query_lower = query.to_lowercase();

        let mut matches: Vec<(String, f32)> = Vec::new();

        // Search for similar patterns
        for lesson in &lessons {
            let lesson_lower = lesson.to_lowercase();
            let mut score = 0.0;

            // Direct match bonus
            if lesson_lower.contains(&query_lower) {
                score += 1.0;
            }

            // Word-level matching
            let query_words: Vec<&str> = query_lower.split_whitespace().collect();
            let lesson_words: Vec<&str> = lesson_lower.split_whitespace().collect();

            let mut word_matches = 0;
            for query_word in &query_words {
                if query_word.len() > 2 {
                    for lesson_word in &lesson_words {
                        if lesson_word.contains(query_word) || query_word.contains(lesson_word) {
                            word_matches += 1;
                            break;
                        }
                    }
                }
            }

            if word_matches > 0 {
                score += (word_matches as f32) / (query_words.len() as f32);
            }

            if score > 0.0 {
                matches.push((lesson.trim().to_string(), score));
            }
        }

        if matches.is_empty() {
            return Ok(vec![format!("No similar patterns found for: {}\n💡 Record this pattern with: lfmf {} \"<topic>: <solution>\"", query, tool)]);
        }

        // Sort by score descending and format results
        matches.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap());

        Ok(matches.into_iter()
            .take(max_results)
            .map(|(lesson, score)| {
                if let Some((topic, body)) = lesson.split_once(':') {
                    format!("Match: {:.2} - [{}] {}", score, topic.trim(), body.trim())
                } else {
                    format!("Match: {:.2} - {}", score, lesson)
                }
            })
            .collect())
    }

    /// List all lessons for a tool
    pub async fn list_lessons(&mut self, tool: &str, count: Option<usize>) -> Result<Vec<String>> {
        let max_results = count.unwrap_or(10);

        // Try vector database first
        if let Some(ref client) = self.grok_client {
            match client.ask(&format!("lessons for {}", tool), Some(tool), Some(max_results)).await {
                Ok(results) => {
                    return Ok(results.results.into_iter()
                        .take(max_results)
                        .map(|r| r.content)
                        .collect());
                },
                Err(_) => {} // Fall through to filesystem
            }
        }

        // Fallback to filesystem
        let learn_dir = Path::new(&self.config.filesystem.learn_dir);
        let tool_file = learn_dir.join(format!("{}.md", tool));

        if !tool_file.exists() {
            return Ok(vec![format!("No lessons found for tool '{}'", tool)]);
        }

        let content = fs::read_to_string(&tool_file)?;
        let lessons: Vec<&str> = content.split("---\n").filter(|s| !s.trim().is_empty()).collect();

        Ok(lessons.into_iter()
            .take(max_results)
            .map(|lesson| {
                let lesson = lesson.trim();
                if let Some((topic, body)) = lesson.split_once(':') {
                    format!("[{}] {}", topic.trim(), body.trim())
                } else {
                    lesson.to_string()
                }
            })
            .collect())
    }

    /// Create example configuration
    pub fn print_config_example(path: &str) {
        println!("💡 Configure {}/lfmf.toml:", path);
        println!();
        println!("[qdrant]");
        println!("url = \"http://localhost:6334\"");
        println!("# api_key = \"your-api-key\"");
        println!("# collection = \"b00t_knowledge\"");
        println!();
        println!("[filesystem]");
        println!("learn_dir = \"learn\"");
        println!();
        println!("Environment variables:");
        println!("export QDRANT_URL=http://localhost:6334");
        println!("export B00T_LEARN_DIR=~/.dotfiles/learn");
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[tokio::test]
    async fn test_lesson_parsing() {
        let config = LfmfConfig::default();
        let system = LfmfSystem::new(config);

        let lesson = system.parse_lesson("rust", "cargo build: Run cargo build to compile").unwrap();
        assert_eq!(lesson.tool, "rust");
        assert_eq!(lesson.topic, "cargo build");
        assert_eq!(lesson.content, "Run cargo build to compile");
    }

    #[tokio::test]
    async fn test_filesystem_storage() {
        let temp_dir = TempDir::new().unwrap();
        let mut config = LfmfConfig::default();
        config.filesystem.learn_dir = temp_dir.path().join("learn").to_string_lossy().to_string();

        let mut system = LfmfSystem::new(config);
        
        let lesson = system.parse_lesson("rust", "linker error: Check LD_LIBRARY_PATH").unwrap();
        system.store_in_filesystem(&lesson).unwrap();

        // Verify file was created
        let tool_file = temp_dir.path().join("learn").join("rust.md");
        assert!(tool_file.exists());

        let content = fs::read_to_string(&tool_file).unwrap();
        assert!(content.contains("linker error: Check LD_LIBRARY_PATH"));
    }

    #[test]
    fn test_config_loading() {
        let temp_dir = TempDir::new().unwrap();
        let config_file = temp_dir.path().join("lfmf.toml");
        
        let config_content = r#"
[qdrant]
url = "http://custom:6334"
api_key = "test-key"

[filesystem]
learn_dir = "custom_learn"
"#;
        
        fs::write(&config_file, config_content).unwrap();
        
        let config = LfmfSystem::load_config(temp_dir.path().to_str().unwrap()).unwrap();
        assert_eq!(config.qdrant.url, "http://custom:6334");
        assert_eq!(config.qdrant.api_key, Some("test-key".to_string()));
        assert_eq!(config.filesystem.learn_dir, "custom_learn");
    }
}