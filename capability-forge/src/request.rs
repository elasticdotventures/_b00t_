use crate::identity::{self, AgentKeyPair};
use anyhow::Result;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CapabilityRequest {
    pub agent_id: String,
    pub requested_skills: Vec<String>,
    pub justification: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SignedRequest {
    pub body: CapabilityRequest,
    pub agent_pubkey: String,
    pub signature: Vec<u8>,
}

impl SignedRequest {
    pub fn sign(kp: &AgentKeyPair, body: CapabilityRequest) -> Result<Self> {
        let bytes = serde_json::to_vec(&body)?;
        let signature = kp.sign(&bytes)?;
        Ok(Self { body, agent_pubkey: kp.public_key(), signature })
    }

    pub fn verify(&self) -> Result<()> {
        let bytes = serde_json::to_vec(&self.body)?;
        identity::verify(&self.agent_pubkey, &bytes, &self.signature)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CapabilityReply {
    pub granted: Vec<String>,
    pub denied: Vec<(String, String)>,
    pub jwt: Option<String>,
    pub expires_at: Option<DateTime<Utc>>,
    /// The persisted [`crate::grant::Grant`]'s `jti` when a grant was minted — the key a
    /// caller needs to later call [`crate::grant::revoke_grant`] on *this specific* grant.
    /// Distinct from the NATS JWT's own internal `jti` claim embedded in `jwt` (that one is a
    /// sha256-hash-derived id nats-jwt computes for the wire token itself and is never
    /// persisted anywhere on our side); the two are unrelated identifiers in unrelated
    /// namespaces, so this field cannot be reconstructed by decoding `jwt`.
    pub jti: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn body() -> CapabilityRequest {
        CapabilityRequest {
            agent_id: "agent-1".into(),
            requested_skills: vec!["skill.read".into()],
            justification: "testing".into(),
        }
    }

    #[test]
    fn signed_request_round_trips_through_json_and_verifies() {
        let kp = AgentKeyPair::generate();
        let signed = SignedRequest::sign(&kp, body()).unwrap();
        let wire = serde_json::to_vec(&signed).unwrap();
        let back: SignedRequest = serde_json::from_slice(&wire).unwrap();
        back.verify().unwrap();
    }

    #[test]
    fn tampered_body_fails_verification() {
        let kp = AgentKeyPair::generate();
        let mut signed = SignedRequest::sign(&kp, body()).unwrap();
        signed.body.requested_skills.push("skill.admin".into());
        assert!(signed.verify().is_err());
    }

    #[test]
    fn wrong_signer_pubkey_fails_verification() {
        let kp = AgentKeyPair::generate();
        let other = AgentKeyPair::generate();
        let mut signed = SignedRequest::sign(&kp, body()).unwrap();
        signed.agent_pubkey = other.public_key();
        assert!(signed.verify().is_err());
    }
}
