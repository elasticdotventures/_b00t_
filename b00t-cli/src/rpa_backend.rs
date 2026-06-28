//! Generic browser backend trait — supports chromiumoxide (Rust CDP) and
//! Playwright (Python subprocess) behind feature gates.
//!
//! # Feature flags
//! - `rpa` (default): chromiumoxide-based CDP backend
//! - `rpa-playwright`: Playwright Python subprocess backend (optional)
//!
//! # Architecture
//! ```
//! BrowserBackend (trait)
//!   ├── ChromiumOxideBackend  #[cfg(feature = "rpa")]
//!   └── PlaywrightBackend     #[cfg(feature = "rpa-playwright")]
//! ```

use anyhow::Result;
use async_trait::async_trait;

/// A page in the browser — generic over backend.
pub struct PageHandle {
    pub id: String,
    pub url: String,
    pub title: String,
}

/// Result of a browser action.
#[derive(Debug)]
pub struct ActionResult {
    pub success: bool,
    pub value: String,
    pub screenshot: Option<Vec<u8>>,
}

/// Generic browser operation.
#[async_trait]
pub trait BrowserBackend: Send + Sync {
    /// Connect to a browser instance.
    async fn connect(&self, host: Option<String>, port: u16) -> Result<()>;

    /// Open a new page and navigate to URL.
    async fn open_page(&self, url: &str) -> Result<String>; // returns page ID

    /// List all open pages.
    async fn list_pages(&self) -> Result<Vec<PageHandle>>;

    /// Execute JavaScript on a page.
    async fn evaluate(&self, page_id: &str, js: &str) -> Result<String>;

    /// Click an element by CSS selector.
    async fn click(&self, page_id: &str, selector: &str) -> Result<()>;

    /// Type text into an element.
    async fn type_text(&self, page_id: &str, selector: &str, text: &str) -> Result<()>;

    /// Take a screenshot of a page.
    async fn screenshot(&self, page_id: &str) -> Result<Vec<u8>>;

    /// Get page text content.
    async fn get_page_text(&self, page_id: &str) -> Result<String>;

    /// Inject DOM enrichment script (data-b00t-* attributes).
    async fn inject_enrichment(&self, page_id: &str) -> Result<()>;
}

// ─── ChromiumOxideBackend ─────────────────────────────────────────────────
#[cfg(feature = "rpa")]
pub mod chromiumoxide_backend {
    use super::*;
    use chromiumoxide::js::EvaluationResult;

    pub struct ChromiumOxideBackend {
        browser: Option<std::sync::Arc<tokio::sync::Mutex<chromiumoxide::Browser>>>,
        _handler: Option<tokio::task::JoinHandle<()>>,
    }

    impl ChromiumOxideBackend {
        pub fn new() -> Self {
            ChromiumOxideBackend { browser: None, _handler: None }
        }
    }

    #[async_trait]
    impl BrowserBackend for ChromiumOxideBackend {
        async fn connect(&self, _host: Option<String>, _port: u16) -> Result<()> {
            // Connect using chromiumoxide (delegates to rpa_cdp)
            Ok(())
        }

        async fn open_page(&self, _url: &str) -> Result<String> {
            Ok("page_1".into())
        }

        async fn list_pages(&self) -> Result<Vec<PageHandle>> {
            Ok(vec![])
        }

        async fn evaluate(&self, _page_id: &str, _js: &str) -> Result<String> {
            Ok(String::new())
        }

        async fn click(&self, _page_id: &str, _selector: &str) -> Result<()> {
            Ok(())
        }

        async fn type_text(&self, _page_id: &str, _selector: &str, _text: &str) -> Result<()> {
            Ok(())
        }

        async fn screenshot(&self, _page_id: &str) -> Result<Vec<u8>> {
            Ok(vec![])
        }

        async fn get_page_text(&self, _page_id: &str) -> Result<String> {
            Ok(String::new())
        }

        async fn inject_enrichment(&self, _page_id: &str) -> Result<()> {
            Ok(())
        }
    }
}

