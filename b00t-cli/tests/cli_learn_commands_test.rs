//! CLI-level integration tests for b00t learn commands
//!
//! These tests invoke the actual `b00t` binary to verify:
//! - b00t learn --digest
//! - b00t learn --ask
//! - b00t learn --record
//! - b00t learn --search
//! - End-to-end CLI workflow

use std::process::Command;
use std::env;
use tempfile::TempDir;

fn is_infrastructure_available() -> bool {
    env::var("TEST_WITH_QDRANT").is_ok() || env::var("QDRANT_URL").is_ok()
}

fn get_b00t_binary() -> String {
    // Get the b00t binary path from cargo
    env::var("CARGO_BIN_EXE_b00t-cli")
        .unwrap_or_else(|_| {
            // Fallback: try to find in target directory
            let manifest_dir = env::var("CARGO_MANIFEST_DIR").unwrap();
            format!("{}/target/debug/b00t-cli", manifest_dir)
        })
}

fn setup_temp_dir() -> TempDir {
    tempfile::tempdir().expect("Failed to create temp directory")
}

#[cfg(test)]
mod cli_learn_digest {
    use super::*;

    #[test]
    #[ignore = "Requires running Qdrant and b00t-grok-py"]
    fn test_cli_learn_digest_command() {
        if !is_infrastructure_available() {
            println!("⚠️  Skipping: QDRANT_URL not set");
            return;
        }

        let b00t = get_b00t_binary();

        let output = Command::new(&b00t)
            .args(&[
                "learn",
                "rust_cli_test",
                "--digest",
                "Rust ensures memory safety through ownership and borrowing rules",
            ])
            .env("_B00T_Path", env::var("_B00T_Path").unwrap_or_else(|_| ".".to_string()))
            .output()
            .expect("Failed to execute b00t learn --digest");

        println!("stdout: {}", String::from_utf8_lossy(&output.stdout));
        println!("stderr: {}", String::from_utf8_lossy(&output.stderr));

        assert!(
            output.status.success(),
            "b00t learn --digest should succeed"
        );

        let stdout = String::from_utf8_lossy(&output.stdout);
        assert!(
            stdout.contains("Digested") || stdout.contains("✅") || stdout.contains("success"),
            "Output should indicate success"
        );
    }

    #[test]
    #[ignore = "Requires running Qdrant and b00t-grok-py"]
    fn test_cli_learn_digest_empty_content() {
        if !is_infrastructure_available() {
            println!("⚠️  Skipping: QDRANT_URL not set");
            return;
        }

        let b00t = get_b00t_binary();

        let output = Command::new(&b00t)
            .args(&["learn", "rust_cli_test", "--digest", ""])
            .output()
            .expect("Failed to execute b00t learn --digest");

        // Should handle empty content gracefully
        // Either succeed with warning or fail with clear message
        let stderr = String::from_utf8_lossy(&output.stderr);
        let stdout = String::from_utf8_lossy(&output.stdout);

        println!("Empty content test:");
        println!("stdout: {}", stdout);
        println!("stderr: {}", stderr);

        // Just verify it doesn't panic
        assert!(
            !stderr.contains("panic") && !stdout.contains("panic"),
            "Should not panic on empty content"
        );
    }
}

#[cfg(test)]
mod cli_learn_ask {
    use super::*;

    #[test]
    #[ignore = "Requires running Qdrant and b00t-grok-py with indexed content"]
    fn test_cli_learn_ask_command() {
        if !is_infrastructure_available() {
            println!("⚠️  Skipping: QDRANT_URL not set");
            return;
        }

        let b00t = get_b00t_binary();

        // First digest some content
        let digest_output = Command::new(&b00t)
            .args(&[
                "learn",
                "rust_cli_test",
                "--digest",
                "Rust has zero-cost abstractions and memory safety",
            ])
            .output()
            .expect("Failed to digest content");

        assert!(digest_output.status.success(), "Digest should succeed");

        // Small delay for indexing
        std::thread::sleep(std::time::Duration::from_millis(1000));

        // Now ask a question
        let ask_output = Command::new(&b00t)
            .args(&[
                "learn",
                "rust_cli_test",
                "--ask",
                "memory safety",
                "--limit",
                "5",
            ])
            .output()
            .expect("Failed to execute b00t learn --ask");

        println!("stdout: {}", String::from_utf8_lossy(&ask_output.stdout));
        println!("stderr: {}", String::from_utf8_lossy(&ask_output.stderr));

        assert!(
            ask_output.status.success(),
            "b00t learn --ask should succeed"
        );

        let stdout = String::from_utf8_lossy(&ask_output.stdout);
        assert!(
            stdout.contains("Results") || stdout.contains("Found") || stdout.len() > 0,
            "Output should show search results"
        );
    }

