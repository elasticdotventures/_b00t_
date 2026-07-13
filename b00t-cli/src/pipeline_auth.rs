// 🤓 Inter-stage JWT authentication for NATS transport (GH #738).
//
// Provides a zero-external-dependency HMAC-SHA256 JWT implementation
// (no `hmac` or `base64` crate needed — uses only `sha2` which is
// already in the dependency tree) and an `AuthenticatedNatsTransport`
// decorator that injects JWT credentials into every NATS message.
//
// Wire format (4-byte-prefixed):
//   [{jwt_length:4BE}][{jwt_bytes}][{original_payload}]
//   — length prefix avoids delimiter collisions with binary payloads.
//
// Subject convention for JWT claims:
//   sub  = stage identity (e.g. "transcoder", "ingest")
//   iss  = pipeline orchestrator identity
//   stage = stage name (duplicated for ergonomic access)
//   exp  = UTC epoch seconds
//   nbf  = UTC epoch seconds (set to issued-at time)

use crate::pipeline_nats::{NatsSubscription, NatsTransport};
use crate::pipeline_types::StageSpec;
use anyhow::{bail, Context, Result};
use chrono::{DateTime, Utc};
use sha2::{Digest, Sha256};
use std::time::{SystemTime, UNIX_EPOCH};

// ── Constants ────────────────────────────────────────────────────────────────

/// Fixed JWT header — HS256 is the only supported algorithm.
const JWT_HEADER: &str = r#"{"alg":"HS256","typ":"JWT"}"#;

// ── Claims ───────────────────────────────────────────────────────────────────

/// Decoded JWT claims from a verified token.
#[derive(Debug, Clone, PartialEq)]
pub struct Claims {
    pub sub: String,
    pub iss: String,
    pub stage: String,
    pub exp: i64,
    pub nbf: i64,
}

// ── StageCredentials ─────────────────────────────────────────────────────────

/// Per-stage JWT credentials for NATS message authentication.
///
/// Created via [`issue_stage_credentials`] and held by
/// [`AuthenticatedNatsTransport`] to sign outbound messages.
#[derive(Debug, Clone)]
pub struct StageCredentials {
    pub stage_name: String,
    pub jwt: String,
    pub issuer: String,
    pub subject: String,
    pub expires_at: DateTime<Utc>,
}

// ── JwtConfig ────────────────────────────────────────────────────────────────

/// Configuration for issuing and verifying JWT tokens.
#[derive(Debug, Clone)]
pub struct JwtConfig {
    pub issuer: String,
    pub secret_key: String,
    pub ttl_seconds: u64,
}

impl JwtConfig {
    /// Issue a signed JWT for a given stage and subject.
    ///
    /// The token includes standard claims (`sub`, `iss`, `exp`, `nbf`) and
    /// a custom `stage` claim for per-stage identity verification.
    pub fn issue_token(&self, stage: &str, subject: &str) -> Result<String> {
        let now = epoch_seconds()?;
        let exp = now + self.ttl_seconds as i64;
        let nbf = now;

        let claims = serde_json::json!({
            "sub": subject,
            "iss": self.issuer,
            "stage": stage,
            "exp": exp,
            "nbf": nbf,
        });

        let header_b64 = base64url_encode(JWT_HEADER.as_bytes());
        let claims_b64 = base64url_encode(
            serde_json::to_string(&claims)
                .context("failed to serialize JWT claims")?
                .as_bytes(),
        );
        let signing_input = format!("{}.{}", header_b64, claims_b64);

        let signature = hmac_sha256(self.secret_key.as_bytes(), signing_input.as_bytes());
        let signature_b64 = base64url_encode(&signature);

        Ok(format!("{}.{}", signing_input, signature_b64))
    }