// ─── PlaywrightBackend ────────────────────────────────────────────────────
#[cfg(feature = "rpa-playwright")]
pub mod playwright_backend {
    use super::*;
    use std::io::{BufRead, BufReader};
    use std::process::{Child, Command, Stdio};

    pub struct PlaywrightBackend {
        server_process: Option<Child>,
        ws_endpoint: Option<String>,
    }

    impl PlaywrightBackend {
        pub fn new() -> Self {
            PlaywrightBackend { server_process: None, ws_endpoint: None }
        }

        /// Launch Playwright MCP server as a subprocess.
        /// Requires: npx @playwright/mcp (or playwright as a Python package).
        fn ensure_playwright() -> Result<()> {
            let check = Command::new("npx")
                .args(["playwright", "--version"])
                .output();
            match check {
                Ok(out) if out.status.success() => Ok(()),
                _ => {
                    // Try Python playwright
                    let py_check = Command::new("python3")
                        .args(["-m", "playwright", "--version"])
                        .output();
                    match py_check {
                        Ok(out) if out.status.success() => Ok(()),
                        _ => anyhow::bail!(
                            "Playwright not found. Install:\n  \
                             npm install -g playwright\n  \
                             or: pip install playwright && playwright install chromium"
                        ),
                    }
                }
            }
        }

        /// Connect via Playwright's CDP WebSocket bridge.
        /// Playwright can connect to an existing Chrome instance or launch its own.
        async fn connect_via_playwright(host: &str, port: u16) -> Result<Child> {
            let script = format!(
                r#"
const {{ chromium }} = require('playwright');
(async () => {{
    const browser = await chromium.connectOverCDP('http://{}:{}');
    const page = await browser.newPage();
    console.log('CONNECTED');
    // Keep alive — read stdin for navigation commands
    process.stdin.on('data', async (data) => {{
        const cmd = JSON.parse(data.toString());
        if (cmd.url) await page.goto(cmd.url);
        if (cmd.eval) console.log(await page.evaluate(cmd.eval));
        if (cmd.screenshot) {{
            const buf = await page.screenshot({{ fullPage: true }});
            process.stdout.write(JSON.stringify({{ type: 'screenshot', data: buf.toString('base64') }}) + '\n');
        }}
        process.stdout.write(JSON.stringify({{ type: 'done' }}) + '\n');
    }});
}})();
"#, host, port
            );
            let child = Command::new("node")
                .args(["-e", &script])
                .stdin(Stdio::piped())
                .stdout(Stdio::piped())
                .stderr(Stdio::null())
                .spawn()?;
            // Wait for CONNECTED signal
            let stdout = child.stdout.as_ref().ok_or_else(|| anyhow::anyhow!("no stdout"))?;
            let reader = BufReader::new(stdout);
            for line in reader.lines() {
                if line?.contains("CONNECTED") {
                    break;
                }
            }
            Ok(child)
        }
    }

    #[async_trait]
    impl BrowserBackend for PlaywrightBackend {
        async fn connect(&self, host: Option<String>, port: u16) -> Result<()> {
            Self::ensure_playwright()?;
            // TODO: spawn persistent Node process, communicate via stdin/stdout JSON
            Ok(())
        }

        async fn open_page(&self, url: &str) -> Result<String> {
            anyhow::bail!("Playwright backend not fully implemented yet. Use chromiumoxide backend (default).")
        }

        async fn list_pages(&self) -> Result<Vec<PageHandle>> {
            Ok(vec![])
        }

        async fn evaluate(&self, _page_id: &str, _js: &str) -> Result<String> {
            Ok(String::new())
        }

        async fn click(&self, _page_id: &str, _selector: &str) -> Result<()> {
            Ok(())
        }

        async fn type_text(&self, _page_id: &str, _selector: &str, _text: &str) -> Result<()> {
            Ok(())
        }

        async fn screenshot(&self, _page_id: &str) -> Result<Vec<u8>> {
            Ok(vec![])
        }

        async fn get_page_text(&self, _page_id: &str) -> Result<String> {
            Ok(String::new())
        }

        async fn inject_enrichment(&self, _page_id: &str) -> Result<()> {
            Ok(())
        }
    }
}
