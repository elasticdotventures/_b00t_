//! Production-scale salvage writer regression tests — issue #934.
//!
//! Ported from the now-superseded/conflicting PR #1183 onto PR #1163's
//! shipped fix. #1183 assumed `record_lesson_parts`/`format_stored_content`
//! functions that were never merged; #1163 instead fixed the duplication at
//! its actual source — `auto_topic()` in `b00t-cli/src/commands/lfmf.rs` now
//! caps a synthesized topic to a short 5-word-prefix + ellipsis summary
//! whenever the whole colon-free lesson would otherwise fit under
//! `topic_max` verbatim (the case that produced a topic == body duplicate).
//!
//! Cases in tests/fixtures/lfmf_writer_cases.json. Unlike lfmf_salvage_test.rs
//! (word-count token stub, small topic_max), this uses the REAL o200k_base
//! tiktoken counter and production topic_max=25 / body_max=250 to lock in,
//! at the scale that actually shipped the bug:
//!
//! 1. Anti-duplication: a colon-free lesson under body_max no longer gets
//!    its entire text echoed back as "topic" (topic == body verbatim). The
//!    two `_regression`/`_no_regression` cases are drawn directly from the
//!    real `_b00t_/learn/cargo-workspace-install.md` and
//!    `_b00t_/learn/guard-session-fixtures.md` entries at the center of
//!    issue #934.
//! 2. No destructive re-split: the CLI still assembles a single
//!    `"{safe_topic}: {body}[ <!-- salvaged:kind -->]"` string and hands it
//!    to `b00t_c0re_lib::LfmfSystem::record_lesson_scoped`, which re-splits
//!    on the first `':'` (`parse_lesson`). That round trip is only lossless
//!    because `handle_lfmf` strips colons out of the topic first
//!    (`parsed.topic.replace(':', "")`) before formatting — this test
//!    exercises that exact reformat-then-resplit pipeline (mirrored here,
//!    since `parse_lesson` is a private method on `LfmfSystem`) to lock the
//!    invariant in, rather than just trusting it by inspection.

use b00t_cli::commands::lfmf::{ParsedLesson, parse_lesson_salvage};
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

/// Mirrors the storage-format assembly inline in
/// `b00t_cli::commands::lfmf::handle_lfmf` (main's #1163 fix did not extract
/// a separate `format_stored_content` helper — the CLI still builds one
/// "topic: body[ marker]" string that the core crate re-splits).
fn stored_content(safe_topic: &str, parsed: &ParsedLesson) -> String {
    match &parsed.salvage {
        Some(kind) => format!("{}: {} <!-- salvaged:{} -->", safe_topic, parsed.body, kind),
        None => format!("{}: {}", safe_topic, parsed.body),
    }
}

/// Mirrors `b00t_c0re_lib::LfmfSystem::parse_lesson`'s `split_once(':')` +
/// `.trim()` re-parse of the stored "topic: body" string (private to that
/// crate, so re-implemented here rather than exercised directly).
fn resplit(stored: &str) -> (String, String) {
    match stored.split_once(':') {
        Some((topic_part, content_part)) => {
            (topic_part.trim().to_string(), content_part.trim().to_string())
        }
        None => ("General".to_string(), stored.to_string()),
    }
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

        if let Some(expect_topic) = &case.expect_topic {
            assert_eq!(&parsed.topic, expect_topic, "topic mismatch in case '{}'", case.name);
        }
        if let Some(expect_body) = &case.expect_body {
            assert_eq!(&parsed.body, expect_body, "body mismatch in case '{}'", case.name);
        }

        if case.assert_anti_duplication {
            // The core regression this issue is about: a synthesized topic
            // must never just BE the body (issue #934's original
            // topic==body duplication in cargo-workspace-install.md) — it's
            // a short derived stub or truncated prefix, not an echo.
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
            // Payload preservation still holds even with the capped/truncated topic.
            for word in case.raw.split_whitespace() {
                assert!(
                    parsed.body.contains(word) || parsed.topic.contains(word),
                    "payload word '{}' lost in case '{}'",
                    word,
                    case.name
                );
            }
        }

        // No destructive re-split: reformat through the CLI's real
        // "topic: body[ marker]" assembly, then re-split the way
        // LfmfSystem::parse_lesson does on the far side of
        // record_lesson_scoped, and confirm nothing was corrupted or lost.
        let safe_topic = parsed.topic.replace(':', "");
        let stored = stored_content(&safe_topic, &parsed);
        let (resplit_topic, resplit_content) = resplit(&stored);

        assert_eq!(
            resplit_topic, safe_topic,
            "destructive re-split: topic changed after reformat+resplit in case '{}'",
            case.name
        );
        let expected_content = match &parsed.salvage {
            Some(kind) => format!("{} <!-- salvaged:{} -->", parsed.body, kind),
            None => parsed.body.clone(),
        };
        assert_eq!(
            resplit_content, expected_content,
            "destructive re-split: content changed after reformat+resplit in case '{}'",
            case.name
        );
    }
}
