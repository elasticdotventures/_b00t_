use anyhow::{Context, Result};
use b00t_c0re_lib::{DocumentSource, GrokClient, LoaderType, RagLightConfig, RagLightManager};
use clap::Subcommand;
use std::{fs, path::PathBuf};
use uuid::Uuid;

#[derive(Subcommand, Clone)]
pub enum GrokCommands {
    /// Digest content into chunks about a topic
    Digest {
        /// Topic to digest content about
        #[arg(short, long)]
        topic: String,
        /// Content to digest
        content: String,
        /// Use RAG backend (default backend: raglight)
        #[arg(
            long = "rag",
            value_name = "BACKEND",
            num_args = 0..=1,
            default_missing_value = "raglight",
            help = "Use RAG backend (optionally specify backend name)"
        )]
        rag: Option<String>,
    },
    /// Ask questions and search the knowledgebase
    Ask {
        /// Query to search for
        query: String,
        /// Optional topic to filter by
        #[arg(short, long)]
        topic: Option<String>,
        /// Maximum results to return (default: 10)
        #[arg(long, help = "Maximum number of results to return (default: 10)")]
        limit: Option<usize>,
        /// Use RAG backend (default backend: raglight)
        #[arg(
            long = "rag",
            value_name = "BACKEND",
            num_args = 0..=1,
            default_missing_value = "raglight",
            help = "Use RAG backend (optionally specify backend name)"
        )]
        rag: Option<String>,
    },
    /// Learn from URLs or files
    Learn {
        /// Source URL or file path
        #[arg(short, long)]
        source: Option<String>,
        /// Content to learn from
        content: String,
        /// Topic to associate with ingested content (required when using --rag)
        #[arg(short, long, help = "Topic to associate with the ingested content")]
        topic: Option<String>,
        /// Use RAG backend (default backend: raglight)
        #[arg(
            long = "rag",
            value_name = "BACKEND",
            num_args = 0..=1,
            default_missing_value = "raglight",
            help = "Use RAG backend (optionally specify backend name)"
        )]
        rag: Option<String>,
    },
}

pub async fn handle_grok_command(command: GrokCommands) -> Result<()> {
    match command {
        GrokCommands::Digest {
            topic,
            content,
            rag,
        } => {
            let backend = parse_backend(rag)?;
            if let Some(backend) = backend {
                handle_rag_digest(&topic, &content, backend).await
            } else {
                let mut client = GrokClient::new();
                client.initialize().await?;
                handle_digest(&client, &topic, &content).await
            }
        }
        GrokCommands::Ask {
            query,
            topic,
            limit,
            rag,
        } => {
            let backend = parse_backend(rag)?;
            if let Some(backend) = backend {
                handle_rag_ask(&query, topic.as_deref(), limit, backend).await
            } else {
                let mut client = GrokClient::new();
                client.initialize().await?;
                handle_ask(&client, &query, topic.as_deref(), limit).await
            }
        }
        GrokCommands::Learn {
            source,
            content,
            topic,
            rag,
        } => {
            let backend = parse_backend(rag)?;
            if let Some(backend) = backend {
                handle_rag_learn(source.as_deref(), &content, topic.as_deref(), backend).await
            } else {
                let mut client = GrokClient::new();
                client.initialize().await?;
                handle_learn(&client, source.as_deref(), &content).await
            }
        }
    }
}

async fn handle_digest(client: &GrokClient, topic: &str, content: &str) -> Result<()> {
    println!("🧠 Digesting content for topic '{}'...", topic);

    let result = client.digest(topic, content).await?;

    if result.success {
        println!("✅ Digested chunk for topic '{}':", topic);
        println!("📄 ID: {}", result.chunk_id);
        println!("💬 Content: {}...", result.content_preview);
        println!("📅 Created: {}", result.created_at);
    } else {
        eprintln!(
            "❌ Digest failed: {}",
            result.message.unwrap_or("Unknown error".to_string())
        );
        return Err(anyhow::anyhow!("Failed to digest content"));
    }

    Ok(())
}

