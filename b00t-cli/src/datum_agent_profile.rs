//! `AgentProfileSpec` — the payload of a `DatumType::AgentProfile` (`.agentprofile.toml`).
//!
//! A signed, versioned bundle assigned to an agent: the tool allow-list, the
//! skills it may learn, its soul/memory shard grants, model tier, budget
//! ceiling and coarse permissions. Issued by b00t.promptexecution.com's product
//! plane (SP1) and enforced by the b00t-mcp proxy (SP3). Signing is SP2-04.

use serde::{Deserialize, Serialize};

use crate::soul_scope::ShardKind;

/// Cognitive tier the r0le is allowed to run at.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ModelTier {
    Sm0l,
    Ch0nky,
    Frontier,
}

impl Default for ModelTier {
    fn default() -> Self {
        ModelTier::Ch0nky
    }
}

/// Read vs read-write access to a soul/memory shard.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ShardMode {
    R,
    Rw,
}

/// One soul/memory shard grant.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SoulShardGrant {
    pub kind: ShardKind,
    pub id: String,
    pub mode: ShardMode,
}

/// Detached signature over the canonical projection of an `AgentProfileSpec`
/// (see SP2-04 for `canonical_bytes()` / `sign()` / `verify()`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DatumSignature {
    pub kid: String,
    #[serde(default = "default_alg")]
    pub alg: String,
    pub sig_b64: String,
    pub signed_fields_hash: String,
}

fn default_alg() -> String {
    "RS256".to_string()
}

/// The body of a `DatumType::AgentProfile` datum.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub struct AgentProfileSpec {
    /// Tool-name globs the r0le may call (`*` = all).
    #[serde(default)]
    pub tool_allowlist: Vec<String>,
    /// Skill keys the r0le may learn.
    #[serde(default)]
    pub skills: Vec<String>,
    /// Soul/memory shard grants.
    #[serde(default)]
    pub soul_shard_grants: Vec<SoulShardGrant>,
    /// Cognitive tier ceiling.
    #[serde(default)]
    pub model_tier: ModelTier,
    /// Per-issuance budget ceiling, in cake.
    #[serde(default)]
    pub budget_ceiling: u64,
    /// Coarse permission strings (free-form, enforced by the proxy).
    #[serde(default)]
    pub permissions: Vec<String>,
    /// Detached signature (absent on freshly-built, unsigned specs).
    #[serde(default)]
    pub signature: Option<DatumSignature>,
}

#[cfg(test)]
mod tests {
    use super::*;

    const FIXTURE: &str = r#"
tool_allowlist = ["cargo.*", "b00t_status"]
skills = ["rust", "worker"]
model_tier = "ch0nky"
budget_ceiling = 5000
permissions = ["net:egress"]

[[soul_shard_grants]]
kind = "skill"
id = "rust"
mode = "r"

[[soul_shard_grants]]
kind = "agent"
id = "worker"
mode = "rw"
"#;

    #[test]
    fn fixture_deserializes() {
        let spec: AgentProfileSpec = toml::from_str(FIXTURE).unwrap();
        assert_eq!(spec.tool_allowlist, ["cargo.*", "b00t_status"]);
        assert_eq!(spec.model_tier, ModelTier::Ch0nky);
        assert_eq!(spec.budget_ceiling, 5000);
        assert_eq!(spec.soul_shard_grants.len(), 2);
        assert_eq!(spec.soul_shard_grants[0].kind, ShardKind::Skill);
        assert_eq!(spec.soul_shard_grants[1].mode, ShardMode::Rw);
        assert!(spec.signature.is_none());
    }

    #[test]
    fn empty_spec_defaults() {
        let spec: AgentProfileSpec = toml::from_str("").unwrap();
        assert!(spec.tool_allowlist.is_empty());
        assert_eq!(spec.model_tier, ModelTier::Ch0nky);
        assert!(spec.signature.is_none());
    }

    #[test]
    fn bad_model_tier_errors() {
        assert!(toml::from_str::<AgentProfileSpec>("model_tier = \"turbo\"").is_err());
    }
}

