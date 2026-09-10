//! Agent identity for the b00t.promptexecution.com product plane (SP1).
//!
//! - [`AgentClaims`] — the RS256 JWT claim set (mirrors the identity worker's).
//! - [`TokenSource`] — obtain a signed agent JWT; [`HttpTokenSource`] talks to
//!   the deployed worker, [`MockTokenSource`] mints locally for tests.
//! - [`verify_jwt`] / [`decode_claims_unverified`].

pub mod claims;
pub mod http_source;
pub mod mock_source;

pub use claims::{AgentClaims, TokenRequest, decode_claims_unverified, verify_jwt, ISS};
pub use http_source::HttpTokenSource;
pub use mock_source::MockTokenSource;

use anyhow::Result;

/// Something that can hand back a signed agent JWT.
#[async_trait::async_trait]
pub trait TokenSource: Send + Sync {
    async fn obtain(&self, req: &TokenRequest) -> Result<String>;
}
