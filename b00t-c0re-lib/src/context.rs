//! B00t context information gathering
//! 
//! Provides functionality to gather system and environment context
//! for template rendering and agent operation.

use crate::B00tResult;
use chrono::Utc;
use serde::{Deserialize, Serialize};
use std::process;

/// Context information for b00t operations
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct B00tContext {
    pub pid: u32,
    pub timestamp: String,
    pub user: String,
    pub branch: String,
    pub agent: String,
    pub model_size: String,
    pub privacy: String,
    pub workspace_root: String,
    pub is_git_repo: bool,
    pub hostname: String,
}

impl B00tContext {
    /// Create context with current system information
    pub fn current() -> B00tResult<Self> {
        Ok(Self {
            pid: process::id(),
            timestamp: Utc::now().format("%Y-%m-%d %H:%M:%S UTC").to_string(),
            user: Self::get_username(),
            branch: Self::get_git_branch(),
            agent: Self::get_agent_name(),
            model_size: Self::get_model_size(),
            privacy: Self::get_privacy_setting(),
            workspace_root: Self::get_workspace_root(),
            is_git_repo: Self::is_git_repository(),
            hostname: Self::get_hostname(),
        })
    }

    /// Get current username
    fn get_username() -> String {
        std::env::var("USER")
            .or_else(|_| std::env::var("USERNAME"))
            .or_else(|_| std::env::var("LOGNAME"))
            .unwrap_or_else(|_| "unknown".to_string())
    }

    /// Get current git branch
    fn get_git_branch() -> String {
        // Try to get git branch using git command
        if let Ok(output) = duct::cmd!("git", "rev-parse", "--abbrev-ref", "HEAD")
            .stderr_null()
            .stdout_capture()
            .run()
        {
            if let Ok(branch) = String::from_utf8(output.stdout) {
                return branch.trim().to_string();
            }
        }
        
        // Fallback to main/master
        "main".to_string()
    }

    /// Get agent identifier using proper detection logic
    fn get_agent_name() -> String {
        // Check if _B00T_Agent is already set
        if let Ok(agent) = std::env::var("_B00T_Agent") {
            if !agent.is_empty() {
                return agent;
            }
        }
        
        // Check for Claude Code
        if std::env::var("CLAUDECODE").unwrap_or_default() == "1" {
            return "🤖 Claude Code".to_string();
        }
        
        // Check other environment variables
        std::env::var("_B00T_AGENT")
            .or_else(|_| std::env::var("B00T_AGENT"))
            .unwrap_or_else(|_| "🤖 Claude Code".to_string())
    }

    /// Get model size identifier  
    fn get_model_size() -> String {
        std::env::var("MODEL_SIZE")
            .or_else(|_| std::env::var("B00T_MODEL_SIZE"))
            .unwrap_or_else(|_| "unknown".to_string())
    }

    /// Get privacy setting
    fn get_privacy_setting() -> String {
        std::env::var("PRIVACY")
            .or_else(|_| std::env::var("B00T_PRIVACY"))
            .unwrap_or_else(|_| "standard".to_string())
    }

    /// Get workspace root directory
    fn get_workspace_root() -> String {
        std::env::current_dir()
            .map(|p| p.display().to_string())
            .unwrap_or_else(|_| "/tmp".to_string())
    }

    /// Check if current directory is a git repository
    fn is_git_repository() -> bool {
        std::path::Path::new(".git").exists() ||
        duct::cmd!("git", "rev-parse", "--git-dir")
            .stderr_null()
            .stdout_null()
            .run()
            .is_ok()
    }

    /// Get system hostname
    fn get_hostname() -> String {
        std::env::var("HOSTNAME")
            .or_else(|_| {
                // Try to get hostname using hostname command
                duct::cmd!("hostname")
                    .stderr_null()
                    .stdout_capture()
                    .run()
                    .ok()
                    .and_then(|output| String::from_utf8(output.stdout).ok())
                    .map(|s| s.trim().to_string())
                    .ok_or(std::env::VarError::NotPresent)
            })
            .unwrap_or_else(|_| "localhost".to_string())
    }

    /// Create a context with custom values (for testing)
    pub fn with_values(
        pid: u32,
        timestamp: String,
        user: String,
        branch: String,
        agent: String,
        model_size: String,
        privacy: String,
        workspace_root: String,
        is_git_repo: bool,
        hostname: String,
    ) -> Self {
        Self {
            pid,
            timestamp,
            user,
            branch,
            agent,
            model_size,
            privacy,
            workspace_root,
            is_git_repo,
            hostname,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_current_context_creation() {
        let context = B00tContext::current().unwrap();
        
        // Basic validation
        assert!(context.pid > 0);
        assert!(!context.timestamp.is_empty());
        assert!(!context.user.is_empty());
        assert!(!context.workspace_root.is_empty());
        assert!(!context.hostname.is_empty());
    }

    #[test]
    fn test_custom_context_creation() {
        let context = B00tContext::with_values(
            12345,
            "2024-01-01".to_string(),
            "testuser".to_string(),
            "feature/test".to_string(),
            "TestAgent".to_string(),
            "large".to_string(),
            "private".to_string(),
            "/test/workspace".to_string(),
            true,
            "testhost".to_string(),
        );

        assert_eq!(context.pid, 12345);
        assert_eq!(context.user, "testuser");
        assert_eq!(context.branch, "feature/test");
        assert!(context.is_git_repo);
    }
}