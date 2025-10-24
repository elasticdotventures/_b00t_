use std::env;

fn main() {
    // 🤓 PyO3 + CONDA_PREFIX conflict detection
    if let Ok(conda_prefix) = env::var("CONDA_PREFIX") {
        if env::var("VIRTUAL_ENV").is_ok() {
            println!("cargo:warning=🤓 DETECTED: Both VIRTUAL_ENV and CONDA_PREFIX set");
            println!("cargo:warning=❌ PyO3 linking will FAIL with undefined Python symbols");
            println!("cargo:warning=✅ SOLUTION: unset CONDA_PREFIX && cargo build");
            println!(
                "cargo:warning=💡 OR: Use justfile commands (handles environment automatically)"
            );
        } else if !conda_prefix.is_empty() {
            println!("cargo:warning=🤓 DETECTED: CONDA_PREFIX={}", conda_prefix);
            println!("cargo:warning=⚠️  PyO3 may conflict with conda Python environment");
            println!("cargo:warning=✅ IF BUILD FAILS: unset CONDA_PREFIX && cargo build");
        }
    }

    // 🤓 Feature guidance for library vs extension usage
    let features = env::var("CARGO_FEATURE_PYO3").is_ok();
    if features {
        println!("cargo:warning=🐍 Building with PyO3 Python bindings enabled");
        if env::var("CONDA_PREFIX").is_ok() {
            println!("cargo:warning=⚠️  CONDA_PREFIX detected - unset if linking fails");
        }
    } else {
        println!("cargo:warning=🦀 Building as Rust library (PyO3 disabled)");
    }
}
