//! SP3-08 — route `<svc>__<tool>` calls to `https://<svc>.<base-domain>/mcp`
//! and forward the caller's JWT. Provider-agnostic: the target host is derived
//! purely from the service name + base domain, never a hosting provider.
//!
//! A non-namespaced tool name (`no __`) yields `route() == None` — the caller
//! then dispatches it against the local registry, unchanged.
//!
//! SP4-09 — when `$B00T_MCP_CONTROL_URL` is set, [`RemoteMcpProxy::call`] first
//! asks that control plane (`GET {control}/_b00t/route/<svc>`) where `<svc>`
//! actually lives — it may need to wake a scaled-to-zero backend — and uses the
//! returned `fqdn`. Unset → straight-through to `<svc>.<base-domain>` (today's
//! behaviour).

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
    /// Full override for tests: when set, every route's URL is `<base_url>/mcp`
    /// and the control plane is skipped.
    base_url_override: Option<String>,
    /// SP4-09 — `$B00T_MCP_CONTROL_URL`. When set (and no `base_url_override`),
    /// `call` resolves the live host via `GET {this}/_b00t/route/<svc>`.
    control_plane_url: Option<String>,
    client: reqwest::Client,
}

impl Default for RemoteMcpProxy {
    fn default() -> Self {
        Self::from_env()
    }
}

impl RemoteMcpProxy {
    /// `$B00T_MCP_ROUTING_DOMAIN` (default `b00t.promptexecution.com`) +
    /// `$B00T_MCP_ROUTING_BASE_URL` (test override — forces every route host) +
    /// `$B00T_MCP_CONTROL_URL` (SP4-09 — control-plane resolve).
    pub fn from_env() -> Self {
        Self {
            base_domain: std::env::var("B00T_MCP_ROUTING_DOMAIN")
                .unwrap_or_else(|_| DEFAULT_BASE_DOMAIN.to_string()),
            base_url_override: std::env::var("B00T_MCP_ROUTING_BASE_URL").ok(),
            control_plane_url: std::env::var("B00T_MCP_CONTROL_URL").ok(),
            client: reqwest::Client::new(),
        }
    }

    pub fn with_base_domain(domain: impl Into<String>) -> Self {
        Self {
            base_domain: domain.into(),
            base_url_override: None,
            control_plane_url: None,
            client: reqwest::Client::new(),
        }
    }

    pub fn with_base_url(base_url: impl Into<String>) -> Self {
        Self {
            base_domain: DEFAULT_BASE_DOMAIN.to_string(),
            base_url_override: Some(base_url.into()),
            control_plane_url: None,
            client: reqwest::Client::new(),
        }
    }

