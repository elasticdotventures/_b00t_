//! Unified learn command - intelligent knowledge management
//!
//! Combines LFMF lessons, curated docs, man pages, and RAG into one command

use anyhow::{Context, Result};
use b00t_c0re_lib::{DisplayOpts, GrokClient, KnowledgeSource, LfmfSystem, ManPage};
use clap::Parser;
use std::fs;
use tiktoken_rs::o200k_base;

#[derive(Parser, Debug)]
pub struct LearnArgs {
    /// Topic to learn about (e.g., git, rust, just)
    #[arg(help = "Topic to learn about")]
    pub topic: Option<String>,

    // Display modifiers
    #[arg(long, help = "Force display man page")]
    pub man: bool,

    #[arg(long, help = "Show table of contents only")]
    pub toc: bool,

    #[arg(long, help = "Jump to specific section number")]
    pub section: Option<usize>,

    #[arg(long, help = "Concise token-optimized output")]
    pub concise: bool,

    // Record lesson (replaces lfmf)
    #[arg(long, help = "Record lesson: '<topic>: <body>'")]
    pub record: Option<String>,

    #[arg(long, help = "Record globally (default: repo)")]
    pub global: bool,

    // Search lessons (replaces advice)
    #[arg(long, help = "Search lessons: '<query>' or 'list'")]
    pub search: Option<String>,

    #[arg(long, help = "Max search results", default_value = "5")]
    pub limit: usize,

    // RAG operations (from grok)
    #[arg(long, help = "Digest content to RAG")]
    pub digest: Option<String>,

    #[arg(long, help = "Query RAG knowledgebase")]
    pub ask: Option<String>,
}

pub fn handle_learn(path: &str, args: LearnArgs) -> Result<()> {
    let rt = tokio::runtime::Runtime::new().context("Failed to create async runtime")?;

    rt.block_on(async {
        // Record lesson
        if let Some(lesson) = args.record {
            return handle_record(path, args.topic.as_deref(), &lesson, args.global).await;
        }

        // Search lessons
        if let Some(query) = args.search {
            return handle_search(path, args.topic.as_deref(), &query, args.limit).await;
        }

        // Digest to RAG
        if let Some(content) = args.digest {
            return handle_digest(path, args.topic.as_deref(), &content).await;
        }

        // Query RAG
        if let Some(query) = args.ask {
            return handle_ask(path, args.topic.as_deref(), &query, args.limit).await;
        }

        // Default: display knowledge
        let topic = args
            .topic
            .ok_or_else(|| anyhow::anyhow!("Topic required. Use: b00t learn <topic>"))?;

        handle_display(
            path,
            &topic,
            DisplayOpts {
                force_man: args.man,
                toc_only: args.toc,
                section: args.section,
                concise: args.concise,
            },
        )
        .await
    })
}

async fn handle_display(path: &str, topic: &str, opts: DisplayOpts) -> Result<()> {
    let knowledge = KnowledgeSource::gather(topic, path).await?;

    // Auto-create datum if man page exists but no datum
    if knowledge.man_page.is_some() && !datum_exists(path, topic)? {
        if let Some(ref man) = knowledge.man_page {
            create_datum_from_man(path, topic, man)?;
            println!("✅ Auto-created datum: _b00t_/{}.cli.toml\n", topic);
        }
    }

    // Check if any knowledge exists
    if !knowledge.has_knowledge() {
        anyhow::bail!(
            "No knowledge found for '{}'. Try:\n  • b00t learn {} --record \"<topic>: <body>\"\n  • b00t learn {} --man (if man page exists)",
            topic, topic, topic
        );
    }

    knowledge.display(&opts)
}

