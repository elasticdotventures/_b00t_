use b00t_c0re_lib::cloud::CloudflareProvider as CoreCloudflareProvider;
use crate::registry::{Capability, ProviderInfo, ProviderRegistry};
use std::path::Path;

/// Cloudflare provider — wraps the core implementation with datum-driven config.
#[allow(dead_code)]
pub struct CloudflareProvider {
    inner: CoreCloudflareProvider,
    datum_path: Option<std::path::PathBuf>,
}

impl CloudflareProvider {
    /// Create a new CloudflareProvider from env vars (same as core).
    pub fn new() -> Self {
        // Register in the global registry
        Self::register_with_registry(None);
        Self {
            inner: CoreCloudflareProvider::new(),
            datum_path: None,
        }
    }

    /// Create from a specific datum file path for extended config.
    pub fn from_datum(path: &Path) -> anyhow::Result<Self> {
        let reg = ProviderRegistry::global();
        reg.load_from_datum(path)?;
        Ok(Self {
            inner: CoreCloudflareProvider::new(),
            datum_path: Some(path.to_path_buf()),
        })
    }

    /// Register Cloudflare capabilities in the global registry.
    fn register_with_registry(_datum_path: Option<&Path>) {
        // The registry is auto-populated by the Lazy static in ProviderRegistry::global().
        // This method is kept for future extensibility (e.g., datum-path overrides).
        let _reg = ProviderRegistry::global();
    }

    /// Get the provider info from the registry.
    pub fn info(&self) -> Option<ProviderInfo> {
        ProviderRegistry::global().find_by_name("cloudflare")
    }

    /// Delegate to inner provider.
    pub fn inner(&self) -> &CoreCloudflareProvider {
        &self.inner
    }

    /// List what capabilities this provider offers (from registry, not hardcoded).
    pub fn capabilities(&self) -> Vec<Capability> {
        ProviderRegistry::global()
            .find_by_name("cloudflare")
            .map(|p| p.capabilities)
            .unwrap_or_default()
    }
}

impl Default for CloudflareProvider {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_provider_creation() {
        let p = CloudflareProvider::new();
        assert!(p.capabilities().contains(&Capability::Inference));
    }

    #[test]
    fn test_provider_info() {
        let p = CloudflareProvider::new();
        let info = p.info();
        assert!(info.is_some());
        assert_eq!(info.unwrap().name, "cloudflare");
    }
}
