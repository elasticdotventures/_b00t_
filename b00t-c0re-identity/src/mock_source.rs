use anyhow::{Context, Result};
use base64::Engine as _;
use jsonwebtoken::{Algorithm, EncodingKey, Header, encode};
use rsa::pkcs8::DecodePrivateKey;
use rsa::traits::PublicKeyParts;
use rsa::RsaPrivateKey;

use crate::claims::ISS;
use crate::{AgentClaims, TokenRequest, TokenSource};

const MOCK_KID: &str = "mock-kid";

// Throwaway 2048-bit PKCS#8 key — mock/test only, signs nothing real.
const MOCK_PEM: &str = "-----BEGIN PRIVATE KEY-----\nMIIEvAIBADANBgkqhkiG9w0BAQEFAASCBKYwggSiAgEAAoIBAQCRQTcE2IGnzryV\nuKDvf7dJ3FuJSXwBwvXSLWUUOyh/O2rl8D9qM4Q+KMT8l+udqj0Utal95mVXAVHd\n+vg5jCEVGnkhE3UZpQShuWwGmAYlgYFCtkPYFc3qD+L2HJtxet6RZFaaeCmV8FO+\nbPStmPbLYXY6dFECG8mYBTITpFYBtLIiLS4S3Qsw0YArrR4CDgdlpCg41wI0z5uX\n/aGkWKhlpxt8lyr3jaUCsi260vb1xS/2zdM4HJ/LDIMljJICac6ugUyoRAZKYtc8\nvTc3xXHpEcxkafPtyOzG3Fl3lFMsgxRRPEgfn1FKzZ+RExTHDsjOpc9Xi4ihZWQn\nySXqmR4hAgMBAAECggEAESXq0eahf+cXOnG+hifEwrKCF/YV7rtOfA6h5T6KrGKe\nXyD6y5XjYdc8Ujm5NjbX2S8NIHDnu9rLCHLNhTW23h/u9umuJGXn4xPZ3flqmFju\noqqT3dnNInnXqIh+DWqdBfsbgkb3Wd0ydcO1Kx1o3V/XLlV3DtGq/gh2/fyjrrWx\n12Db2m5IlhEfbMz2Rc2XqqH9BsI/mVhBmbxPEFys9Q6JyMV/qZvgQ4lc+aYz51/5\nbxZruYfMKsUOJXg0Aa12M+Ud+4kJ9gvMCcUjPhj5ww7ZkP7972Zg5a8D14iJjXKN\na1qrW+qSJhB/JUe+DK4LW4LO7Fhv7xuwlMccPW05sQKBgQDHdJIcPGaRGQOMAs7k\ncKCtjwnum4BqJu67juH1tWviXShToQSHnauwgRxQrGEOxRyQipHOXVMvghvG9hdk\nF4souiVYCryeQaskpeAn2crnnU2FrF9+og6fYiet7YDlvSeJIeLvy0IA2ji2wD5u\nLY/GuQTNTG9k1k/LPBWBiAD3HQKBgQC6bwv1G6UQXbILY8jZBphqaGYGGvagq6VJ\nMFHxMmZC4dUULlDg25GfSqOT9xRAFcuiy7KLp+Fv7249YC83UX4RrS1F0v9/n3C4\nuZEXEZra5YnSlgIW2bJq59Xfb5fXg81nMEXEPhDQK6vueyuPuNO+oh00f0TlCnW6\nQfjDCisf1QKBgHQvU21fQeAD0i0c9afcc7ymNgLoUkWDqE1ZTgbzR4T0/yi4Awt8\nrSaEDxpvT5pq99i633R2qJ5kDAo6ECYeENIInPhMSNNnLWqLtaeBFtEUsLPNVVNO\n03XEl5iZYRxyszUOqENHA4u7ko3iLnu/zqDT5hgxDjKPJKwes+hgcS+BAoGAEo/N\n0/iNpaR+fo3PyHPUpvt/9OmoVnTgfvn1npsS/WO4sEqwOMMDq6Vlxeyasoq4/Jtl\nSmxLkLZ49llmOg6+C4p/cG1CjPVV5r5rCK3zCgpCf5n52UaRcf1lGNrmdkmkILr4\np0I6sE84zgSrYKLZSiif2cM2G8u/zuyUlO6lPoUCgYBscEwq4AC0t5eHARTpCD92\nFKYbb0W9EL9i0ZwxQ0vJKdbLtMzgANAuFzp6+4807Jxnxr5HKuwc20OBTJ4C40st\n2+HLb/1FtvcEjMeEhG15wOyZfMCwJ4tyeitun2iAuGQfTfMRbye0Y2u4k9ZahJuq\nSwxvnEFXx9VJ2QOvD4beOw==\n-----END PRIVATE KEY-----\n";

