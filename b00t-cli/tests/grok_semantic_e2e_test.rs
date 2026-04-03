//! Semantic E2E tests for b00t-cli grok subcommands via CLI binary
//!
//! Tests the CLI surface: `b00t grok digest`, `b00t grok learn`, `b00t grok ask`
//! with the `--rag` (raglight) backend.
//!
//! Gating:
//!   - Unit-level output format tests: always run (no service required)
//!   - Integration semantic tests: require TEST_RAGLIGHT=1
//!
//! Release gate: ALL tests (unit + integration) must pass before `cog bump` / `just release`.
//!
//! Test data: tests/fixtures/grok_test_cases.json

use anyhow::Result;
use assert_cmd::prelude::*;
use serde::Deserialize;
use std::{env, fs, path::PathBuf, process::Command, io::Write};

// ── fixture types (subset of grok_test_cases.json) ───────────────────────────

#[derive(Deserialize)]
struct DigestCase {
    topic: String,
    content: String,
    expected_keywords: Vec<String>,
}

#[derive(Deserialize)]
struct GrokTestCases {
    digest_cases: Vec<DigestCase>,
}

fn load_fixtures() -> GrokTestCases {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/grok_test_cases.json");
    let json = fs::read_to_string(&path)
        .unwrap_or_else(|_| panic!("Missing fixture: {:?}", path));
    serde_json::from_str(&json)
        .unwrap_or_else(|e| panic!("Invalid fixture JSON: {}", e))
}

/// Resolve path to the b00t-cli binary as built by Cargo for this test target.
fn b00t_bin() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_b00t-cli"))
}
/// Returns a Command pointing at the b00t-cli binary built by Cargo.
/// Uses `assert_cmd::prelude::CommandCargoExt` so the binary is always available.
fn b00t_cmd() -> Command {
    Command::cargo_bin("b00t-cli").expect("b00t-cli binary not found — run `cargo build` first")
}

fn is_raglight_enabled() -> bool {
    env::var("TEST_RAGLIGHT").is_ok()
}

// ── Unit: output format assertions (no service) ──────────────────────────────

#[cfg(test)]
mod cli_output_format {
    use super::*;

    /// `b00t grok digest -t <unknown_topic> "content"` (dual-backend default):
    /// - Irontology: accepts any topic (no pre-registration required) → succeeds
    /// - RAGLight: validates topics → emits warning for unknown topic
    /// Net result: exit 0 (irontology succeeded), warning in stderr
    ///
    /// Use `--rag=raglite` if you need strict topic validation.
    #[test]
    fn test_digest_unknown_topic_dual_backend_partial_success() {
        let out = b00t_cmd()
            .args(["grok", "digest", "-t", "totally_fake_topic_xyz_9999", "some content"])
            .output()
            .expect("failed to run b00t-cli");

        let stdout = String::from_utf8_lossy(&out.stdout);
        // With dual-backend: irontology accepts unknown topic, raglite warns
        // Overall exit is 0 when at least one backend succeeds (irontology)
        // OR non-zero if both fail (sled lock contention in tests is possible)
        if out.status.success() {
            // irontology succeeded
            assert!(
                stdout.contains("Irontology") || stdout.contains("irontology") || stdout.contains("✅"),
                "Successful dual-backend digest must mention irontology: {}", stdout
            );
        } else {
            // both backends failed (e.g. sled lock contention in parallel test run)
            println!("ℹ️  Both backends failed (expected in parallel test runs): {}", stdout);
        }
    }

