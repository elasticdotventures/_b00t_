//! SP3-02a — HTTP identity middleware.
//!
//! Verifies `Authorization: Bearer <jwt>` against the identity worker's JWKS
//! ([`JwksVerifier`]) and injects a [`CallerIdentity`] into the request
//! extensions. `B00T_MCP_REQUIRE_AUTH=1` makes a missing/invalid bearer a
//! `401`; otherwise the request proceeds as `CallerIdentity::anon()`.
//!
//! Whether that extension reaches the rmcp tool handlers (`list_tools` /
//! `call_tool` on `B00tMcpServerRusty`) is SP3-09's integration concern — this
//! layer's job is to gate and to stash.

use std::sync::Arc;

use axum::{
    body::Body,
    extract::State,
    http::{Request, StatusCode, header::AUTHORIZATION},
    middleware::Next,
    response::Response,
};

use crate::identity::{CallerIdentity, JwksVerifier};

/// Shared state for [`identity_middleware`].
#[derive(Clone)]
pub struct IdentityLayerState {
    pub verifier: Arc<JwksVerifier>,
    pub require_auth: bool,
}

impl IdentityLayerState {
    /// `verifier` from `$B00T_IDENTITY_JWKS` / `$B00T_IDENTITY_URL`;
    /// `require_auth` from `$B00T_MCP_REQUIRE_AUTH` (`1` / `true`).
    pub fn from_env() -> Self {
        Self {
            verifier: Arc::new(JwksVerifier::from_env()),
            require_auth: std::env::var("B00T_MCP_REQUIRE_AUTH")
                .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
                .unwrap_or(false),
        }
    }
}

fn bearer(req: &Request<Body>) -> Option<String> {
    req.headers()
        .get(AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .and_then(|s| s.strip_prefix("Bearer "))
        .map(|s| s.trim().to_string())
}

/// axum middleware: `Bearer` → verified [`CallerIdentity`] in request extensions.
pub async fn identity_middleware(
    State(state): State<IdentityLayerState>,
    mut req: Request<Body>,
    next: Next,
) -> Response {
    let identity = match bearer(&req) {
        Some(token) => match state.verifier.verify(&token).await {
            Ok(id) => Some(id),
            Err(_) if state.require_auth => None,
            Err(_) => Some(CallerIdentity::anon()),
        },
        None if state.require_auth => None,
        None => Some(CallerIdentity::anon()),
    };

    match identity {
        Some(id) => {
            req.extensions_mut().insert(id);
            next.run(req).await
        }
        None => Response::builder()
            .status(StatusCode::UNAUTHORIZED)
            .header("content-type", "application/json")
            .body(Body::from(r#"{"error":"unauthorized"}"#))
            .expect("static 401 response builds"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::{Router, body::to_bytes, routing::get};
    use b00t_c0re_identity::{MockTokenSource, TokenRequest};
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
            .layer(axum::middleware::from_fn_with_state(state, identity_middleware))
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
            .oneshot(Request::builder().uri("/whoami").body(Body::empty()).unwrap())
            .await
            .unwrap();
        assert_eq!(res.status(), StatusCode::UNAUTHORIZED);
    }

    #[tokio::test]
    async fn anon_when_not_required() {
        let mock = MockTokenSource::new();
        let res = app(false, mock.jwks_json())
            .oneshot(Request::builder().uri("/whoami").body(Body::empty()).unwrap())
            .await
            .unwrap();
        assert_eq!(res.status(), StatusCode::OK);
        let body = to_bytes(res.into_body(), 1024).await.unwrap();
        assert_eq!(&body[..], b"anon");
    }
}
