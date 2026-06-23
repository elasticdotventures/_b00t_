---
build-stable-lightweight: vendor/moltis-b00t default release build pulls whatsapp/wacore-binary 0.2.0, which uses #![feature(portable_simd)] and fails on stable Rust; stable build path is cargo build --manifest-path vendor/moltis-b00t/crates/cli/Cargo.toml --release --no-default-features --features lightweight.
