//! b00t-lsp: Language Server Protocol for b00t datum configuration
//!
//! 用法:
//!   b00t-lsp --stdio
//!
//! VS Code settings.json:
//! ```json
//! {
//!   "languages": [{"id": "toml", "extensions": [".toml", ".tomllm", ".tomllmd"]}],
//!   "server": {"command": "b00t-lsp", "args": ["--stdio"]}
//! }
//! ```

use anyhow::Result;
use b00t_c0re_lib::datum_lsp::run_lsp_server;

#[tokio::main]
async fn main() -> Result<()> {
    eprintln!("🤖 b00t-lsp: Datum Language Server");
    eprintln!("📍 Listening on stdio...");

    run_lsp_server().await?;

    Ok(())
}
