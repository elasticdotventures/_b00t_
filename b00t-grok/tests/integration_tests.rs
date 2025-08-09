// 🤓 Integration tests for grok system end-to-end validation
use b00t_grok::{GrokClient, GrokError, Chunker, BasicChunker};
use std::env;
use uuid::Uuid;
use tokio::time::{sleep, Duration};

#[cfg(feature = "pyo3")]
use b00t_grok::SemanticChunker;

// Test data constants
const TEST_TOPIC: &str = "test_rust_integration";
const TEST_CONTENT: &str = "Rust is a systems programming language that focuses on safety, speed, and concurrency. It achieves memory safety without using a garbage collector.";
const TEST_QUERY: &str = "What is Rust?";

// Utility to get test environment
fn get_test_env() -> Option<(String, String)> {
    let qdrant_url = env::var("QDRANT_URL").ok()?;
    let api_key = env::var("QDRANT_API_KEY").unwrap_or_default();
    Some((qdrant_url, api_key))
}

// Skip integration tests if environment is not set up
macro_rules! skip_if_no_env {
    () => {
        if get_test_env().is_none() {
            eprintln!("⚠️ Skipping integration test - QDRANT_URL not set");
            return;
        }
    };
}

#[tokio::test]
async fn test_full_workflow_with_real_database() {
    skip_if_no_env!();
    
    let (qdrant_url, api_key) = get_test_env().unwrap();
    
    // Create client with unique collection for this test
    let unique_id = Uuid::new_v4().to_string()[..8].to_string();
    let mut client = GrokClient::new(qdrant_url, api_key);
    // Override collection name for isolation
    // client.collection_name = format!("test_b00t_chunks_{}", unique_id);
    
    // Test 1: Client initialization
    let result = client.initialize().await;
    match result {
        Ok(()) => println!("✅ Client initialized successfully"),
        Err(e) => {
            eprintln!("❌ Client initialization failed: {}", e);
            eprintln!("💡 User-friendly message: {}", e.user_message());
            eprintln!("🔍 Error category: {}", e.category());
            eprintln!("🔄 Is recoverable: {}", e.is_recoverable());
            
            if e.is_recoverable() {
                eprintln!("⏳ Retrying in 2 seconds...");
                sleep(Duration::from_secs(2)).await;
                client.initialize().await.expect("Retry failed");
            } else {
                panic!("Non-recoverable initialization error: {}", e);
            }
        }
    }
    
    // Test 2: Learning (chunking and embedding)
    println!("🧠 Testing learn functionality...");
    let learn_result = client.learn("rust_guide.md", TEST_CONTENT).await;
    
    match learn_result {
        Ok(chunks) => {
            println!("✅ Learned {} chunks", chunks.len());
            assert!(!chunks.is_empty(), "Should produce at least one chunk");
            
            // Verify chunk structure
            let first_chunk = &chunks[0];
            assert_eq!(first_chunk.datum, "rust");
            assert!(!first_chunk.content.is_empty());
            assert!(first_chunk.vector.is_some());
            assert!(!first_chunk.id.to_string().is_empty());
        }
        Err(e) => {
            eprintln!("❌ Learn failed: {}", e);
            panic!("Learn operation should succeed: {}", e.user_message());
        }
    }
    
    // Test 3: Direct digest
    println!("💾 Testing digest functionality...");
    let digest_result = client.digest(TEST_TOPIC, "Additional test content for digest").await;
    
    match digest_result {
        Ok(chunk) => {
            println!("✅ Digested chunk: {}", chunk.id);
            assert_eq!(chunk.datum, TEST_TOPIC);
            assert!(chunk.vector.is_some());
        }
        Err(e) => {
            eprintln!("❌ Digest failed: {}", e);
            panic!("Digest operation should succeed: {}", e.user_message());
        }
    }
    
    // Test 4: Querying
    println!("🔍 Testing ask functionality...");
    
    // Wait a moment for database consistency
    sleep(Duration::from_millis(500)).await;
    
    let ask_result = client.ask(TEST_QUERY, Some("rust")).await;
    
    match ask_result {
        Ok(results) => {
            println!("✅ Query returned {} results", results.len());
            
            if !results.is_empty() {
                let best_match = &results[0];
                println!("🎯 Best match: {} chars, topic: {}", 
                        best_match.content.len(), best_match.metadata.topic);
                
                // Verify result structure
                assert!(!best_match.content.is_empty());
                assert!(best_match.content.to_lowercase().contains("rust") || 
                       best_match.datum.to_lowercase().contains("rust"));
            } else {
                println!("⚠️ No results found, but operation succeeded");
            }
        }
        Err(e) => {
            eprintln!("❌ Ask failed: {}", e);
            panic!("Ask operation should succeed: {}", e.user_message());
        }
    }
    
    // Test 5: Query without topic filter
    let ask_all_result = client.ask("programming language", None).await;
    
    match ask_all_result {
        Ok(results) => {
            println!("✅ Unfiltered query returned {} results", results.len());
        }
        Err(e) => {
            eprintln!("❌ Unfiltered ask failed: {}", e);
            panic!("Unfiltered ask should succeed: {}", e.user_message());
        }
    }
    
    println!("🎉 Full workflow integration test completed successfully!");
}

