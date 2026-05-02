/// Shared helpers for b00t-cli integration tests.

use std::env;

/// Resolve the path to the compiled `b00t-cli` binary.
///
/// Resolution order:
/// 1. `CARGO_BIN_EXE_b00t-cli` (set by Cargo for integration-test binaries)
/// 2. `CARGO_BIN_EXE_b00t_cli` (Cargo normalises `-` → `_` in some contexts)
/// 3. Common local build-output paths relative to `CARGO_MANIFEST_DIR`
#[allow(dead_code)]
pub fn get_b00t_binary() -> String {
    // Cargo sets these env vars automatically when running integration tests.
    if let Ok(path) = env::var("CARGO_BIN_EXE_b00t-cli") {
        return path;
    }
    if let Ok(path) = env::var("CARGO_BIN_EXE_b00t_cli") {
        return path;
    }

    let manifest_dir = env::var("CARGO_MANIFEST_DIR").unwrap();
    let candidates = [
        format!("{manifest_dir}/target/debug/b00t-cli"),
        format!("{manifest_dir}/../target/debug/b00t-cli"),
        format!("{manifest_dir}/target/debug/deps/b00t-cli"),
    ];

    candidates
        .into_iter()
        .find(|path| std::path::Path::new(path).exists())
        .unwrap_or_else(|| {
            panic!(
                "b00t-cli binary not found; tried paths relative to {manifest_dir}. \
                 Run `cargo test` so Cargo builds the binary first."
            )
        })
}