    /// SP4-09 — route via a control plane at `control_url`.
    pub fn with_control_plane(control_url: impl Into<String>) -> Self {
        Self {
            base_domain: DEFAULT_BASE_DOMAIN.to_string(),
            base_url_override: None,
            control_plane_url: Some(control_url.into()),
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

    /// Where `route.svc` actually is right now.
    ///
    /// * `base_url_override` set → the override (control plane skipped).
    /// * `control_plane_url` set → `GET {control}/_b00t/route/<svc>`, use `fqdn`
    ///   (waking a cold backend if needed); `503` → the launch was refused.
    /// * neither → `route.url` (SP3-08 straight-through).
    async fn resolve_url(&self, route: &RemoteRoute) -> Result<String> {
        if self.base_url_override.is_some() {
            return Ok(route.url.clone());
        }
        let Some(control) = &self.control_plane_url else {
            return Ok(route.url.clone());
        };

        let ctl_url = format!(
            "{}/_b00t/route/{}",
            control.trim_end_matches('/'),
            route.svc
        );
        let res = self
            .client
            .get(&ctl_url)
            .send()
            .await
            .with_context(|| format!("GET {ctl_url}"))?;
        let status = res.status();
        let text = res.text().await.unwrap_or_default();
        if status.as_u16() == 503 {
            anyhow::bail!("control plane refused to launch '{}': {text}", route.svc);
        }
        if !status.is_success() {
            anyhow::bail!("control plane {ctl_url} -> {status}: {text}");
        }
        let v: Value =
            serde_json::from_str(&text).context("parse control-plane route response")?;
        let fqdn = v
            .get("fqdn")
            .and_then(|f| f.as_str())
            .filter(|s| !s.is_empty())
            .with_context(|| {
                format!("control-plane response for '{}' has no fqdn: {text}", route.svc)
            })?;
        let base = if fqdn.starts_with("http://") || fqdn.starts_with("https://") {
            fqdn.to_string()
        } else {
            format!("https://{fqdn}")
        };
        Ok(format!("{}/mcp", base.trim_end_matches('/')))
    }

    /// Invoke `tool` on its per-service endpoint, forwarding `bearer` (the
    /// caller's JWT) as `Authorization: Bearer`. Returns the JSON-RPC `result`.
    pub async fn call(&self, tool: &str, params: &Value, bearer: Option<&str>) -> Result<Value> {
        let route = self
            .route(tool)
            .with_context(|| format!("'{tool}' is not a routable <svc>__<tool> name"))?;
        let target = self.resolve_url(&route).await?;
        let body = Self::build_call_body(&route.bare_tool, params);

        let mut req = self
            .client
            .post(&target)
            .header("content-type", "application/json")
            .header("accept", "application/json, text/event-stream")
            .json(&body);
        if let Some(b) = bearer {
            req = req.header("authorization", format!("Bearer {b}"));
        }

        let res = req.send().await.with_context(|| format!("POST {target}"))?;
        let status = res.status();
        let text = res.text().await.unwrap_or_default();
        if !status.is_success() {
            anyhow::bail!("{target} -> {status}: {text}");
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
        assert_eq!(p.route("gh__x").unwrap().url, "http://127.0.0.1:9999/mcp");
    }

    #[test]
    fn call_body_shape() {
        let b = RemoteMcpProxy::build_call_body("list_repos", &json!({"org":"acme"}));
        assert_eq!(b["method"], "tools/call");
        assert_eq!(b["params"]["name"], "list_repos");
        assert_eq!(b["params"]["arguments"]["org"], "acme");
    }

    #[tokio::test]
    async fn resolve_url_is_straight_through_without_a_control_plane() {
        let p = RemoteMcpProxy::with_base_domain("b00t.promptexecution.com");
        let route = p.route("gh__x").unwrap();
        assert_eq!(
            p.resolve_url(&route).await.unwrap(),
            "https://gh.b00t.promptexecution.com/mcp"
        );
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

    /// CP-5 — with a control plane, `call` does one `route/<svc>` round-trip and
    /// then hits the FQDN the control plane returned.
    #[tokio::test]
    async fn call_resolves_via_the_control_plane_first() {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();

        let server = tokio::spawn(async move {
            // conn 1: the control-plane GET /_b00t/route/gh
            let (mut sock, _) = listener.accept().await.unwrap();
            let mut buf = vec![0u8; 4096];
            let n = sock.read(&mut buf).await.unwrap();
            let ctl_req = String::from_utf8_lossy(&buf[..n]).to_string();
            let body = format!(r#"{{"fqdn":"http://{addr}","warm":true}}"#);
            let resp = format!(
                "HTTP/1.1 200 OK\r\ncontent-type: application/json\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{}",
                body.len(),
                body
            );
            sock.write_all(resp.as_bytes()).await.unwrap();
            sock.flush().await.unwrap();

            // conn 2: the actual MCP POST /mcp
            let (mut sock, _) = listener.accept().await.unwrap();
            let n = sock.read(&mut buf).await.unwrap();
            let mcp_req = String::from_utf8_lossy(&buf[..n]).to_string();
            let rb = r#"{"jsonrpc":"2.0","id":1,"result":{"ok":true}}"#;
            let resp = format!(
                "HTTP/1.1 200 OK\r\ncontent-type: application/json\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{}",
                rb.len(),
                rb
            );
            sock.write_all(resp.as_bytes()).await.unwrap();
            sock.flush().await.unwrap();
            (ctl_req, mcp_req)
        });

        let proxy = RemoteMcpProxy::with_control_plane(format!("http://{addr}"));
        let result = proxy
            .call("gh__list_repos", &json!({}), Some("jwt"))
            .await
            .unwrap();
        assert_eq!(result, json!({"ok": true}));

        let (ctl_req, mcp_req) = server.await.unwrap();
        assert!(ctl_req.contains("GET /_b00t/route/gh HTTP/1.1"));
        assert!(mcp_req.contains("POST /mcp HTTP/1.1"));
        assert!(mcp_req.to_lowercase().contains("authorization: bearer jwt"));
    }

    /// A `503` from the control plane (budget denied) fails the call.
    #[tokio::test]
    async fn control_plane_503_is_surfaced() {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let server = tokio::spawn(async move {
            let (mut sock, _) = listener.accept().await.unwrap();
            let mut buf = vec![0u8; 4096];
            let _ = sock.read(&mut buf).await.unwrap();
            let body = r#"{"error":"budget_exceeded"}"#;
            let resp = format!(
                "HTTP/1.1 503 Service Unavailable\r\ncontent-type: application/json\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{}",
                body.len(),
                body
            );
            sock.write_all(resp.as_bytes()).await.unwrap();
            sock.flush().await.unwrap();
        });

        let proxy = RemoteMcpProxy::with_control_plane(format!("http://{addr}"));
        let err = proxy
            .call("gh__list_repos", &json!({}), None)
            .await
            .unwrap_err()
            .to_string();
        assert!(err.contains("budget_exceeded"), "unexpected error: {err}");
        server.await.unwrap();
    }
}
