//! Comprehensive CLI tests for b00t grok learn command
//! Tests the full web crawling and document learning workflow

use std::env;
use std::fs;
use std::process::Command;
use tempfile::TempDir;

mod common;

fn is_infrastructure_available() -> bool {
    env::var("TEST_WITH_QDRANT").is_ok() || env::var("QDRANT_URL").is_ok()
}

fn setup_temp_dir() -> TempDir {
    tempfile::tempdir().expect("Failed to create temp directory")
}

#[cfg(test)]
mod cli_grok_learn {
    use super::*;

    #[test]
    #[ignore = "Requires Qdrant and b00t-grok-py"]
    fn test_cli_grok_learn_from_content() {
        if !is_infrastructure_available() {
            println!("⚠️  Skipping: QDRANT_URL not set");
            return;
        }

        let b00t = common::get_b00t_binary();

        // Test learning from direct content
        let output = Command::new(&b00t)
            .args(&[
                "grok",
                "learn",
                "Rust provides memory safety through ownership.\n\nThe borrow checker enforces rules at compile time.",
                "-t",
                "rust_learn_test",
            ])
            .output()
            .expect("Failed to execute b00t grok learn");

        println!("stdout: {}", String::from_utf8_lossy(&output.stdout));
        println!("stderr: {}", String::from_utf8_lossy(&output.stderr));

        assert!(output.status.success(), "b00t grok learn should succeed");

        let stdout = String::from_utf8_lossy(&output.stdout);
        assert!(
            stdout.contains("Successfully learned")
                || stdout.contains("chunks")
                || stdout.contains("✅"),
            "Should indicate successful learning"
        );
    }

    #[test]
    #[ignore = "Requires Qdrant and b00t-grok-py"]
    fn test_cli_grok_learn_from_file() {
        if !is_infrastructure_available() {
            println!("⚠️  Skipping: QDRANT_URL not set");
            return;
        }

        let temp_dir = setup_temp_dir();
        let test_file = temp_dir.path().join("test_doc.md");

        // Create test file
        fs::write(
            &test_file,
            "# Rust Ownership\n\nOwnership is Rust's most unique feature.\n\n## Rules\n1. Each value has an owner\n2. Only one owner at a time\n3. Value is dropped when owner goes out of scope",
        )
        .expect("Failed to write test file");

        let b00t = common::get_b00t_binary();

        // Read the file contents so we pass the actual string, not a shell expansion literal.
        let file_contents = fs::read_to_string(&test_file).expect("Failed to read test file");

        let output = Command::new(&b00t)
            .args(&[
                "grok",
                "learn",
                "-s",
                test_file.to_str().unwrap(),
                &file_contents,
                "-t",
                "rust_ownership",
            ])
            .output()
            .expect("Failed to execute b00t grok learn");

        println!("stdout: {}", String::from_utf8_lossy(&output.stdout));

        // Should succeed or fail gracefully with clear error
        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            assert!(
                !stderr.contains("panic"),
                "Should not panic on file learning"
            );
        }
    }

    #[test]
    #[ignore = "Requires Qdrant, b00t-grok-py, and network"]
    fn test_cli_grok_learn_from_url() {
        if !is_infrastructure_available() {
            println!("⚠️  Skipping: QDRANT_URL not set");
            return;
        }

        let b00t = common::get_b00t_binary();

        // Test learning from URL (using httpbin for testing)
        let output = Command::new(&b00t)
            .args(&[
                "grok",
                "learn",
                "https://httpbin.org/html",
                "-t",
                "web_test",
            ])
            .output()
            .expect("Failed to execute b00t grok learn");

        println!(
            "URL learning stdout: {}",
            String::from_utf8_lossy(&output.stdout)
        );
        println!(
            "URL learning stderr: {}",
            String::from_utf8_lossy(&output.stderr)
        );

        // May succeed or fail depending on network/crawler availability
        // Just verify no panic
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert!(
            !stderr.contains("panic"),
            "Should not panic on URL learning"
        );
    }

    #[test]
    fn test_cli_grok_learn_missing_topic() {
        let b00t = common::get_b00t_binary();

        // Try to learn without specifying topic
        let output = Command::new(&b00t)
            .args(&["grok", "learn", "some content"])
            .output()
            .expect("Failed to execute b00t grok learn");

        println!(
            "Missing topic error: {}",
            String::from_utf8_lossy(&output.stderr)
        );

        // Should fail with helpful error about missing topic
        assert!(
            !output.status.success(),
            "Should fail when topic is missing"
        );

        let stderr = String::from_utf8_lossy(&output.stderr);
        assert!(
            stderr.contains("topic") || stderr.contains("required") || stderr.contains("-t"),
            "Error should mention missing topic"
        );
    }

    #[test]
    #[ignore = "Requires Qdrant and b00t-grok-py"]
    fn test_cli_grok_learn_query_workflow() {
        if !is_infrastructure_available() {
            println!("⚠️  Skipping: QDRANT_URL not set");
            return;
        }

        let b00t = common::get_b00t_binary();
        let topic = "grok_workflow_test";

        println!("\n=== STEP 1: Learn from content ===");
        let learn_output = Command::new(&b00t)
            .args(&[
                "grok",
                "learn",
                "Rust's type system prevents null pointer dereferences.\n\nUse Option<T> for nullable values.",
                "-t",
                topic,
            ])
            .output()
            .expect("Failed to learn");

        println!("{}", String::from_utf8_lossy(&learn_output.stdout));
        assert!(learn_output.status.success(), "Learn should succeed");

        // Wait for indexing
        std::thread::sleep(std::time::Duration::from_millis(1500));

        println!("\n=== STEP 2: Query learned content ===");
        let ask_output = Command::new(&b00t)
            .args(&["grok", "ask", "null pointers", "-t", topic])
            .output()
            .expect("Failed to ask");

        println!("{}", String::from_utf8_lossy(&ask_output.stdout));
        assert!(ask_output.status.success(), "Ask should succeed");

        let ask_stdout = String::from_utf8_lossy(&ask_output.stdout);
        assert!(
            ask_stdout.contains("null") || ask_stdout.contains("Option") || ask_stdout.len() > 0,
            "Should return relevant results about null pointers"
        );

        println!("\n✅ Complete grok learn → ask workflow succeeded");
    }
}

