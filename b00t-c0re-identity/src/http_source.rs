use anyhow::{Context, Result};
use serde::Deserialize;

use crate::{TokenRequest, TokenSource};

/// Obtains tokens from a deployed b00t-identity worker (`POST {base}/identity/tokens`).
pub struct HttpTokenSource {
    pub base_url: String,
    client: reqwest::Client,
    issuance_credential: Option<String>,
}

impl HttpTokenSource {
    pub fn new(base_url: impl Into<String>) -> Self {
        Self {
            base_url: base_url.into(),
            client: reqwest::Client::new(),
            issuance_credential: None,
        }
    }

    /// Registry issuance credential, distinct from the agent JWT being requested.
    pub fn with_issuance_credential(mut self, credential: impl Into<String>) -> Self {
        self.issuance_credential = Some(credential.into());
        self
    }
}

#[derive(Deserialize)]
struct TokenResponse {
    token: String,
}

#[async_trait::async_trait]
impl TokenSource for HttpTokenSource {
    async fn obtain(&self, req: &TokenRequest) -> Result<String> {
        let credential = self
            .issuance_credential
            .as_deref()
            .filter(|s| !s.trim().is_empty())
            .context("identity issuance credential is required")?;
        let url = format!("{}/identity/tokens", self.base_url.trim_end_matches('/'));
        let res = self
            .client
            .post(&url)
            .bearer_auth(credential)
            .json(req)
            .send()
            .await
            .context("POST /identity/tokens")?;
        let status = res.status();
        let body = res.text().await.unwrap_or_default();
        if !status.is_success() {
            anyhow::bail!("token request failed: {status} — {body}");
        }
        let parsed: TokenResponse = serde_json::from_str(&body).context("parse token response")?;
        Ok(parsed.token)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::io::{AsyncReadExt, AsyncWriteExt};

    fn request() -> TokenRequest {
        serde_json::from_str(include_str!("../tests/fixtures/token-request.json")).unwrap()
    }

    #[tokio::test]
    async fn requires_an_explicit_issuance_credential() {
        let error = HttpTokenSource::new("http://127.0.0.1:1")
            .obtain(&request())
            .await
            .unwrap_err();
        assert!(error.to_string().contains("issuance credential"));
    }

    #[tokio::test]
    async fn sends_issuance_bearer_to_the_deployed_token_route() {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let server = tokio::spawn(async move {
            let (mut stream, _) = listener.accept().await.unwrap();
            let mut received = Vec::new();
            loop {
                let mut buffer = [0; 4096];
                let n = stream.read(&mut buffer).await.unwrap();
                assert!(n > 0);
                received.extend_from_slice(&buffer[..n]);
                if received.windows(4).any(|w| w == b"\r\n\r\n") {
                    break;
                }
            }
            let headers = String::from_utf8(received).unwrap().to_ascii_lowercase();
            assert!(headers.starts_with("post /identity/tokens http/1.1\r\n"));
            assert!(headers.contains("authorization: bearer test-issuance\r\n"));
            stream.write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 16\r\nConnection: close\r\n\r\n{\"token\":\"test\"}").await.unwrap();
        });
        let source = HttpTokenSource::new(format!("http://{address}/"))
            .with_issuance_credential("test-issuance");
        assert_eq!(source.obtain(&request()).await.unwrap(), "test");
        server.await.unwrap();
    }
}
