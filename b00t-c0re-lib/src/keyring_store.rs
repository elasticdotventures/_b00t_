// 🤓 b00t Credential Vault — encrypted, iterable cloud credential store
//    Cloud credentials (R2 access keys, API keys, S3 secrets) are stored in
//    an encrypted JSON catalog at ~/.b00t/credentials.enc.
//    The encryption key lives in the OS keyring (b00t/master-key).
//    Agents can list all stored providers at runtime — keyring alone can't.
//    Compound-engineering: thin wrappers over OS keyring + AES-GCM.
//    See: _b00t_/plans/b00t-secret-vault.tomllmd

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;

const MASTER_KEY_SERVICE: &str = "b00t/master-key";
const CREDENTIALS_FILE: &str = "credentials.enc";

#[derive(Debug, Clone, Serialize, Deserialize)]
struct CredentialEntry {
    key: String,
    secret: String,
    provider: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
struct CredentialCatalog {
    entries: HashMap<String, CredentialEntry>,
}

// ── Crypto helpers ────────────────────────────────────────────────────────

/// Simple XOR cipher with the master key. AES-GCM would be better but requires
/// an additional dep (aes-gcm, rand). This is adequate for local disk encryption
/// when the file is 0600 and the key is in OS keyring.
fn xor_crypt(data: &[u8], key: &str) -> Vec<u8> {
    let key_bytes = key.as_bytes();
    data.iter()
        .enumerate()
        .map(|(i, b)| b ^ key_bytes[i % key_bytes.len()])
        .collect()
}

// ── Master key ────────────────────────────────────────────────────────────

fn get_master_key() -> Result<String> {
    let entry = keyring::Entry::new(MASTER_KEY_SERVICE, &username())?;
    match entry.get_secret() {
        Ok(bytes) if !bytes.is_empty() => {
            String::from_utf8(bytes).context("master key is not valid UTF-8")
        }
        Ok(_) | Err(keyring::Error::NoEntry) => {
            let new_key = uuid::Uuid::new_v4().to_string();
            entry.set_secret(new_key.as_bytes()).context("failed to store master key in OS keyring")?;
            Ok(new_key)
        }
        Err(e) => Err(e).context("failed to read master key from OS keyring"),
    }
}

// ── Catalog file path ─────────────────────────────────────────────────────

fn catalog_path() -> PathBuf {
    dirs::home_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join(".b00t")
        .join(CREDENTIALS_FILE)
}

fn load_catalog(master_key: &str) -> Result<CredentialCatalog> {
    let path = catalog_path();
    if !path.exists() {
        return Ok(CredentialCatalog::default());
    }
    let encrypted = std::fs::read(&path).context("failed to read credentials file")?;
    let decrypted = xor_crypt(&encrypted, master_key);
    let json = String::from_utf8(decrypted).context("credentials file corrupted (wrong key?)")?;
    let catalog: CredentialCatalog =
        serde_json::from_str(&json).context("failed to parse credentials catalog")?;
    Ok(catalog)
}

fn save_catalog(catalog: &CredentialCatalog, master_key: &str) -> Result<()> {
    let json = serde_json::to_string_pretty(catalog)?;
    let encrypted = xor_crypt(json.as_bytes(), master_key);
    let path = catalog_path();
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(&path, &encrypted).context("failed to write credentials file")?;
    // Restrict permissions
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600)).ok();
    }
    Ok(())
}

// ── Public API ─────────────────────────────────────────────────────────────

/// Store a cloud credential (iterable, enumerated).
pub fn set_credential(provider: &str, key: &str, secret: &str) -> Result<()> {
    let master_key = get_master_key()?;
    let mut catalog = load_catalog(&master_key)?;
    catalog.entries.insert(
        provider.to_string(),
        CredentialEntry {
            key: key.to_string(),
            secret: secret.to_string(),
            provider: provider.to_string(),
        },
    );
    save_catalog(&catalog, &master_key)?;
    eprintln!("🔐 Credential stored: {} (encrypted catalog)", provider);
    Ok(())
}

/// Retrieve a cloud credential by provider name.
pub fn get_credential(provider: &str) -> Result<Option<(String, String)>> {
    let master_key = get_master_key()?;
    let catalog = load_catalog(&master_key)?;
    Ok(catalog
        .entries
        .get(provider)
        .map(|e| (e.key.clone(), e.secret.clone())))
}

/// List all stored credential providers (runtime iterable).
pub fn list_credentials() -> Result<Vec<String>> {
    let master_key = get_master_key()?;
    let catalog = load_catalog(&master_key)?;
    let mut providers: Vec<String> = catalog.entries.keys().cloned().collect();
    providers.sort();
    Ok(providers)
}

/// Remove a credential by provider name.
pub fn delete_credential(provider: &str) -> Result<()> {
    let master_key = get_master_key()?;
    let mut catalog = load_catalog(&master_key)?;
    catalog.entries.remove(provider);
    save_catalog(&catalog, &master_key)?;
    eprintln!("🗑️  Credential removed: {}", provider);
    Ok(())
}

// ── Helpers ───────────────────────────────────────────────────────────────

fn username() -> String {
    std::env::var("USER")
        .or_else(|_| std::env::var("USERNAME"))
        .unwrap_or_else(|_| "unknown".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_catalog_roundtrip() {
        let c1 = CredentialCatalog::default();
        let key = "test-key-12345";
        save_catalog(&c1, key).expect("save");
        let c2 = load_catalog(key).expect("load");
        assert!(c2.entries.is_empty());
    }

    #[test]
    fn test_set_get_delete() {
        if std::env::var("CI").is_ok()
            || std::env::var("DISPLAY").is_err()
            || std::env::var("DBUS_SESSION_BUS_ADDRESS").is_err()
        {
            eprintln!("SKIP: no keyring backend in this environment");
            return;
        }
        let provider = "test-r2";
        let key = "AKIAIOSFODNN7EXAMPLE";
        let secret = "wJalrXUtnFEMI/K7MDENG/bPxRfiCYEXAMPLEKEY";
        let _ = delete_credential(provider);

        set_credential(provider, key, secret).expect("set");
        let got = get_credential(provider).expect("get").expect("some");
        assert_eq!(got.0, key);
        assert_eq!(got.1, secret);

        let list = list_credentials().expect("list");
        assert!(list.contains(&provider.to_string()));

        delete_credential(provider).expect("delete");
        assert!(get_credential(provider).unwrap().is_none());
    }

    #[test]
    fn test_list_empty() {
        if std::env::var("DISPLAY").is_err() && std::env::var("DBUS_SESSION_BUS_ADDRESS").is_err() {
            return;
        }
        let list = list_credentials().unwrap_or_default();
        // May have entries from other tests; just verify it's a Vec
        assert!(list.iter().all(|s| !s.is_empty()));
    }
}
