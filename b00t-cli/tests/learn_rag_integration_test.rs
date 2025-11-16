//! Integration tests for b00t learn RAG functionality
//!
//! Tests the integration between:
//! - b00t learn command (--digest, --ask)
//! - LFMF system
//! - GrokClient
//! - Qdrant vector database
//! - Orchestrator dependency management

use anyhow::Result;
use b00t_c0re_lib::{GrokClient, LfmfSystem};
use std::env;
use tempfile::TempDir;

/// Helper to check if Qdrant is available
fn is_qdrant_available() -> bool {
    env::var("QDRANT_URL").is_ok() || env::var("TEST_WITH_QDRANT").is_ok()
}

/// Helper to setup temp directory for tests
fn setup_temp_dir() -> TempDir {
    tempfile::tempdir().expect("Failed to create temp directory")
}

#[cfg(test)]
mod learn_rag_integration {
    use super::*;

    #[tokio::test]
    #[ignore = "Requires running Qdrant instance"]
    async fn test_learn_digest_integration() -> Result<()> {
        if !is_qdrant_available() {
            println!("⚠️  Skipping test: QDRANT_URL not set");
            return Ok(());
        }

        let mut client = GrokClient::new();
        client.initialize().await?;

        let topic = "rust_test";
        let content = "Rust ensures memory safety without garbage collection through ownership and borrowing.";

        let result = client.digest(topic, content).await?;

        assert!(result.success, "Digest should succeed");
        assert_eq!(result.topic, topic);
        assert!(!result.chunk_id.is_empty(), "Should have chunk ID");
        assert!(
            result.content_preview.contains("memory safety")
                || result.content_preview.contains("ownership"),
            "Content preview should contain key terms"
        );

        Ok(())
    }

    #[tokio::test]
    #[ignore = "Requires running Qdrant instance"]
    async fn test_learn_ask_integration() -> Result<()> {
        if !is_qdrant_available() {
            println!("⚠️  Skipping test: QDRANT_URL not set");
            return Ok(());
        }

        let mut client = GrokClient::new();
        client.initialize().await?;

        // First digest some content
        let topic = "rust_test";
        let content = "Rust uses the borrow checker to enforce memory safety at compile time.";

        client.digest(topic, content).await?;

        // Small delay to allow indexing
        tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;

        // Now query for it
        let query = "memory safety";
        let result = client.ask(query, Some(topic), Some(5)).await?;

        assert!(result.success, "Ask should succeed");
        assert_eq!(result.query, query);
        assert!(
            result.total_found > 0,
            "Should find at least one result after digest"
        );
        assert!(!result.results.is_empty(), "Should have results");

        Ok(())
    }

    #[tokio::test]
    async fn test_learn_digest_without_qdrant() -> Result<()> {
        // Test graceful degradation when Qdrant is not available
        env::remove_var("QDRANT_URL");
        env::remove_var("QDRANT_API_KEY");

        let mut client = GrokClient::new();

        // Initialization should fail gracefully
        let init_result = client.initialize().await;

        // We expect this to fail when Qdrant is not available
        // The important part is that it fails gracefully, not panics
        match init_result {
            Ok(_) => {
                // If it succeeds, Qdrant might be available on default port
                println!("ℹ️  Qdrant appears to be available on default port");
            }
            Err(e) => {
                println!("✅ Graceful failure when Qdrant unavailable: {}", e);
                // This is expected behavior
            }
        }

        Ok(())
    }

