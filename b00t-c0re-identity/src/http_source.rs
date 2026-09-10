use anyhow::{Context, Result};
use serde::Deserialize;

use crate::{TokenRequest, TokenSource};

/// Obtains tokens from a deployed b00t-identity worker (`POST {base}/tokens`).
pub struct HttpTokenSource {
    pub base_url: String,
    client: reqwest::Client,
}

impl HttpTokenSource {
    pub fn new(base_url: impl Into<String>) -> Self {
        Self {
            base_url: base_url.into(),
            client: reqwest::Client::new(),
        }
    }
}

#[derive(Deserialize)]
struct TokenResponse {
    token: String,
}

#[async_trait::async_trait]
impl TokenSource for HttpTokenSource {
    async fn obtain(&self, req: &TokenRequest) -> Result<String> {
        let url = format!("{}/tokens", self.base_url.trim_end_matches('/'));
        let res = self
            .client
            .post(&url)
            .json(req)
            .send()
            .await
            .context("POST /tokens")?;
        let status = res.status();
        let body = res.text().await.unwrap_or_default();
        if !status.is_success() {
            anyhow::bail!("token request failed: {status} — {body}");
        }
        let parsed: TokenResponse =
            serde_json::from_str(&body).context("parse token response")?;
        Ok(parsed.token)
    }
}
