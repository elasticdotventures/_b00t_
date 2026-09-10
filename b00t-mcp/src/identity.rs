//! SP3-01 — turn an inbound agent JWT into a [`CallerIdentity`].
//!
//! The b00t-mcp proxy verifies the caller's RS256 JWT (issued by the SP1
//! identity worker) against a JWKS — pinned inline for tests, or fetched from
//! `$B00T_IDENTITY_URL/.well-known/jwks.json` in production — and projects the
//! claims down to the fields the proxy actually gates on.

use anyhow::{Context, Result};
use b00t_c0re_identity::{AgentClaims, verify_jwt};

/// The verified caller. `anon()` is the unauthenticated default.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CallerIdentity {
    pub tenant: String,
    pub agent: String,
    pub r0le: String,
    pub scopes: Vec<String>,
    pub budget_ref: String,
}

impl CallerIdentity {
    /// The unauthenticated caller — `r0le == "anon"`, no scopes.
    pub fn anon() -> Self {
        Self {
            tenant: "anon".to_string(),
            agent: "anon".to_string(),
            r0le: "anon".to_string(),
            scopes: Vec::new(),
            budget_ref: String::new(),
        }
    }

    pub fn is_anon(&self) -> bool {
        self.r0le == "anon"
    }
}

impl From<AgentClaims> for CallerIdentity {
    fn from(c: AgentClaims) -> Self {
        Self {
            tenant: c.tenant,
            agent: c.sub,
            r0le: c.r0le,
            scopes: c.scopes,
            budget_ref: c.budget_ref,
        }
    }
}

enum JwksSource {
    /// JWKS JSON held in memory (tests, or `$B00T_IDENTITY_JWKS`).
    Pinned(String),
    /// Fetch on demand from this URL.
    Url(String),
}

/// Verifies agent JWTs against a JWKS.
pub struct JwksVerifier {
    source: JwksSource,
}

impl JwksVerifier {
    /// Verify against a fixed JWKS JSON (no network).
    pub fn pinned(jwks_json: impl Into<String>) -> Self {
        Self {
            source: JwksSource::Pinned(jwks_json.into()),
        }
    }

    /// `$B00T_IDENTITY_JWKS` (inline JSON) wins; otherwise fetch
    /// `$B00T_IDENTITY_URL/.well-known/jwks.json`
    /// (default base `https://b00t.promptexecution.com`).
    pub fn from_env() -> Self {
        if let Ok(inline) = std::env::var("B00T_IDENTITY_JWKS") {
            return Self::pinned(inline);
        }
        let base = std::env::var("B00T_IDENTITY_URL")
            .unwrap_or_else(|_| "https://b00t.promptexecution.com".to_string());
        Self {
            source: JwksSource::Url(format!(
                "{}/.well-known/jwks.json",
                base.trim_end_matches('/')
            )),
        }
    }

    async fn jwks_json(&self) -> Result<String> {
        match &self.source {
            JwksSource::Pinned(s) => Ok(s.clone()),
            JwksSource::Url(u) => reqwest::get(u)
                .await
                .with_context(|| format!("fetch JWKS from {u}"))?
                .error_for_status()
                .context("JWKS endpoint returned an error")?
                .text()
                .await
                .context("read JWKS body"),
        }
    }

    /// Verify `jwt` (RS256 + issuer + `exp`) and project to a [`CallerIdentity`].
    pub async fn verify(&self, jwt: &str) -> Result<CallerIdentity> {
        let jwks = self.jwks_json().await?;
        let claims = verify_jwt(jwt, &jwks).context("verify agent JWT")?;
        Ok(CallerIdentity::from(claims))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use b00t_c0re_identity::{MockTokenSource, TokenRequest};

    fn req() -> TokenRequest {
        TokenRequest {
            tenant_id: "promptexecution".to_string(),
            agent_id: "agent/sm3lly".to_string(),
            node_id: "root".to_string(),
            r0le: "worker".to_string(),
            requested_shards: vec!["project".to_string()],
        }
    }

    #[tokio::test]
    async fn pinned_verifier_accepts_a_mock_token() {
        let mock = MockTokenSource::new();
        let jwt = mock.mint(&req()).unwrap();
        let id = JwksVerifier::pinned(mock.jwks_json())
            .verify(&jwt)
            .await
            .unwrap();
        assert_eq!(id.tenant, "promptexecution");
        assert_eq!(id.agent, "agent/sm3lly");
        assert_eq!(id.r0le, "worker");
        assert_eq!(id.scopes, ["project"]);
        assert!(!id.is_anon());
    }

    #[tokio::test]
    async fn tampered_token_is_rejected() {
        let mock = MockTokenSource::new();
        let jwt = mock.mint(&req()).unwrap();
        let mut parts: Vec<&str> = jwt.split('.').collect();
        let bad_payload = if parts[1].starts_with('a') { "b" } else { "a" };
        let tampered = format!("{}.{}{}.{}", parts[0], bad_payload, &parts[1][1..], parts[2]);
        let _ = &mut parts;
        assert!(
            JwksVerifier::pinned(mock.jwks_json())
                .verify(&tampered)
                .await
                .is_err()
        );
    }

    #[tokio::test]
    async fn wrong_jwks_is_rejected() {
        let mock = MockTokenSource::new();
        let jwt = mock.mint(&req()).unwrap();
        let foreign = r#"{"keys":[{"kty":"RSA","use":"sig","alg":"RS256","kid":"mock-kid","n":"AQAB","e":"AQAB"}]}"#;
        assert!(
            JwksVerifier::pinned(foreign)
                .verify(&jwt)
                .await
                .is_err()
        );
    }

    #[test]
    fn anon_identity() {
        let a = CallerIdentity::anon();
        assert!(a.is_anon());
        assert!(a.scopes.is_empty());
    }
}
