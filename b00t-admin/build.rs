use std::process::Command;

fn main() {
    let timestamp = chrono();
    let git_hash = git_short_hash();
    let version = env!("CARGO_PKG_VERSION");

    let out_dir = std::env::var("OUT_DIR").unwrap();
    let dest = std::path::Path::new(&out_dir).join("build_info.rs");

    std::fs::write(
        &dest,
        format!(
            r#"pub const BUILD_TIMESTAMP: &str = "{timestamp}";
pub const GIT_HASH: &str = "{git_hash}";
pub const VERSION: &str = "{version}";
"#,
        ),
    )
    .unwrap();
}

fn chrono() -> String {
    Command::new("date")
        .args(["-u", "+%Y-%m-%dT%H:%M:%SZ"])
        .output()
        .ok()
        .and_then(|o| String::from_utf8(o.stdout).ok())
        .unwrap_or_default()
        .trim()
        .to_string()
}

fn git_short_hash() -> String {
    Command::new("git")
        .args(["rev-parse", "--short", "HEAD"])
        .output()
        .ok()
        .and_then(|o| String::from_utf8(o.stdout).ok())
        .unwrap_or_default()
        .trim()
        .to_string()
}
