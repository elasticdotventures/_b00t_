use anyhow::Result;
use clap::Subcommand;
use b00t_c0re_lib::GrokClient;

#[derive(Subcommand, Clone)]
pub enum GrokCommands {
    /// Digest content into chunks about a topic
    Digest {
        /// Topic to digest content about
        #[arg(short, long)]
        topic: String,
        /// Content to digest
        content: String,
    },
    /// Ask questions and search the knowledgebase
    Ask {
        /// Query to search for
        query: String,
        /// Optional topic to filter by
        #[arg(short, long)]
        topic: Option<String>,
    },
    /// Learn from URLs or files
    Learn {
        /// Source URL or file path
        #[arg(short, long)]
        source: Option<String>,
        /// Content to learn from
        content: String,
    },
}

pub async fn handle_grok_command(command: GrokCommands) -> Result<()> {
    let mut client = GrokClient::new();
    client.initialize().await?;

    match command {
        GrokCommands::Digest { topic, content } => {
            handle_digest(&client, &topic, &content).await
        }
        GrokCommands::Ask { query, topic } => {
            handle_ask(&client, &query, topic.as_deref()).await
        }
        GrokCommands::Learn { source, content } => {
            handle_learn(&client, source.as_deref(), &content).await
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
        eprintln!("❌ Digest failed: {}", result.message.unwrap_or("Unknown error".to_string()));
        return Err(anyhow::anyhow!("Failed to digest content"));
    }
    
    Ok(())
}

async fn handle_ask(client: &GrokClient, query: &str, topic: Option<&str>) -> Result<()> {
    println!("🔍 Searching knowledgebase for: '{}'", query);
    if let Some(topic) = topic {
        println!("🎯 Filtering by topic: '{}'", topic);
    }
    
    let result = client.ask(query, topic, Some(10)).await?;
    
    if result.success {
        println!("📊 Found {} results:", result.total_found);
        
        for (i, chunk) in result.results.iter().enumerate() {
            println!("\n{}. 📄 {}", i + 1, chunk.topic);
            println!("   💬 {}", chunk.content.chars().take(100).collect::<String>());
            if let Some(ref source) = chunk.source {
                println!("   🔗 Source: {}", source);
            }
            println!("   📅 {}", chunk.created_at);
        }
    } else {
        eprintln!("❌ Search failed: {}", result.message.unwrap_or("Unknown error".to_string()));
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
        eprintln!("❌ Learn failed: {}", result.message.unwrap_or("Unknown error".to_string()));
        return Err(anyhow::anyhow!("Failed to learn from content"));
    }
    
    Ok(())
}

// 🤓 Helper functions removed - configuration now handled by b00t-c0re-lib::GrokClient
// which reads QDRANT_URL and QDRANT_API_KEY from environment variables