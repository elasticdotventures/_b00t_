//! SP3-01 — inbound agent JWT → [`CallerIdentity`].
//!
//! The implementation moved to `b00t-c0re-identity` in SP4-07 so the proxy and
//! every backend MCP server share one verifier. This module re-exports it; the
//! tests below still run here as a regression gate (CP-4 auth parity).

pub use b00t_c0re_identity::{CallerIdentity, JwksVerifier};

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
        let parts: Vec<&str> = jwt.split('.').collect();
        let bad_payload = if parts[1].starts_with('a') { "b" } else { "a" };
        let tampered = format!(
            "{}.{}{}.{}",
            parts[0],
            bad_payload,
            &parts[1][1..],
            parts[2]
        );
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
        assert!(JwksVerifier::pinned(foreign).verify(&jwt).await.is_err());
    }

    #[test]
    fn anon_identity() {
        let a = CallerIdentity::anon();
        assert!(a.is_anon());
        assert!(a.scopes.is_empty());
    }
}