// ── SP2-04: canonical projection + RS256 signing (F4) ────────────────────────

use anyhow::{Context, Result, anyhow};
use base64::Engine as _;
use rsa::pkcs1v15::{Signature, SigningKey, VerifyingKey};
use rsa::pkcs8::DecodePrivateKey;
use rsa::signature::{SignatureEncoding, Signer, Verifier};
use rsa::traits::PublicKeyParts;
use rsa::{BigUint, RsaPrivateKey, RsaPublicKey};
use sha2::{Digest, Sha256};

const B64URL: base64::engine::GeneralPurpose = base64::engine::general_purpose::URL_SAFE_NO_PAD;
const B64STD: base64::engine::GeneralPurpose = base64::engine::general_purpose::STANDARD;

/// Recursively normalise a JSON value: object keys sorted, arrays sorted by the
/// canonical string form of their (recursively canonicalised) elements. Makes
/// the signed projection independent of field / list order.
fn canonicalize(v: &serde_json::Value) -> serde_json::Value {
    match v {
        serde_json::Value::Object(m) => {
            let mut keys: Vec<&String> = m.keys().collect();
            keys.sort();
            let mut out = serde_json::Map::new();
            for k in keys {
                out.insert(k.clone(), canonicalize(&m[k]));
            }
            serde_json::Value::Object(out)
        }
        serde_json::Value::Array(a) => {
            let mut items: Vec<serde_json::Value> = a.iter().map(canonicalize).collect();
            items.sort_by(|x, y| x.to_string().cmp(&y.to_string()));
            serde_json::Value::Array(items)
        }
        other => other.clone(),
    }
}

fn sha256_hex(bytes: &[u8]) -> String {
    let digest = Sha256::digest(bytes);
    let mut s = String::with_capacity(64);
    for b in digest {
        s.push_str(&format!("{b:02x}"));
    }
    s
}

/// PKCS#8 RSA private-key PEM for datum signing, from `$B00T_DATUM_SIGNING_KEY_PEM`.
/// (Same key/kid namespace as the SP1 JWT signing key — its public half lives in
/// the identity worker's JWKS.)
pub fn datum_signing_key_pem() -> Result<String> {
    std::env::var("B00T_DATUM_SIGNING_KEY_PEM")
        .map_err(|_| anyhow!("B00T_DATUM_SIGNING_KEY_PEM is not set"))
}

/// Generalizes SP2-04's canonical-projection RS256 signing (originally
/// `AgentProfileSpec`-only) to any signable payload. Added for SP5's
/// `GraphArtifactManifest` (see
/// docs/superpowers/specs/2026-09-11-sp5-graph-artifact-publish-design.md).
/// `AgentProfileSpec`'s own `canonical_bytes`/`sign`/`verify` below now
/// delegate to this trait's default implementations — public API and
/// behavior are unchanged.
pub trait Signable: serde::Serialize {
    fn signature(&self) -> &Option<DatumSignature>;
    fn set_signature(&mut self, sig: DatumSignature);

    /// The deterministic byte string that gets signed: this payload as
    /// canonical JSON with the `signature` field removed, object keys
    /// sorted and arrays order-normalised. NEVER sign raw TOML — the datum
    /// loader re-serialises files (`.tomllmd` > `.tomllm` > `.toml` rank rule).
    fn canonical_bytes(&self) -> Result<Vec<u8>> {
        let mut v = serde_json::to_value(self).context("serialize signable payload")?;
        if let Some(obj) = v.as_object_mut() {
            obj.remove("signature");
        }
        serde_json::to_vec(&canonicalize(&v)).context("serialize canonical projection")
    }