#[tokio::test]
async fn test_error_handling_scenarios() {
    skip_if_no_env!();
    
    let (qdrant_url, api_key) = get_test_env().unwrap();
    let mut client = GrokClient::new(qdrant_url, api_key);
    client.initialize().await.expect("Client should initialize");
    
    println!("🚨 Testing error handling scenarios...");
    
    // Test 1: Empty content validation
    let empty_digest = client.digest("test", "").await;
    assert!(empty_digest.is_err());
    if let Err(e) = empty_digest {
        assert!(matches!(e, GrokError::InvalidQuery { .. }));
        println!("✅ Empty content properly rejected: {}", e.user_message());
    }
    
    // Test 2: Empty query validation
    let empty_query = client.ask("", Some("test")).await;
    assert!(empty_query.is_err());
    if let Err(e) = empty_query {
        assert!(matches!(e, GrokError::InvalidQuery { .. }));
        println!("✅ Empty query properly rejected: {}", e.user_message());
    }
    
    // Test 3: Very large content
    let large_content = "x".repeat(2_000_000); // 2MB, over the 1MB limit
    let large_digest = client.digest("test", &large_content).await;
    assert!(large_digest.is_err());
    if let Err(e) = large_digest {
        assert!(matches!(e, GrokError::ContentTooLarge { .. }));
        println!("✅ Large content properly rejected: {}", e.user_message());
    }
    
    // Test 4: Empty topic validation
    let empty_topic = client.digest("", "test content").await;
    assert!(empty_topic.is_err());
    if let Err(e) = empty_topic {
        assert!(matches!(e, GrokError::InvalidQuery { .. }));
        println!("✅ Empty topic properly rejected: {}", e.user_message());
    }
    
    // Test 5: Learn with empty source
    let empty_source = client.learn("", "test content").await;
    assert!(empty_source.is_err());
    if let Err(e) = empty_source {
        assert!(matches!(e, GrokError::InvalidQuery { .. }));
        println!("✅ Empty source properly rejected: {}", e.user_message());
    }
    
    println!("✅ All error handling scenarios passed!");
}

#[tokio::test]
async fn test_client_not_initialized() {
    let client = GrokClient::new("http://example.com".to_string(), "test".to_string());
    
    println!("🚨 Testing operations on uninitialized client...");
    
    // All operations should fail with ClientNotInitialized
    let digest_result = client.digest("test", "content").await;
    assert!(digest_result.is_err());
    assert!(matches!(digest_result.unwrap_err(), GrokError::ClientNotInitialized));
    
    let ask_result = client.ask("query", None).await;
    assert!(ask_result.is_err());
    assert!(matches!(ask_result.unwrap_err(), GrokError::ClientNotInitialized));
    
    let learn_result = client.learn("source", "content").await;
    assert!(learn_result.is_err());
    assert!(matches!(learn_result.unwrap_err(), GrokError::ClientNotInitialized));
    
    println!("✅ Uninitialized client properly rejects operations");
}

#[tokio::test]
async fn test_connection_error_handling() {
    println!("🚨 Testing connection error scenarios...");
    
    // Test with invalid URL
    let mut bad_client = GrokClient::new("http://192.168.255.255:9999".to_string(), "test".to_string());
    let result = bad_client.initialize().await;
    
    assert!(result.is_err());
    if let Err(e) = result {
        assert!(e.is_recoverable()); // Connection errors should be recoverable
        assert_eq!(e.category(), "database");
        println!("✅ Connection error properly categorized: {}", e.user_message());
    }
    
    // Test with missing environment variable
    let original_url = env::var("OLLAMA_API_URL").ok();
    unsafe {
        env::remove_var("OLLAMA_API_URL");
    }
    let embedding_result = b00t_grok::EmbeddingModel::new().await;
    assert!(embedding_result.is_err());
    
    if let Err(e) = embedding_result {
        assert!(matches!(e, GrokError::EnvironmentVariable { .. }));
        assert_eq!(e.category(), "configuration");
        assert!(!e.is_recoverable()); // Config errors are not recoverable
        println!("✅ Environment variable error properly handled: {}", e.user_message());
    }
    
    // Restore original environment variable
    if let Some(url) = original_url {
        unsafe {
            env::set_var("OLLAMA_API_URL", url);
        }
    }
    
    println!("✅ Connection error handling tests completed!");
}