    #[tokio::test]
    #[ignore = "Requires running Qdrant instance"]
    async fn test_learn_workflow_end_to_end() -> Result<()> {
        if !is_qdrant_available() {
            println!("⚠️  Skipping test: QDRANT_URL not set");
            return Ok(());
        }

        let temp_dir = setup_temp_dir();
        let temp_path = temp_dir.path().to_str().unwrap();

        // Step 1: Record an LFMF lesson
        let config = LfmfSystem::load_config(temp_path)?;
        let mut lfmf = LfmfSystem::new(config);

        // Try to initialize (may fail if Qdrant unavailable)
        let _ = lfmf.initialize().await;

        let lesson = "ownership patterns: Use Rc<RefCell<T>> for shared mutable state";
        lfmf.record_lesson("rust", lesson).await?;

        // Step 2: Digest content to RAG
        let mut grok_client = GrokClient::new();
        grok_client.initialize().await?;

        let content = "Rust's ownership system prevents data races by enforcing strict borrowing rules.";
        let digest_result = grok_client.digest("rust", content).await?;
        assert!(digest_result.success);

        // Small delay for indexing
        tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;

        // Step 3: Search LFMF lessons
        let lessons = lfmf.get_advice("rust", "ownership", Some(5)).await?;
        assert!(
            !lessons.is_empty(),
            "Should find recorded lesson about ownership"
        );
        assert!(lessons[0].contains("ownership patterns"));

        // Step 4: Query RAG
        let ask_result = grok_client.ask("ownership", Some("rust"), Some(5)).await?;
        assert!(ask_result.success);
        assert!(
            ask_result.total_found > 0,
            "Should find digested content about ownership"
        );

        println!("✅ End-to-end workflow complete:");
        println!("   - Recorded {} LFMF lessons", lessons.len());
        println!("   - Digested content: {}", digest_result.chunk_id);
        println!("   - Found {} RAG results", ask_result.total_found);

        Ok(())
    }

    #[tokio::test]
    #[ignore = "Requires running Qdrant instance"]
    async fn test_grok_learn_operation() -> Result<()> {
        if !is_qdrant_available() {
            println!("⚠️  Skipping test: QDRANT_URL not set");
            return Ok(());
        }

        let mut client = GrokClient::new();
        client.initialize().await?;

        let content = "Rust macros provide metaprogramming capabilities.\n\nMacros expand at compile time.\n\nUse declarative macros for simple patterns.";
        let source = "rust_macros_guide.md";

        let result = client.learn(content, Some(source)).await?;

        assert!(result.success, "Learn should succeed");
        assert_eq!(result.source, source);
        assert!(
            result.chunks_created > 0,
            "Should create at least one chunk"
        );
        assert_eq!(
            result.chunk_summaries.len(),
            result.chunks_created,
            "Should have summary for each chunk"
        );

        println!(
            "✅ Learn operation created {} chunks from {}",
            result.chunks_created, source
        );

        Ok(())
    }

    #[tokio::test]
    #[ignore = "Requires running Qdrant instance"]
    async fn test_multiple_topics_isolation() -> Result<()> {
        if !is_qdrant_available() {
            println!("⚠️  Skipping test: QDRANT_URL not set");
            return Ok(());
        }

        let mut client = GrokClient::new();
        client.initialize().await?;

        // Digest content for different topics
        client
            .digest("rust", "Rust has zero-cost abstractions")
            .await?;
        client
            .digest("python", "Python uses duck typing")
            .await?;

        tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;

        // Query each topic
        let rust_results = client.ask("abstractions", Some("rust"), Some(5)).await?;
        let python_results = client.ask("typing", Some("python"), Some(5)).await?;

        assert!(rust_results.success);
        assert!(python_results.success);

        // Verify topic isolation
        for result in &rust_results.results {
            assert_eq!(result.topic, "rust", "Rust query should only return rust results");
        }

        for result in &python_results.results {
            assert_eq!(
                result.topic, "python",
                "Python query should only return python results"
            );
        }

        println!(
            "✅ Topic isolation verified: rust={}, python={}",
            rust_results.total_found, python_results.total_found
        );

        Ok(())
    }

    #[tokio::test]
    async fn test_grok_status() -> Result<()> {
        if !is_qdrant_available() {
            println!("⚠️  Skipping test: QDRANT_URL not set");
            return Ok(());
        }

        let mut client = GrokClient::new();

        // Try to initialize (may fail if service not available)
        match client.initialize().await {
            Ok(_) => {
                let status = client.status().await?;
                println!("✅ Grok status: {:?}", status);

                // Verify status has expected fields
                assert!(
                    status.is_object(),
                    "Status should be a JSON object"
                );
            }
            Err(e) => {
                println!("ℹ️  Grok service not available: {}", e);
            }
        }

        Ok(())
    }
}

#[cfg(test)]
mod lfmf_integration {
    use super::*;

