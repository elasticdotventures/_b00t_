//! Integration tests for b00t learn command
//!
//! Tests the unified knowledge management system including:
//! - LFMF lesson recording and retrieval
//! - Learn content (curated markdown docs)
//! - Man page integration
//! - RAG operations (digest, ask, learn)
//! - Knowledge source aggregation

use anyhow::Result;
use b00t_c0re_lib::{
    DisplayOpts, GrokClient, KnowledgeSource, LfmfConfig, LfmfSystem, ManPage,
};
use b00t_c0re_lib::lfmf::{FilesystemConfig, QdrantConfig};
use std::fs;
use tempfile::TempDir;

/// Helper to create a temporary b00t workspace for testing
fn create_test_workspace() -> Result<TempDir> {
    let temp_dir = TempDir::new()?;
    let b00t_path = temp_dir.path().join("_b00t_");
    fs::create_dir_all(&b00t_path)?;

    // Create learn directory
    let learn_dir = b00t_path.join("learn");
    fs::create_dir_all(&learn_dir)?;

    // Create a sample learn markdown file
    let rust_md = learn_dir.join("rust.md");
    fs::write(
        &rust_md,
        r#"---
PyO3 feature control: default-features = false in Cargo workspace dependencies
---

# Rust Learning Content

## Memory Safety
Rust ensures memory safety without garbage collection through ownership and borrowing.

## Zero-Cost Abstractions
Abstractions in Rust have no runtime overhead.

## Concurrency
Rust's ownership system prevents data races at compile time.
"#,
    )?;

    // Create learn.toml configuration
    let learn_toml = temp_dir.path().join("learn.toml");
    fs::write(
        &learn_toml,
        r#"# b00t learn topics configuration
[topics]
rust = "_b00t_/learn/rust.md"
docker = "_b00t_/docker.🐳/.md"
"#,
    )?;

    // Create LFMF config directory
    let lfmf_dir = b00t_path.join("lfmf");
    fs::create_dir_all(&lfmf_dir)?;

    Ok(temp_dir)
}

#[test]
fn test_knowledge_source_structure() {
    // Test that KnowledgeSource can be created with all fields
    let knowledge = KnowledgeSource {
        topic: "rust".to_string(),
        lfmf_lessons: vec!["lesson1".to_string(), "lesson2".to_string()],
        learn_content: Some("# Rust\nContent here".to_string()),
        man_page: None,
        rag_results: vec![],
    };

    assert_eq!(knowledge.topic, "rust");
    assert_eq!(knowledge.lfmf_lessons.len(), 2);
    assert!(knowledge.learn_content.is_some());
    assert!(knowledge.man_page.is_none());
}

#[test]
fn test_display_opts_defaults() {
    let opts = DisplayOpts::default();

    assert!(!opts.force_man);
    assert!(!opts.toc_only);
    assert!(opts.section.is_none());
    assert!(!opts.concise);
}

#[test]
fn test_lfmf_config_creation() {
    let config = LfmfConfig {
        qdrant: QdrantConfig {
            url: "http://localhost:6334".to_string(),
            api_key: None,
            collection: Some("lessons".to_string()),
        },
        filesystem: FilesystemConfig {
            learn_dir: "./_b00t_/learn".to_string(),
        },
    };

    assert_eq!(config.qdrant.url, "http://localhost:6334");
    assert!(config.qdrant.collection.is_some());
}

#[tokio::test]
async fn test_lfmf_system_creation() {
    let config = LfmfConfig {
        qdrant: QdrantConfig::default(),
        filesystem: FilesystemConfig {
            learn_dir: "./_b00t_/learn".to_string(),
        },
    };

    let _system = LfmfSystem::new(config.clone());
    // System is created successfully
    assert_eq!(config.filesystem.learn_dir, "./_b00t_/learn");
}

#[test]
fn test_grok_client_creation() {
    let _client = GrokClient::new();
    // Just verify it can be created without panicking
}

#[tokio::test]
#[ignore = "Requires filesystem access"]
async fn test_learn_content_from_file() -> Result<()> {
    let workspace = create_test_workspace()?;
    let b00t_path = workspace.path().join("_b00t_");

    // Use b00t_c0re_lib::learn to get content
    let content = b00t_c0re_lib::learn::get_learn_lesson(
        b00t_path.to_str().unwrap(),
        "rust",
    )?;

    assert!(content.contains("Memory Safety"));
    assert!(content.contains("Zero-Cost Abstractions"));

    Ok(())
}

#[tokio::test]
#[ignore = "Requires filesystem access"]
async fn test_knowledge_source_has_knowledge() -> Result<()> {
    let workspace = create_test_workspace()?;
    let b00t_path = workspace.path().join("_b00t_");

    let knowledge = KnowledgeSource::gather("rust", b00t_path.to_str().unwrap()).await?;

    // Should have at least learn content
    assert!(knowledge.has_knowledge());

    Ok(())
}

