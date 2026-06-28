//! # Credential Backend
//!
//! Cross-platform credential storage abstraction.
//! Uses direct Win32 API on Windows, keyring crate on other platforms.
//!
//! 🤓 The trait pattern enables test mocking and platform-specific backends.

use anyhow::Result;

/// Cross-platform credential storage trait.
///
/// Implementors provide secure storage for service credentials
/// using the platform's native credential system.
pub trait CredentialBackend {
    /// Store a password for the given service and user.
    fn set_password(service: &str, user: &str, password: &str) -> Result<()>;

    /// Retrieve a password for the given service and user.
    /// Returns `None` if no credential exists.
    fn get_password(service: &str, user: &str) -> Result<Option<String>>;

    /// Delete a password for the given service and user.
    fn delete_password(service: &str, user: &str) -> Result<()>;
}

#[cfg(windows)]
pub mod windows_cred;
#[cfg(not(windows))]
pub mod keyring_cred;

/// Platform-specific credential backend.
///
/// On Windows: uses direct Win32 `CredWriteW`/`CredReadW` API calls.
/// On other platforms: uses the `keyring` crate as a fallback.
#[cfg(windows)]
pub use windows_cred::WindowsCred as PlatformCred;
#[cfg(not(windows))]
pub use keyring_cred::KeyringCred as PlatformCred;