async fn handle_ask(
    client: &GrokClient,
    query: &str,
    topic: Option<&str>,
    limit: Option<usize>,
) -> Result<()> {
    println!("🔍 Searching knowledgebase for: '{}'", query);
    if let Some(topic) = topic {
        println!("🎯 Filtering by topic: '{}'", topic);
    }

    let max_results = limit.unwrap_or(10);
    let result = client.ask(query, topic, Some(max_results)).await?;

    if result.success {
        println!("📊 Found {} results:", result.total_found);

        for (i, chunk) in result.results.iter().enumerate() {
            println!("\n{}. 📄 {}", i + 1, chunk.topic);
            println!(
                "   💬 {}",
                chunk.content.chars().take(100).collect::<String>()
            );
            if let Some(ref source) = chunk.source {
                println!("   🔗 Source: {}", source);
            }
            println!("   📅 {}", chunk.created_at);
        }
    } else {
        eprintln!(
            "❌ Search failed: {}",
            result.message.unwrap_or("Unknown error".to_string())
        );
        return Err(anyhow::anyhow!("Failed to search knowledgebase"));
    }

    Ok(())
}

async fn handle_learn(client: &GrokClient, source: Option<&str>, content: &str) -> Result<()> {
    let source_str = source.unwrap_or("direct_input");
    println!("📚 Learning from source: '{}'", source_str);

    let result = client.learn(content, Some(source_str)).await?;

    if result.success {
        println!("✅ Successfully learned from '{}':", result.source);
        println!("📦 Generated {} chunks", result.chunks_created);

        for (i, summary) in result.chunk_summaries.iter().enumerate() {
            println!("\n{}. 📄 Topic: {}", i + 1, summary.topic);
            println!("   💬 {}", summary.content_preview);
            if !summary.tags.is_empty() {
                println!("   🏷️ Tags: {}", summary.tags.join(", "));
            }
        }
    } else {
        eprintln!(
            "❌ Learn failed: {}",
            result.message.unwrap_or("Unknown error".to_string())
        );
        return Err(anyhow::anyhow!("Failed to learn from content"));
    }

    Ok(())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum RagBackend {
    Raglight,
}

impl RagBackend {
    fn parse(value: &str) -> Result<Self> {
        match value.trim().to_lowercase().as_str() {
            "raglight" | "rag-light" | "rag_light" => Ok(Self::Raglight),
            other => Err(anyhow::anyhow!("Unsupported RAG backend '{}'", other)),
        }
    }

    fn display_name(self) -> &'static str {
        match self {
            Self::Raglight => "RAGLight",
        }
    }
}

fn parse_backend(raw: Option<String>) -> Result<Option<RagBackend>> {
    match raw {
        Some(value) => Ok(Some(RagBackend::parse(&value)?)),
        None => Ok(None),
    }
}

async fn handle_rag_digest(topic: &str, content: &str, backend: RagBackend) -> Result<()> {
    match backend {
        RagBackend::Raglight => {
            println!(
                "🧠 [{}] ingesting inline content for topic '{}'",
                backend.display_name(),
                topic
            );
            let mut manager = create_rag_manager()?;

            let stored_path = store_inline_content(topic, content)?;
            let source_path = stored_path
                .to_str()
                .ok_or_else(|| anyhow::anyhow!("Invalid UTF-8 in stored content path"))?
                .to_string();

            let doc = DocumentSource {
                source: source_path.clone(),
                loader_type: Some(LoaderType::Text),
                topic: topic.to_string(),
                metadata: None,
            };

            let job_id = manager.add_document(doc).await?;
            println!(
                "✅ Queued RAGLight indexing job {} for topic '{}'",
                job_id, topic
            );
            println!("📄 Source stored at {}", source_path);
            println!("⌛ Indexing runs asynchronously; monitor RAG logs for completion.");
            Ok(())
        }
    }
}