    /// Sign in place with a PKCS#8 RSA private key PEM.
    fn sign(&mut self, kid: &str, pkcs8_pem: &str) -> Result<()> {
        let bytes = self.canonical_bytes()?;
        let priv_key =
            RsaPrivateKey::from_pkcs8_pem(pkcs8_pem).context("parse PKCS#8 signing key")?;
        let signing_key = SigningKey::<Sha256>::new(priv_key);
        let sig = signing_key.sign(&bytes);
        self.set_signature(DatumSignature {
            kid: kid.to_string(),
            alg: "RS256".to_string(),
            sig_b64: B64STD.encode(sig.to_bytes()),
            signed_fields_hash: sha256_hex(&bytes),
        });
        Ok(())
    }

    /// Verify the signature against a JWKS JSON (`{keys:[{kid,n,e,...}]}`).
    /// Errors: unsigned, unknown kid, hash mismatch (payload mutated), bad signature.
    fn verify(&self, jwks_json: &str) -> Result<()> {
        let sig = self
            .signature()
            .as_ref()
            .ok_or_else(|| anyhow!("payload is unsigned"))?;
        let bytes = self.canonical_bytes()?;
        if sha256_hex(&bytes) != sig.signed_fields_hash {
            return Err(anyhow!(
                "signed_fields_hash mismatch — payload mutated after signing"
            ));
        }
        let jwks: serde_json::Value = serde_json::from_str(jwks_json).context("parse JWKS")?;
        let jwk = jwks["keys"]
            .as_array()
            .and_then(|ks| ks.iter().find(|k| k["kid"].as_str() == Some(sig.kid.as_str())))
            .ok_or_else(|| anyhow!("kid '{}' not present in JWKS", sig.kid))?;
        let n_b64 = jwk["n"].as_str().ok_or_else(|| anyhow!("JWK missing 'n'"))?;
        let e_b64 = jwk["e"].as_str().ok_or_else(|| anyhow!("JWK missing 'e'"))?;
        let n = BigUint::from_bytes_be(&B64URL.decode(n_b64).context("decode JWK n")?);
        let e = BigUint::from_bytes_be(&B64URL.decode(e_b64).context("decode JWK e")?);
        let pub_key = RsaPublicKey::new(n, e).context("reconstruct RSA public key")?;
        let verifying_key = VerifyingKey::<Sha256>::new(pub_key);
        let sig_bytes = B64STD.decode(&sig.sig_b64).context("decode sig_b64")?;
        let signature =
            Signature::try_from(sig_bytes.as_slice()).context("parse RSA signature")?;
        verifying_key
            .verify(&bytes, &signature)
            .context("RS256 signature verification failed")?;
        Ok(())
    }
}

impl Signable for AgentProfileSpec {
    fn signature(&self) -> &Option<DatumSignature> {
        &self.signature
    }

    fn set_signature(&mut self, sig: DatumSignature) {
        self.signature = Some(sig);
    }
}

impl AgentProfileSpec {
    /// Delegates to `Signable::canonical_bytes` — kept as an inherent method
    /// for API stability; see that trait for the shared implementation.
    pub fn canonical_bytes(&self) -> Result<Vec<u8>> {
        Signable::canonical_bytes(self)
    }

    /// Delegates to `Signable::sign`.
    pub fn sign(&mut self, kid: &str, pkcs8_pem: &str) -> Result<()> {
        Signable::sign(self, kid, pkcs8_pem)
    }

    /// Delegates to `Signable::verify`.
    pub fn verify(&self, jwks_json: &str) -> Result<()> {
        Signable::verify(self, jwks_json)
    }
}

/// The signed manifest published alongside a tagged graph artifact (KerML
/// view + `iso_ir` JSON). See SP5 continuation:
/// docs/superpowers/specs/2026-09-11-sp5-graph-artifact-publish-design.md.
/// Signing reuses the generalized `Signable` primitive above.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GraphArtifactManifest {
    pub tag: String,
    pub commit_sha: String,
    pub kerml_digest: String,
    pub iso_ir_digest: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub signature: Option<DatumSignature>,
}

impl Signable for GraphArtifactManifest {
    fn signature(&self) -> &Option<DatumSignature> {
        &self.signature
    }

    fn set_signature(&mut self, sig: DatumSignature) {
        self.signature = Some(sig);
    }
}