#[tokio::test]
async fn test_chunking_strategies() {
    println!("📝 Testing chunking strategies...");
    
    let test_content = "First paragraph with important information.\n\nSecond paragraph with different content.\n\nThird paragraph for testing.";
    
    // Test basic chunker (always available)
    let basic_chunker = b00t_grok::BasicChunker;
    let basic_chunks = basic_chunker.chunk(test_content);
    
    match basic_chunks {
        Ok(chunks) => {
            println!("✅ Basic chunker produced {} chunks", chunks.len());
            assert!(!chunks.is_empty());
            
            // Verify chunks contain expected content
            let combined = chunks.join(" ");
            assert!(combined.contains("First paragraph"));
            assert!(combined.contains("Second paragraph"));
            assert!(combined.contains("Third paragraph"));
        }
        Err(e) => panic!("Basic chunker should never fail: {}", e),
    }
    
    // Test semantic chunker (if available)
    #[cfg(feature = "pyo3")]
    {
        let semantic_chunker = b00t_grok::SemanticChunker::new(500);
        let semantic_result = semantic_chunker.chunk(test_content);
        
        match semantic_result {
            Ok(chunks) => {
                println!("✅ Semantic chunker produced {} chunks", chunks.len());
                assert!(!chunks.is_empty());
            }
            Err(e) => {
                // Expected if chonkie is not installed
                println!("⚠️ Semantic chunker failed (expected if chonkie not installed): {}", e.user_message());
                assert!(matches!(e, GrokError::SemanticChunking { .. }));
            }
        }
    }
    
    #[cfg(not(feature = "pyo3"))]
    {
        println!("📝 Semantic chunker not available (PyO3 feature disabled)");
    }
    
    println!("✅ Chunking strategy tests completed!");
}

#[tokio::test] 
async fn test_performance_limits() {
    skip_if_no_env!();
    
    let (qdrant_url, api_key) = get_test_env().unwrap();
    let mut client = GrokClient::new(qdrant_url, api_key);
    client.initialize().await.expect("Client should initialize");
    
    println!("⚡ Testing performance and limits...");
    
    // Test batch learning with multiple chunks
    let large_content = (0..10)
        .map(|i| format!("This is paragraph number {} with unique content about topic {}. It contains several sentences to make it substantial enough for testing chunking and embedding performance.", i, i))
        .collect::<Vec<_>>()
        .join("\n\n");
    
    let start = std::time::Instant::now();
    let learn_result = client.learn("performance_test.md", &large_content).await;
    let duration = start.elapsed();
    
    match learn_result {
        Ok(chunks) => {
            println!("✅ Processed {} chunks in {:?}", chunks.len(), duration);
            assert!(chunks.len() >= 10, "Should produce multiple chunks");
            
            // Verify all chunks have valid structure
            for (i, chunk) in chunks.iter().enumerate() {
                assert!(!chunk.content.is_empty(), "Chunk {} should have content", i);
                assert!(chunk.vector.is_some(), "Chunk {} should have vector", i);
                assert!(!chunk.id.to_string().is_empty(), "Chunk {} should have valid UUID", i);
            }
        }
        Err(e) => panic!("Performance test should succeed: {}", e),
    }
    
    // Test query performance
    let query_start = std::time::Instant::now();
    let query_result = client.ask("performance topic", None).await;
    let query_duration = query_start.elapsed();
    
    match query_result {
        Ok(results) => {
            println!("✅ Query completed in {:?}, returned {} results", query_duration, results.len());
        }
        Err(e) => panic!("Query performance test should succeed: {}", e),
    }
    
    println!("✅ Performance tests completed!");
}

#[test]
fn test_error_categorization_and_recovery() {
    println!("🏷️ Testing error categorization...");
    
    // Test all error categories
    let errors = vec![
        (GrokError::ClientNotInitialized, "initialization", false),
        (GrokError::EnvironmentVariable { variable: "TEST".to_string() }, "configuration", false),
        (GrokError::InvalidQuery { message: "test".to_string() }, "validation", false),
        (GrokError::ContentTooLarge { size: 1000, limit: 500 }, "limits", false),
        (GrokError::Timeout { seconds: 30, operation: "test".to_string() }, "timeout", true),
    ];
    
    for (error, expected_category, expected_recoverable) in errors {
        assert_eq!(error.category(), expected_category);
        assert_eq!(error.is_recoverable(), expected_recoverable);
        
        let user_msg = error.user_message();
        assert!(!user_msg.is_empty());
        assert!(!user_msg.contains("Error"), "User messages should not contain 'Error'");
        
        println!("✅ {} error properly categorized: {}", expected_category, user_msg);
    }
    
    println!("✅ Error categorization tests completed!");
}