use assert_cmd::prelude::*;
use std::process::Command;

/// End-to-end test for the `crew recruit` command.
///
/// Verifies that:
/// - The subcommand runs successfully
/// - Output contains the expected header ("Top candidates")
/// - Candidate agents are ranked and displayed correctly
#[test]
fn test_crew_recruit_e2e() {
    let mut cmd = Command::cargo_bin("b00t-cli").unwrap();
    cmd.arg("crew").arg("recruit").arg("rust,python");

    let output = cmd.output().expect("failed to run crew recruit");
    assert!(
        output.status.success(),
        "crew recruit failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("Top candidates"),
        "Should show 'Top candidates' header, got: {}",
        stdout
    );
    assert!(
        stdout.contains("RustCoder"),
        "RustCoder should be ranked (matches 'rust' skill), got: {}",
        stdout
    );
    assert!(
        stdout.contains("DataEngineer"),
        "DataEngineer should be ranked (matches 'python' skill), got: {}",
        stdout
    );
}
