//! b00t → ledgrrr client (F2). b00t calls ledgrrr ONLY to authorize spend and
//! record usage; everything is fail-closed in prod. `LedgrrrMode::Mock`
//! short-circuits to an always-ok response so SP1–3 never block on ledgrrr.
//!
//! Mirrors `workers/b00t-identity/src/ledgrrr.ts` — same routes, same bodies,
//! same fail-closed semantics. Contract: `docs/schemas/ledgrrr-v1.json`.

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

pub const MOCK_BUDGET: u64 = 1_000_000;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LedgrrrMode {
    Mock,
    Http,
}

impl LedgrrrMode {
    /// `$B00T_LEDGRRR_MODE` — `http` → Http, anything else → Mock.
    pub fn from_env() -> Self {
        match std::env::var("B00T_LEDGRRR_MODE").as_deref() {
            Ok("http") => LedgrrrMode::Http,
            _ => LedgrrrMode::Mock,
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct AuthorizeSpend<'a> {
    pub tenant: &'a str,
    pub agent: &'a str,
    pub cost: u64,
    pub r#ref: &'a str,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AuthorizeResp {
    pub ok: bool,
    pub budget_remaining: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct RecordUsage<'a> {
    pub tenant: &'a str,
    pub agent: &'a str,
    pub units: u64,
    #[serde(default)]
    pub meta: serde_json::Value,
}

/// Authorize spend + record usage against ledgrrr.
#[async_trait::async_trait]
pub trait SpendAuthorizer: Send + Sync {
    async fn authorize_spend(
        &self,
        tenant: &str,
        agent: &str,
        cost: u64,
        idempotency_ref: &str,
    ) -> Result<AuthorizeResp>;

    async fn record_usage(
        &self,
        tenant: &str,
        agent: &str,
        units: u64,
        meta: serde_json::Value,
    ) -> Result<()>;
}

/// Always-ok (or always-deny) in-memory authorizer for tests / dev.
pub struct MockSpendAuthorizer {
    pub always_ok: bool,
    pub budget: u64,
}

impl Default for MockSpendAuthorizer {
    fn default() -> Self {
        Self {
            always_ok: true,
            budget: MOCK_BUDGET,
        }
    }
}

impl MockSpendAuthorizer {
    pub fn always_ok() -> Self {
        Self::default()
    }
    pub fn always_deny() -> Self {
        Self {
            always_ok: false,
            budget: 0,
        }
    }
}

#[async_trait::async_trait]
impl SpendAuthorizer for MockSpendAuthorizer {
    async fn authorize_spend(
        &self,
        _tenant: &str,
        _agent: &str,
        _cost: u64,
        _idempotency_ref: &str,
    ) -> Result<AuthorizeResp> {
        Ok(AuthorizeResp {
            ok: self.always_ok,
            budget_remaining: self.budget,
            reason: (!self.always_ok).then(|| "mock deny".to_string()),
        })
    }

    async fn record_usage(
        &self,
        _tenant: &str,
        _agent: &str,
        _units: u64,
        _meta: serde_json::Value,
    ) -> Result<()> {
        Ok(())
    }
}

/// Talks to a real ledgrrr over HTTP. Fail-closed: any non-2xx or transport
/// error → `{ ok: false, budget_remaining: 0 }`.
pub struct HttpSpendAuthorizer {
    pub base_url: String,
    client: reqwest::Client,
}

impl HttpSpendAuthorizer {
    pub fn new(base_url: impl Into<String>) -> Self {
        Self {
            base_url: base_url.into(),
            client: reqwest::Client::new(),
        }
    }

    fn url(&self, path: &str) -> String {
        format!("{}{path}", self.base_url.trim_end_matches('/'))
    }
}

#[async_trait::async_trait]
impl SpendAuthorizer for HttpSpendAuthorizer {
    async fn authorize_spend(
        &self,
        tenant: &str,
        agent: &str,
        cost: u64,
        idempotency_ref: &str,
    ) -> Result<AuthorizeResp> {
        let body = AuthorizeSpend {
            tenant,
            agent,
            cost,
            r#ref: idempotency_ref,
        };
        let res = self
            .client
            .post(self.url("/v1/authorize-spend"))
            .header("idempotency-key", idempotency_ref)
            .json(&body)
            .send()
            .await;
        match res {
            Ok(r) if r.status().is_success() => {
                let parsed: AuthorizeResp =
                    r.json().await.context("parse authorize-spend response")?;
                Ok(parsed)
            }
            Ok(r) => Ok(AuthorizeResp {
                ok: false,
                budget_remaining: 0,
                reason: Some(format!("ledgrrr {}", r.status())),
            }),
            Err(e) => Ok(AuthorizeResp {
                ok: false,
                budget_remaining: 0,
                reason: Some(format!("ledgrrr unreachable: {e}")),
            }),
        }
    }

    async fn record_usage(
        &self,
        tenant: &str,
        agent: &str,
        units: u64,
        meta: serde_json::Value,
    ) -> Result<()> {
        let body = RecordUsage {
            tenant,
            agent,
            units,
            meta,
        };
        // usage is best-effort — log-and-swallow, never fail the caller
        let _ = self
            .client
            .post(self.url("/v1/usage"))
            .json(&body)
            .send()
            .await;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn mock_ok_and_deny() {
        let ok = MockSpendAuthorizer::always_ok();
        let r = ok.authorize_spend("t", "a", 1, "ref-1").await.unwrap();
        assert!(r.ok);
        assert_eq!(r.budget_remaining, MOCK_BUDGET);
        ok.record_usage("t", "a", 1, serde_json::json!({})).await.unwrap();

        let deny = MockSpendAuthorizer::always_deny();
        let r = deny.authorize_spend("t", "a", 1, "ref-2").await.unwrap();
        assert!(!r.ok);
        assert_eq!(r.reason.as_deref(), Some("mock deny"));
    }

    #[test]
    fn mode_from_env_defaults_to_mock() {
        // (env not set in CI for this var) -> Mock
        assert_eq!(LedgrrrMode::from_env(), LedgrrrMode::Mock);
    }

    #[test]
    fn authorize_body_serialises_with_ref_key() {
        let b = AuthorizeSpend { tenant: "acme", agent: "a1", cost: 5, r#ref: "idem" };
        let v = serde_json::to_value(&b).unwrap();
        assert_eq!(v["ref"], "idem");
        assert_eq!(v["cost"], 5);
    }
}
