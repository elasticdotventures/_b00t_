//! SP3-08 — route `<svc>__<tool>` calls to `https://<svc>.<base-domain>/mcp`
//! and forward the caller's JWT. Provider-agnostic: the target host is derived
//! purely from the service name + base domain, never a hosting provider.
//!
//! A non-namespaced tool name (`no __`) yields `route() == None` — the caller
//! then dispatches it against the local registry, unchanged.

use anyhow::{Context, Result};
use serde_json::{Value, json};

/// The default base domain for on-demand MCP services.
pub const DEFAULT_BASE_DOMAIN: &str = "b00t.promptexecution.com";

/// A resolved remote route.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RemoteRoute {
    /// Service name (the part before `__`).
    pub svc: String,
    /// Tool name as the remote server knows it (the part after `__`).
    pub bare_tool: String,
    /// `https://<svc>.<base-domain>/mcp` (or the `_BASE_URL` override + `/mcp`).
    pub url: String,
}

/// Routes namespaced tool calls to per-service MCP endpoints.
pub struct RemoteMcpProxy {
    base_domain: String,
    /// Full override for tests: when set, every route's URL is `<base_url>/mcp`.
    base_url_override: Option<String>,
    client: reqwest::Client,
}

impl Default for RemoteMcpProxy {
    fn default() -> Self {
        Self::from_env()
    }
}

impl RemoteMcpProxy {
    /// `$B00T_MCP_ROUTING_DOMAIN` (default `b00t.promptexecution.com`) +
    /// `$B00T_MCP_ROUTING_BASE_URL` (test override — forces every route host).
    pub fn from_env() -> Self {
        Self {
            base_domain: std::env::var("B00T_MCP_ROUTING_DOMAIN")
                .unwrap_or_else(|_| DEFAULT_BASE_DOMAIN.to_string()),
            base_url_override: std::env::var("B00T_MCP_ROUTING_BASE_URL").ok(),
            client: reqwest::Client::new(),
        }
    }

    pub fn with_base_domain(domain: impl Into<String>) -> Self {
        Self {
            base_domain: domain.into(),
            base_url_override: None,
            client: reqwest::Client::new(),
        }
    }

    pub fn with_base_url(base_url: impl Into<String>) -> Self {
        Self {
            base_domain: DEFAULT_BASE_DOMAIN.to_string(),
            base_url_override: Some(base_url.into()),
            client: reqwest::Client::new(),
        }
    }

    /// `Some(route)` for a `<svc>__<tool>` name; `None` for a bare name.
    pub fn route(&self, tool: &str) -> Option<RemoteRoute> {
        let (svc, bare_tool) = tool.split_once("__")?;
        if svc.is_empty() || bare_tool.is_empty() {
            return None;
        }
        let url = match &self.base_url_override {
            Some(base) => format!("{}/mcp", base.trim_end_matches('/')),
            None => format!("https://{svc}.{}/mcp", self.base_domain),
        };
        Some(RemoteRoute {
            svc: svc.to_string(),
            bare_tool: bare_tool.to_string(),
            url,
        })
    }

    /// The MCP `tools/call` JSON-RPC body for a routed call.
    pub fn build_call_body(bare_tool: &str, params: &Value) -> Value {
        json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "tools/call",
            "params": { "name": bare_tool, "arguments": params }
        })
    }

    /// Invoke `tool` on its per-service endpoint, forwarding `bearer` (the
    /// caller's JWT) as `Authorization: Bearer`. Returns the JSON-RPC `result`.
    pub async fn call(&self, tool: &str, params: &Value, bearer: Option<&str>) -> Result<Value> {
        let route = self
            .route(tool)
            .with_context(|| format!("'{tool}' is not a routable <svc>__<tool> name"))?;
        let body = Self::build_call_body(&route.bare_tool, params);

        let mut req = self
            .client
            .post(&route.url)
            .header("content-type", "application/json")
            .header("accept", "application/json, text/event-stream")
            .json(&body);
        if let Some(b) = bearer {
            req = req.header("authorization", format!("Bearer {b}"));
        }

        let res = req.send().await.with_context(|| format!("POST {}", route.url))?;
        let status = res.status();
        let text = res.text().await.unwrap_or_default();
        if !status.is_success() {
            anyhow::bail!("{} -> {status}: {text}", route.url);
        }
        let parsed: Value = serde_json::from_str(&text).context("parse JSON-RPC response")?;
        if let Some(err) = parsed.get("error") {
            anyhow::bail!("remote MCP error: {err}");
        }
        Ok(parsed.get("result").cloned().unwrap_or(Value::Null))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    use tokio::net::TcpListener;

    #[test]
    fn route_parses_namespaced_names_only() {
        let p = RemoteMcpProxy::with_base_domain("b00t.promptexecution.com");
        let r = p.route("gh__list_repos").unwrap();
        assert_eq!(r.svc, "gh");
        assert_eq!(r.bare_tool, "list_repos");
        assert_eq!(r.url, "https://gh.b00t.promptexecution.com/mcp");

        assert!(p.route("b00t_status").is_none()); // bare name -> local
        assert!(p.route("__x").is_none());
        assert!(p.route("x__").is_none());
    }

    #[test]
    fn base_url_override_wins() {
        let p = RemoteMcpProxy::with_base_url("http://127.0.0.1:9999");
        assert_eq!(
            p.route("gh__x").unwrap().url,
            "http://127.0.0.1:9999/mcp"
        );
    }

    #[test]
    fn call_body_shape() {
        let b = RemoteMcpProxy::build_call_body("list_repos", &json!({"org":"acme"}));
        assert_eq!(b["method"], "tools/call");
        assert_eq!(b["params"]["name"], "list_repos");
        assert_eq!(b["params"]["arguments"]["org"], "acme");
    }

    #[tokio::test]
    async fn call_forwards_the_bearer_and_returns_result() {
        // minimal raw-HTTP stub — captures the request, returns a canned result
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let server = tokio::spawn(async move {
            let (mut sock, _) = listener.accept().await.unwrap();
            let mut buf = vec![0u8; 4096];
            let n = sock.read(&mut buf).await.unwrap();
            let reqtxt = String::from_utf8_lossy(&buf[..n]).to_string();
            let resp_body = r#"{"jsonrpc":"2.0","id":1,"result":{"ok":true}}"#;
            let resp = format!(
                "HTTP/1.1 200 OK\r\ncontent-type: application/json\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{}",
                resp_body.len(),
                resp_body
            );
            sock.write_all(resp.as_bytes()).await.unwrap();
            sock.flush().await.unwrap();
            reqtxt
        });

        let proxy = RemoteMcpProxy::with_base_url(format!("http://{addr}"));
        let result = proxy
            .call("gh__list_repos", &json!({"org": "acme"}), Some("test.jwt.token"))
            .await
            .unwrap();
        assert_eq!(result, json!({"ok": true}));

        let reqtxt = server.await.unwrap();
        assert!(reqtxt.contains("POST /mcp HTTP/1.1"));
        assert!(reqtxt.to_lowercase().contains("authorization: bearer test.jwt.token"));
        assert!(reqtxt.contains("\"name\":\"list_repos\""));
    }
}
