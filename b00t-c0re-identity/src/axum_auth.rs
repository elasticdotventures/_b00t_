//! SP3-02a / SP4-07 — the shared axum identity middleware.
//!
//! Originally `b00t-mcp/src/http_auth.rs`; factored here in SP4-07 so the proxy
//! and every backend MCP server run byte-identical bearer verification.
//! Enabled by the crate's `axum` feature.
//!
//! Verifies `Authorization: Bearer <jwt>` against the identity worker's JWKS
//! ([`JwksVerifier`]) and injects a [`CallerIdentity`] into the request
//! extensions. `B00T_MCP_REQUIRE_AUTH=1` makes a missing/invalid bearer a
//! `401`. Invalid supplied credentials always fail; only missing credentials
//! may proceed as `CallerIdentity::anon()` when authentication is optional.
//! rmcp forwards these extensions inside the HTTP request Parts.

use std::sync::Arc;

use axum::{
    body::Body,
    extract::State,
    http::{Request, StatusCode, header::AUTHORIZATION},
    middleware::Next,
    response::Response,
};

use crate::caller::{CallerIdentity, JwksVerifier};

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
            Err(_) => None,
        },
        None if state.require_auth || req.headers().contains_key(AUTHORIZATION) => None,
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