#[cfg(test)]
mod signing_tests {
    use super::*;

    // Throwaway 2048-bit PKCS#8 key — test fixture only.
    const TEST_PEM: &str = "-----BEGIN PRIVATE KEY-----\nMIIEvAIBADANBgkqhkiG9w0BAQEFAASCBKYwggSiAgEAAoIBAQCRQTcE2IGnzryV\nuKDvf7dJ3FuJSXwBwvXSLWUUOyh/O2rl8D9qM4Q+KMT8l+udqj0Utal95mVXAVHd\n+vg5jCEVGnkhE3UZpQShuWwGmAYlgYFCtkPYFc3qD+L2HJtxet6RZFaaeCmV8FO+\nbPStmPbLYXY6dFECG8mYBTITpFYBtLIiLS4S3Qsw0YArrR4CDgdlpCg41wI0z5uX\n/aGkWKhlpxt8lyr3jaUCsi260vb1xS/2zdM4HJ/LDIMljJICac6ugUyoRAZKYtc8\nvTc3xXHpEcxkafPtyOzG3Fl3lFMsgxRRPEgfn1FKzZ+RExTHDsjOpc9Xi4ihZWQn\nySXqmR4hAgMBAAECggEAESXq0eahf+cXOnG+hifEwrKCF/YV7rtOfA6h5T6KrGKe\nXyD6y5XjYdc8Ujm5NjbX2S8NIHDnu9rLCHLNhTW23h/u9umuJGXn4xPZ3flqmFju\noqqT3dnNInnXqIh+DWqdBfsbgkb3Wd0ydcO1Kx1o3V/XLlV3DtGq/gh2/fyjrrWx\n12Db2m5IlhEfbMz2Rc2XqqH9BsI/mVhBmbxPEFys9Q6JyMV/qZvgQ4lc+aYz51/5\nbxZruYfMKsUOJXg0Aa12M+Ud+4kJ9gvMCcUjPhj5ww7ZkP7972Zg5a8D14iJjXKN\na1qrW+qSJhB/JUe+DK4LW4LO7Fhv7xuwlMccPW05sQKBgQDHdJIcPGaRGQOMAs7k\ncKCtjwnum4BqJu67juH1tWviXShToQSHnauwgRxQrGEOxRyQipHOXVMvghvG9hdk\nF4souiVYCryeQaskpeAn2crnnU2FrF9+og6fYiet7YDlvSeJIeLvy0IA2ji2wD5u\nLY/GuQTNTG9k1k/LPBWBiAD3HQKBgQC6bwv1G6UQXbILY8jZBphqaGYGGvagq6VJ\nMFHxMmZC4dUULlDg25GfSqOT9xRAFcuiy7KLp+Fv7249YC83UX4RrS1F0v9/n3C4\nuZEXEZra5YnSlgIW2bJq59Xfb5fXg81nMEXEPhDQK6vueyuPuNO+oh00f0TlCnW6\nQfjDCisf1QKBgHQvU21fQeAD0i0c9afcc7ymNgLoUkWDqE1ZTgbzR4T0/yi4Awt8\nrSaEDxpvT5pq99i633R2qJ5kDAo6ECYeENIInPhMSNNnLWqLtaeBFtEUsLPNVVNO\n03XEl5iZYRxyszUOqENHA4u7ko3iLnu/zqDT5hgxDjKPJKwes+hgcS+BAoGAEo/N\n0/iNpaR+fo3PyHPUpvt/9OmoVnTgfvn1npsS/WO4sEqwOMMDq6Vlxeyasoq4/Jtl\nSmxLkLZ49llmOg6+C4p/cG1CjPVV5r5rCK3zCgpCf5n52UaRcf1lGNrmdkmkILr4\np0I6sE84zgSrYKLZSiif2cM2G8u/zuyUlO6lPoUCgYBscEwq4AC0t5eHARTpCD92\nFKYbb0W9EL9i0ZwxQ0vJKdbLtMzgANAuFzp6+4807Jxnxr5HKuwc20OBTJ4C40st\n2+HLb/1FtvcEjMeEhG15wOyZfMCwJ4tyeitun2iAuGQfTfMRbye0Y2u4k9ZahJuq\nSwxvnEFXx9VJ2QOvD4beOw==\n-----END PRIVATE KEY-----\n";

