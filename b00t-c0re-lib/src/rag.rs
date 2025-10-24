//! RAGLight Integration for b00t MCP Server
//!
//! Provides document loading, indexing, and querying capabilities using RAGLight
//! with b00t datum topics and async processing.

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
use tokio::process::Command;
use tracing::{debug, error, info, warn};

/// Document loader types supported by b00t RAG system
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum LoaderType {
    /// Load from URL (with spider capabilities)
    Url,
    /// Load from Git repository
    Git,
    /// Load PDF document
    Pdf,
    /// Load text document
    Text,
    /// Load markdown document
    Markdown,
    /// Auto-detect based on source
    Auto,
}

/// Document source for RAG ingestion
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DocumentSource {
    /// Source identifier (URL, file path, git repo, etc.)
    pub source: String,
    /// Loader type (auto-detected if None)
    pub loader_type: Option<LoaderType>,
    /// Target topic/datum for indexing
    pub topic: String,
    /// Optional metadata for the document
    pub metadata: Option<HashMap<String, String>>,
}

/// RAG indexing job status
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IndexingJob {
    /// Unique job identifier
    pub job_id: String,
    /// Document source being processed
    pub source: DocumentSource,
    /// Current job status
    pub status: IndexingStatus,
    /// Progress percentage (0-100)
    pub progress: u8,
    /// Status message
    pub message: String,
    /// Job creation timestamp
    pub created_at: chrono::DateTime<chrono::Utc>,
    /// Last update timestamp
    pub updated_at: chrono::DateTime<chrono::Utc>,
}

/// Status of an indexing job
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum IndexingStatus {
    /// Job is queued for processing
    Queued,
    /// Job is currently being processed
    Processing,
    /// Job completed successfully
    Completed,
    /// Job failed with error
    Failed,
    /// Job was cancelled
    Cancelled,
}

/// RAGLight configuration for b00t integration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RagLightConfig {
    /// Python virtual environment path (optional)
    pub venv_path: Option<PathBuf>,
    /// RAGLight installation path
    pub raglight_path: PathBuf,
    /// Vector database path for topic storage
    pub vector_db_path: PathBuf,
    /// Maximum concurrent indexing jobs
    pub max_concurrent_jobs: usize,
    /// Default embedding model
    pub embedding_model: String,
    /// LLM provider configuration
    pub llm_config: LlmConfig,
}

/// LLM provider configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LlmConfig {
    /// Provider type (openai, anthropic, ollama, etc.)
    pub provider: String,
    /// Model name
    pub model: String,
    /// API key (optional, can use environment)
    pub api_key: Option<String>,
    /// Additional configuration
    pub config: HashMap<String, serde_json::Value>,
}

/// b00t RAGLight integration manager
pub struct RagLightManager {
    config: RagLightConfig,
    active_jobs: HashMap<String, IndexingJob>,
    /// Available b00t datums as topics
    available_topics: Vec<String>,
}

impl RagLightManager {
    /// Create new RAGLight manager with configuration
    pub fn new(config: RagLightConfig) -> Result<Self> {
        let available_topics = Self::discover_b00t_topics(&config)?;

        Ok(Self {
            config,
            active_jobs: HashMap::new(),
            available_topics,
        })
    }

    /// Discover available b00t datums as RAG topics
    fn discover_b00t_topics(_config: &RagLightConfig) -> Result<Vec<String>> {
        // Scan ~/.dotfiles/_b00t_/ for available topics
        let b00t_path = dirs::home_dir()
            .ok_or_else(|| anyhow::anyhow!("Could not find home directory"))?
            .join(".dotfiles")
            .join("_b00t_");

        let mut topics = Vec::new();

        if b00t_path.exists() {
            let entries = std::fs::read_dir(&b00t_path).context("Failed to read b00t directory")?;

            for entry in entries {
                let entry = entry?;
                let path = entry.path();

                if let Some(stem) = path.file_stem() {
                    if let Some(name) = stem.to_str() {
                        // Include TOML configs and learn directories
                        if path.extension().map(|e| e == "toml").unwrap_or(false) || path.is_dir() {
                            topics.push(name.to_string());
                        }
                    }
                }
            }
        }

        // Add core b00t topics
        topics.extend([
            "rust".to_string(),
            "python".to_string(),
            "typescript".to_string(),
            "bash".to_string(),
            "git".to_string(),
            "docker".to_string(),
            "kubernetes".to_string(),
            "just".to_string(),
            "mcp".to_string(),
            "acp".to_string(),
        ]);

        topics.sort();
        topics.dedup();

        info!("Discovered {} b00t topics for RAG", topics.len());
        Ok(topics)
    }