async fn handle_rag_ask(
    query: &str,
    topic: Option<&str>,
    limit: Option<usize>,
    backend: RagBackend,
) -> Result<()> {
    let topic = topic
        .ok_or_else(|| anyhow::anyhow!("--topic is required when using --rag for grok ask"))?;

    match backend {
        RagBackend::Raglight => {
            println!(
                "🔍 [{}] querying topic '{}' for '{}'",
                backend.display_name(),
                topic,
                query
            );
            let manager = create_rag_manager()?;
            let raw = manager.query(topic, query, limit).await?;

            if raw.trim().is_empty() {
                println!("⚠️ No results returned from {}", backend.display_name());
                return Ok(());
            }

            if let Ok(json_value) = serde_json::from_str::<serde_json::Value>(&raw) {
                println!("{}", serde_json::to_string_pretty(&json_value)?);
            } else {
                println!("{}", raw);
            }
            Ok(())
        }
    }
}

async fn handle_rag_learn(
    source: Option<&str>,
    content: &str,
    topic: Option<&str>,
    backend: RagBackend,
) -> Result<()> {
    let topic = topic
        .ok_or_else(|| anyhow::anyhow!("--topic is required when using --rag for grok learn"))?;

    match backend {
        RagBackend::Raglight => {
            println!(
                "📚 [{}] learning for topic '{}'",
                backend.display_name(),
                topic
            );
            let mut manager = create_rag_manager()?;

            let (source_path, loader_type) = if let Some(src) = source {
                (normalize_source_path(src), None)
            } else {
                let stored = store_inline_content(topic, content)?;
                let src_path = stored
                    .to_str()
                    .ok_or_else(|| anyhow::anyhow!("Invalid UTF-8 in stored content path"))?
                    .to_string();
                (src_path, Some(LoaderType::Text))
            };

            let doc = DocumentSource {
                source: source_path.clone(),
                loader_type,
                topic: topic.to_string(),
                metadata: None,
            };

            let job_id = manager.add_document(doc).await?;
            println!(
                "✅ Queued RAGLight learn job {} for topic '{}'",
                job_id, topic
            );
            println!("📄 Source: {}", source_path);
            println!("⌛ Indexing runs asynchronously; monitor RAG logs for completion.");
            Ok(())
        }
    }
}

fn create_rag_manager() -> Result<RagLightManager> {
    let config = RagLightConfig::default();
    RagLightManager::new(config)
}

fn store_inline_content(topic: &str, content: &str) -> Result<PathBuf> {
    let base_dir = dirs::home_dir()
        .ok_or_else(|| anyhow::anyhow!("Unable to determine home directory for RAG storage"))?
        .join(".b00t")
        .join("raglight")
        .join("uploads");

    fs::create_dir_all(&base_dir).with_context(|| {
        format!(
            "Failed to create RAG upload directory {}",
            base_dir.display()
        )
    })?;

    let sanitized = sanitize_for_filename(topic);
    let file_path = base_dir.join(format!("{}-{}.txt", sanitized, Uuid::new_v4()));

    fs::write(&file_path, content).with_context(|| {
        format!(
            "Failed to write inline RAG content to {}",
            file_path.display()
        )
    })?;

    Ok(file_path)
}

fn sanitize_for_filename(input: &str) -> String {
    let mut sanitized: String = input
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '_' })
        .collect();

    if sanitized.is_empty() {
        sanitized.push_str("topic");
    }

    while sanitized.contains("__") {
        sanitized = sanitized.replace("__", "_");
    }

    sanitized.trim_matches('_').to_string()
}

fn normalize_source_path(source: &str) -> String {
    if let Some(rest) = source.strip_prefix("~/") {
        if let Some(home) = dirs::home_dir() {
            return home.join(rest).to_string_lossy().into_owned();
        }
    }

    source.to_string()
}

// 🤓 Helper functions removed - configuration now handled by b00t-c0re-lib::GrokClient
// which reads QDRANT_URL and QDRANT_API_KEY from environment variables
