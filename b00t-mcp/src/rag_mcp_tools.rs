//! MCP Tools for RAGLight Integration
//! 
//! Provides MCP tools for document loading, indexing, and querying using RAGLight
//! with b00t datum topics.

use anyhow::{Result, Context};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::sync::Mutex;
use tracing::{info, warn, error};

use crate::rag_light::{RagLightManager, RagLightConfig, DocumentSource, LoaderType};

/// Global RAGLight manager instance
type RagLightRegistry = Arc<Mutex<Option<RagLightManager>>>;

lazy_static::lazy_static! {
    static ref RAG_MANAGER: RagLightRegistry = Arc::new(Mutex::new(None));
}

/// Parameters for adding a document to RAG
#[derive(Debug, Serialize, Deserialize)]
pub struct RagAddDocumentParams {
    /// Document source (URL, file path, git repo, etc.)
    pub source: String,
    /// Target topic/datum for indexing
    pub topic: String,
    /// Optional loader type (auto-detected if not specified)
    pub loader_type: Option<String>,
    /// Optional metadata for the document
    pub metadata: Option<serde_json::Value>,
}

/// Parameters for querying RAG system
#[derive(Debug, Serialize, Deserialize)]
pub struct RagQueryParams {
    /// Topic to query
    pub topic: String,
    /// Query text
    pub query: String,
    /// Maximum number of results (default: 10)
    pub max_results: Option<usize>,
}

/// Parameters for getting job status
#[derive(Debug, Serialize, Deserialize)]
pub struct RagJobStatusParams {
    /// Job ID to check
    pub job_id: String,
}

/// Parameters for cancelling a job
#[derive(Debug, Serialize, Deserialize)]
pub struct RagCancelJobParams {
    /// Job ID to cancel
    pub job_id: String,
}

/// Parameters for listing topics
#[derive(Debug, Serialize, Deserialize)]
pub struct RagListTopicsParams {
    /// Optional filter pattern for topic names
    pub filter: Option<String>,
}

/// MCP tool: Add document to RAG system for indexing
pub async fn rag_add_document(params: RagAddDocumentParams) -> Result<String> {
    let mut manager_guard = RAG_MANAGER.lock().await;
    
    // Initialize manager if not already done
    if manager_guard.is_none() {
        let config = RagLightConfig::default();
        let manager = RagLightManager::new(config)
            .context("Failed to initialize RAGLight manager")?;
        *manager_guard = Some(manager);
    }
    
    let manager = manager_guard.as_mut().unwrap();
    
    // Parse loader type
    let loader_type = params.loader_type.as_ref().map(|t| {
        match t.to_lowercase().as_str() {
            "url" => LoaderType::Url,
            "git" => LoaderType::Git,
            "pdf" => LoaderType::Pdf,
            "text" => LoaderType::Text,
            "markdown" => LoaderType::Markdown,
            "auto" => LoaderType::Auto,
            _ => LoaderType::Auto,
        }
    });
    
    // Convert metadata
    let metadata = params.metadata.map(|m| {
        if let serde_json::Value::Object(obj) = m {
            obj.into_iter()
                .filter_map(|(k, v)| v.as_str().map(|s| (k, s.to_string())))
                .collect()
        } else {
            std::collections::HashMap::new()
        }
    });
    
    let source = DocumentSource {
        source: params.source.clone(),
        loader_type,
        topic: params.topic.clone(),
        metadata,
    };
    
    info!("🧠 Adding document to RAG: {} -> topic '{}'", params.source, params.topic);
    
    let job_id = manager.add_document(source).await
        .context("Failed to add document to RAG system")?;
    
    let response = serde_json::json!({
        "success": true,
        "message": format!("Document '{}' queued for indexing into topic '{}'", params.source, params.topic),
        "job_id": job_id,
        "topic": params.topic,
        "source": params.source,
        "status": "queued"
    });
    
    Ok(serde_json::to_string_pretty(&response)?)
}

/// MCP tool: Query RAG system
pub async fn rag_query(params: RagQueryParams) -> Result<String> {
    let manager_guard = RAG_MANAGER.lock().await;
    
    let manager = manager_guard.as_ref()
        .ok_or_else(|| anyhow::anyhow!("RAGLight manager not initialized"))?;
    
    info!("🧠 Querying RAG topic '{}': {}", params.topic, params.query);
    
    let result = manager.query(&params.topic, &params.query, params.max_results).await
        .context("Failed to query RAG system")?;
    
    let response = serde_json::json!({
        "success": true,
        "topic": params.topic,
        "query": params.query,
        "result": result,
        "max_results": params.max_results.unwrap_or(10)
    });
    
    Ok(serde_json::to_string_pretty(&response)?)
}

/// MCP tool: Get indexing job status
pub async fn rag_job_status(params: RagJobStatusParams) -> Result<String> {
    let manager_guard = RAG_MANAGER.lock().await;
    
    let manager = manager_guard.as_ref()
        .ok_or_else(|| anyhow::anyhow!("RAGLight manager not initialized"))?;
    
    if let Some(job) = manager.get_job_status(&params.job_id) {
        let response = serde_json::json!({
            "success": true,
            "job": job
        });
        Ok(serde_json::to_string_pretty(&response)?)
    } else {
        Err(anyhow::anyhow!("Job '{}' not found", params.job_id))
    }
}

