//! Z3 authorization tests for EdlQuery constraint validation.
//!
//! Tests cover:
//! 1. Pure-Rust syntax validation (no z3 binary required)
//! 2. Graceful degradation when z3 binary is absent
//! 3. Authorization-level constraint checks (complexity, tier gates)

use b00t_datum_core::edl::{check_z3_syntax, EdlQuery, EdlTagFilter};
use b00t_datum_core::index::DatumIndexEntry;

fn make_entry(tier: &str, complexity: u8, tags: &[&str]) -> DatumIndexEntry {
    DatumIndexEntry {
        key: "auth-test".into(),
        path: "auth-test.tomllmd".into(),
        datum_type: Some("prd".into()),
        tier: Some(tier.into()),
        complexity: Some(complexity),
        type_tags: tags.iter().map(|s| s.to_string()).collect(),
        summary: None,
    }
}

// ─── Z3 syntax validation ────────────────────────────────────────────────────

#[test]
fn z3_syntax_valid_simple_expression() {
    assert!(check_z3_syntax("(= tier \"frontier\")").is_ok());
}

#[test]
fn z3_syntax_valid_and_expression() {
    assert!(check_z3_syntax("(and (<= complexity 5) (not (= tier \"frontier\")))").is_ok());
}

#[test]
fn z3_syntax_rejects_empty() {
    let err = check_z3_syntax("").unwrap_err();
    assert!(err.to_string().contains("empty"), "got: {err}");
}

#[test]
fn z3_syntax_rejects_unbalanced_open() {
    let err = check_z3_syntax("(= tier \"sm0l\"").unwrap_err();
    assert!(
        err.to_string().contains("unclosed"),
        "got: {err}"
    );
}

#[test]
fn z3_syntax_rejects_unbalanced_close() {
    let err = check_z3_syntax("= tier \"sm0l\")").unwrap_err();
    assert!(
        err.to_string().contains("unbalanced"),
        "got: {err}"
    );
}

#[test]
fn z3_syntax_whitespace_only_is_empty() {
    assert!(check_z3_syntax("   ").is_err());
}

// ─── EdlQuery::validate_z3_syntax ────────────────────────────────────────────

#[test]
fn query_validate_syntax_no_constraint_is_ok() {
    let q = EdlQuery {
        type_tags: None,
        datum_type: None,
        tier: None,
        complexity_max: None,
        z3_constraint: None,
    };
    assert!(q.validate_z3_syntax().is_ok());
}

#[test]
fn query_validate_syntax_valid_constraint_ok() {
    let q = EdlQuery {
        z3_constraint: Some("(= tier \"frontier\")".into()),
        type_tags: None,
        datum_type: None,
        tier: None,
        complexity_max: None,
    };
    assert!(q.validate_z3_syntax().is_ok());
}

#[test]
fn query_validate_syntax_invalid_constraint_err() {
    let q = EdlQuery {
        z3_constraint: Some("(unclosed".into()),
        type_tags: None,
        datum_type: None,
        tier: None,
        complexity_max: None,
    };
    assert!(q.validate_z3_syntax().is_err());
}

// ─── Authorization gate patterns ─────────────────────────────────────────────

/// Only frontier-tier datums with complexity ≤ 7 pass the executive gate.
#[test]
fn auth_frontier_complexity_gate() {
    let gate = EdlQuery {
        type_tags: None,
        datum_type: None,
        tier: Some("frontier".into()),
        complexity_max: Some(7),
        z3_constraint: Some("(<= complexity 7)".into()),
    };
    assert!(gate.validate_z3_syntax().is_ok());

    let pass = make_entry("frontier", 5, &["prd"]);
    let fail_tier = make_entry("sm0l", 5, &["prd"]);
    let fail_complexity = make_entry("frontier", 8, &["prd"]);

    assert!(gate.matches(&pass));
    assert!(!gate.matches(&fail_tier));
    assert!(!gate.matches(&fail_complexity));
}

/// ch0nky-tier agents can only access datums with complexity ≤ 5.
#[test]
fn auth_chonky_tier_access_control() {
    let gate = EdlQuery {
        type_tags: Some(EdlTagFilter::Any(vec!["prd".into(), "pattern".into()])),
        datum_type: None,
        tier: None,
        complexity_max: Some(5),
        z3_constraint: Some("(and (<= complexity 5) (or (= type_tags \"prd\") (= type_tags \"pattern\")))".into()),
    };
    assert!(gate.validate_z3_syntax().is_ok());

    let accessible = make_entry("ch0nky", 4, &["prd"]);
    let too_complex = make_entry("ch0nky", 6, &["prd"]);

    assert!(gate.matches(&accessible));
    assert!(!gate.matches(&too_complex));
}

/// sm0l tier can only see non-frontier datums.
#[test]
fn auth_sm0l_tier_blocks_frontier() {
    let gate = EdlQuery {
        type_tags: None,
        datum_type: None,
        tier: Some("sm0l".into()),
        complexity_max: Some(3),
        z3_constraint: Some("(and (= tier \"sm0l\") (<= complexity 3))".into()),
    };
    assert!(gate.validate_z3_syntax().is_ok());

    let pass = make_entry("sm0l", 2, &["task"]);
    let fail = make_entry("frontier", 2, &["task"]);

    assert!(gate.matches(&pass));
    assert!(!gate.matches(&fail));
}

// ─── z3 subprocess feature tests (skip if z3 not in PATH) ───────────────────

#[cfg(feature = "z3-subprocess")]
#[test]
fn z3_subprocess_not_found_returns_clear_error() {
    // This tests the error message when z3 is not in PATH.
    // If z3 IS available, we skip this test gracefully.
    if std::process::Command::new("z3").arg("--version").output().is_ok() {
        // z3 is available; skip the "not found" test path
        return;
    }
    let q = EdlQuery {
        z3_constraint: Some("(= tier \"frontier\")".into()),
        type_tags: None,
        datum_type: None,
        tier: None,
        complexity_max: None,
    };
    let err = q.validate_z3().unwrap_err();
    assert!(
        err.to_string().contains("z3 not found"),
        "Expected 'z3 not found' error, got: {err}"
    );
}
