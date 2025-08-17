use std::env;

fn main() {
    println!("cargo:rerun-if-env-changed=CARGO_FEATURE_PYTHON");
    println!("cargo:rerun-if-env-changed=CARGO_FEATURE_WASM");
    
    // Set up conditional compilation flags
    if env::var("CARGO_FEATURE_PYTHON").is_ok() {
        println!("cargo:rustc-cfg=feature=\"python\"");
    }
    
    if env::var("CARGO_FEATURE_WASM").is_ok() {
        println!("cargo:rustc-cfg=feature=\"wasm\"");
    }
}