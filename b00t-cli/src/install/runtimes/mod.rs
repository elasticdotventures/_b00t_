pub mod claude;
pub mod gemini;
pub mod codex;
pub mod opencode;
pub mod copilot;

pub use claude::ClaudeAdapter;
pub use gemini::GeminiAdapter;
pub use codex::CodexAdapter;
pub use opencode::OpenCodeAdapter;
pub use copilot::CopilotAdapter;

use anyhow::Result;
use std::path::PathBuf;

/// Return the user's home directory, or an error naming the runtime that requires it.
pub(super) fn require_home_dir(runtime_name: &str) -> Result<PathBuf> {
    dirs::home_dir().ok_or_else(|| {
        anyhow::anyhow!(
            "Cannot determine home directory; unable to resolve {} config path",
            runtime_name
        )
    })
}