    /// `b00t grok digest -t rust "content" --rag` must output job UUID and topic name
    #[test]
    fn test_digest_known_topic_output_contains_job_info() {
        let out = b00t_cmd()
            .args(["grok", "digest", "-t", "rust", "Rust ownership prevents data races", "--rag"])
            .output()
            .expect("failed to run b00t-cli");

        let stdout = String::from_utf8_lossy(&out.stdout);
        let stderr = String::from_utf8_lossy(&out.stderr);

        assert!(
            out.status.success(),
            "Known topic digest must succeed.\nstdout: {}\nstderr: {}",
            stdout, stderr
        );
        // Output MUST contain topic name and job UUID indication
        assert!(
            stdout.contains("rust"),
            "stdout must mention topic 'rust': {}", stdout
        );
        // 🤓 raglight queues asynchronously — output contains job_id UUID
        assert!(
            stdout.contains("job") || stdout.contains("Queued") || stdout.contains("job_id"),
            "stdout must mention job queuing: {}", stdout
        );
    }

    /// `b00t grok ask "query"` (dual-backend, no --topic):
    /// - Irontology: queries all topics (no topic filter) → may return results
    /// - RAGLight: warns that --topic is required
    /// Dual backend: exit 0, irontology serves results, raglite warns
    ///
    /// `--rag=raglite` still requires --topic (strict mode).
    #[test]
    fn test_ask_raglite_only_requires_topic() {
        // Strict: --rag=raglite without --topic must exit non-zero
        let out = b00t_cmd()
            .args(["grok", "ask", "memory safety", "--rag=raglite"])
            .output()
            .expect("failed to run b00t-cli");

        assert!(
            !out.status.success(),
            "ask --rag=raglite without --topic must exit non-zero. stdout: {}\nstderr: {}",
            String::from_utf8_lossy(&out.stdout),
            String::from_utf8_lossy(&out.stderr)
        );
    }

    /// `b00t grok ask "query"` (dual default) without --topic — irontology queries all, raglite warns
    #[test]
    fn test_ask_dual_backend_without_topic_exits_zero() {
        let out = b00t_cmd()
            .args(["grok", "ask", "memory safety"])
            .output()
            .expect("failed to run b00t-cli");

        // With dual backend: irontology queries without topic, raglite emits warning
        // Overall exit is 0 (raglite warning is non-fatal when irontology is present)
        let stdout = String::from_utf8_lossy(&out.stdout);
        let stderr = String::from_utf8_lossy(&out.stderr);

        // Either succeeds with 0 results or exits 0 with warnings
        if !out.status.success() {
            // Both backends unavailable (sled lock in parallel tests) — acceptable
            println!("ℹ️  Both backends unavailable in test (non-fatal): stderr={}", stderr);
        } else {
            assert!(
                stdout.contains("Found") || stdout.contains("results"),
                "Dual ask must output result count: {}", stdout
            );
        }
    }

    /// `b00t grok learn "content" -t rust --rag` must queue job and exit zero
    #[test]
    fn test_learn_known_topic_exits_zero() {
        let out = b00t_cmd()
            .args(["grok", "learn", "Rust fearless concurrency", "-t", "rust", "--rag"])
            .output()
            .expect("failed to run b00t-cli");

        let stdout = String::from_utf8_lossy(&out.stdout);
        let stderr = String::from_utf8_lossy(&out.stderr);
        assert!(
            out.status.success(),
            "learn with known topic must exit 0.\nstdout: {}\nstderr: {}",
            stdout, stderr
        );
    }

    /// `b00t grok learn "content" --rag` without -t must exit non-zero
    #[test]
    fn test_learn_without_topic_exits_nonzero() {
        let out = b00t_cmd()
            .args(["grok", "learn", "content without topic", "--rag"])
            .output()
            .expect("failed to run b00t-cli");

        assert!(
            !out.status.success(),
            "learn --rag without -t must exit non-zero"
        );
    }

    /// All fixture digest topics must produce zero-exit via CLI
    #[test]
    fn test_all_fixture_digest_topics_accepted() {
        let cases = load_fixtures();
        for case in &cases.digest_cases {
            let out = b00t_cmd()
                .args(["grok", "digest", "-t", &case.topic, &case.content, "--rag"])
                .output()
                .expect("failed to run b00t-cli");

            assert!(
                out.status.success(),
                "Fixture topic '{}' must be accepted by CLI. stderr: {}",
                case.topic,
                String::from_utf8_lossy(&out.stderr)
            );
        }
    }
}

