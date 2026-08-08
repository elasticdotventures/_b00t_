//! Integration tests for `b00t exec --vetted` — the deterministic vetted-script
//! grant path (PRD-SUDO-OPERATOR-GOVERNANCE's vetted-script extension).
//!
//! Drives the compiled `b00t-cli` binary end-to-end against a throwaway git
//! fixture repo, following the same `fixture_repo()` pattern as
//! `b00t-c0re-lib/src/sudo_operator/vetted.rs`'s unit tests (duplicated here
//! since b00t-cli's integration tests don't share a test-utils crate with
//! b00t-c0re-lib today).

mod common;

use common::get_b00t_binary;
use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::PathBuf;
use std::process::Command;

/// Builds a throwaway git repo with a committed `origin` remote pointing at
/// an equally throwaway bare repo, a `_b00t_/vetted-scripts.toml` registry,
/// and one registered + one unregistered executable script — each just
/// touches its own marker file, so "did the grant actually execute" is
/// observable without needing real privilege.
///
/// Also drops a `hive-guards.hive.toml` into the same `_b00t_` dir so that
/// invoking either script path trips a Block guard — required to even
/// reach the `--vetted` branch in `handle_exec`, which lives inside
/// `GuardResult::Block`.
///
/// Returns (tempdir [kept alive for the test's duration], work checkout
/// path, fake $HOME path). The fake $HOME isolates `~/.b00t/exec-log.jsonl`
/// and `~/.b00t/exec-audit.json` from the real developer machine.
fn fixture_repo() -> (tempfile::TempDir, PathBuf, PathBuf) {
    let tmp = tempfile::tempdir().unwrap();
    let bare = tmp.path().join("origin.git");
    let work = tmp.path().join("work");
    let fake_home = tmp.path().join("home");
    fs::create_dir_all(&bare).unwrap();
    fs::create_dir_all(&work).unwrap();
    fs::create_dir_all(&fake_home).unwrap();

    Command::new("git").args(["init", "--bare"]).arg(&bare).status().unwrap();
    Command::new("git").arg("init").current_dir(&work).status().unwrap();
    Command::new("git")
        .args(["config", "user.email", "test@example.com"])
        .current_dir(&work)
        .status()
        .unwrap();
    Command::new("git")
        .args(["config", "user.name", "test"])
        .current_dir(&work)
        .status()
        .unwrap();
    Command::new("git")
        .args(["remote", "add", "origin"])
        .arg(&bare)
        .current_dir(&work)
        .status()
        .unwrap();

    fs::create_dir_all(work.join("_b00t_")).unwrap();
    fs::write(
        work.join("_b00t_/vetted-scripts.toml"),
        "[[vetted]]\npath = \"_b00t_/vetted-hello.sh\"\ndescription = \"test script\"\n",
    )
    .unwrap();
    fs::write(
        work.join("_b00t_/vetted-hello.sh"),
        "#!/bin/sh\ntouch vetted-marker.txt\n",
    )
    .unwrap();
    fs::set_permissions(
        work.join("_b00t_/vetted-hello.sh"),
        fs::Permissions::from_mode(0o755),
    )
    .unwrap();
    // Deliberately NOT added to vetted-scripts.toml — used by the deny test.
    fs::write(
        work.join("_b00t_/not-vetted.sh"),
        "#!/bin/sh\ntouch not-vetted-marker.txt\n",
    )
    .unwrap();
    fs::set_permissions(
        work.join("_b00t_/not-vetted.sh"),
        fs::Permissions::from_mode(0o755),
    )
    .unwrap();

    Command::new("git").args(["add", "-A"]).current_dir(&work).status().unwrap();
    Command::new("git")
        .args(["commit", "-m", "init"])
        .current_dir(&work)
        .status()
        .unwrap();
    Command::new("git")
        .args(["push", "-u", "origin", "HEAD:main"])
        .current_dir(&work)
        .status()
        .unwrap();

    // Guard config: any command containing "_b00t_/" is Block-tier, so both
    // the registered and unregistered script paths route through
    // handle_exec's GuardResult::Block arm (where --vetted is handled).
    // Written AFTER the commit/push above — it's a local CLI config file
    // consumed via `--path`, not part of the git-tracked content the
    // vetted check compares against origin/main.
    fs::write(
        work.join("_b00t_/hive-guards.hive.toml"),
        r#"[b00t]
name = "hive-guards"
hint = "test guard forcing Block for fixture scripts"

[[b00t.hive.guards]]
pattern = "_b00t_/"
action = "block"
message = "test-only block gate"
"#,
    )
    .unwrap();

    (tmp, work, fake_home)
}