    fn test_jwks(kid: &str) -> String {
        let priv_key = RsaPrivateKey::from_pkcs8_pem(TEST_PEM).unwrap();
        let pub_key = priv_key.to_public_key();
        let n = B64URL.encode(pub_key.n().to_bytes_be());
        let e = B64URL.encode(pub_key.e().to_bytes_be());
        format!(
            r#"{{"keys":[{{"kty":"RSA","use":"sig","alg":"RS256","kid":"{kid}","n":"{n}","e":"{e}"}}]}}"#
        )
    }

    fn sample() -> AgentProfileSpec {
        AgentProfileSpec {
            tool_allowlist: vec!["cargo.*".into(), "b00t_status".into()],
            skills: vec!["rust".into()],
            model_tier: ModelTier::Ch0nky,
            budget_ceiling: 100,
            permissions: vec!["net:egress".into()],
            ..Default::default()
        }
    }

    #[test]
    fn sign_then_verify_roundtrips() {
        let mut spec = sample();
        spec.sign("datum-1", TEST_PEM).unwrap();
        assert_eq!(spec.signature.as_ref().unwrap().alg, "RS256");
        spec.verify(&test_jwks("datum-1")).unwrap();
    }

    #[test]
    fn canonical_bytes_ignores_field_and_list_order() {
        let mut a = sample();
        let mut b = sample();
        b.tool_allowlist = vec!["b00t_status".into(), "cargo.*".into()]; // reordered
        assert_eq!(a.canonical_bytes().unwrap(), b.canonical_bytes().unwrap());
        a.sign("k", TEST_PEM).unwrap();
        // b (reordered) verifies against a's signature — same canonical projection
        b.signature = a.signature.clone();
        b.verify(&test_jwks("k")).unwrap();
    }

    #[test]
    fn tampering_a_glob_breaks_verification() {
        let mut spec = sample();
        spec.sign("k", TEST_PEM).unwrap();
        spec.tool_allowlist.push("rm -rf /".into());
        assert!(spec.verify(&test_jwks("k")).is_err());
    }

    #[test]
    fn unknown_kid_errors() {
        let mut spec = sample();
        spec.sign("k", TEST_PEM).unwrap();
        assert!(spec.verify(&test_jwks("other-kid")).is_err());
    }

    #[test]
    fn unsigned_errors() {
        assert!(sample().verify(&test_jwks("k")).is_err());
    }

    fn sample_manifest() -> GraphArtifactManifest {
        GraphArtifactManifest {
            tag: "v1.2.0".into(),
            commit_sha: "deadbeef".into(),
            kerml_digest: "abc123".into(),
            iso_ir_digest: "def456".into(),
            signature: None,
        }
    }

    #[test]
    fn graph_artifact_manifest_sign_then_verify_roundtrips() {
        let mut m = sample_manifest();
        m.sign("graph-kid", TEST_PEM).unwrap();
        assert!(m.signature.is_some());
        m.verify(&test_jwks("graph-kid")).unwrap();
    }

    #[test]
    fn graph_artifact_manifest_tampering_breaks_verification() {
        let mut m = sample_manifest();
        m.sign("graph-kid", TEST_PEM).unwrap();
        m.tag = "v1.2.1-tampered".into();
        let err = m.verify(&test_jwks("graph-kid")).unwrap_err();
        assert!(err.to_string().contains("mutated"));
    }

    #[test]
    fn graph_artifact_manifest_unknown_kid_errors() {
        let mut m = sample_manifest();
        m.sign("graph-kid", TEST_PEM).unwrap();
        assert!(m.verify(&test_jwks("other-kid")).is_err());
    }
}
