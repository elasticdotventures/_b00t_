//! # Credential Datum — b00t-c0re-lib
//!
//! Schema for encrypted cloud credentials within the b00t datum system.
//! Each credential is a `.credential.toml` file in `_b00t_/`.
//! The secret is encrypted with the OS keyring master key.
//! Queryable via: b00t datum list --type credential, b00t grok digest -t credential
//!
//! 🤓 DatumType::Credential — first-class in the 24-variant taxonomy.

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

/// A cloud provider credential stored as a b00t datum.
/// The `secret_encrypted` field is base64-encoded ciphertext.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CredentialDatum {
    /// b00t metadata
    pub b00t: CredentialMeta,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CredentialMeta {
    pub name: String,
    #[serde(rename = "type", default)]
    pub datum_type: Option<String>,
    #[serde(default)]
    pub hint: Option<String>,

    /// Credential-specific fields
    #[serde(default)]
    pub credential: Option<CredentialFields>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CredentialFields {
    /// Provider identifier (openai, cloudflare-r2, aws-s3, etc.)
    pub provider: String,
    /// Access key / key ID (stored in plaintext — not secret)
    #[serde(default)]
    pub access_key: Option<String>,
    /// Encrypted secret (base64, XOR'd with master key from OS keyring)
    #[serde(default)]
    pub secret_encrypted: Option<String>,
    /// Environment variable name for the key ID
    #[serde(default)]
    pub key_env: Option<String>,
    /// Environment variable name for the secret
    #[serde(default)]
    pub secret_env: Option<String>,
}

// ── Encryption helpers ────────────────────────────────────────────────────

/// Simple XOR cipher with the master key. AES-GCM would be better but requires aes-gcm + rand deps.
/// XOR is adequate when: (a) the master key is in OS keyring, (b) the file is 0600, (c) the ciphertext
/// is only on local disk. The real protection is the OS keyring + file permissions; XOR prevents
/// accidental exposure (e.g., `cat .credential.toml` doesn't show the secret).
/// TODO(#544): Replace XOR with AES-256-GCM — see tracking issue for migration plan
fn xor_crypt(data: &[u8], key: &str) -> Vec<u8> {
    let key_bytes = key.as_bytes();
    data.iter()
        .enumerate()
        .map(|(i, b)| b ^ key_bytes[i % key_bytes.len()])
        .collect()
}

/// Get or create the master key from OS keyring.
pub fn master_key() -> Result<String> {
    let entry = keyring::Entry::new("b00t/master-key", &username())?;
    match entry.get_secret() {
        Ok(bytes) if !bytes.is_empty() => {
            String::from_utf8(bytes).context("master key is not valid UTF-8")
        }
        Ok(_) | Err(keyring::Error::NoEntry) => {
            let new_key = uuid::Uuid::new_v4().to_string();
            entry
                .set_secret(new_key.as_bytes())
                .context("failed to store master key in OS keyring")?;
            Ok(new_key)
        }
        Err(e) => Err(e).context("failed to read master key from OS keyring"),
    }
}

/// Encrypt a secret string with the master key, returning base64.
pub fn encrypt_secret(secret: &str) -> Result<String> {
    let key = master_key()?;
    let encrypted = xor_crypt(secret.as_bytes(), &key);
    Ok(base64::Engine::encode(&base64::engine::general_purpose::STANDARD, &encrypted))
}

/// Decrypt a base64-encoded ciphertext with the master key.
pub fn decrypt_secret(encrypted_b64: &str) -> Result<String> {
    let key = master_key()?;
    let encrypted = base64::Engine::decode(
        &base64::engine::general_purpose::STANDARD,
        encrypted_b64,
    )
    .context("invalid base64 in credential datum")?;
    let decrypted = xor_crypt(&encrypted, &key);
    String::from_utf8(decrypted).context("decrypted credential is not valid UTF-8 (wrong key?)")
}

fn username() -> String {
    std::env::var("USER")
        .or_else(|_| std::env::var("USERNAME"))
        .unwrap_or_else(|_| "unknown".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_encrypt_decrypt_roundtrip() {
        let secret = "sk-test-secret-12345";
        let test_key = uuid::Uuid::new_v4().to_string();
        let encrypted = xor_crypt(secret.as_bytes(), &test_key);
        let decrypted = xor_crypt(&encrypted, &test_key);
        assert_eq!(String::from_utf8(decrypted).unwrap(), secret);
    }
}

// ── Datum operations (filesystem is the database) ─────────────────────────

/// Scan `_b00t_/*.credential.toml` and find a credential by provider name.
/// Returns (access_key, secret) with secret decrypted from the encrypted field.
pub fn find_credential_by_name(provider: &str) -> Result<Option<(String, String)>> {
    for datum in list_credential_files()? {
        if datum.b00t.name == provider
            || datum.b00t.credential.as_ref().map(|c| c.provider.as_str()) == Some(provider)
        {
            if let Some(ref cred) = datum.b00t.credential {
                let secret = if let Some(ref enc) = cred.secret_encrypted {
                    decrypt_secret(enc).unwrap_or_default()
                } else {
                    String::new()
                };
                let key = cred.access_key.clone().unwrap_or_default();
                return Ok(Some((key, secret)));
            }
        }
    }
    Ok(None)
}

/// List all credential provider names from `_b00t_/*.credential.toml`.
pub fn list_credential_names() -> Result<Vec<String>> {
    let mut names = Vec::new();
    for datum in list_credential_files()? {
        names.push(datum.b00t.name.clone());
    }
    names.sort();
    Ok(names)
}

/// Create or update a credential datum file.
pub fn save_credential(provider: &str, key: &str, secret: &str) -> Result<()> {
    let encrypted = encrypt_secret(secret)?;
    let toml_content = format!(
        r#"[b00t]
name = "{provider}"
type = "credential"
hint = "Cloud credential for {provider} — encrypted at rest"

[b00t.credential]
provider = "{provider}"
access_key = "{key}"
secret_encrypted = "{encrypted}"
key_env = "{key_env}"
secret_env = "{secret_env}"
"#,
        provider = provider,
        key = key,
        encrypted = encrypted,
        key_env = key_env_for(provider).0,
        secret_env = key_env_for(provider).1,
    );
    let path = credential_path(provider);
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(&path, &toml_content)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600)).ok();
    }
    Ok(())
}

