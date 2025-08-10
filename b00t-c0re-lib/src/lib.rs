//! # b00t-c0re-lib
//! 
//! Core shared library for b00t ecosystem providing:
//! - Template rendering with b00t context variables
//! - Common data structures and utilities
//! - Shared functionality between b00t-cli and b00t-mcp
//! - Single source of truth for version management

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Version management - single source of truth for the b00t ecosystem
pub mod version {
    /// The current version of the b00t ecosystem
    /// 🤓 This is the SINGLE SOURCE OF TRUTH - all other crates reference this
    pub const VERSION: &str = env!("CARGO_PKG_VERSION");
    
    /// Get the current b00t ecosystem version
    pub fn get() -> &'static str {
        VERSION
    }
    
    /// Check if we're running a development/git build
    pub fn is_dev_build() -> bool {
        VERSION.contains("git") || VERSION.contains("dev")
    }
}

pub mod template;
pub mod context;
pub mod utils;
pub mod redis;
pub mod agent_coordination;
pub mod grok;
pub mod learn;
pub mod datum_ai_model;
pub mod lfmf;

// Re-export commonly used types
pub use template::TemplateRenderer;
pub use context::B00tContext;
pub use grok::{GrokClient, DigestResult, AskResult, LearnResult, ChunkResult, ChunkSummary};
pub use lfmf::{LfmfSystem, LfmfConfig, Lesson};

/// Common configuration structure for b00t components
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct B00tConfig {
    pub name: String,
    pub version: Option<String>,
    pub description: Option<String>,
    pub env: Option<HashMap<String, String>>,
}

/// Result type alias for b00t operations
pub type B00tResult<T> = Result<T, anyhow::Error>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_version_available() {
        assert!(!version::VERSION.is_empty());
    }
}