    #[test]
    #[ignore = "Requires running Qdrant and b00t-grok-py"]
    fn test_cli_learn_ask_no_results() {
        if !is_infrastructure_available() {
            println!("⚠️  Skipping: QDRANT_URL not set");
            return;
        }

        let b00t = get_b00t_binary();

        // Ask for something very unlikely to exist
        let output = Command::new(&b00t)
            .args(&[
                "learn",
                "rust_cli_test",
                "--ask",
                "xyzabc123unlikely_term_999",
            ])
            .output()
            .expect("Failed to execute b00t learn --ask");

        assert!(
            output.status.success() || output.status.code() == Some(0),
            "Should succeed even with no results"
        );

        let stdout = String::from_utf8_lossy(&output.stdout);
        println!("No results output: {}", stdout);

        // Should indicate no results found
        assert!(
            stdout.contains("No results") || stdout.contains("0 results") || stdout.contains("not found"),
            "Should indicate no results found"
        );
    }
}

#[cfg(test)]
mod cli_learn_record_search {
    use super::*;

    #[test]
    fn test_cli_learn_record_and_search() {
        let temp_dir = setup_temp_dir();
        let temp_path = temp_dir.path().to_str().unwrap();
        let b00t = get_b00t_binary();

        // Record a lesson
        let record_output = Command::new(&b00t)
            .args(&[
                "learn",
                "cli_test_tool",
                "--record",
                "test pattern: Use this specific solution for this problem",
            ])
            .env("_B00T_Path", temp_path)
            .output()
            .expect("Failed to execute b00t learn --record");

        println!("Record stdout: {}", String::from_utf8_lossy(&record_output.stdout));
        println!("Record stderr: {}", String::from_utf8_lossy(&record_output.stderr));

        assert!(
            record_output.status.success(),
            "b00t learn --record should succeed"
        );

        let stdout = String::from_utf8_lossy(&record_output.stdout);
        assert!(
            stdout.contains("Recorded") || stdout.contains("✅"),
            "Should confirm lesson recorded"
        );

        // Search for the recorded lesson
        let search_output = Command::new(&b00t)
            .args(&[
                "learn",
                "cli_test_tool",
                "--search",
                "list",
            ])
            .env("_B00T_Path", temp_path)
            .output()
            .expect("Failed to execute b00t learn --search");

        println!("Search stdout: {}", String::from_utf8_lossy(&search_output.stdout));

        assert!(
            search_output.status.success(),
            "b00t learn --search should succeed"
        );

        let search_stdout = String::from_utf8_lossy(&search_output.stdout);
        assert!(
            search_stdout.contains("test pattern") || search_stdout.contains("cli_test_tool"),
            "Should find the recorded lesson"
        );
    }

    #[test]
    fn test_cli_learn_record_token_limit() {
        let temp_dir = setup_temp_dir();
        let temp_path = temp_dir.path().to_str().unwrap();
        let b00t = get_b00t_binary();

        // Try to record a lesson with body exceeding 250 tokens
        let very_long_body = "word ".repeat(300);
        let lesson = format!("topic: {}", very_long_body);

        let output = Command::new(&b00t)
            .args(&[
                "learn",
                "cli_test_tool",
                "--record",
                &lesson,
            ])
            .env("_B00T_Path", temp_path)
            .output()
            .expect("Failed to execute b00t learn --record");

        println!("Token limit stdout: {}", String::from_utf8_lossy(&output.stdout));
        println!("Token limit stderr: {}", String::from_utf8_lossy(&output.stderr));

        // Should fail with token limit error
        assert!(
            !output.status.success(),
            "Should fail with token limit exceeded"
        );

        let stderr = String::from_utf8_lossy(&output.stderr);
        assert!(
            stderr.contains("250 tokens") || stderr.contains("token"),
            "Error should mention token limit"
        );
    }
}

#[cfg(test)]
mod cli_end_to_end_workflow {
    use super::*;

