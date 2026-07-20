//! Salvage-first lesson parsing — cases in tests/fixtures/lfmf_salvage_cases.json
//! Meta-pattern under test: NEVER lose the payload. CLI validation must not be
//! stricter than the storage layer it fronts (LfmfSystem::parse_lesson accepts
//! colon-free lessons; the CLI used to bail and discard them).

use b00t_cli::commands::lfmf::parse_lesson_salvage;
use serde::Deserialize;
use std::path::PathBuf;

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
    topic: String,
    body: String,
    salvage: Option<String>,
}

fn word_count(s: &str) -> usize {
    s.split_whitespace().count()
}

#[test]
fn salvage_cases_from_fixture() {
    let path =
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/lfmf_salvage_cases.json");
    let fixture: Fixture =
        serde_json::from_str(&std::fs::read_to_string(&path).expect("fixture readable"))
            .expect("fixture parses");

    for case in &fixture.cases {
        let parsed =
            parse_lesson_salvage(&case.raw, &word_count, fixture.topic_max, fixture.body_max);
        assert_eq!(
            parsed.topic, case.topic,
            "topic mismatch in case '{}'",
            case.name
        );
        assert_eq!(
            parsed.body, case.body,
            "body mismatch in case '{}'",
            case.name
        );
        assert_eq!(
            parsed.salvage, case.salvage,
            "salvage mismatch in case '{}'",
            case.name
        );
        // Payload preservation invariant: every input word survives somewhere in topic+body
        for word in case.raw.split_whitespace().map(|w| w.trim_matches(':')) {
            if word.is_empty() {
                continue;
            }
            assert!(
                parsed.body.contains(word) || parsed.topic.contains(word),
                "payload word '{}' lost in case '{}'",
                word,
                case.name
            );
        }
    }
}
