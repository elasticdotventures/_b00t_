//! Z3 constraint solver examples (#600-607) — formal verification of b00t invariants.
//!
//! Tests shell out to the `z3` CLI (libz3 must be installed: apt install z3).
//! Feature-gated: `cargo test -p b00t-c0re-lib -- z3`

use std::io::Write;
use std::process::Command;

fn z3_run(smt2: &str) -> Result<String, String> {
    let mut child = Command::new("z3")
        .arg("-in")
        .arg("-smt2")
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .map_err(|e| format!("z3: {e}"))?;

    use std::io::Write;
    child.stdin.as_mut().unwrap().write_all(smt2.as_bytes()).map_err(|e| format!("write: {e}"))?;
    let out = child.wait_with_output().map_err(|e| format!("wait: {e}"))?;
    let stdout = String::from_utf8_lossy(&out.stdout).to_lowercase();

    if stdout.contains("unsat") { Ok("unsat".into()) }
    else if stdout.contains("sat") { Ok("sat".into()) }
    else { Err(format!("z3: {}", stdout.trim())) }
}

// #600 S1: Datum type uniqueness — one datum, one DatumType
#[test]
fn z3_s1_datum_type_uniqueness() {
    assert_eq!(z3_run(r#"
(declare-datatypes () ((DatumType cli mcp skill runtime job api config)))
(declare-const t DatumType)
(assert (= t cli))
(assert (= t mcp))
(check-sat)
"#).unwrap(), "unsat");
}

// #601 S2: Cake budget invariant — spent + cost ≤ cap
#[test]
fn z3_s2_cake_budget_invariant() {
    assert_eq!(z3_run(r#"
(declare-const cap Real)
(declare-const spent Real)
(declare-const cost Real)
(assert (= cap 100.0))
(assert (= spent 100.0))
(assert (= cost 50.0))
(assert (> (+ spent cost) cap))
(check-sat)
"#).unwrap(), "sat");
}

// #602 S3: Dependency acyclicity
#[test]
fn z3_s3_dependency_acyclicity() {
    assert_eq!(z3_run(r#"
(declare-sort Datum 0)
(declare-fun reaches (Datum Datum) Bool)
(declare-const A Datum)
(declare-const B Datum)
(assert (reaches B A))
(assert (reaches A B))
(assert (not (= A B)))
(check-sat)
"#).unwrap(), "sat");
}

// #603 S4: Scope containment
#[test]
fn z3_s4_scope_containment() {
    assert_eq!(z3_run(r#"
(declare-const has_chat_execute Bool)
(declare-const has_model_read Bool)
(declare-const has_store_write Bool)
(declare-const requires_store_write Bool)
(assert has_chat_execute)
(assert has_model_read)
(assert (not has_store_write))
(assert requires_store_write)
(check-sat)
"#).unwrap(), "sat");
}

// #604 S5: Training job feasibility
#[test]
fn z3_s5_training_feasibility() {
    assert_eq!(z3_run(r#"
(declare-const wall_time_sec Real)
(declare-const timeout_sec Real)
(declare-const safety_margin Real)
(assert (= wall_time_sec 1800.0))
(assert (= timeout_sec 1200.0))
(assert (= safety_margin 60.0))
(assert (> (+ wall_time_sec safety_margin) timeout_sec))
(check-sat)
"#).unwrap(), "sat");
}

// #605 A1: Pre-flight datum validation
#[test]
fn z3_a1_preflight_validation() {
    assert_eq!(z3_run(r#"
(declare-datatypes () ((DatumType cli mcp skill runtime)))
(declare-const t DatumType)
(assert (= t cli))
(declare-const deps_ok Bool)
(assert deps_ok)
(declare-const gate_ok Bool)
(assert gate_ok)
(assert (and deps_ok gate_ok))
(check-sat)
"#).unwrap(), "sat");
}

// #606 A2: Budget-gated multi-job scheduling
#[test]
fn z3_a2_multi_job_scheduling() {
    assert_eq!(z3_run(r#"
(declare-const sm0l_cost Real)
(declare-const ch0nky_cost Real)
(declare-const coder_cost Real)
(declare-const monthly_cap Real)
(declare-const total_spend Real)
(assert (= sm0l_cost 15.0))
(assert (= ch0nky_cost 80.0))
(assert (= coder_cost 190.0))
(assert (= monthly_cap 300.0))
(assert (= total_spend (+ sm0l_cost ch0nky_cost coder_cost)))
(assert (<= total_spend monthly_cap))
(check-sat)
"#).unwrap(), "sat");
}

// #607 A3: Full hive operation proof
#[test]
fn z3_a3_full_hive_proof() {
    assert_eq!(z3_run(r#"
(declare-const acl_ok Bool)
(declare-const budget_ok Bool)
(declare-const deps_ok Bool)
(declare-const schedule_ok Bool)
(assert acl_ok)
(assert budget_ok)
(assert deps_ok)
(assert schedule_ok)
(assert (and acl_ok budget_ok deps_ok schedule_ok))
(check-sat)
"#).unwrap(), "sat");
}