/// Delete a credential datum file.
pub fn delete_credential(provider: &str) -> Result<()> {
    let path = credential_path(provider);
    if path.exists() {
        std::fs::remove_file(&path)?;
    }
    Ok(())
}

// ── Internal helpers ──────────────────────────────────────────────────────

fn credential_path(provider: &str) -> std::path::PathBuf {
    b00t_data_dir().join(format!("{}.credential.toml", provider))
}

fn b00t_data_dir() -> std::path::PathBuf {
    dirs::home_dir()
        .unwrap_or_else(|| std::path::PathBuf::from("."))
        .join(".b00t")
        .join("_b00t_")
}

fn list_credential_files() -> Result<Vec<CredentialDatum>> {
    let dir = b00t_data_dir();
    let mut datums = Vec::new();
    if !dir.exists() {
        return Ok(datums);
    }
    for entry in std::fs::read_dir(&dir).context("failed to read _b00t_ directory")? {
        let entry = entry?;
        let path = entry.path();
        if path.extension().map_or(false, |e| e == "toml")
            && path
                .file_name()
                .and_then(|n| n.to_str())
                .map_or(false, |n| n.ends_with(".credential.toml"))
        {
            let content = std::fs::read_to_string(&path)?;
            if let Ok(datum) = toml::from_str::<CredentialDatum>(&content) {
                datums.push(datum);
            }
        }
    }
    Ok(datums)
}

/// Map provider name to environment variable names.
pub fn key_env_for(provider: &str) -> (String, String) {
    match provider {
        "openai" => ("OPENAI_API_KEY".into(), "OPENAI_API_KEY".into()),
        "anthropic" => ("ANTHROPIC_API_KEY".into(), "ANTHROPIC_API_KEY".into()),
        "openrouter" => ("OPENROUTER_API_KEY".into(), "OPENROUTER_API_KEY".into()),
        "cloudflare-r2" => ("R2_ACCESS_KEY_ID".into(), "R2_SECRET_ACCESS_KEY".into()),
        "aws-s3" | "aws" => ("AWS_ACCESS_KEY_ID".into(), "AWS_SECRET_ACCESS_KEY".into()),
        "qdrant" => ("QDRANT_API_KEY".into(), "QDRANT_API_KEY".into()),
        _ => {
            let prefix = provider.to_uppercase().replace('-', "_");
            (format!("{}_KEY", prefix), format!("{}_SECRET", prefix))
        }
    }
}
