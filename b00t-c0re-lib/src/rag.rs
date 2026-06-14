//! RAGLight Integration for b00t MCP Server
//!
//! Provides document loading, indexing, and querying capabilities using RAGLight
//! with b00t datum topics and async processing.

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
use tokio::process::Command;
use tracing::{error, info};

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
    /// Model name — served by vLLM at api_base
    pub model: String,
    /// OpenAI-compatible base URL (vLLM endpoint)
    pub api_base: String,
    /// API key (local vLLM accepts any non-empty value)
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
        // 🤓 b00t datums live in ~/.b00t/_b00t_/ (not ~/.dotfiles/_b00t_/)
        //    B00T_DIR env var overrides; stem of each .toml (minus extension suffixes) = topic
        let home =
            dirs::home_dir().ok_or_else(|| anyhow::anyhow!("Could not find home directory"))?;
        let b00t_path = std::env::var("B00T_DIR")
            .map(|d| PathBuf::from(d).join("_b00t_"))
            .unwrap_or_else(|_| home.join(".b00t").join("_b00t_"));

        let mut topics = Vec::new();

        if b00t_path.exists() {
            let entries = std::fs::read_dir(&b00t_path).context("Failed to read b00t directory")?;

            for entry in entries {
                let entry = entry?;
                let path = entry.path();
                let is_toml = path.extension().map(|e| e == "toml").unwrap_or(false);

                if (is_toml || path.is_dir())
                    && let Some(fname) = path.file_name().and_then(|n| n.to_str())
                {
                    // 🤓 Strip ALL extensions: "gemma-4-26b-a4b-local.model.toml" → "gemma-4-26b-a4b-local"
                    //    Also handle "pi.agent.toml" → "pi", "b00t-mcp.mcp.toml" → "b00t-mcp"
                    let topic = fname.split('.').next().unwrap_or(fname);
                    if !topic.is_empty() {
                        topics.push(topic.to_string());
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
        // 🤓 raglite API: insert_documents([path_or_url], config=RAGLiteConfig(...))
        //    db_url uses DuckDB format; llm uses litellm "openai/model" pointing to vLLM :8001
        //    embedder: llama-cpp bge-m3 GGUF (~2GB VRAM) — concurrent with Gemma4 (12-16GB)
        let db_url = format!("duckdb:///{}", config.vector_db_path.display());
        let llm_spec = format!("{}/{}", config.llm_config.provider, config.llm_config.model);
        let api_base = config.llm_config.api_base.clone();
        let api_key = config
            .llm_config
            .api_key
            .clone()
            .unwrap_or_else(|| "local".to_string());
        let loader_type =
            format!("{:?}", source.loader_type.unwrap_or(LoaderType::Auto)).to_lowercase();

        let mut cmd = Command::new(&python_cmd);
        cmd.arg("-c").arg(format!(
            r#"
import os, sys, tempfile
from pathlib import Path

os.environ.setdefault('OPENAI_BASE_URL', '{api_base}')
os.environ.setdefault('OPENAI_API_KEY', '{api_key}')

from raglite import RAGLiteConfig, insert_documents

config = RAGLiteConfig(
    db_url='{db_url}',
    llm='{llm_spec}',
    embedder='{embedder}',
)

loader_type = '{loader_type}'
source = r'''{source}'''

if loader_type == 'url' or source.startswith('http'):
    insert_documents([source], config=config)
elif loader_type in ('pdf', 'markdown') and os.path.isfile(source):
    insert_documents([Path(source)], config=config)
elif os.path.isfile(source):
    insert_documents([Path(source)], config=config)
else:
    # raw text content — write to tempfile then ingest
    with tempfile.NamedTemporaryFile(suffix='.txt', mode='w', delete=False, encoding='utf-8') as f:
        f.write(source)
        tmp_path = f.name
    try:
        insert_documents([Path(tmp_path)], config=config)
    finally:
        os.unlink(tmp_path)

print(f'Indexed into {db_url}')
"#,
            api_base = api_base,
            api_key = api_key,
            db_url = db_url,
            llm_spec = llm_spec,
            embedder = config.embedding_model,
            loader_type = loader_type,
            source = source.source.replace("'", r"\'"),
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

        // 🤓 raglite query: retrieve_chunks → rerank_chunks → print chunk bodies
        //    No topic filter in raglite (it's global DB); topic used for metadata context only
        let db_url = format!("duckdb:///{}", self.config.vector_db_path.display());
        let llm_spec = format!(
            "{}/{}",
            self.config.llm_config.provider, self.config.llm_config.model
        );
        let api_base = self.config.llm_config.api_base.clone();
        let api_key = self
            .config
            .llm_config
            .api_key
            .clone()
            .unwrap_or_else(|| "local".to_string());

        let mut cmd = Command::new(&python_cmd);
        cmd.arg("-c").arg(format!(
            r#"
import os, sys
os.environ.setdefault('OPENAI_BASE_URL', '{api_base}')
os.environ.setdefault('OPENAI_API_KEY', '{api_key}')

from raglite import RAGLiteConfig, search_and_rerank_chunks

config = RAGLiteConfig(
    db_url='{db_url}',
    llm='{llm_spec}',
    embedder='{embedder}',
)

query = r'''{query}'''
# 🤓 raglite API change: retrieve_chunks(query) removed; search_and_rerank_chunks combines hybrid_search + rerank
chunks = search_and_rerank_chunks(query, num_results={k}, config=config)
for chunk in chunks:
    print(chunk.body)
    print('---')
"#,
            api_base = api_base,
            api_key = api_key,
            db_url = db_url,
            llm_spec = llm_spec,
            embedder = self.config.embedding_model,
            query = query.replace("'", r"\'"),
            k = max_results.unwrap_or(10),
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
            venv_path: Some(home.join(".venv")),
            // 🤓 raglight_path unused — raglite installed via uv into .venv; kept for compat
            raglight_path: home.join(".venv/lib/python3.12/site-packages"),
            // 🤓 DuckDB path: formatted as duckdb:////path at runtime
            vector_db_path: home.join(".local/share/raglite/raglite.db"),
            max_concurrent_jobs: 3,
            // 🤓 bge-m3 GGUF via llama-cpp: ~2GB VRAM, concurrent with Gemma4 (12-16GB) on RTX 3090
            embedding_model: "llama-cpp-python/lm-kit/bge-m3-gguf/*F16.gguf@512".to_string(),
            llm_config: LlmConfig {
                provider: "openai".to_string(),
                // 🤓 ch0nky = Gemma4 served by vLLM at :8001; raglite LLM calls go to same endpoint
                model: "ch0nky".to_string(),
                // 🤓 GEMMA4_API_BASE takes precedence: avoids OPENAI_BASE_URL collision
                //    when direnv is not active (outside ~/.b00t); fallback to :8001
                api_base: std::env::var("GEMMA4_API_BASE")
                    .or_else(|_| std::env::var("OPENAI_BASE_URL"))
                    .unwrap_or_else(|_| "http://127.0.0.1:8001/v1".to_string()),
                api_key: Some(
                    std::env::var("GEMMA4_API_KEY")
                        .or_else(|_| std::env::var("OPENAI_API_KEY"))
                        .unwrap_or_else(|_| "local-gemma4".to_string()),
                ),
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
