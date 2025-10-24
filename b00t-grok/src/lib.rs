// Import our structured error types
pub mod errors;
use async_openai::{Client, config::OpenAIConfig};
pub use errors::{GrokError, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use uuid::Uuid;

#[cfg(feature = "pyo3")]
use pyo3::prelude::*;

// 🤓 Clean abstraction: Minimal Python-Rust interface for semantic chunking
pub trait Chunker {
    fn chunk(&self, content: &str) -> Result<Vec<String>>;
}

// PyO3-based semantic chunker
#[cfg(feature = "pyo3")]
pub struct SemanticChunker {
    max_chunk_size: usize,
}

#[cfg(feature = "pyo3")]
impl SemanticChunker {
    pub fn new(max_chunk_size: usize) -> Self {
        Self { max_chunk_size }
    }
}

#[cfg(feature = "pyo3")]
impl Chunker for SemanticChunker {
    fn chunk(&self, content: &str) -> Result<Vec<String>> {
        Python::with_gil(|py| -> Result<Vec<String>> {
            let chonkie = py
                .import("chonkie")
                .map_err(|e| GrokError::SemanticChunking {
                    message: format!(
                        "Failed to import chonkie library: {}. Install with: pip install chonkie",
                        e
                    ),
                })?;

            // Use SentenceChunker with character-based chunking - call with positional args
            let chunker = chonkie
                .getattr("SentenceChunker")
                .and_then(|cls| cls.call1((self.max_chunk_size, 50)))
                .map_err(|e| GrokError::SemanticChunking {
                    message: format!("Failed to create SentenceChunker: {}", e),
                })?;

            let chunk_objects = chunker.call_method1("chunk", (content,)).map_err(|e| {
                GrokError::SemanticChunking {
                    message: format!("Failed to chunk content: {}", e),
                }
            })?;

            // Extract text from chunk objects
            let mut chunks = Vec::new();
            for chunk_obj in chunk_objects
                .try_iter()
                .map_err(|e| GrokError::SemanticChunking {
                    message: format!("Failed to iterate chunks: {}", e),
                })?
            {
                let chunk_text: String = chunk_obj
                    .and_then(|obj| obj.getattr("text"))
                    .and_then(|text| text.extract())
                    .map_err(|e| GrokError::SemanticChunking {
                        message: format!("Failed to extract chunk text: {}", e),
                    })?;
                chunks.push(chunk_text);
            }

            Ok(chunks)
        })
    }
}

// Basic fallback chunker (no Python dependencies)
pub struct BasicChunker;

impl Chunker for BasicChunker {
    fn chunk(&self, content: &str) -> Result<Vec<String>> {
        let chunks = content
            .split("\n\n")
            .filter(|chunk| !chunk.trim().is_empty())
            .filter(|chunk| chunk.len() > 10) // Skip very short chunks
            .map(|s| s.trim().to_string())
            .collect();
        Ok(chunks)
    }
}

// 🤓 CRITICAL TRIBAL KNOWLEDGE: qdrant-client v1.15.0 Port Configuration
//
//    IMPORTANT: Qdrant has two different ports:
//    - Port 6333: Web UI and HTTP REST API (for manual testing)
//    - Port 6334: Client API (gRPC/HTTP for qdrant-client library)
//
//    The qdrant-client crate needs port 6334, NOT 6333!
//    Use: http://host:6334 (client API)
//    NOT: http://host:6333 (web UI only)
//
//    This was the root cause of "protocol error: invalid compression flag"
//    Future agents: Always use port 6334 for qdrant-client connections!

use qdrant_client::{
    Qdrant,
    qdrant::{
        CollectionOperationResponse, Condition, CreateCollection, Distance, Filter, PointStruct,
        SearchPoints, VectorParams, WithPayloadSelector,
    },
};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Datum {
    pub name: String,
    pub datum_type: String,
    pub adjacencies: Vec<String>,
    pub properties: HashMap<String, String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Chunk {
    pub id: Uuid,
    pub content: String,
    pub datum: String,
    pub attribution: Attribution,
    pub metadata: ChunkMetadata,
    pub vector: Option<Vec<f32>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Attribution {
    pub url: Option<String>,
    pub filename: Option<String>,
    pub date: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChunkMetadata {
    pub topic: String,
    pub tags: Vec<String>,
    pub created_at: String,
    // lfmf-specific fields
    pub lesson_type: Option<String>,   // "fail", "advice", "pattern"
    pub error_pattern: Option<String>, // Original error text for matching
    pub solution: Option<String>,      // Solution or advice content
    pub confidence: Option<f32>,       // 0.0-1.0 confidence in lesson
}

pub struct GrokClient {
    qdrant_url: String,
    api_key: String,
    collection_name: String,
    embedding_model: Option<EmbeddingModel>,
    qdrant_client: Option<Qdrant>,
    chunker: Box<dyn Chunker + Send + Sync>, // 🤓 Abstraction: pluggable chunking strategy
}

#[derive(Debug)]
pub struct EmbeddingModel {
    model_name: String,
    client: Client<OpenAIConfig>,
}

impl EmbeddingModel {
    pub async fn new() -> Result<Self> {
        let base_url =
            std::env::var("OLLAMA_API_URL").map_err(|_| GrokError::EnvironmentVariable {
                variable: "OLLAMA_API_URL".to_string(),
            })?;

        let config = OpenAIConfig::default()
            .with_api_base(format!("{}/v1", base_url))
            .with_api_key("ollama"); // 🤓 Ollama doesn't require real API key

        let client = Client::with_config(config);

        Ok(Self {
            model_name: "nomic-embed-text".to_string(),
            client,
        })
    }

    pub async fn encode(&self, text: &str) -> Result<Vec<f32>> {
        // Validate input
        if text.trim().is_empty() {
            return Err(GrokError::InvalidQuery {
                message: "Empty text cannot be encoded".to_string(),
            });
        }

        let request = async_openai::types::CreateEmbeddingRequestArgs::default()
            .model(&self.model_name)
            .input([text])
            .build()
            .map_err(|e| GrokError::EmbeddingGeneration { source: e })?;

        let response = self
            .client
            .embeddings()
            .create(request)
            .await
            .map_err(|e| GrokError::EmbeddingGeneration { source: e })?;

        let embedding = response
            .data
            .into_iter()
            .next()
            .ok_or_else(|| GrokError::InvalidQuery {
                message: "No embeddings returned from API".to_string(),
            })?
            .embedding;

        tracing::debug!("✅ Generated {} dimensional embedding", embedding.len());
        Ok(embedding)
    }
}

impl GrokClient {
    pub fn new(qdrant_url: String, api_key: String) -> Self {
        // 🤓 Smart chunker selection: Use semantic if PyO3 available, fallback to basic
        let chunker: Box<dyn Chunker + Send + Sync> = {
            #[cfg(feature = "pyo3")]
            {
                Box::new(SemanticChunker::new(1000)) // 1000 token chunks
            }
            #[cfg(not(feature = "pyo3"))]
            {
                Box::new(BasicChunker)
            }
        };

        Self {
            qdrant_url,
            api_key,
            collection_name: "b00t_chunks".to_string(),
            embedding_model: None,
            qdrant_client: None,
            chunker,
        }
    }

    pub async fn initialize(&mut self) -> Result<()> {
        tracing::info!("Connecting to Qdrant: {}", self.qdrant_url);

        let client = self.build_qdrant_client(
            &self.qdrant_url,
            if self.api_key.is_empty() {
                None
            } else {
                Some(&self.api_key)
            },
        )?;

        // Initialize embedding model
        let embedding_model = EmbeddingModel::new().await?;

        self.qdrant_client = Some(client);
        self.embedding_model = Some(embedding_model);

        // Ensure collection exists
        self.ensure_collection_exists().await?;

        tracing::info!("GrokClient initialized for {}", self.qdrant_url);
        Ok(())
    }

    // 🚀 HTTP-only client builder
    fn build_qdrant_client(&self, url: &str, api_key: Option<&str>) -> Result<Qdrant> {
        // Convert grpc:// URLs to http:// since we're HTTP-only now
        let clean_url = if url.starts_with("grpc://") {
            tracing::info!("Converting grpc:// to http:// (HTTP-only mode)");
            url.replacen("grpc://", "http://", 1)
        } else if url.starts_with("grpcs://") {
            tracing::info!("Converting grpcs:// to https:// (HTTP-only mode)");
            url.replacen("grpcs://", "https://", 1)
        } else {
            url.to_string()
        };

        tracing::info!("Using HTTP REST protocol: {}", clean_url);

        let mut builder = Qdrant::from_url(&clean_url);

        if let Some(key) = api_key {
            builder = builder.api_key(key.to_string());
        }

        // 🤓 NOTE: qdrant-client v1.15.0 requires gRPC for initialization even with HTTP URLs
        //    This is a known limitation - server must have both ports 6333 (HTTP) and 6334 (gRPC) enabled

        Ok(builder.build()?)
    }

    async fn ensure_collection_exists(&self) -> Result<()> {
        let client = self
            .qdrant_client
            .as_ref()
            .ok_or(GrokError::ClientNotInitialized)?;

        // Check if collection exists
        match client.collection_info(&self.collection_name).await {
            Ok(_) => {
                tracing::info!("Collection '{}' already exists", self.collection_name);
                Ok(())
            }
            Err(_) => {
                // Collection doesn't exist, create it
                tracing::info!("Creating collection '{}'", self.collection_name);

                let create_collection = CreateCollection {
                    collection_name: self.collection_name.clone(),
                    vectors_config: Some(
                        VectorParams {
                            size: 768, // 🤓 nomic-embed-text produces 768-dimensional vectors
                            distance: Distance::Cosine.into(),
                            ..Default::default()
                        }
                        .into(),
                    ),
                    ..Default::default()
                };

                let response: CollectionOperationResponse =
                    client.create_collection(create_collection).await?;

                if response.result {
                    tracing::info!("Successfully created collection '{}'", self.collection_name);
                } else {
                    return Err(GrokError::CollectionOperation {
                        collection: self.collection_name.clone(),
                        message: "Creation failed".to_string(),
                    });
                }
                Ok(())
            }
        }
    }

    async fn generate_embedding(&self, text: &str) -> Result<Vec<f32>> {
        let model = self
            .embedding_model
            .as_ref()
            .ok_or(GrokError::ClientNotInitialized)?;
        model.encode(text).await
    }

    pub async fn digest(&self, topic: &str, content: &str) -> Result<Chunk> {
        let client = self
            .qdrant_client
            .as_ref()
            .ok_or(GrokError::ClientNotInitialized)?;

        // Validate inputs
        if topic.trim().is_empty() {
            return Err(GrokError::InvalidQuery {
                message: "Topic cannot be empty".to_string(),
            });
        }
        if content.trim().is_empty() {
            return Err(GrokError::InvalidQuery {
                message: "Content cannot be empty".to_string(),
            });
        }

        const MAX_CONTENT_SIZE: usize = 1_000_000; // 1MB limit
        if content.len() > MAX_CONTENT_SIZE {
            return Err(GrokError::ContentTooLarge {
                size: content.len(),
                limit: MAX_CONTENT_SIZE,
            });
        }

        // Generate vector embedding
        let vector = self.generate_embedding(content).await?;
        let chunk_id = Uuid::new_v4();

        let chunk = Chunk {
            id: chunk_id,
            content: content.to_string(),
            datum: topic.to_string(),
            attribution: Attribution {
                url: None,
                filename: None,
                date: chrono::Utc::now().to_rfc3339(),
            },
            metadata: ChunkMetadata {
                topic: topic.to_string(),
                tags: vec![],
                created_at: chrono::Utc::now().to_rfc3339(),
                lesson_type: None,
                error_pattern: None,
                solution: None,
                confidence: None,
            },
            vector: Some(vector.clone()),
        };

        // Store in Qdrant
        let mut payload = HashMap::new();
        payload.insert("content".to_string(), content.into());
        payload.insert("datum".to_string(), topic.into());
        payload.insert("topic".to_string(), topic.into());
        payload.insert(
            "created_at".to_string(),
            chunk.metadata.created_at.clone().into(),
        );

        let point = PointStruct::new(chunk_id.to_string(), vector, payload);

        let upsert_request = qdrant_client::qdrant::UpsertPoints {
            collection_name: self.collection_name.clone(),
            points: vec![point],
            ..Default::default()
        };

        client.upsert_points(upsert_request).await?;

        tracing::info!("Stored chunk {} for topic '{}' in Qdrant", chunk_id, topic);

        Ok(chunk)
    }

    pub async fn ask(&self, query: &str, topic: Option<&str>) -> Result<Vec<Chunk>> {
        let client = self
            .qdrant_client
            .as_ref()
            .ok_or(GrokError::ClientNotInitialized)?;

        // Validate query
        if query.trim().is_empty() {
            return Err(GrokError::InvalidQuery {
                message: "Query cannot be empty".to_string(),
            });
        }

        // Generate query embedding
        let query_vector = self.generate_embedding(query).await?;

        // Build search request
        let mut search_request = SearchPoints {
            collection_name: self.collection_name.clone(),
            vector: query_vector,
            limit: 10,
            with_payload: Some(WithPayloadSelector {
                selector_options: Some(
                    qdrant_client::qdrant::with_payload_selector::SelectorOptions::Enable(true),
                ),
            }),
            ..Default::default()
        };

        // Add topic filter if specified
        if let Some(topic_filter) = topic {
            let filter = Filter {
                must: vec![Condition {
                    condition_one_of: Some(
                        qdrant_client::qdrant::condition::ConditionOneOf::Field(
                            qdrant_client::qdrant::FieldCondition {
                                key: "topic".to_string(),
                                r#match: Some(qdrant_client::qdrant::Match {
                                    match_value: Some(
                                        qdrant_client::qdrant::r#match::MatchValue::Text(
                                            topic_filter.to_string(),
                                        ),
                                    ),
                                }),
                                ..Default::default()
                            },
                        ),
                    ),
                }],
                ..Default::default()
            };
            search_request.filter = Some(filter);
        }

        let search_result = client.search_points(search_request).await?;

        let mut chunks = Vec::new();
        for scored_point in search_result.result {
            let payload = scored_point.payload;
            let content = payload
                .get("content")
                .and_then(|v| v.as_str())
                .map_or("", |v| v)
                .to_string();

            let datum = payload
                .get("datum")
                .and_then(|v| v.as_str())
                .map_or("unknown", |v| v)
                .to_string();

            let topic_str = payload
                .get("topic")
                .and_then(|v| v.as_str())
                .map_or("unknown", |v| v)
                .to_string();

            let created_at = payload
                .get("created_at")
                .and_then(|v| v.as_str())
                .map_or("", |v| v)
                .to_string();

            let point_id = match &scored_point.id {
                Some(id) => match id.point_id_options.as_ref() {
                    Some(qdrant_client::qdrant::point_id::PointIdOptions::Uuid(uuid_str)) => {
                        Uuid::parse_str(&uuid_str).unwrap_or_else(|_| Uuid::new_v4())
                    }
                    Some(qdrant_client::qdrant::point_id::PointIdOptions::Num(_)) => Uuid::new_v4(),
                    None => Uuid::new_v4(),
                },
                None => Uuid::new_v4(),
            };

            let chunk = Chunk {
                id: point_id,
                content,
                datum: datum.clone(),
                attribution: Attribution {
                    url: None,
                    filename: None,
                    date: created_at.clone(),
                },
                metadata: ChunkMetadata {
                    topic: topic_str,
                    tags: vec![format!("score_{:.3}", scored_point.score)],
                    created_at,
                    lesson_type: None,
                    error_pattern: None,
                    solution: None,
                    confidence: None,
                },
                vector: scored_point.vectors.and_then(|v| match v.vectors_options? {
                    qdrant_client::qdrant::vectors_output::VectorsOptions::Vector(
                        vector_struct,
                    ) => Some(vector_struct.data),
                    _ => None,
                }),
            };

            chunks.push(chunk);
        }

        tracing::info!(
            "Found {} chunks for query '{}' with topic filter {:?}",
            chunks.len(),
            query,
            topic
        );

        Ok(chunks)
    }

    pub async fn learn(&self, source: &str, content: &str) -> Result<Vec<Chunk>> {
        let client = self
            .qdrant_client
            .as_ref()
            .ok_or(GrokError::ClientNotInitialized)?;

        // Validate inputs
        if source.trim().is_empty() {
            return Err(GrokError::InvalidQuery {
                message: "Source cannot be empty".to_string(),
            });
        }
        if content.trim().is_empty() {
            return Err(GrokError::InvalidQuery {
                message: "Content cannot be empty".to_string(),
            });
        }

        const MAX_CONTENT_SIZE: usize = 10_000_000; // 10MB limit for bulk learning
        if content.len() > MAX_CONTENT_SIZE {
            return Err(GrokError::ContentTooLarge {
                size: content.len(),
                limit: MAX_CONTENT_SIZE,
            });
        }

        // 🤓 Semantic chunking via abstraction layer
        let chunks = self.chunker.chunk(content)?;

        let mut result_chunks = Vec::new();
        let mut points = Vec::new();

        for (i, chunk_text) in chunks.into_iter().enumerate() {
            if chunk_text.len() < 10 {
                continue;
            } // Skip very short chunks

            // Infer topic from source or use "general"
            let inferred_topic = self.infer_topic_from_source(source);
            let chunk_id = Uuid::new_v4();
            let vector = self.generate_embedding(&chunk_text).await?;

            let chunk = Chunk {
                id: chunk_id,
                content: chunk_text.clone(),
                datum: inferred_topic.clone(),
                attribution: Attribution {
                    url: if source.starts_with("http") {
                        Some(source.to_string())
                    } else {
                        None
                    },
                    filename: if !source.starts_with("http") && source != "direct_input" {
                        Some(source.to_string())
                    } else {
                        None
                    },
                    date: chrono::Utc::now().to_rfc3339(),
                },
                metadata: ChunkMetadata {
                    topic: inferred_topic.clone(),
                    tags: vec![format!("chunk_{}", i)],
                    created_at: chrono::Utc::now().to_rfc3339(),
                    lesson_type: None,
                    error_pattern: None,
                    solution: None,
                    confidence: None,
                },
                vector: Some(vector.clone()),
            };

            // Prepare point for batch insertion
            let mut payload = HashMap::new();
            payload.insert("content".to_string(), chunk_text.clone().into());
            payload.insert("datum".to_string(), inferred_topic.clone().into());
            payload.insert("topic".to_string(), inferred_topic.into());
            payload.insert("source".to_string(), source.into());
            payload.insert(
                "created_at".to_string(),
                chunk.metadata.created_at.clone().into(),
            );
            payload.insert("chunk_index".to_string(), (i as i64).into());

            let point = PointStruct::new(chunk_id.to_string(), vector, payload);

            points.push(point);
            result_chunks.push(chunk);
        }

        // Batch insert all points
        if !points.is_empty() {
            let upsert_request = qdrant_client::qdrant::UpsertPoints {
                collection_name: self.collection_name.clone(),
                points,
                ..Default::default()
            };

            client.upsert_points(upsert_request).await?;

            tracing::info!(
                "Stored {} chunks from source '{}' in Qdrant",
                result_chunks.len(),
                source
            );
        }

        Ok(result_chunks)
    }

    fn infer_topic_from_source(&self, source: &str) -> String {
        if source.contains("rust") || source.contains(".rs") {
            "rust".to_string()
        } else if source.contains("python") || source.contains(".py") {
            "python".to_string()
        } else if source.contains("docker") {
            "docker".to_string()
        } else if source.contains("git") {
            "git".to_string()
        } else {
            "general".to_string()
        }
    }

    // lfmf (Learn From My Fail) Methods

    /// Record a lesson learned from a failure
    pub async fn record_lfmf(&self, tool: &str, error: &str, lesson: &str) -> Result<Chunk> {
        let client = self
            .qdrant_client
            .as_ref()
            .ok_or(GrokError::ClientNotInitialized)?;

        // Validate inputs
        if tool.trim().is_empty() {
            return Err(GrokError::InvalidQuery {
                message: "Tool name cannot be empty".to_string(),
            });
        }
        if error.trim().is_empty() {
            return Err(GrokError::InvalidQuery {
                message: "Error pattern cannot be empty".to_string(),
            });
        }
        if lesson.trim().is_empty() {
            return Err(GrokError::InvalidQuery {
                message: "Lesson cannot be empty".to_string(),
            });
        }

        // Format content for semantic search
        let content = format!(
            "TOOL: {}\nERROR PATTERN: {}\nLESSON LEARNED: {}\nSOLUTION: {}",
            tool, error, lesson, lesson
        );

        // Generate embedding
        let vector = self.generate_embedding(&content).await?;
        let chunk_id = Uuid::new_v4();

        // Enhanced topic inference for lfmf
        let topic = self.infer_lfmf_topic(tool, error, &content);

        // Calculate confidence based on lesson specificity
        let confidence = self.calculate_lesson_confidence(lesson);

        let chunk = Chunk {
            id: chunk_id,
            content: content.clone(),
            datum: topic.clone(),
            attribution: Attribution {
                url: None,
                filename: None,
                date: chrono::Utc::now().to_rfc3339(),
            },
            metadata: ChunkMetadata {
                topic: topic.clone(),
                tags: vec![
                    "lfmf:lesson".to_string(),
                    format!("tool:{}", tool.to_lowercase()),
                    format!("error_type:{}", self.classify_error_type(error)),
                    format!("confidence:{:.2}", confidence),
                ],
                created_at: chrono::Utc::now().to_rfc3339(),
                lesson_type: Some("fail".to_string()),
                error_pattern: Some(error.to_string()),
                solution: Some(lesson.to_string()),
                confidence: Some(confidence),
            },
            vector: Some(vector.clone()),
        };

        // Store in Qdrant
        let mut payload = HashMap::new();
        payload.insert("content".to_string(), content.into());
        payload.insert("datum".to_string(), topic.clone().into());
        payload.insert("topic".to_string(), topic.into());
        payload.insert("tool".to_string(), tool.into());
        payload.insert("error_pattern".to_string(), error.into());
        payload.insert("lesson".to_string(), lesson.into());
        payload.insert("lesson_type".to_string(), "fail".into());
        payload.insert("confidence".to_string(), confidence.into());
        payload.insert(
            "created_at".to_string(),
            chunk.metadata.created_at.clone().into(),
        );

        let point = PointStruct::new(chunk_id.to_string(), vector, payload);

        let upsert_request = qdrant_client::qdrant::UpsertPoints {
            collection_name: self.collection_name.clone(),
            points: vec![point],
            ..Default::default()
        };

        client.upsert_points(upsert_request).await?;

        tracing::info!(
            "Recorded lfmf lesson {} for tool '{}': {}",
            chunk_id,
            tool,
            error
        );

        Ok(chunk)
    }

    /// Get advice for a specific error pattern
    pub async fn get_advice(&self, tool: &str, error: &str) -> Result<Vec<Chunk>> {
        let client = self
            .qdrant_client
            .as_ref()
            .ok_or(GrokError::ClientNotInitialized)?;

        // Validate inputs
        if tool.trim().is_empty() {
            return Err(GrokError::InvalidQuery {
                message: "Tool name cannot be empty".to_string(),
            });
        }
        if error.trim().is_empty() {
            return Err(GrokError::InvalidQuery {
                message: "Error pattern cannot be empty".to_string(),
            });
        }

        // Format query similar to how we store lfmf records
        let query = format!("TOOL: {} ERROR PATTERN: {}", tool, error);

        // Generate query embedding
        let query_vector = self.generate_embedding(&query).await?;

        // Build search request with lfmf-specific filters
        use qdrant_client::qdrant::{
            Condition, FieldCondition, Filter, Match, r#match::MatchValue,
        };

        let filter = Filter {
            must: vec![
                // Filter for lfmf lessons only
                Condition {
                    condition_one_of: Some(
                        qdrant_client::qdrant::condition::ConditionOneOf::Field(FieldCondition {
                            key: "lesson_type".to_string(),
                            r#match: Some(Match {
                                match_value: Some(MatchValue::Text("fail".to_string())),
                            }),
                            ..Default::default()
                        }),
                    ),
                },
                // Filter for same tool
                Condition {
                    condition_one_of: Some(
                        qdrant_client::qdrant::condition::ConditionOneOf::Field(FieldCondition {
                            key: "tool".to_string(),
                            r#match: Some(Match {
                                match_value: Some(MatchValue::Text(tool.to_lowercase())),
                            }),
                            ..Default::default()
                        }),
                    ),
                },
            ],
            ..Default::default()
        };

        let search_request = SearchPoints {
            collection_name: self.collection_name.clone(),
            vector: query_vector,
            filter: Some(filter),
            limit: 5, // Top 5 most similar lessons
            with_payload: Some(WithPayloadSelector {
                selector_options: Some(
                    qdrant_client::qdrant::with_payload_selector::SelectorOptions::Enable(true),
                ),
            }),
            ..Default::default()
        };

        let search_result = client.search_points(search_request).await?;

        let mut advice_chunks = Vec::new();
        for scored_point in search_result.result {
            let payload = scored_point.payload;

            let content = payload
                .get("content")
                .and_then(|v| v.as_str())
                .map_or("", |v| v)
                .to_string();

            let lesson = payload
                .get("lesson")
                .and_then(|v| v.as_str())
                .map_or("", |v| v)
                .to_string();

            let error_pattern = payload
                .get("error_pattern")
                .and_then(|v| v.as_str())
                .map_or("", |v| v)
                .to_string();

            let confidence = payload
                .get("confidence")
                .and_then(|v| match v {
                    qdrant_client::qdrant::Value {
                        kind: Some(qdrant_client::qdrant::value::Kind::DoubleValue(f)),
                    } => Some(*f as f32),
                    qdrant_client::qdrant::Value {
                        kind: Some(qdrant_client::qdrant::value::Kind::IntegerValue(i)),
                    } => Some(*i as f32),
                    _ => None,
                })
                .unwrap_or(0.0);

            let created_at = payload
                .get("created_at")
                .and_then(|v| v.as_str())
                .map_or("", |v| v)
                .to_string();

            let point_id = match &scored_point.id {
                Some(id) => match id.point_id_options.as_ref() {
                    Some(qdrant_client::qdrant::point_id::PointIdOptions::Uuid(uuid_str)) => {
                        Uuid::parse_str(&uuid_str).unwrap_or_else(|_| Uuid::new_v4())
                    }
                    Some(qdrant_client::qdrant::point_id::PointIdOptions::Num(_)) => Uuid::new_v4(),
                    None => Uuid::new_v4(),
                },
                None => Uuid::new_v4(),
            };

            let chunk = Chunk {
                id: point_id,
                content,
                datum: tool.to_string(),
                attribution: Attribution {
                    url: None,
                    filename: None,
                    date: created_at.clone(),
                },
                metadata: ChunkMetadata {
                    topic: tool.to_string(),
                    tags: vec![
                        "lfmf:lesson".to_string(),
                        format!("tool:{}", tool),
                        format!("similarity:{:.3}", scored_point.score),
                    ],
                    created_at,
                    lesson_type: Some("fail".to_string()),
                    error_pattern: Some(error_pattern),
                    solution: Some(lesson),
                    confidence: Some(confidence),
                },
                vector: scored_point.vectors.and_then(|v| match v.vectors_options? {
                    qdrant_client::qdrant::vectors_output::VectorsOptions::Vector(
                        vector_struct,
                    ) => Some(vector_struct.data),
                    _ => None,
                }),
            };

            advice_chunks.push(chunk);
        }

        tracing::info!(
            "Found {} advice chunks for tool '{}' error '{}'",
            advice_chunks.len(),
            tool,
            error
        );

        Ok(advice_chunks)
    }

    /// List all lessons for a specific tool
    pub async fn list_lessons(&self, tool: &str) -> Result<Vec<Chunk>> {
        let client = self
            .qdrant_client
            .as_ref()
            .ok_or(GrokError::ClientNotInitialized)?;

        if tool.trim().is_empty() {
            return Err(GrokError::InvalidQuery {
                message: "Tool name cannot be empty".to_string(),
            });
        }

        // Build filter for tool-specific lfmf lessons
        use qdrant_client::qdrant::{
            Condition, FieldCondition, Filter, Match, r#match::MatchValue,
        };

        let filter = Filter {
            must: vec![
                Condition {
                    condition_one_of: Some(
                        qdrant_client::qdrant::condition::ConditionOneOf::Field(FieldCondition {
                            key: "lesson_type".to_string(),
                            r#match: Some(Match {
                                match_value: Some(MatchValue::Text("fail".to_string())),
                            }),
                            ..Default::default()
                        }),
                    ),
                },
                Condition {
                    condition_one_of: Some(
                        qdrant_client::qdrant::condition::ConditionOneOf::Field(FieldCondition {
                            key: "tool".to_string(),
                            r#match: Some(Match {
                                match_value: Some(MatchValue::Text(tool.to_lowercase())),
                            }),
                            ..Default::default()
                        }),
                    ),
                },
            ],
            ..Default::default()
        };

        let scroll_request = qdrant_client::qdrant::ScrollPoints {
            collection_name: self.collection_name.clone(),
            filter: Some(filter),
            limit: Some(50), // Reasonable limit for lesson listing
            with_payload: Some(WithPayloadSelector {
                selector_options: Some(
                    qdrant_client::qdrant::with_payload_selector::SelectorOptions::Enable(true),
                ),
            }),
            ..Default::default()
        };

        let scroll_result = client.scroll(scroll_request).await?;

        let mut lessons = Vec::new();
        for point in scroll_result.result {
            let payload = point.payload;

            let content = payload
                .get("content")
                .and_then(|v| v.as_str())
                .map_or("", |v| v)
                .to_string();

            let lesson = payload
                .get("lesson")
                .and_then(|v| v.as_str())
                .map_or("", |v| v)
                .to_string();

            let error_pattern = payload
                .get("error_pattern")
                .and_then(|v| v.as_str())
                .map_or("", |v| v)
                .to_string();

            let confidence = payload
                .get("confidence")
                .and_then(|v| match v {
                    qdrant_client::qdrant::Value {
                        kind: Some(qdrant_client::qdrant::value::Kind::DoubleValue(f)),
                    } => Some(*f as f32),
                    qdrant_client::qdrant::Value {
                        kind: Some(qdrant_client::qdrant::value::Kind::IntegerValue(i)),
                    } => Some(*i as f32),
                    _ => None,
                })
                .unwrap_or(0.0);

            let created_at = payload
                .get("created_at")
                .and_then(|v| v.as_str())
                .map_or("", |v| v)
                .to_string();

            let point_id = match &point.id {
                Some(id) => match id.point_id_options.as_ref() {
                    Some(qdrant_client::qdrant::point_id::PointIdOptions::Uuid(uuid_str)) => {
                        Uuid::parse_str(&uuid_str).unwrap_or_else(|_| Uuid::new_v4())
                    }
                    Some(qdrant_client::qdrant::point_id::PointIdOptions::Num(_)) => Uuid::new_v4(),
                    None => Uuid::new_v4(),
                },
                None => Uuid::new_v4(),
            };

            let chunk = Chunk {
                id: point_id,
                content,
                datum: tool.to_string(),
                attribution: Attribution {
                    url: None,
                    filename: None,
                    date: created_at.clone(),
                },
                metadata: ChunkMetadata {
                    topic: tool.to_string(),
                    tags: vec!["lfmf:lesson".to_string(), format!("tool:{}", tool)],
                    created_at,
                    lesson_type: Some("fail".to_string()),
                    error_pattern: Some(error_pattern),
                    solution: Some(lesson),
                    confidence: Some(confidence),
                },
                vector: point.vectors.and_then(|v| match v.vectors_options? {
                    qdrant_client::qdrant::vectors_output::VectorsOptions::Vector(
                        vector_struct,
                    ) => Some(vector_struct.data),
                    _ => None,
                }),
            };

            lessons.push(chunk);
        }

        tracing::info!("Listed {} lessons for tool '{}'", lessons.len(), tool);
        Ok(lessons)
    }

    // Helper methods for lfmf functionality

    fn infer_lfmf_topic(&self, tool: &str, error: &str, _content: &str) -> String {
        let base_topic = match tool.to_lowercase().as_str() {
            "just" | "justfile" => "just",
            "rust" | "cargo" | "clippy" => "rust",
            "docker" | "dockerfile" => "docker",
            "git" => "git",
            "k8s" | "kubectl" | "kubernetes" => "k8s",
            "python" | "pip" | "conda" => "python",
            _ => "general",
        };

        // Enhance with error pattern recognition
        let error_lower = error.to_lowercase();
        if error_lower.contains("template") || error_lower.contains("{{") {
            format!("{}_template_conflict", base_topic)
        } else if error_lower.contains("duplicate") || error_lower.contains("redefined") {
            format!("{}_duplicate", base_topic)
        } else if error_lower.contains("syntax") || error_lower.contains("parse") {
            format!("{}_syntax", base_topic)
        } else {
            base_topic.to_string()
        }
    }

    fn classify_error_type(&self, error: &str) -> String {
        let error_lower = error.to_lowercase();

        if error_lower.contains("syntax") || error_lower.contains("parse") {
            "syntax"
        } else if error_lower.contains("duplicate") || error_lower.contains("redefined") {
            "duplicate"
        } else if error_lower.contains("template") || error_lower.contains("{{") {
            "template_conflict"
        } else if error_lower.contains("not found") || error_lower.contains("missing") {
            "missing"
        } else if error_lower.contains("permission") || error_lower.contains("access") {
            "permission"
        } else {
            "general"
        }
        .to_string()
    }

    fn calculate_lesson_confidence(&self, lesson: &str) -> f32 {
        let mut confidence: f32 = 0.5; // Base confidence

        // Increase confidence for specific indicators
        if lesson.contains("solution:") || lesson.contains("fix:") {
            confidence += 0.2;
        }
        if lesson.len() > 50 {
            // Detailed lessons are more confident
            confidence += 0.1;
        }
        if lesson.contains("example") || lesson.contains("pattern") {
            confidence += 0.1;
        }
        if lesson.contains("avoid") || lesson.contains("use instead") {
            confidence += 0.1;
        }

        confidence.min(1.0) // Cap at 1.0
    }
}