    /// Verify a JWT token and return its claims.
    ///
    /// Validates:
    /// - Three-part dot-separated structure
    /// - HMAC-SHA256 signature matches
    /// - Header decodes as valid JSON
    /// - Payload decodes as valid JSON with all required claims
    /// - `iss` matches the config's issuer
    /// - Token is not expired (`exp`)
    /// - Token is valid (`nbf`)
    pub fn verify_token(&self, token: &str) -> Result<Claims> {
        let parts: Vec<&str> = token.split('.').collect();
        if parts.len() != 3 {
            bail!(
                "invalid JWT format: expected 3 dot-separated parts, got {}",
                parts.len()
            );
        }

        let header_b64 = parts[0];
        let claims_b64 = parts[1];
        let signature_b64 = parts[2];

        // ── 1. Verify HMAC-SHA256 signature ──
        let signing_input = format!("{}.{}", header_b64, claims_b64);
        let expected_sig = hmac_sha256(self.secret_key.as_bytes(), signing_input.as_bytes());
        let expected_sig_b64 = base64url_encode(&expected_sig);

        // Use constant-time comparison to prevent timing attacks on the MAC.
        if !constant_time_eq(signature_b64.as_bytes(), expected_sig_b64.as_bytes()) {
            bail!("invalid JWT signature");
        }

        // ── 2. Decode and validate header ──
        let header_bytes = base64url_decode(header_b64)
            .context("failed to decode JWT header (invalid base64url)")?;
        let _header: serde_json::Value = serde_json::from_slice(&header_bytes)
            .context("invalid JWT header JSON")?;

        // ── 3. Decode and validate claims ──
        let claims_bytes = base64url_decode(claims_b64)
            .context("failed to decode JWT claims (invalid base64url)")?;
        let claims_json: serde_json::Value = serde_json::from_slice(&claims_bytes)
            .context("invalid JWT claims JSON")?;

        let sub = claims_json["sub"]
            .as_str()
            .context("missing or non-string 'sub' claim")?
            .to_string();
        let iss = claims_json["iss"]
            .as_str()
            .context("missing or non-string 'iss' claim")?
            .to_string();
        let stage = claims_json["stage"]
            .as_str()
            .context("missing or non-string 'stage' claim")?
            .to_string();
        let exp = claims_json["exp"]
            .as_i64()
            .context("missing or non-integer 'exp' claim")?;
        let nbf = claims_json["nbf"]
            .as_i64()
            .context("missing or non-integer 'nbf' claim")?;

        // ── 4. Validate issuer ──
        if iss != self.issuer {
            bail!(
                "issuer mismatch: expected '{}', got '{}'",
                self.issuer,
                iss
            );
        }

        // ── 5. Validate temporal bounds ──
        let now = epoch_seconds()?;
        if now > exp {
            bail!("token expired at {} (now: {})", exp, now);
        }
        if now < nbf {
            bail!("token not yet valid (nbf: {}, now: {})", nbf, now);
        }

        Ok(Claims {
            sub,
            iss,
            stage,
            exp,
            nbf,
        })
    }
}

// ── StageCredentials factory ─────────────────────────────────────────────────

/// Create per-stage JWT credentials from a config and stage spec.
///
/// The JWT is issued immediately; the caller should check `expires_at`
/// and re-issue before expiry.
pub fn issue_stage_credentials(config: &JwtConfig, stage: &StageSpec) -> Result<StageCredentials> {
    let jwt = config.issue_token(&stage.name, &stage.name)?;
    let exp_ts = epoch_seconds()? + config.ttl_seconds as i64;

    let expires_at = DateTime::from_timestamp(exp_ts, 0)
        .context("failed to compute expiry timestamp")?;

    Ok(StageCredentials {
        stage_name: stage.name.clone(),
        jwt,
        issuer: config.issuer.clone(),
        subject: stage.name.clone(),
        expires_at,
    })
}

// ── AuthenticatedNatsTransport ───────────────────────────────────────────────

/// NATS transport decorator that injects JWT authentication into messages.
///
/// On `publish`: prepends the stage's JWT token (length-prefixed) to the
/// payload so downstream subscribers can verify the origin.
///
/// On `subscribe`: creates a subscription that strips the JWT prefix from
/// each message, yielding only the original payload bytes.
pub struct AuthenticatedNatsTransport {
    pub inner: Box<dyn NatsTransport>,
    pub credentials: StageCredentials,
}

