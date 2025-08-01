use std::process::Command;

fn main() {
    // 🤓 Get version from git tags - cocogitto is the SINGLE source of authority for versioning
    // This ensures version consistency across all workspace crates without duplicating version info
    let version = get_version_from_git().unwrap_or_else(|| "0.0.0-dev".to_string());
    
    // 🤓 Set custom environment variable for runtime version display in --version output
    println!("cargo:rustc-env=GIT_REPO_TAG_VERSION={}", version);
    
    // 🤓 Override CARGO_PKG_VERSION so build output shows actual version being built
    // This makes "cargo install" show "Installing b00t-cli v0.6.1" instead of "v0.0.0-git"
    println!("cargo:rustc-env=CARGO_PKG_VERSION={}", version);
    
    // 🤓 Tell cargo to rerun build script when git tags or HEAD changes
    println!("cargo:rerun-if-changed=.git/refs/tags");
    println!("cargo:rerun-if-changed=.git/HEAD");
}

fn get_version_from_git() -> Option<String> {
    // Try to get the latest git tag
    let output = Command::new("git")
        .args(&["describe", "--tags", "--exact-match", "HEAD"])
        .output()
        .ok()?;
    
    if output.status.success() {
        let version = String::from_utf8(output.stdout).ok()?;
        return Some(version.trim().to_string());
    }
    
    // If no exact tag match, get the latest tag
    let output = Command::new("git")
        .args(&["describe", "--tags", "--abbrev=0"])
        .output()
        .ok()?;
    
    if output.status.success() {
        let version = String::from_utf8(output.stdout).ok()?;
        Some(version.trim().to_string())
    } else {
        None
    }
}