#[test]
fn test_man_page_parsing() {
    // Test that we can attempt to parse a man page
    // This will fail for non-existent commands but tests the API
    let result = ManPage::from_command("nonexistent_command_xyz123");
    assert!(result.is_err()); // Should fail for non-existent command
}

#[tokio::test]
#[ignore = "Requires LFMF storage setup"]
async fn test_lfmf_lesson_recording() -> Result<()> {
    let workspace = create_test_workspace()?;
    let b00t_path = workspace.path().join("_b00t_");

    let config = LfmfSystem::load_config(b00t_path.to_str().unwrap())?;
    let mut system = LfmfSystem::new(config);

    // Note: This test is ignored because it requires Redis/Qdrant
    // But it tests the API surface
    let lesson = "atomic commits: Commit small, focused changes for easier review";
    let result = system.record_lesson("git", lesson).await;

    // In a real environment with Redis/Qdrant, this would succeed
    // In test env without these services, it may fail - that's OK
    println!("Record result: {:?}", result);

    Ok(())
}

#[tokio::test]
#[ignore = "Requires Qdrant and MCP server"]
async fn test_grok_digest_api() {
    let client = GrokClient::new();

    // Test the API surface - this will fail without services but validates types
    let result = client.digest("rust", "Rust is memory safe").await;

    // Expected to fail without Qdrant/MCP server running
    assert!(result.is_err());
}

#[tokio::test]
#[ignore = "Requires Qdrant and MCP server"]
async fn test_grok_ask_api() {
    let client = GrokClient::new();

    let result = client.ask("memory safety", Some("rust"), Some(5)).await;

    // Expected to fail without Qdrant/MCP server running
    assert!(result.is_err());
}

#[test]
fn test_learn_topics_discovery() -> Result<()> {
    let workspace = create_test_workspace()?;
    let b00t_path = workspace.path().join("_b00t_");

    let topics = b00t_c0re_lib::learn::get_learn_topics(b00t_path.to_str().unwrap())?;

    // Should discover rust from both learn.toml and learn/rust.md
    assert!(topics.contains(&"rust".to_string()));

    Ok(())
}

/// Test that learn content can be retrieved without errors
#[tokio::test]
#[ignore = "Requires filesystem"]
async fn test_full_knowledge_gathering() -> Result<()> {
    let workspace = create_test_workspace()?;
    let b00t_path = workspace.path().join("_b00t_");

    // Gather all knowledge for rust
    let knowledge = KnowledgeSource::gather("rust", b00t_path.to_str().unwrap()).await?;

    // Verify structure
    assert_eq!(knowledge.topic, "rust");

    // Should have learn content from our test file
    if let Some(content) = &knowledge.learn_content {
        assert!(content.contains("Memory Safety"));
    }

    // Display with default options (just ensures no panic)
    let _opts = DisplayOpts::default();
    // Note: This will print to stdout but shouldn't panic
    // knowledge.display(&_opts)?;

    Ok(())
}

#[test]
fn test_display_opts_builder() {
    // Test various DisplayOpts configurations
    let opts1 = DisplayOpts {
        force_man: true,
        toc_only: false,
        section: Some(1),
        concise: false,
    };
    assert!(opts1.force_man);
    assert_eq!(opts1.section, Some(1));

    let opts2 = DisplayOpts {
        force_man: false,
        toc_only: true,
        section: None,
        concise: true,
    };
    assert!(opts2.toc_only);
    assert!(opts2.concise);
}

/// Test RAG result types
#[test]
fn test_chunk_result_structure() {
    let chunk = b00t_c0re_lib::ChunkResult {
        id: "chunk-123".to_string(),
        content: "Test content about Rust".to_string(),
        topic: "rust".to_string(),
        tags: vec!["memory-safety".to_string(), "ownership".to_string()],
        source: Some("rust-book.md".to_string()),
        created_at: "2025-01-01T00:00:00Z".to_string(),
    };

    assert_eq!(chunk.id, "chunk-123");
    assert_eq!(chunk.tags.len(), 2);
    assert!(chunk.source.is_some());
}

/// Test that knowledge source can handle empty results gracefully
#[test]
fn test_knowledge_source_empty() {
    let knowledge = KnowledgeSource {
        topic: "unknown".to_string(),
        lfmf_lessons: vec![],
        learn_content: None,
        man_page: None,
        rag_results: vec![],
    };

    assert!(!knowledge.has_knowledge());
}

/// Test LFMF config loading from TOML
#[test]
fn test_lfmf_config_toml_parsing() -> Result<()> {
    let toml_content = r#"
[qdrant]
url = "http://localhost:6334"
api_key = "test-key"
collection = "lessons"

[filesystem]
learn_dir = "./_b00t_/learn"
"#;

    let config: LfmfConfig = toml::from_str(toml_content)?;

    assert_eq!(config.qdrant.url, "http://localhost:6334");
    assert_eq!(config.qdrant.api_key, Some("test-key".to_string()));
    assert_eq!(config.qdrant.collection, Some("lessons".to_string()));
    assert_eq!(config.filesystem.learn_dir, "./_b00t_/learn");

    Ok(())
}