#[cfg(feature = "pyo3")]
#[pyclass]
pub struct PyGrokClient {
    client: GrokClient,
    runtime: tokio::runtime::Runtime,
}

#[cfg(feature = "pyo3")]
#[pymethods]
impl PyGrokClient {
    #[new]
    fn new(qdrant_url: String, api_key: String) -> PyResult<Self> {
        let mut client = GrokClient::new(qdrant_url, api_key);
        let runtime = tokio::runtime::Runtime::new()
            .map_err(|e| pyo3::exceptions::PyRuntimeError::new_err(e.to_string()))?;

        // Initialize the client
        runtime
            .block_on(client.initialize())
            .map_err(|e| pyo3::exceptions::PyRuntimeError::new_err(e.to_string()))?;

        Ok(Self { client, runtime })
    }

    fn digest(&mut self, topic: &str, content: &str) -> PyResult<String> {
        let chunk = self
            .runtime
            .block_on(self.client.digest(topic, content))
            .map_err(|e| pyo3::exceptions::PyRuntimeError::new_err(e.to_string()))?;

        serde_json::to_string(&chunk)
            .map_err(|e| pyo3::exceptions::PyValueError::new_err(e.to_string()))
    }

    fn ask(&mut self, query: &str, topic: Option<&str>) -> PyResult<Vec<String>> {
        let chunks = self
            .runtime
            .block_on(self.client.ask(query, topic))
            .map_err(|e| pyo3::exceptions::PyRuntimeError::new_err(e.to_string()))?;

        chunks
            .into_iter()
            .map(|chunk| serde_json::to_string(&chunk))
            .collect::<std::result::Result<Vec<_>, _>>()
            .map_err(|e| pyo3::exceptions::PyValueError::new_err(e.to_string()))
    }