async fn handle_record(
    path: &str,
    topic: Option<&str>,
    lesson: &str,
    global: bool,
) -> Result<()> {
    let topic = topic.ok_or_else(|| anyhow::anyhow!("Topic required for recording lesson"))?;

    // Parse "<topic>: <body>" format
    let parts: Vec<&str> = lesson.splitn(2, ':').map(|s| s.trim()).collect();
    if parts.len() != 2 {
        anyhow::bail!("Lesson must be in '<topic>: <body>' format.\n\nExample:\n  b00t learn git --record \"atomic commits: Commit small, focused changes for easier review\"");
    }

    let lesson_topic = parts[0];
    let body = parts[1];

    // Token count enforcement (using tiktoken)
    let bpe = o200k_base().map_err(|e| anyhow::anyhow!("Failed to load tokenizer: {e}"))?;
    let topic_tokens = bpe.encode_with_special_tokens(lesson_topic).len();
    let body_tokens = bpe.encode_with_special_tokens(body).len();

    if topic_tokens > 25 {
        anyhow::bail!(
            "Topic must be <25 tokens (OpenAI tiktoken). Yours: {}.",
            topic_tokens
        );
    }

    if body_tokens > 250 {
        anyhow::bail!(
            "Body must be <250 tokens (OpenAI tiktoken). Yours: {}.",
            body_tokens
        );
    }

    if lesson_topic.is_empty() || body.is_empty() {
        anyhow::bail!("Topic and body must not be empty.");
    }

    // Affirmative style check
    if body.to_lowercase().contains("don't") || body.to_lowercase().contains("never") {
        println!("⚠️  Consider using positive, affirmative style (e.g., 'Do X for Y benefit').\n");
    }

    // Use LFMF system for recording
    let config = LfmfSystem::load_config(path)?;
    let mut lfmf_system = LfmfSystem::new(config);

    // Set datum lookup for category resolution
    let lookup = crate::datum_utils::B00tDatumLookup::new(path.to_string());
    lfmf_system.set_datum_lookup(lookup);

    let scope = if global { "global" } else { "repo" };

    lfmf_system
        .record_lesson(topic, lesson_topic, body, scope)
        .await
        .context("Failed to record lesson")?;

    println!("✅ Recorded lesson for '{}' in {} scope", topic, scope);
    println!("\nView: b00t learn {} --search list", topic);

    Ok(())
}

async fn handle_search(
    path: &str,
    topic: Option<&str>,
    query: &str,
    limit: usize,
) -> Result<()> {
    let topic = topic.ok_or_else(|| anyhow::anyhow!("Topic required for searching lessons"))?;

    let config = LfmfSystem::load_config(path)?;
    let mut lfmf_system = LfmfSystem::new(config);

    let lookup = crate::datum_utils::B00tDatumLookup::new(path.to_string());
    lfmf_system.set_datum_lookup(lookup);

    if query.eq_ignore_ascii_case("list") {
        // List all lessons
        let lessons = lfmf_system.list_lessons(topic).await?;

        if lessons.is_empty() {
            println!("No lessons found for '{}'", topic);
            println!("\nRecord one: b00t learn {} --record \"<topic>: <body>\"", topic);
            return Ok(());
        }

        println!("## Lessons for '{}' ({} total)\n", topic, lessons.len());
        for (idx, lesson) in lessons.iter().enumerate() {
            println!("{}. **{}**: {}", idx + 1, lesson.topic, lesson.body);
        }
    } else {
        // Search lessons
        let results = lfmf_system.search_lessons(topic, query, limit).await?;

        if results.is_empty() {
            println!("No matching lessons found for query: '{}'", query);
            return Ok(());
        }

        println!("## Search Results for '{}'\n", query);
        for (idx, result) in results.iter().enumerate() {
            println!(
                "{}. **{}** (score: {:.2})\n   {}\n",
                idx + 1,
                result.lesson.topic,
                result.score,
                result.lesson.body
            );
        }
    }

    Ok(())
}

async fn handle_digest(path: &str, topic: Option<&str>, content: &str) -> Result<()> {
    let topic = topic
        .ok_or_else(|| anyhow::anyhow!("Topic required for digesting content to RAG"))?
        .to_string();

    let client = GrokClient::new().context("Failed to create Grok client")?;

    client
        .digest(&topic, content)
        .await
        .context("Failed to digest content")?;

    println!("✅ Digested content to RAG under topic '{}'", topic);
    println!("\nQuery: b00t learn {} --ask \"<question>\"", topic);

    Ok(())
}

async fn handle_ask(
    path: &str,
    topic: Option<&str>,
    query: &str,
    limit: usize,
) -> Result<()> {
    let client = GrokClient::new().context("Failed to create Grok client")?;

    let results = client
        .ask(query, topic, Some(limit))
        .await
        .context("Failed to query RAG")?;

    if results.chunks.is_empty() {
        println!("No results found for query: '{}'", query);
        return Ok(());
    }

    println!("## RAG Results for '{}'\n", query);
    for (idx, chunk) in results.chunks.iter().enumerate() {
        println!(
            "{}. (score: {:.2}, topic: {})\n   {}\n",
            idx + 1,
            chunk.score,
            chunk.topic.as_deref().unwrap_or("unknown"),
            chunk.content.lines().next().unwrap_or("")
        );
    }

    Ok(())
}

/// Check if datum exists for topic
fn datum_exists(path: &str, topic: &str) -> Result<bool> {
    let datum_path = format!("{}/{}.cli.toml", path, topic);
    Ok(std::path::Path::new(&datum_path).exists())
}

/// Create datum from man page
fn create_datum_from_man(path: &str, topic: &str, man: &ManPage) -> Result<()> {
    let datum_content = man.to_datum_toml();
    let datum_path = format!("{}/{}.cli.toml", path, topic);
    fs::write(datum_path, datum_content).context("Failed to write datum file")?;
    Ok(())
}