    /// Add document source for async indexing
    pub async fn add_document(&mut self, mut source: DocumentSource) -> Result<String> {
        // Auto-detect loader type if not specified
        if source.loader_type.is_none() {
            source.loader_type = Some(self.detect_loader_type(&source.source)?);
        }

        // Validate topic exists
        if !self.available_topics.contains(&source.topic) {
            return Err(anyhow::anyhow!(
                "Topic '{}' not found in available b00t datums",
                source.topic
            ));
        }

        // Create indexing job
        let job_id = uuid::Uuid::new_v4().to_string();
        let job = IndexingJob {
            job_id: job_id.clone(),
            source,
            status: IndexingStatus::Queued,
            progress: 0,
            message: "Job queued for processing".to_string(),
            created_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
        };

        self.active_jobs.insert(job_id.clone(), job.clone());

        // Start async processing
        self.process_indexing_job(job_id.clone()).await?;

        info!(
            "Started indexing job {} for topic '{}'",
            job_id, job.source.topic
        );
        Ok(job_id)
    }

    /// Detect appropriate loader type based on source
    fn detect_loader_type(&self, source: &str) -> Result<LoaderType> {
        if source.ends_with(".pdf") {
            Ok(LoaderType::Pdf)
        } else if source.ends_with(".md") || source.ends_with(".markdown") {
            Ok(LoaderType::Markdown)
        } else if source.ends_with(".txt") {
            Ok(LoaderType::Text)
        } else if source.starts_with("git@") || source.contains(".git") {
            Ok(LoaderType::Git)
        } else if source.starts_with("http://") || source.starts_with("https://") {
            if source.contains("github.com")
                || source.contains("gitlab.com")
                || source.ends_with(".git")
            {
                Ok(LoaderType::Git)
            } else {
                Ok(LoaderType::Url)
            }
        } else {
            Ok(LoaderType::Auto)
        }
    }

    /// Process indexing job asynchronously
    async fn process_indexing_job(&mut self, job_id: String) -> Result<()> {
        let job = self
            .active_jobs
            .get_mut(&job_id)
            .ok_or_else(|| anyhow::anyhow!("Job {} not found", job_id))?;

        job.status = IndexingStatus::Processing;
        job.progress = 10;
        job.message = "Starting document processing".to_string();
        job.updated_at = chrono::Utc::now();

        let source = job.source.clone();

        // Spawn background task for actual processing
        let config = self.config.clone();
        tokio::spawn(async move {
            if let Err(e) = Self::run_raglight_indexing(config, source).await {
                error!("RAGLight indexing failed: {}", e);
            }
        });

        Ok(())
    }

    /// Run RAGLight indexing using Python subprocess
    async fn run_raglight_indexing(config: RagLightConfig, source: DocumentSource) -> Result<()> {
        let python_cmd = if let Some(venv) = &config.venv_path {
            venv.join("bin").join("python")
        } else {
            PathBuf::from("python3")
        };

        // Create RAGLight indexing script arguments
        let mut cmd = Command::new(&python_cmd);
        cmd.arg("-c").arg(format!(
            r#"
import sys
sys.path.insert(0, '{}')

from raglight import RagLightConfig, RagLight
import asyncio

async def index_document():
    # Configure RAGLight
    config = RagLightConfig(
        provider='{provider}',
        model='{model}',
        k=10,
        vector_db_path='{vector_db_path}',
    )
    
    # Initialize RAG system
    rag = RagLight(config)
    
    # Load and index document
    if '{loader_type}' == 'url':
        await rag.load_url('{source}', topic='{topic}')
    elif '{loader_type}' == 'git':
        await rag.load_git_repo('{source}', topic='{topic}')
    elif '{loader_type}' == 'pdf':
        await rag.load_pdf('{source}', topic='{topic}')
    else:
        await rag.load_text_file('{source}', topic='{topic}')
    
    print(f"Successfully indexed {{'{source}'}} into topic {{'{topic}'}}")

if __name__ == '__main__':
    asyncio.run(index_document())
"#,
            raglight_path = config.raglight_path.display(),
            provider = config.llm_config.provider,
            model = config.llm_config.model,
            vector_db_path = config.vector_db_path.display(),
            loader_type =
                format!("{:?}", source.loader_type.unwrap_or(LoaderType::Auto)).to_lowercase(),
            source = source.source,
            topic = source.topic,
        ));

        info!("Running RAGLight indexing for source: {}", source.source);

        let output = cmd
            .output()
            .await
            .context("Failed to execute RAGLight indexing")?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(anyhow::anyhow!("RAGLight indexing failed: {}", stderr));
        }

