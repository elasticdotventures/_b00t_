use std::env;

fn main() {
    // 🤓 PyO3 + CONDA_PREFIX conflict detection (only when both overlap — the actual failure case)
    if let Ok(_conda_prefix) = env::var("CONDA_PREFIX") {
        if env::var("VIRTUAL_ENV").is_ok() {
            println!("cargo:warning=🤓 CONDA_PREFIX + VIRTUAL_ENV both set — PyO3 linking may fail");
            println!("cargo:warning=✅ unset CONDA_PREFIX && cargo build");
        }
    }
    // Feature guidance: only on first build
    if env::var("CARGO_FEATURE_PYO3").is_ok() && env::var("B00T_GROK_FIRST_BUILD").is_ok() {
        println!("cargo:note=🐍 Building with PyO3 Python bindings");
    }
}
