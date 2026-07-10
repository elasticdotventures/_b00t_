//! TDD tests for b00t-lsp analysis — pure functions, no LSP transport.
//! Fixture datum files live in tests/fixtures/; expected diagnostic counts in
//! tests/fixtures/expected_diagnostics.json (datasets in files, not embedded).

use std::path::{Path, PathBuf};

use b00t_lsp::analysis::{
    datum_rank, datum_stem, dep_ref_at, diagnostics, extract_dep_refs, hover, is_datum_file,
    Severity, WorkspaceIndex,
};

fn fixtures_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures")
}

fn read(name: &str) -> (PathBuf, String) {
    let path = fixtures_dir().join(name);
    let content = std::fs::read_to_string(&path).expect("fixture readable");
    (path, content)
}

// ── rank + stem semantics (mirror datum_utils scan_datums_recursive) ──────

#[test]
fn rank_precedence_tomllmd_over_tomllm_over_toml() {
    assert_eq!(datum_rank(Path::new("x.cli.tomllmd")), Some(3));
    assert_eq!(datum_rank(Path::new("x.cli.tomllm")), Some(2));
    assert_eq!(datum_rank(Path::new("x.cli.toml")), Some(1));
    assert_eq!(datum_rank(Path::new("x.cli.yaml")), None);
}

#[test]
fn stem_strips_outer_extension_only() {
    assert_eq!(datum_stem(Path::new("foo.cli.tomllm")).as_deref(), Some("foo.cli"));
    assert_eq!(datum_stem(Path::new("foo.cli.toml")).as_deref(), Some("foo.cli"));
    assert_eq!(datum_stem(Path::new("bar.tomllmd")).as_deref(), Some("bar"));
}

#[test]
fn non_datum_files_are_skipped() {
    assert!(!is_datum_file(Path::new("bootstrap.toml")));
    assert!(!is_datum_file(Path::new("Cargo.toml")));
    assert!(is_datum_file(Path::new("foo.cli.toml")));
}

// ── diagnostics: dataset-driven over all fixtures ──────────────────────────

#[derive(serde::Deserialize)]
struct Expected {
    cases: Vec<ExpectedCase>,
}

#[derive(serde::Deserialize)]
struct ExpectedCase {
    file: String,
    errors: usize,
    warnings: usize,
    message_contains: Option<String>,
}

#[test]
fn diagnostics_match_expected_dataset() {
    let expected: Expected = serde_json::from_str(
        &std::fs::read_to_string(fixtures_dir().join("expected_diagnostics.json")).unwrap(),
    )
    .unwrap();
    let index = WorkspaceIndex::scan(&fixtures_dir());

    for case in expected.cases {
        let (path, content) = read(&case.file);
        let diags = diagnostics(&path, &content, Some(&index));
        let errors = diags.iter().filter(|d| d.severity == Severity::Error).count();
        let warnings = diags.iter().filter(|d| d.severity == Severity::Warning).count();
        assert_eq!(errors, case.errors, "{}: errors {:?}", case.file, diags);
        assert_eq!(warnings, case.warnings, "{}: warnings {:?}", case.file, diags);
        if let Some(needle) = case.message_contains {
            assert!(
                diags.iter().any(|d| d.message.contains(&needle)),
                "{}: no diagnostic contains '{}' in {:?}",
                case.file,
                needle,
                diags
            );
        }
    }
}

#[test]
fn shadow_warning_names_the_winner() {
    let index = WorkspaceIndex::scan(&fixtures_dir());
    let (path, content) = read("shadowed.config.toml");
    let diags = diagnostics(&path, &content, Some(&index));
    assert!(
        diags
            .iter()
            .any(|d| d.message.contains("shadowed.config.tomllmd")),
        "warning should name the winning file: {diags:?}"
    );
}

#[test]
fn parse_error_positions_are_nonzero_len() {
    let (path, content) = read("broken.syntax.toml");
    let diags = diagnostics(&path, &content, None);
    assert_eq!(diags.len(), 1);
    assert_eq!(diags[0].severity, Severity::Error);
}