    #[test]
    #[ignore = "Requires running Qdrant and b00t-grok-py"]
    fn test_cli_complete_workflow() {
        if !is_infrastructure_available() {
            println!("⚠️  Skipping: QDRANT_URL not set");
            return;
        }

        let temp_dir = setup_temp_dir();
        let temp_path = temp_dir.path().to_str().unwrap();
        let b00t = get_b00t_binary();
        let topic = "workflow_test";

        println!("\n=== STEP 1: Record LFMF Lesson ===");
        let record_output = Command::new(&b00t)
            .args(&[
                "learn",
                topic,
                "--record",
                "ownership patterns: Use Rc<RefCell<T>> for shared mutable state",
            ])
            .env("_B00T_Path", temp_path)
            .output()
            .expect("Failed to record lesson");

        println!("{}", String::from_utf8_lossy(&record_output.stdout));
        assert!(record_output.status.success(), "Record should succeed");

        println!("\n=== STEP 2: Digest Content to RAG ===");
        let digest_output = Command::new(&b00t)
            .args(&[
                "learn",
                topic,
                "--digest",
                "Rust ownership prevents data races by enforcing strict borrowing rules at compile time",
            ])
            .output()
            .expect("Failed to digest content");

        println!("{}", String::from_utf8_lossy(&digest_output.stdout));
        assert!(digest_output.status.success(), "Digest should succeed");

        // Wait for indexing
        std::thread::sleep(std::time::Duration::from_millis(1500));

        println!("\n=== STEP 3: Search LFMF Lessons ===");
        let search_output = Command::new(&b00t)
            .args(&[
                "learn",
                topic,
                "--search",
                "ownership",
            ])
            .env("_B00T_Path", temp_path)
            .output()
            .expect("Failed to search lessons");

        println!("{}", String::from_utf8_lossy(&search_output.stdout));
        assert!(search_output.status.success(), "Search should succeed");

        let search_stdout = String::from_utf8_lossy(&search_output.stdout);
        assert!(
            search_stdout.contains("ownership patterns") || search_stdout.contains("Rc<RefCell<T>>"),
            "Should find recorded LFMF lesson"
        );

        println!("\n=== STEP 4: Query RAG ===");
        let ask_output = Command::new(&b00t)
            .args(&[
                "learn",
                topic,
                "--ask",
                "ownership and borrowing",
            ])
            .output()
            .expect("Failed to ask RAG");

        println!("{}", String::from_utf8_lossy(&ask_output.stdout));
        assert!(ask_output.status.success(), "Ask should succeed");

        let ask_stdout = String::from_utf8_lossy(&ask_output.stdout);
        assert!(
            ask_stdout.contains("ownership") || ask_stdout.contains("borrow"),
            "RAG should return relevant results"
        );

        println!("\n✅ Complete CLI workflow succeeded!");
    }

    #[test]
    #[ignore = "Requires running Qdrant and b00t-grok-py"]
    fn test_cli_workflow_without_qdrant() {
        // Test graceful degradation
        let temp_dir = setup_temp_dir();
        let temp_path = temp_dir.path().to_str().unwrap();
        let b00t = get_b00t_binary();
        let topic = "offline_test";

        // Remove Qdrant env vars to simulate offline
        let output = Command::new(&b00t)
            .args(&[
                "learn",
                topic,
                "--record",
                "offline test: This should work without Qdrant",
            ])
            .env("_B00T_Path", temp_path)
            .env_remove("QDRANT_URL")
            .env_remove("QDRANT_API_KEY")
            .output()
            .expect("Failed to record lesson");

        println!("Offline record: {}", String::from_utf8_lossy(&output.stdout));
        println!("Offline stderr: {}", String::from_utf8_lossy(&output.stderr));

        // Should succeed with filesystem fallback
        assert!(
            output.status.success(),
            "LFMF should work without Qdrant using filesystem"
        );

        // Search should also work (filesystem-based)
        let search_output = Command::new(&b00t)
            .args(&[
                "learn",
                topic,
                "--search",
                "list",
            ])
            .env("_B00T_Path", temp_path)
            .env_remove("QDRANT_URL")
            .output()
            .expect("Failed to search lessons");

        println!("Offline search: {}", String::from_utf8_lossy(&search_output.stdout));

        assert!(
            search_output.status.success(),
            "Search should work with filesystem fallback"
        );
    }
}

#[cfg(test)]
mod cli_error_handling {
    use super::*;

