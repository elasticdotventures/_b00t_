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

/// Resolve the CDP WebSocket URL by probing known endpoints.
/// Rewrites the WS URL to use the probe host:port (Chrome only binds 127.0.0.1,
/// relay/proxy may be on a different interface).
async fn resolve_ws_url(probe_host: &str, probe_port: u16) -> Result<String> {
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(3))
        .build()?;
    let targets = [
        format!("http://{}:{}/json/version", probe_host, probe_port),
        format!("http://localhost:{}/json/version", probe_port),
    ];
    for target in &targets {
        if let Ok(resp) = client.get(target).send().await {
            if let Ok(json) = resp.json::<serde_json::Value>().await {
                if let Some(ws) = json["webSocketDebuggerUrl"].as_str() {
                    eprintln!("  ✅ CDP endpoint: {}", target);
                    // Rebase Chrome's WS URL path onto our probe host:port
                    let path_idx = ws
                        .match_indices('/')
                        .nth(2)
                        .map(|(i, _)| i + 1)
                        .unwrap_or(0);
                    let path = &ws[path_idx..];
                    let rewritten = format!("ws://{}:{}/{}", probe_host, probe_port, path);
                    return Ok(rewritten);
                }
            }
        }
    }
    // Fallback: construct URL directly
    Ok(format!("ws://{}:{}", probe_host, probe_port))
}

impl RpaSession {
    /// Connect to a Windows Chrome instance via CDP.
    pub async fn connect(host: Option<String>, port: u16) -> Result<Self> {
        let host = host.unwrap_or_else(windows_host_ip);
        let ws_url = resolve_ws_url(&host, port).await?;

        eprintln!("🔌 Connecting to Chrome at {} ...", ws_url);

        let (browser, mut handler): (Browser, Handler) =
            Browser::connect(&ws_url).await.context(format!(
                "Failed to connect to Chrome at {}. \
                 Make sure Chrome is running with --remote-debugging-port={} on Windows host.",
                ws_url, port
            ))?;

        let handle = tokio::spawn(async move {
            while let Some(_event) = handler.next().await {
                // Process CDP events (keeps connection alive)
            }
        });

        Ok(RpaSession {
            browser,
            _handler: handle,
        })
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
    /// Optionally inject DOM enrichment (opt-in, off by default).
    /// Enrichment adds a single `data-b00t` attribute with compact format:
    ///   data-b00t="type|label"  (e.g., "button|Sign In", "input|Search")
    /// This is capped at 500 elements and viewport-only to avoid memory issues.
    pub async fn open_page(&self, url: &str, enrich: bool) -> Result<Page> {
        use tokio::time::{Duration, timeout};
        let page = timeout(Duration::from_secs(15), self.browser.new_page(url))
            .await
            .map_err(|_| anyhow::anyhow!("Timeout opening page: {}", url))??;
        if enrich {
            let _ = page.evaluate(ENRICH_SCRIPT).await;
        }
        Ok(page)
    }
    /// Alias without enrich parameter for backward compat
    pub async fn open_page_no_enrich(&self, url: &str) -> Result<Page> {
        self.open_page(url, false).await
    }
}

/// Lightweight DOM enrichment script — single `data-b00t` attribute.
/// Injects a compact interactive-element index into the page for LLM discovery.
/// The LLM can query ALL interactive elements with one call:
///   JSON.stringify(window.__b00t.findAll())
///   → [{id:1, t:"button", l:"Sign In"}, {id:2, t:"input", l:"Search"}, ...]
///
/// Memory-safe: capped at 500 elements, viewport-only, no MutationObserver.
const ENRICH_SCRIPT: &str = r#"
(function(){if(window.__b00t)return;
var MAX=500,sel='a[href],button,input,textarea,select,[role="button"],[role="dialog"]';
var els=document.querySelectorAll(sel);
var reg=[];
for(var i=0;i<els.length&&i<MAX;i++){
 var e=els[i];
 if(!e.offsetParent)continue; // skip hidden
 var l=e.getAttribute('aria-label')||e.getAttribute('placeholder')||(e.textContent||'').trim().slice(0,60)||e.getAttribute('name')||'';
 var t=e.tagName.toLowerCase();
 if(t==='input'){var it=e.getAttribute('type')||'text';t='input-'+it;}
 else if(t==='a')t='link';
 else if(t==='button'||e.getAttribute('role')==='button')t='button';
 else if(t==='textarea')t='input';
 else if(t==='select')t='select';
 e.setAttribute('data-b00t',t+'|'+l.slice(0,80));
 reg.push({id:i+1,t:t,l:l.slice(0,120)});
}
window.__b00t={
 findAll:function(){return reg;},
 find:function(type){return reg.filter(function(r){return r.t===type||r.t.startsWith(type);});},
 count:function(){var c={};reg.forEach(function(r){c[r.t]=(c[r.t]||0)+1;});return c;}
};
})();
"#;

impl RpaSession {
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

    /// Take a screenshot of the current page. Returns PNG bytes.
    pub async fn screenshot(&self, page: &Page) -> Result<Vec<u8>> {
        use chromiumoxide::page::ScreenshotParams;
        let params = ScreenshotParams::builder().build(); // PNG is the default format
        let bytes = page.screenshot(params).await?;
        Ok(bytes)
    }
}
