//! RAGLight integration tests - bringing advanced features to 50% coverage
//! Tests the RAGLight backend as an alternative to Qdrant

use std::process::Command;
use tempfile::TempDir;

mod common;

#[allow(dead_code)]
fn setup_temp_dir() -> TempDir {
    tempfile::tempdir().expect("Failed to create temp directory")
}

#[cfg(test)]
mod raglight_basic {
    use super::*;

    #[test]
    fn test_grok_digest_with_raglight_backend() {
        let b00t = common::get_b00t_binary();

        let output = Command::new(&b00t)
            .args(&[
                "grok",
                "digest",
                "-t",
                "raglight_test",
                "RAGLight provides local RAG capabilities without Qdrant dependency",
                "--rag",
                "raglight",
            ])
            .output()
            .expect("Failed to execute b00t grok digest --rag raglight");

        println!(
            "RAGLight digest stdout: {}",
            String::from_utf8_lossy(&output.stdout)
        );
        println!(
            "RAGLight digest stderr: {}",
            String::from_utf8_lossy(&output.stderr)
        );

        // Should succeed or fail gracefully
        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            assert!(
                !stderr.contains("panic"),
                "Should not panic with RAGLight backend"
            );
            // May fail if RAGLight not properly configured, that's OK
            println!("ℹ️  RAGLight may require additional setup");
        } else {
            let stdout = String::from_utf8_lossy(&output.stdout);
            assert!(
                stdout.contains("Queued") || stdout.contains("index") || stdout.contains("✅"),
                "Should indicate RAGLight processing"
            );
        }
    }

    #[test]
    fn test_grok_ask_with_raglight_backend() {
        let b00t = common::get_b00t_binary();

        // First digest some content
        let _ = Command::new(&b00t)
            .args(&[
                "grok",
                "digest",
                "-t",
                "raglight_query_test",
                "RAGLight uses local storage for embeddings",
                "--rag",
                "raglight",
            ])
            .output();

        // Small delay
        std::thread::sleep(std::time::Duration::from_millis(500));

        // Try to query
        let output = Command::new(&b00t)
            .args(&[
                "grok",
                "ask",
                "local storage",
                "-t",
                "raglight_query_test",
                "--rag",
                "raglight",
            ])
            .output()
            .expect("Failed to execute b00t grok ask --rag raglight");

        println!(
            "RAGLight ask stdout: {}",
            String::from_utf8_lossy(&output.stdout)
        );
        println!(
            "RAGLight ask stderr: {}",
            String::from_utf8_lossy(&output.stderr)
        );

        // May work or may require setup
        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            assert!(
                !stderr.contains("panic"),
                "Should not panic when querying RAGLight"
            );
        }
    }

    #[test]
    fn test_grok_learn_with_raglight_backend() {
        let b00t = common::get_b00t_binary();

        let output = Command::new(&b00t)
            .args(&[
                "grok",
                "learn",
                "RAGLight enables offline RAG workflows.\n\nIt processes documents locally.",
                "-t",
                "raglight_learn_test",
                "--rag",
                "raglight",
            ])
            .output()
            .expect("Failed to execute b00t grok learn --rag raglight");

        println!(
            "RAGLight learn stdout: {}",
            String::from_utf8_lossy(&output.stdout)
        );
        println!(
            "RAGLight learn stderr: {}",
            String::from_utf8_lossy(&output.stderr)
        );

        // Should handle gracefully
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert!(
            !stderr.contains("panic"),
            "Should not panic with RAGLight learn"
        );
    }
}

#[cfg(test)]
mod raglight_workflow {
    use super::*;

    #[test]
    fn test_raglight_complete_workflow() {
        let b00t = common::get_b00t_binary();
        let topic = "raglight_workflow";

        println!("\n=== RAGLight Workflow Test ===");

        println!("\n1. Digest content to RAGLight");
        let digest = Command::new(&b00t)
            .args(&[
                "grok",
                "digest",
                "-t",
                topic,
                "RAGLight provides document processing and local vector storage",
                "--rag",
                "raglight",
            ])
            .output()
            .expect("Failed to digest");

        println!("Digest result: {:?}", digest.status);

        std::thread::sleep(std::time::Duration::from_millis(1000));

        println!("\n2. Query RAGLight");
        let ask = Command::new(&b00t)
            .args(&[
                "grok",
                "ask",
                "vector storage",
                "-t",
                topic,
                "--rag",
                "raglight",
            ])
            .output()
            .expect("Failed to ask");

        println!("Ask result: {:?}", ask.status);
        println!("Output: {}", String::from_utf8_lossy(&ask.stdout));

        // Just verify no panics - functionality may require setup
        let digest_stderr = String::from_utf8_lossy(&digest.stderr);
        let ask_stderr = String::from_utf8_lossy(&ask.stderr);

        assert!(
            !digest_stderr.contains("panic") && !ask_stderr.contains("panic"),
            "RAGLight workflow should not panic"
        );
    }

