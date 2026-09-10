use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

/// The token issuer — must match the identity worker.
pub const ISS: &str = "https://b00t.promptexecution.com";

/// RS256 JWT claim set. `exp - iat` is 900 for worker-issued tokens.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AgentClaims {
    pub iss: String,
    pub sub: String,
    pub tenant: String,
    pub r0le: String,
    pub scopes: Vec<String>,
    pub budget_ref: String,
    pub iat: i64,
    pub exp: i64,
    pub jti: String,
}

/// Request body for `POST /tokens` (field names match the worker's JSON).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TokenRequest {
    #[serde(rename = "tenantId")]
    pub tenant_id: String,
    #[serde(rename = "agentId")]
    pub agent_id: String,
    #[serde(rename = "nodeId")]
    pub node_id: String,
    pub r0le: String,
    #[serde(rename = "requestedShards")]
    pub requested_shards: Vec<String>,
}

/// Verify an RS256 JWT against a JWKS JSON (`{keys:[{kid,n,e,...}]}`) and the
/// expected issuer. Also enforces `exp`.
pub fn verify_jwt(token: &str, jwks_json: &str) -> Result<AgentClaims> {
    use jsonwebtoken::{Algorithm, DecodingKey, Validation, decode, decode_header};

    let header = decode_header(token).context("decode JWT header")?;
    let kid = header.kid.context("JWT header has no kid")?;

    let jwks: serde_json::Value = serde_json::from_str(jwks_json).context("parse JWKS")?;
    let jwk = jwks["keys"]
        .as_array()
        .and_then(|ks| ks.iter().find(|k| k["kid"].as_str() == Some(kid.as_str())))
        .context("kid not present in JWKS")?;
    let n = jwk["n"].as_str().context("JWK missing 'n'")?;
    let e = jwk["e"].as_str().context("JWK missing 'e'")?;

    let key = DecodingKey::from_rsa_components(n, e).context("build decoding key")?;
    let mut validation = Validation::new(Algorithm::RS256);
    validation.leeway = 0;
    validation.set_issuer(&[ISS]);
    validation.set_required_spec_claims(&["exp"]);

    let data = decode::<AgentClaims>(token, &key, &validation).context("verify JWT")?;
    Ok(data.claims)
}

/// Decode the claims WITHOUT verifying the signature or `exp`. For inspection
/// only — never trust the result for authz.
pub fn decode_claims_unverified(token: &str) -> Result<AgentClaims> {
    let data =
        jsonwebtoken::dangerous::insecure_decode::<AgentClaims>(token).context("decode claims")?;
    Ok(data.claims)
}