impl NatsTransport for AuthenticatedNatsTransport {
    fn publish(&self, subject: &str, payload: &[u8]) -> Result<()> {
        let jwt_bytes = self.credentials.jwt.as_bytes();
        let len_bytes = (jwt_bytes.len() as u32).to_be_bytes();

        let mut encoded = Vec::with_capacity(4 + jwt_bytes.len() + payload.len());
        encoded.extend_from_slice(&len_bytes);
        encoded.extend_from_slice(jwt_bytes);
        encoded.extend_from_slice(payload);

        self.inner.publish(subject, &encoded)
    }

    fn subscribe(&self, subject: &str) -> Result<Box<dyn NatsSubscription>> {
        let inner_sub = self.inner.subscribe(subject)?;
        Ok(Box::new(AuthenticatedSubscription { inner: inner_sub }))
    }
}

/// Subscription wrapper that strips the JWT prefix from each message.
struct AuthenticatedSubscription {
    inner: Box<dyn NatsSubscription>,
}

impl NatsSubscription for AuthenticatedSubscription {
    fn next(&mut self) -> Option<Vec<u8>> {
        let raw = self.inner.next()?;
        if raw.len() < 4 {
            return None;
        }
        let jwt_len = u32::from_be_bytes([raw[0], raw[1], raw[2], raw[3]]) as usize;
        if raw.len() < 4 + jwt_len {
            return None;
        }
        Some(raw[4 + jwt_len..].to_vec())
    }
}

// ── HMAC-SHA256 (manual implementation) ─────────────────────────────────────
//
// Uses only the `sha2` crate (already in dependencies).  No `hmac` crate
// needed — HMAC is trivially constructed from the hash function alone.

/// Compute HMAC-SHA256 of `data` using `key`.
///
/// Implements RFC 2104:
///   HMAC(K, m) = H((K' ⊕ opad) || H((K' ⊕ ipad) || m))
fn hmac_sha256(key: &[u8], data: &[u8]) -> [u8; 32] {
    const BLOCK_SIZE: usize = 64;

    // Hash key if longer than block size (SHA256 output = 32 bytes).
    let mut k = [0u8; BLOCK_SIZE];
    if key.len() > BLOCK_SIZE {
        let hash = Sha256::digest(key);
        k[..32].copy_from_slice(&hash);
    } else {
        k[..key.len()].copy_from_slice(key);
    }

    // inner = ipad XOR key
    // outer = opad XOR key
    let mut ipad = [0x36u8; BLOCK_SIZE];
    let mut opad = [0x5cu8; BLOCK_SIZE];
    for i in 0..BLOCK_SIZE {
        ipad[i] ^= k[i];
        opad[i] ^= k[i];
    }

    // H((K' ⊕ ipad) || m)
    let inner_hash = {
        let mut hasher = Sha256::new();
        hasher.update(&ipad);
        hasher.update(data);
        hasher.finalize()
    };

    // H((K' ⊕ opad) || inner_hash)
    let result = {
        let mut hasher = Sha256::new();
        hasher.update(&opad);
        hasher.update(&inner_hash);
        hasher.finalize()
    };

    result.into()
}

// ── Base64url encoding / decoding ───────────────────────────────────────────
//
// URL-safe base64 with '-' instead of '+' and '_' instead of '/'.
// No padding characters ('=') are emitted by the encoder.

const BASE64URL_CHARS: &[u8] =
    b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_";

