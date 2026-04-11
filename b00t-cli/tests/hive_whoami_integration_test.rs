// Integration tests: b00t hive status, b00t whoami, b00t up --help
// H1 gap-fill (OpenHarness analysis) — elasticdotventures/_b00t_#343
// 🤓 Tests use `cargo run --bin b00t-cli` to avoid binary path assumptions;
//    assert_cmd is available as a dev-dep but cargo run is consistent with other test files here.
use std::process::Command;
use std::sync::Mutex;

// 🤓 Serialize cargo invocations to avoid lock file contention in parallel test runs
static CARGO_LOCK: Mutex<()> = Mutex::new(());

const DOTFILES_DIR: &str = env!("CARGO_MANIFEST_DIR");

// ── H1-a: b00t-cli --help exits 0 ─────────────────────────────────────────────
#[test]
fn test_b00t_cli_help_exits_zero() {
    let _lock = CARGO_LOCK.lock().unwrap();
    let output = Command::new("cargo")
        .args(["run", "--bin", "b00t-cli", "--", "--help"])
        .current_dir(DOTFILES_DIR)
        .output()
        .expect("cargo run b00t-cli --help failed to launch");

    assert!(
        output.status.success(),
        "b00t-cli --help non-zero exit: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let stdout = String::from_utf8_lossy(&output.stdout);
    // Verify core subcommands are advertised
    assert!(
        stdout.contains("hive") || stdout.contains("whoami") || stdout.contains("up"),
        "expected core subcommands in --help output, got: {stdout}"
    );
}

// ── H1-b: b00t whoami subcommand runs without panic ────────────────────────────
#[test]
fn test_b00t_whoami_runs() {
    let _lock = CARGO_LOCK.lock().unwrap();
    let output = Command::new("cargo")
        .args(["run", "--bin", "b00t-cli", "--", "whoami"])
        .current_dir(DOTFILES_DIR)
        .output()
        .expect("cargo run b00t-cli whoami failed to launch");

    // whoami may fail (missing config) but MUST NOT panic (exit 101 = rust panic)
    let exit_code = output.status.code().unwrap_or(0);
    assert_ne!(
        exit_code, 101,
        "b00t whoami panicked (exit 101): {}",
        String::from_utf8_lossy(&output.stderr)
    );

    // Either stdout or stderr should have content — no silent failures
    let has_output = !output.stdout.is_empty() || !output.stderr.is_empty();
    assert!(has_output, "b00t whoami produced no output");
}

// ── H1-c: b00t hive status subcommand runs without panic ──────────────────────
#[test]
fn test_b00t_hive_status_runs() {
    let _lock = CARGO_LOCK.lock().unwrap();
    let output = Command::new("cargo")
        .args(["run", "--bin", "b00t-cli", "--", "hive", "status"])
        .current_dir(DOTFILES_DIR)
        .output()
        .expect("cargo run b00t-cli hive status failed to launch");

    let exit_code = output.status.code().unwrap_or(0);
    assert_ne!(
        exit_code, 101,
        "b00t hive status panicked (exit 101): {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let has_output = !output.stdout.is_empty() || !output.stderr.is_empty();
    assert!(has_output, "b00t hive status produced no output");
}

// ── H1-d: b00t up --help exits 0 ──────────────────────────────────────────────
#[test]
fn test_b00t_up_help_exits_zero() {
    let _lock = CARGO_LOCK.lock().unwrap();
    let output = Command::new("cargo")
        .args(["run", "--bin", "b00t-cli", "--", "up", "--help"])
        .current_dir(DOTFILES_DIR)
        .output()
        .expect("cargo run b00t-cli up --help failed to launch");

    assert!(
        output.status.success(),
        "b00t up --help non-zero exit: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}
