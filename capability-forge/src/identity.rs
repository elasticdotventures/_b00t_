use anyhow::{Context, Result};
use nkeys::KeyPair;

pub struct AgentKeyPair(KeyPair);

impl AgentKeyPair {
    pub fn generate() -> Self {
        Self(KeyPair::new_user())
    }

    pub fn from_seed(seed: &str) -> Result<Self> {
        Ok(Self(KeyPair::from_seed(seed).context("invalid nkeys seed")?))
    }

    pub fn public_key(&self) -> String {
        self.0.public_key()
    }

    pub fn seed(&self) -> Result<String> {
        self.0.seed().context("keypair has no seed (public-key-only instance)")
    }

    pub fn sign(&self, data: &[u8]) -> Result<Vec<u8>> {
        self.0.sign(data).context("signing failed")
    }
}

pub fn verify(public_key: &str, data: &[u8], sig: &[u8]) -> Result<()> {
    let kp = KeyPair::from_public_key(public_key).context("invalid nkeys public key")?;
    kp.verify(data, sig).context("signature verification failed")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sign_and_verify_round_trips() {
        let kp = AgentKeyPair::generate();
        let sig = kp.sign(b"hello capability-forge").unwrap();
        verify(&kp.public_key(), b"hello capability-forge", &sig).unwrap();
    }

    #[test]
    fn verify_rejects_tampered_payload() {
        let kp = AgentKeyPair::generate();
        let sig = kp.sign(b"hello").unwrap();
        assert!(verify(&kp.public_key(), b"goodbye", &sig).is_err());
    }

    #[test]
    fn from_seed_reconstructs_same_public_key() {
        let kp = AgentKeyPair::generate();
        let seed = kp.seed().unwrap();
        let restored = AgentKeyPair::from_seed(&seed).unwrap();
        assert_eq!(kp.public_key(), restored.public_key());
    }
}
