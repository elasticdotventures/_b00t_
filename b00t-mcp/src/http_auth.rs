//! SP3-02a — HTTP identity middleware.
//!
//! The implementation moved to `b00t-c0re-identity::axum_auth` in SP4-07 (enabled
//! via that crate's `axum` feature) so the proxy and every backend MCP server
//! run byte-identical bearer verification. This module re-exports it; the tests
//! below still run here as the CP-4 auth-parity regression gate.

pub use b00t_c0re_identity::axum_auth::{IdentityLayerState, identity_middleware};

#[cfg(test)]
mod tests {
    use super::*;
    use crate::identity::{CallerIdentity, JwksVerifier};
    use axum::{
        body::{Body, to_bytes},
        http::{Request, StatusCode},
        routing::get,
        Router,
    };
    use b00t_c0re_identity::{MockTokenSource, TokenRequest};
    use std::sync::Arc;
    use tower::ServiceExt; // oneshot

    fn app(require_auth: bool, jwks: String) -> Router {
        let state = IdentityLayerState {
            verifier: Arc::new(JwksVerifier::pinned(jwks)),
            require_auth,
        };
        Router::new()
            .route(
                "/whoami",
                get(|req: Request<Body>| async move {
                    req.extensions()
                        .get::<CallerIdentity>()
                        .map(|id| id.r0le.clone())
                        .unwrap_or_else(|| "no-identity".to_string())
                }),
            )
            .layer(axum::middleware::from_fn_with_state(
                state,
                identity_middleware,
            ))
    }

    fn req() -> TokenRequest {
        TokenRequest {
            tenant_id: "promptexecution".into(),
            agent_id: "agent/x".into(),
            node_id: "root".into(),
            r0le: "worker".into(),
            requested_shards: vec![],
        }
    }

    #[tokio::test]
    async fn valid_bearer_populates_identity() {
        let mock = MockTokenSource::new();
        let jwt = mock.mint(&req()).unwrap();
        let res = app(true, mock.jwks_json())
            .oneshot(
                Request::builder()
                    .uri("/whoami")
                    .header("authorization", format!("Bearer {jwt}"))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(res.status(), StatusCode::OK);
        let body = to_bytes(res.into_body(), 1024).await.unwrap();
        assert_eq!(&body[..], b"worker");
    }

    #[tokio::test]
    async fn missing_bearer_is_401_when_required() {
        let mock = MockTokenSource::new();
        let res = app(true, mock.jwks_json())
            .oneshot(
                Request::builder()
                    .uri("/whoami")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(res.status(), StatusCode::UNAUTHORIZED);
    }

    #[tokio::test]
    async fn anon_when_not_required() {
        let mock = MockTokenSource::new();
        let res = app(false, mock.jwks_json())
            .oneshot(
                Request::builder()
                    .uri("/whoami")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(res.status(), StatusCode::OK);
        let body = to_bytes(res.into_body(), 1024).await.unwrap();
        assert_eq!(&body[..], b"anon");
    }

    #[tokio::test]
    async fn invalid_bearer_is_rejected_even_when_auth_is_optional() {
        let mock = MockTokenSource::new();
        let response = app(false, mock.jwks_json())
            .oneshot(
                Request::builder()
                    .uri("/whoami")
                    .header("authorization", "Bearer invalid")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    }

    #[tokio::test]
    async fn bootstrap_routes_remain_public_when_mcp_requires_identity() {
        let mock = MockTokenSource::new();
        let app = app(true, mock.jwks_json()).merge(
            Router::new()
                .route(
                    "/.well-known/oauth-authorization-server",
                    get(|| async { "discovery" }),
                )
                .route("/oauth/authorize", get(|| async { "authorize" }))
                .route("/oauth/token", axum::routing::post(|| async { "token" }))
                .route("/auth/github/callback", get(|| async { "callback" })),
        );
        for (path, method) in [
            ("/.well-known/oauth-authorization-server", "GET"),
            ("/oauth/authorize", "GET"),
            ("/oauth/token", "POST"),
            ("/auth/github/callback", "GET"),
        ] {
            let response = app
                .clone()
                .oneshot(
                    Request::builder()
                        .uri(path)
                        .method(method)
                        .body(Body::empty())
                        .unwrap(),
                )
                .await
                .unwrap();
            assert_eq!(response.status(), StatusCode::OK);
        }
        let response = app
            .oneshot(
                Request::builder()
                    .uri("/whoami")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    }
}
