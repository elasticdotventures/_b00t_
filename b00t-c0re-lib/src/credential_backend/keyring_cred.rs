//! Keyring fallback backend (non-Windows platforms).
//!
//! Wraps the `keyring` crate to implement the `CredentialBackend` trait.
//! This is the fallback for Linux, macOS, and other Unix-like platforms.

use anyhow::{Context, Result};

use super::CredentialBackend;

/// Keyring crate-based credential backend.
pub struct KeyringCred;

impl CredentialBackend for KeyringCred {
    fn set_password(service: &str, user: &str, password: &str) -> Result<()> {
        let entry = keyring::Entry::new(service, user)?;
        entry
            .set_secret(password.as_bytes())
            .context("failed to set password in OS keyring")?;
        Ok(())
    }

    fn get_password(service: &str, user: &str) -> Result<Option<String>> {
        let entry = keyring::Entry::new(service, user)?;
        match entry.get_secret() {
            Ok(bytes) if !bytes.is_empty() => Ok(Some(
                String::from_utf8(bytes).context("password from keyring is not valid UTF-8")?,
            )),
            Ok(_) => Ok(None),
            Err(keyring::Error::NoEntry) => Ok(None),
            Err(e) => Err(e).context("failed to get password from OS keyring"),
        }
    }

    fn delete_password(service: &str, user: &str) -> Result<()> {
        let entry = keyring::Entry::new(service, user)?;
        entry
            .delete_credential()
            .context("failed to delete password from OS keyring")?;
        Ok(())
    }
}