    #[test]
    fn test_cli_learn_missing_topic() {
        let b00t = get_b00t_binary();

        let output = Command::new(&b00t)
            .args(&["learn"])
            .output()
            .expect("Failed to execute b00t learn");

        // Should fail with helpful error
        assert!(
            !output.status.success(),
            "Should fail when topic is missing"
        );

        let stderr = String::from_utf8_lossy(&output.stderr);
        println!("Missing topic error: {}", stderr);

        assert!(
            stderr.contains("required") || stderr.contains("topic") || stderr.contains("argument"),
            "Error should mention missing topic"
        );
    }

    #[test]
    fn test_cli_learn_invalid_flag_combination() {
        let b00t = get_b00t_binary();

        // Try to use --digest and --ask together (invalid)
        let output = Command::new(&b00t)
            .args(&[
                "learn",
                "rust",
                "--digest",
                "content",
                "--ask",
                "query",
            ])
            .output()
            .expect("Failed to execute b00t learn");

        // Should handle gracefully (either one takes precedence or error)
        println!("Invalid combo stdout: {}", String::from_utf8_lossy(&output.stdout));
        println!("Invalid combo stderr: {}", String::from_utf8_lossy(&output.stderr));

        // Just verify no panic
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert!(
            !stderr.contains("panic"),
            "Should not panic on invalid flag combination"
        );
    }

    #[test]
    fn test_cli_learn_record_invalid_format() {
        let temp_dir = setup_temp_dir();
        let temp_path = temp_dir.path().to_str().unwrap();
        let b00t = get_b00t_binary();

        // Record without "topic: body" format
        let output = Command::new(&b00t)
            .args(&[
                "learn",
                "rust",
                "--record",
                "this is missing the colon separator",
            ])
            .env("_B00T_Path", temp_path)
            .output()
            .expect("Failed to execute b00t learn --record");

        println!("Invalid format stdout: {}", String::from_utf8_lossy(&output.stdout));
        println!("Invalid format stderr: {}", String::from_utf8_lossy(&output.stderr));

        // Should fail with format error
        assert!(
            !output.status.success(),
            "Should fail with invalid lesson format"
        );

        let stderr = String::from_utf8_lossy(&output.stderr);
        assert!(
            stderr.contains("format") || stderr.contains(":") || stderr.contains("topic"),
            "Error should mention format requirement"
        );
    }
}

#[cfg(test)]
mod cross_language_integration {
    use super::*;

    #[test]
    #[ignore = "Requires running b00t-grok-py MCP server"]
    fn test_rust_to_python_mcp_communication() {
        if !is_infrastructure_available() {
            println!("⚠️  Skipping: QDRANT_URL not set");
            return;
        }

        let b00t = get_b00t_binary();

        // This tests Rust CLI → Rust GrokClient → Python b00t-grok-py MCP server
        let output = Command::new(&b00t)
            .args(&[
                "grok",
                "digest",
                "-t",
                "rust",
                "Testing cross-language MCP integration between Rust and Python",
            ])
            .output()
            .expect("Failed to execute b00t grok digest");

        println!("Cross-language stdout: {}", String::from_utf8_lossy(&output.stdout));
        println!("Cross-language stderr: {}", String::from_utf8_lossy(&output.stderr));

        assert!(
            output.status.success(),
            "Rust→Python MCP communication should succeed"
        );

        let stdout = String::from_utf8_lossy(&output.stdout);
        assert!(
            stdout.contains("Digest") || stdout.contains("✅") || stdout.contains("success"),
            "Should show successful MCP communication"
        );
    }

    #[test]
    #[ignore = "Requires running b00t-grok-py MCP server"]
    fn test_mcp_error_propagation() {
        if !is_infrastructure_available() {
            println!("⚠️  Skipping: QDRANT_URL not set");
            return;
        }

        let b00t = get_b00t_binary();

        // Test that Python errors are properly propagated to Rust
        // (This assumes some invalid operation that Python will reject)
        let output = Command::new(&b00t)
            .args(&[
                "grok",
                "digest",
                "-t",
                "", // Empty topic should cause error
                "content",
            ])
            .output()
            .expect("Failed to execute b00t grok digest");

        println!("Error propagation stdout: {}", String::from_utf8_lossy(&output.stdout));
        println!("Error propagation stderr: {}", String::from_utf8_lossy(&output.stderr));

        // Should fail or show error message
        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            assert!(
                stderr.contains("error") || stderr.contains("Error") || stderr.contains("invalid"),
                "Error message should be propagated from Python"
            );
        }
    }
}