        let stdout = String::from_utf8_lossy(&output.stdout);
        info!("RAGLight indexing completed: {}", stdout);

        Ok(())
    }

    /// Query RAG system for a topic
    pub async fn query(
        &self,
        topic: &str,
        query: &str,
        max_results: Option<usize>,
    ) -> Result<String> {
        let python_cmd = if let Some(venv) = &self.config.venv_path {
            venv.join("bin").join("python")
        } else {
            PathBuf::from("python3")
        };

        let mut cmd = Command::new(&python_cmd);
        cmd.arg("-c").arg(format!(
            r#"
import sys
sys.path.insert(0, '{}')

from raglight import RagLightConfig, RagLight
import asyncio

async def query_rag():
    config = RagLightConfig(
        provider='{provider}',
        model='{model}',
        k={k},
        vector_db_path='{vector_db_path}',
    )
    
    rag = RagLight(config)
    result = await rag.query('{query}', topic='{topic}')
    print(result)

if __name__ == '__main__':
    asyncio.run(query_rag())
"#,
            raglight_path = self.config.raglight_path.display(),
            provider = self.config.llm_config.provider,
            model = self.config.llm_config.model,
            k = max_results.unwrap_or(10),
            vector_db_path = self.config.vector_db_path.display(),
            query = query.replace("'", "\\'"),
            topic = topic,
        ));

        let output = cmd
            .output()
            .await
            .context("Failed to execute RAGLight query")?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(anyhow::anyhow!("RAGLight query failed: {}", stderr));
        }

        let result = String::from_utf8_lossy(&output.stdout);
        Ok(result.trim().to_string())
    }

    /// Get job status
    pub fn get_job_status(&self, job_id: &str) -> Option<&IndexingJob> {
        self.active_jobs.get(job_id)
    }

    /// List all active jobs
    pub fn list_jobs(&self) -> Vec<&IndexingJob> {
        self.active_jobs.values().collect()
    }

    /// Get available topics
    pub fn get_topics(&self) -> &[String] {
        &self.available_topics
    }

    /// Cancel indexing job
    pub fn cancel_job(&mut self, job_id: &str) -> Result<()> {
        if let Some(job) = self.active_jobs.get_mut(job_id) {
            job.status = IndexingStatus::Cancelled;
            job.message = "Job cancelled by user".to_string();
            job.updated_at = chrono::Utc::now();
            Ok(())
        } else {
            Err(anyhow::anyhow!("Job {} not found", job_id))
        }
    }
}

impl Default for RagLightConfig {
    fn default() -> Self {
        let home = dirs::home_dir().unwrap_or_else(|| PathBuf::from("."));
        Self {
            venv_path: None,
            raglight_path: home.join(".local/lib/python3.12/site-packages"),
            vector_db_path: home.join(".b00t/raglight/vector_db"),
            max_concurrent_jobs: 3,
            embedding_model: "sentence-transformers/all-MiniLM-L6-v2".to_string(),
            llm_config: LlmConfig {
                provider: "openai".to_string(),
                model: "gpt-4o-mini".to_string(),
                api_key: None,
                config: HashMap::new(),
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_loader_type_detection() {
        let config = RagLightConfig::default();
        let manager = RagLightManager::new(config).unwrap();

        assert!(matches!(
            manager
                .detect_loader_type("https://github.com/owner/repo")
                .unwrap(),
            LoaderType::Git
        ));

        assert!(matches!(
            manager
                .detect_loader_type("https://example.com/doc.pdf")
                .unwrap(),
            LoaderType::Pdf
        ));

        assert!(matches!(
            manager
                .detect_loader_type("https://example.com/page")
                .unwrap(),
            LoaderType::Url
        ));
    }

    #[test]
    fn test_document_source_serialization() {
        let source = DocumentSource {
            source: "https://github.com/example/repo".to_string(),
            loader_type: Some(LoaderType::Git),
            topic: "rust".to_string(),
            metadata: Some([("author".to_string(), "example".to_string())].into()),
        };

        let json = serde_json::to_string(&source).unwrap();
        let deserialized: DocumentSource = serde_json::from_str(&json).unwrap();

        assert_eq!(source.source, deserialized.source);
        assert_eq!(source.topic, deserialized.topic);
    }
}
