#![cfg(unix)]

use b00t_cli::checklist::{ChecklistCheck, ChecklistFile, ChecklistSection};
use b00t_cli::gates::GateSpec;
use serde::Deserialize;
use std::time::{Duration, Instant};

#[derive(Deserialize)]
struct Case {
    id: String,
    gate: GateSpec,
    status: String,
    exit_code: i32,
}

#[test]
fn bounded_probes_preserve_all_three_dispositions() {
    let cases: Vec<Case> =
        serde_json::from_str(include_str!("fixtures/checklist_probe_cases.json")).unwrap();
    let directory = tempfile::tempdir().unwrap();
    for case in cases {
        let checklist = ChecklistFile {
            b00t: ChecklistSection {
                name: "system-normal".into(),
                kind: "checklist".into(),
                hint: None,
                checks: vec![ChecklistCheck {
                    id: case.id.clone(),
                    gate: case.gate,
                }],
            },
        };
        let start = Instant::now();
        let result = checklist.evaluate(directory.path().to_str().unwrap());
        assert_eq!(result.outcomes[0].status, case.status, "{}", case.id);
        assert_eq!(
            result.disposition.exit_code(),
            case.exit_code,
            "{}",
            case.id
        );
        assert!(
            start.elapsed() < Duration::from_millis(1500),
            "{} exceeded deadline",
            case.id
        );
    }
}

#[test]
fn seeded_system_normal_is_not_a_placeholder() {
    let datum = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../_b00t_/system-normal.checklist.toml");
    let checklist = ChecklistFile::load(&datum).unwrap();
    assert_eq!(checklist.b00t.name, "system-normal");
    assert!(!checklist.b00t.checks.is_empty());
    let checks = serde_json::to_value(checklist).unwrap();
    assert!(
        checks["b00t"]["check"]
            .as_array()
            .unwrap()
            .iter()
            .any(|check| check["probe"].is_object())
    );
}

#[test]
fn json_import_preserves_probe_or_returns_unknown() {
    #[derive(Deserialize)]
    struct ImportCase {
        input: serde_json::Value,
        status: String,
    }
    let cases: Vec<ImportCase> =
        serde_json::from_str(include_str!("fixtures/checklist_probe_import_cases.json")).unwrap();
    let directory = tempfile::tempdir().unwrap();
    for case in cases {
        let datum = b00t_cli::dispatch::normalize_mcp_json(&case.input.to_string(), false).unwrap();
        let gates = datum.gate.unwrap();
        assert_eq!(gates.len(), 1);
        assert!(gates[0].probe.is_some());
        let status = match gates[0].eval_disposition(directory.path().to_str().unwrap()) {
            ufo_types::Disposition::Satisfied => "satisfied",
            ufo_types::Disposition::Violated { .. } => "violated",
            ufo_types::Disposition::Unknown => "unknown",
        };
        assert_eq!(status, case.status);
    }
}
