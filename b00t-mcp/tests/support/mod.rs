use serde_json::{Value, json};

/// Stateful test client: preserves the MCP session and completes initialization.
pub struct HttpMcpClient {
    client: reqwest::Client,
    url: String,
    pub jwt: String,
    session: Option<String>,
    next_id: u64,
}

impl HttpMcpClient {
    pub async fn connect(base: &str, jwt: String) -> Self {
        let mut client = Self {
            client: reqwest::Client::builder()
                .timeout(std::time::Duration::from_secs(10))
                .build()
                .unwrap(),
            url: format!("{}/mcp", base.trim_end_matches('/')),
            jwt,
            session: None,
            next_id: 0,
        };
        let init = client
            .call(
                "initialize",
                json!({
                    "protocolVersion": "2025-06-18", "capabilities": {},
                    "clientInfo": { "name": "identity-regression", "version": "0" }
                }),
            )
            .await;
        assert!(init.get("result").is_some(), "initialize failed: {init}");
        client
            .send(json!({"jsonrpc":"2.0", "method":"notifications/initialized"}))
            .await;
        client
    }

    pub async fn call(&mut self, method: &str, params: Value) -> Value {
        self.next_id += 1;
        self.send(json!({"jsonrpc":"2.0", "id":self.next_id, "method":method, "params":params}))
            .await
    }

    async fn send(&mut self, body: Value) -> Value {
        let mut request = self
            .client
            .post(&self.url)
            .bearer_auth(&self.jwt)
            .header("accept", "application/json, text/event-stream")
            .header("mcp-protocol-version", "2025-06-18")
            .json(&body);
        if let Some(session) = &self.session {
            request = request.header("mcp-session-id", session);
        }
        let response = request.send().await.unwrap();
        let status = response.status();
        if let Some(session) = response.headers().get("mcp-session-id") {
            self.session = Some(session.to_str().unwrap().into());
        }
        let text = response.text().await.unwrap();
        assert!(status.is_success(), "HTTP {status}: {text}");
        if text.is_empty() {
            return Value::Null;
        }
        serde_json::from_str(&text).unwrap_or_else(|_| {
            text.lines()
                .filter_map(|line| line.strip_prefix("data: "))
                .find_map(|line| serde_json::from_str(line).ok())
                .unwrap_or_else(|| panic!("no JSON in MCP response: {text}"))
        })
    }
}
