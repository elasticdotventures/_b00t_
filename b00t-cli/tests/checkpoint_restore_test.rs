// H2 integration test: b00t.sh restore_task_state reads task_state.json → populates tasks.json
// Acceptance criteria from TODO-next.md:
//   - write mock task_state.json, run restore, verify tasks.json populated
use std::process::Command;
use std::sync::Mutex;

static CARGO_LOCK: Mutex<()> = Mutex::new(());

const REPO_ROOT: &str = "/home/brianh/.b00t";

// ── H2-a: checkpoint restore fixture exits 0 (PASS) ──────────────────────────
#[test]
fn test_checkpoint_restore_passes() {
    let _lock = CARGO_LOCK.lock().unwrap();
    let fixture = format!("{REPO_ROOT}/b00t-cli/tests/fixtures/test_checkpoint_restore.sh");

    let output = Command::new("bash")
        .arg(&fixture)
        .output()
        .expect("failed to launch checkpoint restore fixture");

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);

    assert!(
        output.status.success(),
        "checkpoint restore test FAILED:\nstdout: {stdout}\nstderr: {stderr}"
    );

    assert!(
        stdout.contains("PASS"),
        "expected PASS in output, got: {stdout}"
    );
}

// ── H2-b: B00T_TEST_MODE guard — b00t.sh source exits without running loop ───
#[test]
fn test_b00t_test_mode_skips_loop() {
    // Verify B00T_TEST_MODE=1 causes b00t.sh to exit 0 without running the main loop
    let b00t_sh = format!("{REPO_ROOT}/b00t.sh");

    let output = Command::new("bash")
        .arg("-c")
        .arg(format!(
            "B00T_TEST_MODE=1 B00T_STATE_DIR=/tmp/b00t-test-$$ bash '{b00t_sh}' && echo 'exited-cleanly'"
        ))
        .output()
        .expect("failed to run b00t.sh in test mode");

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);

    // Must exit 0 and not start the loop (no "starting b00t Ralph loop" in stderr)
    assert!(
        output.status.success(),
        "B00T_TEST_MODE=1 non-zero exit: {stderr}"
    );

    assert!(
        !stderr.contains("starting b00t Ralph loop"),
        "main loop ran despite B00T_TEST_MODE=1: {stderr}"
    );

    assert!(
        stdout.contains("exited-cleanly"),
        "expected 'exited-cleanly' in stdout: {stdout}"
    );
}