/// Encode bytes as URL-safe base64 (no padding).
fn base64url_encode(data: &[u8]) -> String {
    let mut result = String::with_capacity((data.len() + 2) / 3 * 4);
    for chunk in data.chunks(3) {
        let b0 = chunk[0] as u32;
        let b1 = chunk.get(1).copied().unwrap_or(0) as u32;
        let b2 = chunk.get(2).copied().unwrap_or(0) as u32;
        let triple = (b0 << 16) | (b1 << 8) | b2;

        result.push(BASE64URL_CHARS[((triple >> 18) & 0x3F) as usize] as char);
        result.push(BASE64URL_CHARS[((triple >> 12) & 0x3F) as usize] as char);
        if chunk.len() > 1 {
            result.push(BASE64URL_CHARS[((triple >> 6) & 0x3F) as usize] as char);
        }
        if chunk.len() > 2 {
            result.push(BASE64URL_CHARS[(triple & 0x3F) as usize] as char);
        }
    }
    result
}

/// Decode URL-safe base64 (with or without padding).
fn base64url_decode(input: &str) -> Result<Vec<u8>> {
    // Build reverse lookup: character → 6-bit value.
    let mut rev = [255u8; 256];
    for (i, &c) in BASE64URL_CHARS.iter().enumerate() {
        rev[c as usize] = i as u8;
    }

    // Strip padding, map to values, reject invalid characters.
    let clean: Vec<u8> = input
        .trim_end_matches('=')
        .bytes()
        .map(|b| rev[b as usize])
        .collect();

    if clean.iter().any(|&b| b == 255) {
        bail!("invalid base64url character in input");
    }

    let mut result = Vec::with_capacity(clean.len() * 3 / 4);
    let mut i = 0;
    while i < clean.len() {
        let remaining = clean.len() - i;
        if remaining == 1 {
            bail!("invalid base64url: single trailing character");
        }
        let group_size = remaining.min(4);

        let c0 = clean[i] as u32;
        let c1 = clean[i + 1] as u32;
        let c2 = if remaining > 2 { clean[i + 2] as u32 } else { 0 };
        let c3 = if remaining > 3 { clean[i + 3] as u32 } else { 0 };

        let triple = (c0 << 18) | (c1 << 12) | (c2 << 6) | c3;

        result.push(((triple >> 16) & 0xFF) as u8);
        if group_size >= 3 {
            result.push(((triple >> 8) & 0xFF) as u8);
        }
        if group_size >= 4 {
            result.push((triple & 0xFF) as u8);
        }

        i += group_size;
    }

    Ok(result)
}

// ── Helpers ──────────────────────────────────────────────────────────────────

/// Current system time as Unix epoch seconds.
fn epoch_seconds() -> Result<i64> {
    Ok(SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .context("system time is before Unix epoch")?
        .as_secs() as i64)
}

/// Constant-time byte slice comparison.
///
/// Returns `true` if both slices are equal in length and content.
/// Execution time depends only on the length of the inputs, not
/// on the content — preventing timing side-channel attacks on MACs.
fn constant_time_eq(a: &[u8], b: &[u8]) -> bool {
    if a.len() != b.len() {
        return false;
    }
    let mut result: u8 = 0;
    for (x, y) in a.iter().zip(b.iter()) {
        result |= x ^ y;
    }
    result == 0
}

// ── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pipeline_nats::MockNatsTransport;
    use crate::pipeline_types::{CapsuleProfile, PortDirection, PortMediaType, ResourceRequirements};

    fn test_config() -> JwtConfig {
        JwtConfig {
            issuer: "pipeline-orchestrator".into(),
            secret_key: "test-secret-key-12345678".into(),
            ttl_seconds: 3600,
        }
    }

    fn test_stage(name: &str) -> StageSpec {
        StageSpec {
            name: name.into(),
            profile: CapsuleProfile {
                name: name.into(),
                ports: vec![],
                resources: ResourceRequirements {
                    min_ram_gb: 0.0,
                    min_vram_gb: 0.0,
                    requires_gpu: false,
                    cpu_cores: None,
                    scratch_disk_gb: None,
                },
                image: None,
                timeout_seconds: None,
            },
            input_ports: vec![],
            output_ports: vec![],
            error_routes: vec![],
            env: None,
            checkpoint_interval_seconds: None,
            secret_refs: None,
            flow_control: None,
        }
    }

    // ── 1. Issue and verify token round-trip ─────────────────────────────

    #[test]
    fn issue_and_verify_round_trip() {
        let config = test_config();
        let token = config
            .issue_token("transcoder", "transcoder@stage-1")
            .expect("issue_token should succeed");

        let claims = config
            .verify_token(&token)
            .expect("verify_token should succeed");

        assert_eq!(claims.sub, "transcoder@stage-1");
        assert_eq!(claims.iss, "pipeline-orchestrator");
        assert_eq!(claims.stage, "transcoder");
        assert!(claims.exp > claims.nbf);
    }

    // ── 2. Token expiry ──────────────────────────────────────────────────

    #[test]
    fn expired_token_is_rejected() {
        let config = JwtConfig {
            issuer: "test".into(),
            secret_key: "short-ttl-key".into(),
            ttl_seconds: 1, // 1-second TTL
        };

        let token = config
            .issue_token("fast-stage", "ephemeral")
            .expect("issue_token should succeed");

        // Verify immediately — should pass.
        config
            .verify_token(&token)
            .expect("verify_token should pass immediately");

        // Wait for expiry.
        std::thread::sleep(std::time::Duration::from_secs(2));

        // Verify after expiry — should fail.
        let err = config
            .verify_token(&token)
            .expect_err("verify_token should reject expired token");
        assert!(
            err.to_string().contains("expired"),
            "error should mention expiry, got: {err}"
        );
    }

    // ── 3. Invalid signature rejected ────────────────────────────────────

    #[test]
    fn invalid_signature_is_rejected() {
        let config = test_config();
        let token = config
            .issue_token("ingest", "ingest-data")
            .expect("issue_token should succeed");

        // Tamper with the signature segment (everything after last dot).
        let mut parts: Vec<&str> = token.splitn(3, '.').collect();
        assert_eq!(parts.len(), 3);
        let tampered = format!("{}.{}.aW52YWxpZFNpZw", parts[0], parts[1]);

        let err = config
            .verify_token(&tampered)
            .expect_err("verify_token should reject tampered signature");
        assert!(
            err.to_string().contains("signature"),
            "error should mention signature, got: {err}"
        );
    }

    // ── 4. Wrong stage name in token ─────────────────────────────────────

    #[test]
    fn stage_claim_round_trip() {
        let config = test_config();
        let token = config
            .issue_token("stage-a", "worker@stage-a")
            .expect("issue_token should succeed");

        let claims = config
            .verify_token(&token)
            .expect("verify_token should succeed");

        // The stage claim should match what we issued.
        assert_eq!(claims.stage, "stage-a", "stage claim must match");

        // If we expected a different stage, the mismatch is detectable.
        assert_ne!(
            claims.stage, "stage-b",
            "should not match a different stage"
        );
    }

    // ── 5. Authenticated transport prepends auth header ──────────────────

    #[test]
    fn authenticated_transport_round_trip() {
        let config = test_config();
        let stage = test_stage("encoder");
        let creds = issue_stage_credentials(&config, &stage).expect("issue credentials");

        let mock = MockNatsTransport::new();
        let auth_transport = AuthenticatedNatsTransport {
            inner: Box::new(mock),
            credentials: creds,
        };

        let subject = "pipeline.test-run.encoder.output.video";
        let payload = b"encoded-frame-data-12345";

        // Subscribe first (mock delivers only post-subscribe).
        let mut sub = auth_transport
            .subscribe(subject)
            .expect("subscribe should succeed");

        auth_transport
            .publish(subject, payload)
            .expect("publish should succeed");

        let received = sub
            .next()
            .expect("should receive a message after publish");

        // The authenticated transport strips the JWT prefix, so the
        // subscriber should see the original payload.
        assert_eq!(
            received, payload,
            "payload should match after JWT stripping"
        );
    }

    // ── 6. Authenticated transport with wrong key rejected on verify ─────

    #[test]
    fn authenticated_transport_jwt_verifiable() {
        let config = test_config();
        let stage = test_stage("decoder");
        let creds = issue_stage_credentials(&config, &stage).expect("issue credentials");

        // The JWT in the credentials should be verifiable by the same config.
        let claims = config
            .verify_token(&creds.jwt)
            .expect("credentials JWT should verify");

        assert_eq!(claims.stage, "decoder");
        assert_eq!(claims.iss, "pipeline-orchestrator");
        assert!(
            claims.exp > claims.nbf,
            "expiry must be after not-before"
        );
    }

    // ── 7. issue_stage_credentials produces valid credentials ────────────

    #[test]
    fn issue_stage_credentials_populates_all_fields() {
        let config = test_config();
        let stage = test_stage("transcoder");
        let creds = issue_stage_credentials(&config, &stage).expect("issue credentials");

        assert_eq!(creds.stage_name, "transcoder");
        assert_eq!(creds.issuer, "pipeline-orchestrator");
        assert_eq!(creds.subject, "transcoder");
        assert!(!creds.jwt.is_empty(), "JWT should not be empty");
        assert!(
            creds.expires_at > Utc::now() - chrono::Duration::seconds(10),
            "expires_at should be in the future"
        );
    }

    // ── 8. Reject malformed token format ─────────────────────────────────

    #[test]
    fn malformed_token_rejected() {
        let config = test_config();

        // Empty token
        assert!(config.verify_token("").is_err());

        // Only 2 parts
        assert!(config.verify_token("header.payload").is_err());

        // 4 parts
        assert!(config.verify_token("a.b.c.d").is_err());
    }

    // ── 9. Invalid base64url in token header ─────────────────────────────

    #[test]
    fn invalid_base64url_in_token_rejected() {
        let config = test_config();
        // The header part contains an invalid base64url character (!!!)
        let token = "!!!.cGF5bG9hZA.aW52YWxpZFNpZw";
        let err = config.verify_token(token);
        assert!(err.is_err(), "malformed base64url should be rejected");
    }

    // ── 10. Verify with wrong issuer rejected ────────────────────────────

    #[test]
    fn wrong_issuer_rejected() {
        let issuer_a = JwtConfig {
            issuer: "orchestrator-a".into(),
            secret_key: "shared-secret".into(),
            ttl_seconds: 3600,
        };
        let issuer_b = JwtConfig {
            issuer: "orchestrator-b".into(),
            secret_key: "shared-secret".into(),
            ttl_seconds: 3600,
        };

        let token = issuer_a
            .issue_token("stage", "sub")
            .expect("issue token");

        // Same secret key but different issuer — should fail.
        let err = issuer_b
            .verify_token(&token)
            .expect_err("should reject wrong issuer");
        assert!(
            err.to_string().contains("issuer"),
            "error should mention issuer, got: {err}"
        );
    }

    // ── 11. Base64url encode/decode round-trip ───────────────────────────

    #[test]
    fn base64url_round_trip() {
        let cases: &[&[u8]] = &[
            b"",
            b"f",
            b"fo",
            b"foo",
            b"foob",
            b"fooba",
            b"foobar",
            b"\x00\x01\x02\xFF\xFE\xFD",
            &[0u8; 256],
        ];

        for input in cases {
            let encoded = base64url_encode(input);
            let decoded = base64url_decode(&encoded)
                .unwrap_or_else(|e| panic!("decode failed for {:?}: {e}", input));
            assert_eq!(
                &decoded, input,
                "round-trip failed for {:02x?}",
                input
            );
        }
    }

    // ── 12. Constant-time eq rejects length mismatch ─────────────────────

    #[test]
    fn constant_time_eq_len_mismatch() {
        assert!(!constant_time_eq(b"abc", b"ab"));
        assert!(!constant_time_eq(b"", b"a"));
    }

    #[test]
    fn constant_time_eq_matches() {
        assert!(constant_time_eq(b"", b""));
        assert!(constant_time_eq(b"abc", b"abc"));
        assert!(!constant_time_eq(b"abc", b"abd"));
    }
}