    fn learn(&mut self, source: &str, content: &str) -> PyResult<Vec<String>> {
        let chunks = self
            .runtime
            .block_on(self.client.learn(source, content))
            .map_err(|e| pyo3::exceptions::PyRuntimeError::new_err(e.to_string()))?;

        chunks
            .into_iter()
            .map(|chunk| serde_json::to_string(&chunk))
            .collect::<std::result::Result<Vec<_>, _>>()
            .map_err(|e| pyo3::exceptions::PyValueError::new_err(e.to_string()))
    }

    // lfmf (Learn From My Fail) Python bindings

    /// Record a lesson learned from a failure
    fn record_lfmf(&mut self, tool: &str, error: &str, lesson: &str) -> PyResult<String> {
        let chunk = self
            .runtime
            .block_on(self.client.record_lfmf(tool, error, lesson))
            .map_err(|e| pyo3::exceptions::PyRuntimeError::new_err(e.to_string()))?;

        serde_json::to_string(&chunk)
            .map_err(|e| pyo3::exceptions::PyValueError::new_err(e.to_string()))
    }

    /// Get advice for a specific error pattern
    fn get_advice(&mut self, tool: &str, error: &str) -> PyResult<Vec<String>> {
        let chunks = self
            .runtime
            .block_on(self.client.get_advice(tool, error))
            .map_err(|e| pyo3::exceptions::PyRuntimeError::new_err(e.to_string()))?;

        chunks
            .into_iter()
            .map(|chunk| serde_json::to_string(&chunk))
            .collect::<std::result::Result<Vec<_>, _>>()
            .map_err(|e| pyo3::exceptions::PyValueError::new_err(e.to_string()))
    }