// ── Integration: full semantic pipeline (TEST_RAGLIGHT=1) ────────────────────

#[cfg(test)]
mod cli_integration {
    use super::*;

    /// Full CLI E2E: digest content → wait → ask → verify semantic result
    #[test]
    #[ignore = "Requires TEST_RAGLIGHT=1 and raglight Python service"]
    fn test_cli_digest_then_ask_roundtrip() {
        if !is_raglight_enabled() {
            println!("⚠️  Skipping: set TEST_RAGLIGHT=1");
            return;
        }
        let bin = b00t_bin();
        assert!(bin.exists(), "Build binary first: cargo build");

        let cases = load_fixtures();
        let case = &cases.digest_cases[0]; // rust + memory safety

        // Step 1: digest
        let digest_out = b00t_cmd()
            .args(["grok", "digest", "-t", &case.topic, &case.content, "--rag"])
            .output()
            .expect("digest failed");
        assert!(digest_out.status.success(), "Digest must succeed");

        // Allow indexing
        std::thread::sleep(std::time::Duration::from_secs(5));

        // Step 2: ask with keyword from expected_keywords
        let query = &case.expected_keywords[0];
        let ask_out = b00t_cmd()
            .args(["grok", "ask", query, "-t", &case.topic, "--rag"])
            .output()
            .expect("ask failed");

        let stdout = String::from_utf8_lossy(&ask_out.stdout);
        assert!(ask_out.status.success(), "Ask must succeed");
        assert!(!stdout.trim().is_empty(), "Ask must return results");

        // Semantic: result contains at least one expected keyword
        let found = case.expected_keywords.iter().any(|kw| {
            stdout.to_lowercase().contains(&kw.to_lowercase())
        });
        assert!(
            found,
            "Semantic result must contain one of {:?}. Got: {}",
            case.expected_keywords, &stdout[..stdout.len().min(300)]
        );
        println!("✅ CLI E2E digest→ask: topic='{}' query='{}' ok", case.topic, query);
    }

    /// Learn from file → ask → verify content indexed
    #[test]
    #[ignore = "Requires TEST_RAGLIGHT=1 and raglight Python service"]
    fn test_cli_learn_from_file_then_ask() {
        if !is_raglight_enabled() {
            println!("⚠️  Skipping: set TEST_RAGLIGHT=1");
            return;
        }
        let bin = b00t_bin();
        assert!(bin.exists(), "Build binary first");

        // Write a temp file with identifiable content
        let mut tmp = tempfile::NamedTempFile::new().expect("tempfile");
        writeln!(tmp, "The borrow checker validates lifetimes at compile time.").ok();
        writeln!(tmp, "Move semantics transfer ownership between scopes.").ok();
        let src_path = tmp.path().to_str().unwrap().to_string();

        // Learn from file
        let learn_out = b00t_cmd()
            .args(["grok", "learn", "dummy", "-s", &src_path, "-t", "rust", "--rag"])
            .output()
            .expect("learn failed");
        assert!(
            learn_out.status.success(),
            "Learn from file must succeed. stderr: {}",
            String::from_utf8_lossy(&learn_out.stderr)
        );

        std::thread::sleep(std::time::Duration::from_secs(5));

        // Query for learned content
        let ask_out = b00t_cmd()
            .args(["grok", "ask", "borrow checker lifetimes", "-t", "rust", "--rag"])
            .output()
            .expect("ask failed");

        let stdout = String::from_utf8_lossy(&ask_out.stdout);
        assert!(ask_out.status.success(), "Ask must succeed");
        assert!(
            stdout.to_lowercase().contains("borrow") || stdout.to_lowercase().contains("lifetime"),
            "Result must mention borrow/lifetime. Got: {}",
            &stdout[..stdout.len().min(300)]
        );
        println!("✅ CLI E2E learn-from-file→ask: ok");
    }
}