/// MCP tool: List all indexing jobs
pub async fn rag_list_jobs() -> Result<String> {
    let manager_guard = RAG_MANAGER.lock().await;
    
    let manager = manager_guard.as_ref()
        .ok_or_else(|| anyhow::anyhow!("RAGLight manager not initialized"))?;
    
    let jobs = manager.list_jobs();
    
    let response = serde_json::json!({
        "success": true,
        "jobs": jobs,
        "total_jobs": jobs.len()
    });
    
    Ok(serde_json::to_string_pretty(&response)?)
}

/// MCP tool: Cancel indexing job
pub async fn rag_cancel_job(params: RagCancelJobParams) -> Result<String> {
    let mut manager_guard = RAG_MANAGER.lock().await;
    
    let manager = manager_guard.as_mut()
        .ok_or_else(|| anyhow::anyhow!("RAGLight manager not initialized"))?;
    
    manager.cancel_job(&params.job_id)
        .context("Failed to cancel job")?;
    
    let response = serde_json::json!({
        "success": true,
        "message": format!("Job '{}' has been cancelled", params.job_id),
        "job_id": params.job_id
    });
    
    Ok(serde_json::to_string_pretty(&response)?)
}

/// MCP tool: List available topics
pub async fn rag_list_topics(params: RagListTopicsParams) -> Result<String> {
    let manager_guard = RAG_MANAGER.lock().await;
    
    // Initialize manager if not already done
    if manager_guard.is_none() {
        drop(manager_guard);
        let mut manager_guard = RAG_MANAGER.lock().await;
        let config = RagLightConfig::default();
        let manager = RagLightManager::new(config)
            .context("Failed to initialize RAGLight manager")?;
        *manager_guard = Some(manager);
    }
    
    let manager_guard = RAG_MANAGER.lock().await;
    let manager = manager_guard.as_ref().unwrap();
    
    let mut topics = manager.get_topics().to_vec();
    
    // Apply filter if provided
    if let Some(filter) = &params.filter {
        topics.retain(|topic| topic.contains(filter));
    }
    
    let response = serde_json::json!({
        "success": true,
        "topics": topics,
        "total_topics": topics.len(),
        "filter": params.filter
    });
    
    Ok(serde_json::to_string_pretty(&response)?)
}

/// MCP tool: Get RAG system status
pub async fn rag_status() -> Result<String> {
    let manager_guard = RAG_MANAGER.lock().await;
    
    if let Some(manager) = manager_guard.as_ref() {
        let jobs = manager.list_jobs();
        let topics = manager.get_topics();
        
        let active_jobs = jobs.iter().filter(|j| {
            matches!(j.status, crate::rag_light::IndexingStatus::Processing | 
                              crate::rag_light::IndexingStatus::Queued)
        }).count();
        
        let completed_jobs = jobs.iter().filter(|j| {
            matches!(j.status, crate::rag_light::IndexingStatus::Completed)
        }).count();
        
        let failed_jobs = jobs.iter().filter(|j| {
            matches!(j.status, crate::rag_light::IndexingStatus::Failed)
        }).count();
        
        let response = serde_json::json!({
            "success": true,
            "status": "initialized",
            "available_topics": topics.len(),
            "total_jobs": jobs.len(),
            "active_jobs": active_jobs,
            "completed_jobs": completed_jobs,
            "failed_jobs": failed_jobs,
            "topics": topics
        });
        
        Ok(serde_json::to_string_pretty(&response)?)
    } else {
        let response = serde_json::json!({
            "success": true,
            "status": "not_initialized",
            "message": "RAGLight manager not yet initialized. Use rag-add-document or rag-list-topics to initialize."
        });
        
        Ok(serde_json::to_string_pretty(&response)?)
    }
}

/// Initialize RAGLight system with custom configuration
pub async fn rag_init(config: Option<RagLightConfig>) -> Result<String> {
    let mut manager_guard = RAG_MANAGER.lock().await;
    
    let config = config.unwrap_or_default();
    let manager = RagLightManager::new(config)
        .context("Failed to initialize RAGLight manager")?;
    
    let topics_count = manager.get_topics().len();
    *manager_guard = Some(manager);
    
    info!("🧠 RAGLight system initialized with {} topics", topics_count);
    
    let response = serde_json::json!({
        "success": true,
        "message": "RAGLight system initialized successfully",
        "available_topics": topics_count
    });
    
    Ok(serde_json::to_string_pretty(&response)?)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_rag_list_topics() {
        let params = RagListTopicsParams {
            filter: Some("rust".to_string()),
        };
        
        let result = rag_list_topics(params).await;
        assert!(result.is_ok());
        
        let response: serde_json::Value = serde_json::from_str(&result.unwrap()).unwrap();
        assert_eq!(response["success"], true);
        assert!(response["topics"].is_array());
    }

    #[tokio::test]
    async fn test_rag_status() {
        let result = rag_status().await;
        assert!(result.is_ok());
        
        let response: serde_json::Value = serde_json::from_str(&result.unwrap()).unwrap();
        assert_eq!(response["success"], true);
    }
}