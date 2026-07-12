fn main() {
    // Copy blessed manifest into crate at build time
    let src = std::path::Path::new("_b00t_/blessed/rust.toml");
    let dst = std::path::Path::new("rust.toml");
    if src.exists() {
        std::fs::copy(src, dst).ok();
    }
}