/// Mints locally-signed JWTs with a fixed RSA key; `jwks_json()` returns the
/// matching public key so `verify_jwt` round-trips without a live worker.
pub struct MockTokenSource {
    ttl_secs: i64,
}

impl Default for MockTokenSource {
    fn default() -> Self {
        Self { ttl_secs: 900 }
    }
}

impl MockTokenSource {
    pub fn new() -> Self {
        Self::default()
    }

    fn now() -> i64 {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs() as i64)
            .unwrap_or(0)
    }

    /// The public half of the mock key, as a JWKS JSON string.
    pub fn jwks_json(&self) -> String {
        let pk = RsaPrivateKey::from_pkcs8_pem(MOCK_PEM).expect("mock PEM parses");
        let pub_key = pk.to_public_key();
        let b64 = base64::engine::general_purpose::URL_SAFE_NO_PAD;
        let n = b64.encode(pub_key.n().to_bytes_be());
        let e = b64.encode(pub_key.e().to_bytes_be());
        format!(
            r#"{{"keys":[{{"kty":"RSA","use":"sig","alg":"RS256","kid":"{MOCK_KID}","n":"{n}","e":"{e}"}}]}}"#
        )
    }

    /// Mint a JWT for `req` (sync — no network).
    pub fn mint(&self, req: &TokenRequest) -> Result<String> {
        let iat = Self::now();
        let claims = AgentClaims {
            iss: ISS.to_string(),
            sub: req.agent_id.clone(),
            tenant: req.tenant_id.clone(),
            r0le: req.r0le.clone(),
            scopes: req.requested_shards.clone(),
            budget_ref: format!("mock-{iat}"),
            iat,
            exp: iat + self.ttl_secs,
            jti: format!("mock-jti-{iat}"),
        };
        let mut header = Header::new(Algorithm::RS256);
        header.kid = Some(MOCK_KID.to_string());
        let key =
            EncodingKey::from_rsa_pem(MOCK_PEM.as_bytes()).context("load mock signing key")?;
        encode(&header, &claims, &key).context("sign mock JWT")
    }
}

#[async_trait::async_trait]
impl TokenSource for MockTokenSource {
    async fn obtain(&self, req: &TokenRequest) -> Result<String> {
        self.mint(req)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{decode_claims_unverified, verify_jwt};

    fn req() -> TokenRequest {
        TokenRequest {
            tenant_id: "promptexecution".into(),
            agent_id: "agent/sm3lly".into(),
            node_id: "root".into(),
            r0le: "worker".into(),
            requested_shards: vec!["project".into()],
        }
    }

    #[test]
    fn mint_then_verify_roundtrips() {
        let src = MockTokenSource::new();
        let jwt = src.mint(&req()).unwrap();
        let claims = verify_jwt(&jwt, &src.jwks_json()).unwrap();
        assert_eq!(claims.tenant, "promptexecution");
        assert_eq!(claims.sub, "agent/sm3lly");
        assert_eq!(claims.r0le, "worker");
        assert_eq!(claims.scopes, ["project"]);
        assert_eq!(claims.exp - claims.iat, 900);
    }

    #[test]
    fn verify_rejects_a_foreign_jwks() {
        let src = MockTokenSource::new();
        let jwt = src.mint(&req()).unwrap();
        let other = r#"{"keys":[{"kty":"RSA","use":"sig","alg":"RS256","kid":"mock-kid","n":"AQAB","e":"AQAB"}]}"#;
        assert!(verify_jwt(&jwt, other).is_err());
    }

    #[test]
    fn decode_unverified_reads_claims_without_a_key() {
        let src = MockTokenSource::new();
        let jwt = src.mint(&req()).unwrap();
        let claims = decode_claims_unverified(&jwt).unwrap();
        assert_eq!(claims.iss, ISS);
    }

    #[tokio::test]
    async fn token_source_trait_object_works() {
        let src: Box<dyn TokenSource> = Box::new(MockTokenSource::new());
        let jwt = src.obtain(&req()).await.unwrap();
        assert!(jwt.split('.').count() == 3);
    }
}