#[test]
fn unknown_type_warning_points_at_value() {
    let (path, content) = read("badtype.mystery.toml");
    let diags = diagnostics(&path, &content, None);
    let diag = diags
        .iter()
        .find(|d| d.message.contains("unknown datum type"))
        .expect("unknown-type warning");
    // `type = "flurble"` is on line 3 (0-based 2) in the fixture.
    assert_eq!(diag.line, 2);
    assert!(diag.col_end > diag.col_start);
}

// ── dep refs: extraction, goto, references ─────────────────────────────────

#[test]
fn extracts_dep_refs_including_multiline_arrays() {
    let (_, content) = read("good-datum.skill.tomllm");
    let refs = extract_dep_refs(&content);
    let names: Vec<(&str, &str)> = refs
        .iter()
        .map(|r| (r.key.as_str(), r.name.as_str()))
        .collect();
    assert_eq!(
        names,
        vec![
            ("depends_on", "dep-a"),
            ("depends_on", "dep-b.cli"),
            ("composes_with", "dep-a"),
        ]
    );
    // Positions: every ref's span must slice back to its own name.
    for r in &refs {
        let line = content.lines().nth(r.line as usize).unwrap();
        assert_eq!(&line[r.col_start as usize..r.col_end as usize], r.name);
    }
}

#[test]
fn dep_ref_at_cursor_hits_and_misses() {
    let (_, content) = read("good-datum.skill.tomllm");
    let refs = extract_dep_refs(&content);
    let first = &refs[0];
    let hit = dep_ref_at(&content, first.line, first.col_start + 1).expect("cursor on name");
    assert_eq!(hit.name, "dep-a");
    assert!(dep_ref_at(&content, 0, 0).is_none(), "comment line is not a dep ref");
}

#[test]
fn resolve_by_name_and_dotted_stem() {
    let index = WorkspaceIndex::scan(&fixtures_dir());
    let a = index.resolve("dep-a").expect("dep-a resolves");
    assert!(a.path.ends_with("dep-a.cli.toml"));
    let b = index.resolve("dep-b.cli").expect("dotted name resolves");
    assert!(b.path.ends_with("dep-b.cli.toml"));
    assert!(index.resolve("no-such-datum").is_none());
}

#[test]
fn resolve_shadowed_stem_prefers_highest_rank() {
    let index = WorkspaceIndex::scan(&fixtures_dir());
    let winner = index.resolve("shadowed.config").expect("resolves");
    assert!(winner.path.ends_with("shadowed.config.tomllmd"));
}

#[test]
fn find_references_lists_dependents() {
    let index = WorkspaceIndex::scan(&fixtures_dir());
    let target = fixtures_dir().join("dep-a.cli.toml");
    let refs = index.references_to(&target);
    // good-datum lists dep-a twice: depends_on + composes_with.
    assert_eq!(refs.len(), 2);
    for (file, dep) in &refs {
        assert!(file.path.ends_with("good-datum.skill.tomllm"));
        assert_eq!(dep.name, "dep-a");
    }
}

// ── hover ──────────────────────────────────────────────────────────────────

#[test]
fn hover_shows_name_type_hint_and_summary() {
    let (path, content) = read("good-datum.skill.tomllm");
    let md = hover(&path, &content).expect("hover produced");
    assert!(md.contains("good-datum"));
    assert!(md.contains("`skill`"));
    assert!(md.contains("fixture skill datum for b00t-lsp tests"));
    assert!(md.contains("fixture skill datum with dependencies"));
}

#[test]
fn hover_on_broken_file_is_none() {
    let (path, content) = read("broken.syntax.toml");
    assert!(hover(&path, &content).is_none());
}

// ── schema (task #109 surface) ─────────────────────────────────────────────

#[test]
fn datum_schema_enumerates_types_and_content_tags() {
    let schema = b00t_lsp::schema::datum_schema();
    let type_enum = schema["properties"]["b00t"]["properties"]["type"]["enum"]
        .as_array()
        .expect("type enum present");
    let tokens: Vec<&str> = type_enum.iter().filter_map(|v| v.as_str()).collect();
    assert!(tokens.contains(&"cli"));
    assert!(tokens.contains(&"skill"));
    assert!(tokens.contains(&"prd"), "content tags included");
    assert!(!tokens.contains(&"flurble"));
}