#[test]
fn test_vetted_flag_denies_when_path_not_registered() {
    let (_tmp, work, fake_home) = fixture_repo();
    let marker = work.join("not-vetted-marker.txt");

    let output = Command::new(get_b00t_binary())
        .args([
            "--path",
            work.join("_b00t_").to_str().unwrap(),
            "exec",
            "--vetted",
            "_b00t_/not-vetted.sh",
        ])
        .current_dir(&work)
        .env("HOME", &fake_home)
        .output()
        .expect("failed to run b00t-cli");

    assert!(
        !output.status.success(),
        "expected non-zero exit for unregistered --vetted target; stdout={} stderr={}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("not a registered vetted script"),
        "stderr did not mention the NotVetted reason: {stderr}"
    );
    assert!(
        !marker.exists(),
        "unregistered script's side effect occurred despite deny"
    );
}

#[test]
fn test_vetted_flag_grants_when_content_matches_origin_main() {
    let (_tmp, work, fake_home) = fixture_repo();
    let marker = work.join("vetted-marker.txt");
    let log_path = fake_home.join(".b00t/exec-log.jsonl");

    let output = Command::new(get_b00t_binary())
        .args([
            "--path",
            work.join("_b00t_").to_str().unwrap(),
            "exec",
            "--vetted",
            "_b00t_/vetted-hello.sh",
        ])
        .current_dir(&work)
        .env("HOME", &fake_home)
        .output()
        .expect("failed to run b00t-cli");

    assert!(
        output.status.success(),
        "expected zero exit for a vetted grant; stdout={} stderr={}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        marker.exists(),
        "registered script's side effect (marker file) did not occur"
    );

    let log = fs::read_to_string(&log_path).expect("~/.b00t/exec-log.jsonl was not written");
    // Not asserting on the literal last line: handle_exec's post-match
    // "Synchronous execution" section unconditionally appends one more
    // audit entry (result = "{guard-derived label}:{sandbox}") after ANY
    // Block-arm fall-through — including the pre-existing TTL-bypass and
    // --justification grant paths, not something new to --vetted. The
    // meaningful assertion is that the vetted-grant branch's own entry
    // exists in the log at all.
    let granted = log
        .lines()
        .filter_map(|l| serde_json::from_str::<serde_json::Value>(l).ok())
        .any(|v| v.get("result").and_then(|r| r.as_str()) == Some("sudo-vetted-granted"));
    assert!(
        granted,
        "no sudo-vetted-granted entry found in exec-log.jsonl:\n{log}"
    );
}

#[test]
fn test_vetted_flag_denies_when_content_mismatches_origin_main() {
    let (_tmp, work, fake_home) = fixture_repo();
    let marker = work.join("vetted-marker.txt");

    // Tamper AFTER fixture_repo()'s commit+push — origin/main still has
    // the original content; the working tree now differs.
    fs::write(
        work.join("_b00t_/vetted-hello.sh"),
        "#!/bin/sh\ntouch vetted-marker.txt\necho TAMPERED\n",
    )
    .unwrap();

    let output = Command::new(get_b00t_binary())
        .args([
            "--path",
            work.join("_b00t_").to_str().unwrap(),
            "exec",
            "--vetted",
            "_b00t_/vetted-hello.sh",
        ])
        .current_dir(&work)
        .env("HOME", &fake_home)
        .output()
        .expect("failed to run b00t-cli");

    assert!(
        !output.status.success(),
        "expected non-zero exit for content-mismatched --vetted target"
    );
    assert!(
        !marker.exists(),
        "tampered script's side effect occurred despite content mismatch"
    );
}

#[test]
fn test_justification_path_unaffected_by_vetted_registration() {
    let (_tmp, work, fake_home) = fixture_repo();
    let log_path = fake_home.join(".b00t/exec-log.jsonl");

    // Target the SAME script that's registered+vetted, but use
    // --justification instead of --vetted — must NOT take the vetted-grant
    // shortcut just because the path happens to be registered.
    let _output = Command::new(get_b00t_binary())
        .args([
            "--path",
            work.join("_b00t_").to_str().unwrap(),
            "exec",
            "--justification",
            "test justification",
            "_b00t_/vetted-hello.sh",
        ])
        .current_dir(&work)
        .env("HOME", &fake_home)
        .output()
        .expect("failed to run b00t-cli");

    // No live adversarial-model endpoint in this test environment, so we
    // only assert on what --vetted's presence must NOT cause: a
    // sudo-vetted-granted entry. The adversarial path's own Grant/Deny/
    // Escalate behavior is covered by verdict.rs's/governance.rs's own
    // tests, not this one.
    if let Ok(log) = fs::read_to_string(&log_path) {
        let has_vetted_grant = log
            .lines()
            .filter_map(|l| serde_json::from_str::<serde_json::Value>(l).ok())
            .any(|v| v.get("result").and_then(|r| r.as_str()) == Some("sudo-vetted-granted"));
        assert!(
            !has_vetted_grant,
            "--justification must never take the vetted-grant shortcut, even on a registered script:\n{log}"
        );
    }
}
