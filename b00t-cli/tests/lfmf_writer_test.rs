//! Production-scale salvage writer regression tests — issue #934.
//!
//! Cases in tests/fixtures/lfmf_writer_cases.json. Unlike
//! lfmf_salvage_test.rs (word-count token stub, small topic_max), this uses
//! the REAL o200k_base tiktoken counter and production topic_max=25 /
//! body_max=250 to lock in two things at the scale that actually shipped a
//! bug:
//!
//! 1. Anti-duplication: a colon-free lesson under body_max no longer gets
//!    its entire text echoed back as "topic" (topic == body verbatim) —
//!    auto_topic() for synthesized topics is now capped at
//!    DERIVED_TOPIC_STUB_TOKENS (8), not the full topic_max (25).
//! 2. format_stored_content() renders the stored body text exactly once —
//!    no redundant re-parse of an already-formatted "topic: body" string
//!    (that redundant split_once(':') lived in
//!    LfmfSystem::record_lesson/parse_lesson; the CLI now calls
//!    record_lesson_parts with topic/body already separated).

use b00t_cli::commands::lfmf::{format_stored_content, parse_lesson_salvage};
use serde::Deserialize;
use std::path::PathBuf;
use tiktoken_rs::o200k_base;

#[derive(Deserialize)]
struct Fixture {
    topic_max: usize,
    body_max: usize,
    cases: Vec<Case>,
}

#[derive(Deserialize)]
struct Case {
    name: String,
    raw: String,
    expect_salvage: Option<String>,
    #[serde(default)]
    assert_anti_duplication: bool,
    #[serde(default)]
    expect_topic: Option<String>,
    #[serde(default)]
    expect_body: Option<String>,
}

#[test]
fn writer_cases_from_fixture_production_scale() {
    let path =
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/lfmf_writer_cases.json");
    let fixture: Fixture =
        serde_json::from_str(&std::fs::read_to_string(&path).expect("fixture readable"))
            .expect("fixture parses");

    let bpe = o200k_base().expect("tiktoken o200k_base loads");
    let count_tokens = |s: &str| bpe.encode_with_special_tokens(s).len();

    for case in &fixture.cases {
        let parsed =
            parse_lesson_salvage(&case.raw, &count_tokens, fixture.topic_max, fixture.body_max);

        assert_eq!(
            parsed.salvage, case.expect_salvage,
            "salvage mismatch in case '{}'",
            case.name
        );

        if case.assert_anti_duplication {
            // The core regression this issue is about: a synthesized topic
            // must never just BE the body (issue #934's "salvaged:no_colon"
            // duplication) — it's a short derived stub, not an echo.
            assert_ne!(
                parsed.topic, parsed.body,
                "topic duplicated body verbatim in case '{}' — issue #934 regression",
                case.name
            );
            assert!(
                parsed.topic.len() < parsed.body.len() / 2,
                "topic not meaningfully shorter than body in case '{}': topic={:?} body={:?}",
                case.name,
                parsed.topic,
                parsed.body
            );
            // Payload preservation still holds even with the capped stub.
            for word in case.raw.split_whitespace() {
                assert!(
                    parsed.body.contains(word) || parsed.topic.contains(word),
                    "payload word '{}' lost in case '{}'",
                    word,
                    case.name
                );
            }
        }

        if let Some(expect_topic) = &case.expect_topic {
            assert_eq!(&parsed.topic, expect_topic, "topic mismatch in case '{}'", case.name);
        }
        if let Some(expect_body) = &case.expect_body {
            assert_eq!(&parsed.body, expect_body, "body mismatch in case '{}'", case.name);
        }

        // Well-formed cases (no salvage) must round-trip through the stored
        // content helper as an exact match to the parsed body — no marker,
        // no re-formatting surprises.
        if case.expect_salvage.is_none() {
            assert_eq!(
                format_stored_content(&parsed),
                parsed.body,
                "stored content mismatch (well-formed) in case '{}'",
                case.name
            );
        } else {
            let expected_kind = case.expect_salvage.as_ref().unwrap();
            assert_eq!(
                format_stored_content(&parsed),
                format!("{} <!-- salvaged:{} -->", parsed.body, expected_kind),
                "stored content mismatch (salvaged) in case '{}'",
                case.name
            );
        }
    }
}