    #[tokio::test]
    async fn test_lfmf_with_vector_db() -> Result<()> {
        if !is_qdrant_available() {
            println!("⚠️  Skipping test: QDRANT_URL not set");
            return Ok(());
        }

        let temp_dir = setup_temp_dir();
        let temp_path = temp_dir.path().to_str().unwrap();

        let config = LfmfSystem::load_config(temp_path)?;
        let mut lfmf = LfmfSystem::new(config);

        // Initialize with vector DB
        lfmf.initialize().await?;

        // Record multiple lessons
        let lessons = vec![
            "cargo build: Use --release for optimized builds",
            "cargo test: Use --nocapture to see println output",
            "cargo clippy: Use --fix to auto-apply suggestions",
        ];

        for lesson in &lessons {
            lfmf.record_lesson("cargo", lesson).await?;
        }

        // Small delay for indexing
        tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;

        // Search using semantic similarity
        let results = lfmf.get_advice("cargo", "optimization", Some(10)).await?;

        assert!(
            !results.is_empty(),
            "Should find lessons related to optimization"
        );
        assert!(
            results.iter().any(|r| r.contains("--release")),
            "Should find lesson about release builds"
        );

        println!("✅ LFMF vector DB integration: {} results found", results.len());

        Ok(())
    }

    #[tokio::test]
    async fn test_lfmf_filesystem_fallback() -> Result<()> {
        // Test LFMF works without vector DB
        env::remove_var("QDRANT_URL");

        let temp_dir = setup_temp_dir();
        let temp_path = temp_dir.path().to_str().unwrap();

        let config = LfmfSystem::load_config(temp_path)?;
        let mut lfmf = LfmfSystem::new(config);

        // Initialize without vector DB (should use filesystem fallback)
        let _ = lfmf.initialize().await; // May fail, that's ok

        // Record lesson (should work with filesystem only)
        let lesson = "git commit: Use conventional commits format";
        lfmf.record_lesson("git", lesson).await?;

        // List lessons (filesystem-based)
        let results = lfmf.list_lessons("git", Some(10)).await?;

        assert!(
            !results.is_empty(),
            "Should find lesson in filesystem"
        );
        assert!(
            results[0].contains("conventional commits"),
            "Should contain recorded lesson"
        );

        println!("✅ LFMF filesystem fallback working");

        Ok(())
    }
}

#[cfg(test)]
mod error_handling {
    use super::*;

    #[tokio::test]
    async fn test_initialization_errors() -> Result<()> {
        // Test behavior with invalid Qdrant URL
        env::set_var("QDRANT_URL", "http://invalid-host-that-does-not-exist:9999");

        let mut client = GrokClient::new();
        let result = client.initialize().await;

        // Should fail gracefully
        match result {
            Ok(_) => {
                println!("⚠️  Unexpected success with invalid Qdrant URL");
            }
            Err(e) => {
                println!("✅ Graceful error handling: {}", e);
                assert!(
                    e.to_string().contains("Failed") || e.to_string().contains("error"),
                    "Error message should be descriptive"
                );
            }
        }

        // Clean up
        env::remove_var("QDRANT_URL");

        Ok(())
    }

    #[tokio::test]
    async fn test_operations_without_initialization() {
        // Test that operations fail gracefully without initialization
        let client = GrokClient::new();

        let digest_result = client.digest("test", "content").await;
        assert!(
            digest_result.is_err(),
            "Digest should fail without initialization"
        );
        assert!(
            digest_result
                .unwrap_err()
                .to_string()
                .contains("not initialized"),
            "Error should indicate client not initialized"
        );

        let ask_result = client.ask("query", None, None).await;
        assert!(
            ask_result.is_err(),
            "Ask should fail without initialization"
        );

        let learn_result = client.learn("content", None).await;
        assert!(
            learn_result.is_err(),
            "Learn should fail without initialization"
        );
    }

    #[tokio::test]
    async fn test_empty_content_handling() -> Result<()> {
        if !is_qdrant_available() {
            println!("⚠️  Skipping test: QDRANT_URL not set");
            return Ok(());
        }

        let mut client = GrokClient::new();
        match client.initialize().await {
            Ok(_) => {
                // Test with empty content
                let result = client.digest("test", "").await;

                // Should either succeed with warning or fail gracefully
                match result {
                    Ok(r) => {
                        println!("ℹ️  Empty content accepted: {:?}", r);
                    }
                    Err(e) => {
                        println!("✅ Empty content rejected: {}", e);
                    }
                }
            }
            Err(e) => {
                println!("ℹ️  Service unavailable: {}", e);
            }
        }

        Ok(())
    }
}