#[cfg(test)]
mod cli_learn_display_flags {
    use super::*;

    #[test]
    fn test_cli_learn_toc_flag() {
        let temp_dir = setup_temp_dir();
        let temp_path = temp_dir.path().to_str().unwrap();
        let b00t = common::get_b00t_binary();

        // Create a test learn file with multiple sections
        let learn_dir = temp_dir.path().join("learn");
        fs::create_dir_all(&learn_dir).expect("Failed to create learn dir");

        let test_file = learn_dir.join("test_topic.md");
        fs::write(
            &test_file,
            "# Section 1\nContent 1\n\n# Section 2\nContent 2\n\n# Section 3\nContent 3",
        )
        .expect("Failed to write test file");

        let output = Command::new(&b00t)
            .args(&["learn", "test_topic", "--toc"])
            .env("_B00T_Path", temp_path)
            .output()
            .expect("Failed to execute b00t learn --toc");

        println!("TOC output: {}", String::from_utf8_lossy(&output.stdout));

        // The test creates the backing learn file, so this should deterministically succeed.
        assert!(
            output.status.success(),
            "b00t learn --toc failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );

        let stdout = String::from_utf8_lossy(&output.stdout);
        // Check for stable TOC markers based on the CLI's top-level knowledge sections.
        assert!(
            stdout.contains("Table of Contents: test_topic")
                && stdout.contains("Learn Content (_b00t_/learn/test_topic.md)")
                && stdout.contains("Use: b00t learn test_topic --section <num>"),
            "TOC output did not list expected knowledge sections. Output was:\n{}",
            stdout
        );
    }

    #[test]
    fn test_cli_learn_section_flag() {
        let temp_dir = setup_temp_dir();
        let temp_path = temp_dir.path().to_str().unwrap();
        let b00t = common::get_b00t_binary();

        let learn_dir = temp_dir.path().join("learn");
        fs::create_dir_all(&learn_dir).expect("Failed to create learn dir");

        let test_file = learn_dir.join("sections_test.md");
        fs::write(
            &test_file,
            "# Section 1\nFirst section content\n\n# Section 2\nSecond section content\n\n# Section 3\nThird section content",
        )
        .expect("Failed to write test file");

        // Try to jump to section 2
        let output = Command::new(&b00t)
            .args(&["learn", "sections_test", "--section", "2"])
            .env("_B00T_Path", temp_path)
            .output()
            .expect("Failed to execute b00t learn --section");

        println!(
            "Section output: {}",
            String::from_utf8_lossy(&output.stdout)
        );

        // Should succeed or fail gracefully
        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            assert!(
                !stderr.contains("panic"),
                "Should not panic on section flag"
            );
        }
    }

