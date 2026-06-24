//! RPA CDP bridge — connect to Windows Chrome from WSL via chromiumoxide.
//!
//! Uses [`chromiumoxide`](https://github.com/mattsse/chromiumoxide) as the CDP client.

use anyhow::{Context, Result};
use chromiumoxide::js::EvaluationResult;
use chromiumoxide::{Browser, Element, Handler, Page};
use futures::StreamExt;

/// Discover the Windows host IP from inside WSL2.
pub fn windows_host_ip() -> String {
    if let Ok(output) = std::process::Command::new("ip")
        .args(["route", "show", "default"])
        .output()
    {
        let stdout = String::from_utf8_lossy(&output.stdout);
        if let Some(ip) = stdout
            .lines()
            .next()
            .and_then(|l| l.split_whitespace().nth(2))
            .map(|s| s.to_string())
        {
            return ip;
        }
    }
    "127.0.0.1".to_string()
}

/// Connected browser session.
pub struct RpaSession {
    browser: Browser,
    _handler: tokio::task::JoinHandle<()>,
}

impl RpaSession {
    /// Connect to a Windows Chrome instance via CDP.
    pub async fn connect(host: Option<String>, port: u16) -> Result<Self> {
        let host = host.unwrap_or_else(windows_host_ip);
        let ws_url = format!("ws://{}:{}", host, port);

        eprintln!("🔌 Connecting to Chrome at {} ...", ws_url);

        let (browser, mut handler): (Browser, Handler) = Browser::connect(&ws_url)
            .await
            .context(format!(
                "Failed to connect to Chrome at {}. \
                 Make sure Chrome is running with --remote-debugging-port={} on Windows host.",
                ws_url, port
            ))?;

        let handle = tokio::spawn(async move {
            while let Some(_event) = handler.next().await {
                // Process CDP events (keeps connection alive)
            }
        });

        Ok(RpaSession { browser, _handler: handle })
    }

    /// List all open page targets with their titles and URLs.
    pub async fn list_pages(&self) -> Result<Vec<(String, String)>> {
        let pages = self.browser.pages().await?;
        let mut result = Vec::new();
        for page in &pages {
            let title = page.get_title().await?.unwrap_or_else(|| "?".to_string());
            let url = page.url().await?.unwrap_or_else(|| "?".to_string());
            result.push((title, url));
        }
        Ok(result)
    }

    /// Open a new page and navigate to URL.
    pub async fn open_page(&self, url: &str) -> Result<Page> {
        let page = self.browser.new_page(url).await?;
        Ok(page)
    }

    /// Execute JavaScript in a page context and return the result as a string.
    pub async fn evaluate(&self, page: &Page, js: &str) -> Result<String> {
        let result: EvaluationResult = page.evaluate(js).await?;
        let value: serde_json::Value = result.into_value()?;
        Ok(value.to_string())
    }

    /// Find an element by CSS selector and click it.
    pub async fn click(&self, page: &Page, selector: &str) -> Result<()> {
        page.find_element(selector).await?.click().await?;
        Ok(())
    }

    /// Type text into an input element.
    pub async fn type_text(&self, page: &Page, selector: &str, text: &str) -> Result<()> {
        let elem = page.find_element(selector).await?;
        elem.click().await?;
        elem.type_str(text).await?;
        Ok(())
    }

    /// Get full page DOM as text (innerText of body).
    pub async fn get_page_text(&self, page: &Page) -> Result<String> {
        let result: EvaluationResult = page.evaluate("document.body.innerText").await?;
        let value: serde_json::Value = result.into_value()?;
        Ok(value.to_string())
    }
}