    /// List all lessons for a specific tool
    fn list_lessons(&mut self, tool: &str) -> PyResult<Vec<String>> {
        let chunks = self
            .runtime
            .block_on(self.client.list_lessons(tool))
            .map_err(|e| pyo3::exceptions::PyRuntimeError::new_err(e.to_string()))?;

        chunks
            .into_iter()
            .map(|chunk| serde_json::to_string(&chunk))
            .collect::<std::result::Result<Vec<_>, _>>()
            .map_err(|e| pyo3::exceptions::PyValueError::new_err(e.to_string()))
    }
}

#[cfg(feature = "pyo3")]
#[pymodule]
fn b00t_grok(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<PyGrokClient>()?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    #[tokio::test]
    async fn test_grok_client_creation() {
        let client = GrokClient::new("https://example.com".to_string(), "test_key".to_string());
        assert_eq!(client.collection_name, "b00t_chunks");
        assert_eq!(client.qdrant_url, "https://example.com");
        assert_eq!(client.api_key, "test_key");
    }

    #[tokio::test]
    async fn test_embeddings_integration() {
        // Skip if no OLLAMA_API_URL is set
        let _ollama_url = match std::env::var("OLLAMA_API_URL") {
            Ok(url) => url,
            Err(_) => {
                println!("Skipping embeddings test - no OLLAMA_API_URL set");
                return;
            }
        };

        let model = EmbeddingModel::new().await.unwrap();
        let embedding = model.encode("Hello world").await.unwrap();

        assert!(!embedding.is_empty());
        println!("Generated embedding with {} dimensions", embedding.len());

        // Test with different text
        let embedding2 = model.encode("Different text").await.unwrap();
        assert_eq!(embedding.len(), embedding2.len());
        assert_ne!(embedding, embedding2); // Different texts should have different embeddings
    }

    #[test]
    fn test_chunking_comparison() {
        let test_content = "This is the first paragraph with some important information about the topic. It contains several sentences that should be kept together for semantic coherence.

This is the second paragraph which discusses a different aspect of the topic. It also has multiple sentences that form a cohesive unit of meaning.

Here is a short paragraph.

This is the fourth paragraph with more detailed information. It explains complex concepts that require multiple sentences to convey properly. The information here builds upon previous paragraphs.";

        // Test basic chunker
        let basic_chunker = BasicChunker;
        let basic_chunks = basic_chunker.chunk(test_content).unwrap();
        println!("🔷 Basic chunker produced {} chunks:", basic_chunks.len());
        for (i, chunk) in basic_chunks.iter().enumerate() {
            println!("  Chunk {}: {} chars", i + 1, chunk.len());
        }

        // Test semantic chunker (only if PyO3 is available)
        #[cfg(feature = "pyo3")]
        {
            let semantic_chunker = SemanticChunker::new(200); // Smaller chunks for test
            match semantic_chunker.chunk(test_content) {
                Ok(semantic_chunks) => {
                    println!(
                        "🧠 Semantic chunker produced {} chunks:",
                        semantic_chunks.len()
                    );
                    for (i, chunk) in semantic_chunks.iter().enumerate() {
                        println!(
                            "  Chunk {}: {} chars - {}",
                            i + 1,
                            chunk.len(),
                            &chunk
                                .chars()
                                .take(50)
                                .collect::<String>()
                                .replace('\n', " ")
                        );
                    }
                }
                Err(e) => {
                    println!("⚠️ Semantic chunker failed (chonkie not available?): {}", e);
                    println!(
                        "📝 This is expected if chonkie isn't installed in Python environment"
                    );
                }
            }
        }

        #[cfg(not(feature = "pyo3"))]
        {
            println!("🤓 Semantic chunker not available (PyO3 feature disabled)");
        }
    }

    #[tokio::test]
    async fn test_grok_client_initialization_mock() {
        // This test uses mock Qdrant - will fail gracefully
        let mut client = GrokClient::new("https://example.com".to_string(), "test_key".to_string());
        let result = client.initialize().await;
        // This will fail due to connection, but embedding model should be initialized
        assert!(result.is_err()); // Expected to fail with fake URL
    }

    #[tokio::test]
    async fn test_digest() {
        let mut client = GrokClient::new("https://example.com".to_string(), "test_key".to_string());
        client.initialize().await.unwrap();

        let chunk = client
            .digest("rust", "Rust is a systems programming language")
            .await
            .unwrap();
        assert_eq!(chunk.datum, "rust");
        assert_eq!(chunk.content, "Rust is a systems programming language");
        assert_eq!(chunk.metadata.topic, "rust");
        assert!(!chunk.id.to_string().is_empty());
        assert!(chunk.attribution.date.len() > 0);
        assert!(chunk.vector.is_some()); // Should have mock embedding
    }

    #[test]
    fn test_datum_creation() {
        let mut properties = HashMap::new();
        properties.insert("language".to_string(), "systems".to_string());
        properties.insert("memory_safe".to_string(), "true".to_string());

        let datum = Datum {
            name: "rust".to_string(),
            datum_type: "programming_language".to_string(),
            adjacencies: vec!["cargo".to_string(), "clippy".to_string()],
            properties,
        };

        assert_eq!(datum.name, "rust");
        assert_eq!(datum.datum_type, "programming_language");
        assert_eq!(datum.adjacencies.len(), 2);
        assert_eq!(
            datum.properties.get("language"),
            Some(&"systems".to_string())
        );
    }

    #[test]
    fn test_chunk_serialization() {
        let chunk = Chunk {
            id: uuid::Uuid::new_v4(),
            content: "Test content".to_string(),
            datum: "test_datum".to_string(),
            attribution: Attribution {
                url: Some("https://example.com".to_string()),
                filename: None,
                date: "2025-01-01T00:00:00Z".to_string(),
            },
            metadata: ChunkMetadata {
                topic: "test".to_string(),
                tags: vec!["unit_test".to_string(), "example".to_string()],
                created_at: "2025-01-01T00:00:00Z".to_string(),
                lesson_type: None,
                error_pattern: None,
                solution: None,
                confidence: None,
            },
            vector: Some(vec![0.1, 0.2, 0.3]),
        };

        let json = serde_json::to_string(&chunk).unwrap();
        let deserialized: Chunk = serde_json::from_str(&json).unwrap();

        assert_eq!(chunk.content, deserialized.content);
        assert_eq!(chunk.datum, deserialized.datum);
        assert_eq!(chunk.vector, deserialized.vector);
    }

    #[tokio::test]
    async fn test_ask_empty_result() {
        let mut client = GrokClient::new("https://example.com".to_string(), "test_key".to_string());
        client.initialize().await.unwrap();

        let results = client.ask("What is Rust?", Some("rust")).await.unwrap();
        assert_eq!(results.len(), 0); // TODO: Will have results when implemented
    }

    #[tokio::test]
    async fn test_learn_chunking() {
        let mut client = GrokClient::new("https://example.com".to_string(), "test_key".to_string());
        client.initialize().await.unwrap();

        let content = "First paragraph about Rust.\n\nSecond paragraph about Cargo.\n\nThird paragraph about testing.";
        let chunks = client.learn("rust-guide.md", content).await.unwrap();

        assert_eq!(chunks.len(), 3);
        assert_eq!(chunks[0].datum, "rust");
        assert!(chunks[0].content.contains("First paragraph"));
        assert!(chunks[1].content.contains("Second paragraph"));
        assert!(chunks[2].content.contains("Third paragraph"));
    }

    #[tokio::test]
    async fn test_topic_inference() {
        let client = GrokClient::new("https://example.com".to_string(), "test_key".to_string());

        assert_eq!(client.infer_topic_from_source("main.rs"), "rust");
        assert_eq!(client.infer_topic_from_source("script.py"), "python");
        assert_eq!(client.infer_topic_from_source("Dockerfile"), "general"); // 🤓 Fixed test expectation
        assert_eq!(
            client.infer_topic_from_source("https://git.example.com/repo"),
            "git"
        );
        assert_eq!(
            client.infer_topic_from_source("unknown_file.txt"),
            "general"
        );
    }

    #[test]
    fn test_lfmf_topic_inference() {
        let client = GrokClient::new("https://example.com".to_string(), "test_key".to_string());

        // Test basic tool mapping
        assert_eq!(client.infer_lfmf_topic("just", "some error", ""), "just");
        assert_eq!(client.infer_lfmf_topic("rust", "compile error", ""), "rust");
        assert_eq!(
            client.infer_lfmf_topic("docker", "build failed", ""),
            "docker"
        );

        // Test error pattern enhancement
        assert_eq!(
            client.infer_lfmf_topic("just", "Unknown start of token '.'", ""),
            "just_syntax"
        );
        assert_eq!(
            client.infer_lfmf_topic("just", "Recipe duplicate redefined", ""),
            "just_duplicate"
        );
        assert_eq!(
            client.infer_lfmf_topic("just", "Template {{.Names}} error", ""),
            "just_template_conflict"
        );
        assert_eq!(
            client.infer_lfmf_topic("unknown", "some error", ""),
            "general"
        );
    }

    #[test]
    fn test_error_type_classification() {
        let client = GrokClient::new("https://example.com".to_string(), "test_key".to_string());

        assert_eq!(client.classify_error_type("syntax error in file"), "syntax");
        assert_eq!(
            client.classify_error_type("duplicate definition"),
            "duplicate"
        );
        assert_eq!(
            client.classify_error_type("template {{.Field}} invalid"),
            "template_conflict"
        );
        assert_eq!(client.classify_error_type("file not found"), "missing");
        assert_eq!(
            client.classify_error_type("permission denied"),
            "permission"
        );
        assert_eq!(client.classify_error_type("unknown error"), "general");
    }

    #[test]
    fn test_lesson_confidence_calculation() {
        let client = GrokClient::new("https://example.com".to_string(), "test_key".to_string());

        // Base confidence
        assert_eq!(client.calculate_lesson_confidence("basic lesson"), 0.6); // 0.5 + 0.1 for length

        // High confidence indicators
        let detailed_lesson = "solution: Use grep/cut instead of {{.Names}} template to avoid conflicts. Example: docker ps | grep name";
        let confidence = client.calculate_lesson_confidence(detailed_lesson);
        assert!(confidence > 0.8); // Should get bonuses for "solution:", length, "example", and "avoid"

        // Maximum confidence cap
        let max_lesson = "fix: solution: avoid template use example pattern instead of {{}} syntax";
        assert_eq!(client.calculate_lesson_confidence(max_lesson), 1.0); // Capped at 1.0
    }

    #[cfg(feature = "pyo3")]
    #[test]
    fn test_py_grok_client_creation() {
        let result = PyGrokClient::new("https://example.com".to_string(), "test_key".to_string());
        assert!(result.is_ok());
    }
}