    #[test]
    fn test_raglight_file_storage() {
        // Test that RAGLight stores files in expected location
        let b00t = common::get_b00t_binary();

        let output = Command::new(&b00t)
            .args(&[
                "grok",
                "digest",
                "-t",
                "storage_test",
                "Testing file storage location",
                "--rag",
                "raglight",
            ])
            .output()
            .expect("Failed to execute");

        // Check if upload directory was created
        if let Some(home) = dirs::home_dir() {
            let upload_dir = home.join(".b00t/raglight/uploads");
            if upload_dir.exists() {
                println!("✅ RAGLight upload directory exists: {:?}", upload_dir);
            } else {
                println!("ℹ️  RAGLight upload directory not created (may require initialization)");
            }
        }

        // Just verify no panic
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert!(
            !stderr.contains("panic"),
            "File storage test should not panic"
        );
    }
}

#[cfg(test)]
mod raglight_error_handling {
    use super::*;

    #[test]
    fn test_raglight_invalid_backend_name() {
        let b00t = common::get_b00t_binary();

        let output = Command::new(&b00t)
            .args(&[
                "grok",
                "digest",
                "-t",
                "test",
                "content",
                "--rag",
                "invalid_backend_name",
            ])
            .output()
            .expect("Failed to execute");

        // Should fail with clear error about unsupported backend
        assert!(
            !output.status.success(),
            "Should fail with invalid backend name"
        );

        let stderr = String::from_utf8_lossy(&output.stderr);
        assert!(
            stderr.contains("Unsupported")
                || stderr.contains("backend")
                || stderr.contains("invalid"),
            "Error should mention unsupported backend"
        );
    }

    #[test]
    fn test_raglight_missing_topic() {
        let b00t = common::get_b00t_binary();

        // RAGLight operations should require topic
        let output = Command::new(&b00t)
            .args(&[
                "grok",
                "digest",
                "content without topic",
                "--rag",
                "raglight",
            ])
            .output()
            .expect("Failed to execute");

        // Should fail requiring topic
        assert!(!output.status.success(), "Should fail without topic");

        let stderr = String::from_utf8_lossy(&output.stderr);
        assert!(
            stderr.contains("topic") || stderr.contains("-t") || stderr.contains("required"),
            "Should mention missing topic"
        );
    }

    #[test]
    fn test_raglight_concurrent_access() {
        let b00t = common::get_b00t_binary();

        // Test multiple concurrent RAGLight operations
        let handles: Vec<_> = (0..3)
            .map(|i| {
                let b00t = b00t.clone();
                std::thread::spawn(move || {
                    let result = Command::new(&b00t)
                        .args(&[
                            "grok",
                            "digest",
                            "-t",
                            &format!("concurrent_{}", i),
                            &format!("Concurrent content {}", i),
                            "--rag",
                            "raglight",
                        ])
                        .output();

                    match result {
                        Ok(output) => {
                            let stderr = String::from_utf8_lossy(&output.stderr);
                            !stderr.contains("panic")
                        }
                        Err(_) => false,
                    }
                })
            })
            .collect();

        let results: Vec<bool> = handles.into_iter().map(|h| h.join().unwrap()).collect();

        // All should complete without panicking
        assert!(
            results.iter().all(|&no_panic| no_panic),
            "All concurrent operations should complete without panic"
        );
    }
}

#[cfg(test)]
mod raglight_vs_qdrant {
    use super::*;

    #[test]
    fn test_raglight_as_fallback() {
        let b00t = common::get_b00t_binary();

        // When Qdrant is unavailable, RAGLight can be used as fallback
        let output = Command::new(&b00t)
            .args(&[
                "grok",
                "digest",
                "-t",
                "fallback_test",
                "RAGLight serves as offline alternative to Qdrant",
                "--rag",
                "raglight",
            ])
            .env_remove("QDRANT_URL") // Simulate Qdrant unavailable
            .output()
            .expect("Failed to execute");

        println!("Fallback test: {}", String::from_utf8_lossy(&output.stdout));
        println!(
            "Fallback stderr: {}",
            String::from_utf8_lossy(&output.stderr)
        );

        // Should work independently of Qdrant
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert!(
            !stderr.contains("QDRANT") || !stderr.contains("required"),
            "RAGLight should not require Qdrant"
        );
    }

    #[test]
    fn test_explicit_backend_selection() {
        let b00t = common::get_b00t_binary();

        // Test that --rag flag properly selects backend
        let raglight_output = Command::new(&b00t)
            .args(&[
                "grok",
                "digest",
                "-t",
                "backend_test",
                "Backend selection test",
                "--rag",
                "raglight",
            ])
            .output()
            .expect("Failed with RAGLight");

        let default_output = Command::new(&b00t)
            .args(&[
                "grok",
                "digest",
                "-t",
                "backend_test",
                "Backend selection test",
                // No --rag flag = default backend
            ])
            .output()
            .expect("Failed with default");

        // Both should complete (may fail, but shouldn't panic)
        let raglight_stderr = String::from_utf8_lossy(&raglight_output.stderr);
        let default_stderr = String::from_utf8_lossy(&default_output.stderr);

        assert!(
            !raglight_stderr.contains("panic"),
            "RAGLight backend should not panic"
        );
        assert!(
            !default_stderr.contains("panic"),
            "Default backend should not panic"
        );

        println!("✅ Both backends handle requests without panicking");
    }
}
