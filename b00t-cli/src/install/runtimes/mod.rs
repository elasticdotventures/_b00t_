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