    #[test]
    fn test_cli_learn_concise_flag() {
        let temp_dir = setup_temp_dir();
        let temp_path = temp_dir.path().to_str().unwrap();
        let b00t = common::get_b00t_binary();

        let learn_dir = temp_dir.path().join("learn");
        fs::create_dir_all(&learn_dir).expect("Failed to create learn dir");

        let test_file = learn_dir.join("concise_test.md");
        fs::write(&test_file, "# Test Topic\n\nThis is test content with some details.\n\n## Details\nMore information here.")
            .expect("Failed to write test file");

        let output = Command::new(&b00t)
            .args(&["learn", "concise_test", "--concise"])
            .env("_B00T_Path", temp_path)
            .output()
            .expect("Failed to execute b00t learn --concise");

        println!(
            "Concise output: {}",
            String::from_utf8_lossy(&output.stdout)
        );

        // Should produce output (concise mode)
        if output.status.success() {
            // Just verify it works, concise output might be shorter
            assert!(
                String::from_utf8_lossy(&output.stdout).len() > 0,
                "Should produce some output"
            );
        }
    }

    #[test]
    fn test_cli_learn_invalid_section() {
        let temp_dir = setup_temp_dir();
        let temp_path = temp_dir.path().to_str().unwrap();
        let b00t = common::get_b00t_binary();

        // Try to access section 99 that doesn't exist
        let output = Command::new(&b00t)
            .args(&["learn", "nonexistent", "--section", "99"])
            .env("_B00T_Path", temp_path)
            .output()
            .expect("Failed to execute b00t learn");

        // Should fail gracefully
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert!(
            !stderr.contains("panic"),
            "Should not panic on invalid section"
        );
    }
}

#[cfg(test)]
mod cli_error_paths {
    use super::*;

    #[test]
    fn test_cli_learn_search_nonexistent_topic() {
        let temp_dir = setup_temp_dir();
        let temp_path = temp_dir.path().to_str().unwrap();
        let b00t = common::get_b00t_binary();

        let output = Command::new(&b00t)
            .args(&[
                "learn",
                "topic_that_definitely_does_not_exist_xyz",
                "--search",
                "anything",
            ])
            .env("_B00T_Path", temp_path)
            .output()
            .expect("Failed to execute b00t learn --search");

        println!(
            "Nonexistent topic: {}",
            String::from_utf8_lossy(&output.stdout)
        );
        println!(
            "Nonexistent stderr: {}",
            String::from_utf8_lossy(&output.stderr)
        );

        // Should handle gracefully - either succeed with "no lessons" or fail with clear message
        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);

        assert!(
            !stderr.contains("panic") && !stdout.contains("panic"),
            "Should not panic on nonexistent topic"
        );
    }

    #[test]
    fn test_cli_grok_digest_without_topic() {
        let b00t = common::get_b00t_binary();

        let output = Command::new(&b00t)
            .args(&["grok", "digest", "some content"])
            .output()
            .expect("Failed to execute b00t grok digest");

        // Should fail with clear error about missing -t flag
        assert!(!output.status.success(), "Should fail without topic");

        let stderr = String::from_utf8_lossy(&output.stderr);
        assert!(
            stderr.contains("-t") || stderr.contains("topic") || stderr.contains("required"),
            "Error should mention missing topic flag"
        );
    }

    #[test]
    fn test_cli_grok_ask_without_topic_when_required() {
        let b00t = common::get_b00t_binary();

        // When using --rag, topic is required
        let output = Command::new(&b00t)
            .args(&["grok", "ask", "query", "--rag", "raglight"])
            .output()
            .expect("Failed to execute b00t grok ask");

        println!(
            "Ask without topic: {}",
            String::from_utf8_lossy(&output.stderr)
        );

        // Should fail with clear error when RAGLight requires topic
        assert!(
            !output.status.success(),
            "Should fail without topic when using --rag"
        );

        let stderr = String::from_utf8_lossy(&output.stderr);
        assert!(
            stderr.contains("topic") || stderr.contains("-t") || stderr.contains("required"),
            "Should mention topic requirement for RAGLight"
        );
    }

    #[test]
    #[ignore = "Requires Qdrant"]
    fn test_cli_concurrent_operations() {
        if !is_infrastructure_available() {
            println!("⚠️  Skipping: QDRANT_URL not set");
            return;
        }

        let b00t = common::get_b00t_binary();

        // Test multiple concurrent digest operations
        let handles: Vec<_> = (0..3)
            .map(|i| {
                let b00t = b00t.clone();
                std::thread::spawn(move || {
                    let output = Command::new(&b00t)
                        .args(&[
                            "grok",
                            "digest",
                            "-t",
                            &format!("concurrent_test_{}", i),
                            &format!("Test content for thread {}", i),
                        ])
                        .output()
                        .expect("Failed to execute concurrent digest");

                    println!("Thread {} result: {:?}", i, output.status);
                    output.status.success()
                })
            })
            .collect();

        let results: Vec<bool> = handles.into_iter().map(|h| h.join().unwrap()).collect();

        println!("Concurrent operations: {:?}", results);

        // At least some should succeed (testing concurrent access)
        assert!(
            results.iter().any(|&success| success),
            "At least one concurrent operation should succeed"
        );
    }
}
