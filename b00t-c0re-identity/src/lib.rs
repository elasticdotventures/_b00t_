//! Agent identity for the b00t.promptexecution.com product plane (SP1).
//!
//! - [`AgentClaims`] — the RS256 JWT claim set (mirrors the identity worker's).
//! - [`TokenSource`] — obtain a signed agent JWT; [`HttpTokenSource`] talks to
//!   the deployed worker, [`MockTokenSource`] mints locally for tests.
//! - [`verify_jwt`] / [`decode_claims_unverified`].
//! - [`CallerIdentity`] / [`JwksVerifier`] — verify an inbound caller JWT
//!   (SP3-01); [`axum_auth`] (feature `axum`) is the shared HTTP middleware
//!   used by the proxy and every backend MCP server (SP4-07).

pub mod caller;
pub mod claims;
pub mod http_source;
pub mod mock_source;

#[cfg(feature = "axum")]
pub mod axum_auth;

pub use caller::{CallerIdentity, JwksVerifier};
pub use claims::{AgentClaims, TokenRequest, decode_claims_unverified, verify_jwt, ISS};
pub use http_source::HttpTokenSource;
pub use mock_source::MockTokenSource;

#[cfg(feature = "axum")]
pub use axum_auth::{IdentityLayerState, identity_middleware};

use anyhow::Result;

/// Something that can hand back a signed agent JWT.
#[async_trait::async_trait]
pub trait TokenSource: Send + Sync {
    async fn obtain(&self, req: &TokenRequest) -> Result<String>;
}
