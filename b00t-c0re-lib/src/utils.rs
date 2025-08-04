//! Utility functions shared across b00t ecosystem

use crate::B00tResult;
use anyhow::Context;
use std::path::{Path, PathBuf};

/// Expand shell-style paths (e.g., ~ to home directory)
pub fn expand_path<P: AsRef<Path>>(path: P) -> B00tResult<PathBuf> {
    let path_str = path.as_ref().to_string_lossy();
    let expanded = shellexpand::full(&path_str)
        .with_context(|| format!("Failed to expand path: {}", path_str))?;
    Ok(PathBuf::from(expanded.as_ref()))
}

/// Get the b00t configuration directory (~/.dotfiles/_b00t_)
pub fn get_b00t_config_dir() -> B00tResult<PathBuf> {
    let home = dirs::home_dir()
        .with_context(|| "Could not determine home directory")?;
    Ok(home.join(".dotfiles").join("_b00t_"))
}

/// Check if a command exists in PATH
pub fn command_exists(command: &str) -> bool {
    duct::cmd!("which", command)
        .stderr_null()
        .stdout_null()
        .run()
        .is_ok()
}

/// Execute a shell command and return its output
pub fn run_command(command: &str, args: &[&str]) -> B00tResult<String> {
    let output = duct::cmd(command, args)
        .stderr_to_stdout()
        .stdout_capture()
        .run()
        .with_context(|| format!("Failed to execute command: {} {:?}", command, args))?;
    
    String::from_utf8(output.stdout)
        .with_context(|| "Command output was not valid UTF-8")
}

/// Get git information from current directory
pub fn get_git_info() -> GitInfo {
    GitInfo {
        is_repo: is_git_repository(),
        branch: get_git_branch(),
        remote: get_git_remote(),
    }
}

/// Git repository information
#[derive(Debug, Clone)]
pub struct GitInfo {
    pub is_repo: bool,
    pub branch: String,
    pub remote: Option<String>,
}

fn is_git_repository() -> bool {
    Path::new(".git").exists() ||
    duct::cmd!("git", "rev-parse", "--git-dir")
        .stderr_null()
        .stdout_null()
        .run()
        .is_ok()
}

fn get_git_branch() -> String {
    duct::cmd!("git", "rev-parse", "--abbrev-ref", "HEAD")
        .stderr_null()
        .stdout_capture()
        .run()
        .ok()
        .and_then(|output| String::from_utf8(output.stdout).ok())
        .map(|s| s.trim().to_string())
        .unwrap_or_else(|| "main".to_string())
}

fn get_git_remote() -> Option<String> {
    duct::cmd!("git", "remote", "get-url", "origin")
        .stderr_null()
        .stdout_capture()
        .run()
        .ok()
        .and_then(|output| String::from_utf8(output.stdout).ok())
        .map(|s| s.trim().to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_expand_path() {
        let expanded = expand_path("~/.dotfiles").unwrap();
        assert!(expanded.is_absolute());
        assert!(expanded.to_string_lossy().contains(".dotfiles"));
    }

    #[test]
    fn test_command_exists() {
        // Test with a command that should exist on most systems
        assert!(command_exists("echo"));
        
        // Test with a command that shouldn't exist
        assert!(!command_exists("nonexistent_command_12345"));
    }

    #[test]
    fn test_run_command() {
        let output = run_command("echo", &["hello", "world"]).unwrap();
        assert_eq!(output.trim(), "hello world");
    }

    #[test]
    fn test_get_git_info() {
        let git_info = get_git_info();
        // Just verify it doesn't panic and returns some information
        assert!(!git_info.branch.is_empty());
    }
}