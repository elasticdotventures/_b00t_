use crate::traits::*;
use crate::{ApiProvides, BootDatum, CapabilityRequirement, get_expanded_path};
use anyhow::Result;
use std::collections::HashMap;

pub struct ApiDatum {
    pub datum: BootDatum,
}

impl ApiDatum {
    pub fn from_config(name: &str, path: &str) -> Result<Self> {
        let mut path_buf = get_expanded_path(path)?;
        path_buf.push(format!("{}.api.toml", name));

        if !path_buf.exists() {
            anyhow::bail!("API datum '{}' not found at {}", name, path_buf.display());
        }

        let content = std::fs::read_to_string(&path_buf)?;
        let config: crate::UnifiedConfig = toml::from_str(&content)?;

        Ok(ApiDatum {
            datum: config.b00t,
        })
    }

    /// Check if this API is available (reachable)
    pub async fn is_available(&self) -> bool {
        // Basic check: if depends_on infrastructure, check that
        // For now, just check if env vars are configured
        if let Some(env) = &self.datum.env {
            // If API has OPENAI_API_BASE or similar, consider it potentially available
            for (key, value) in env {
                if key.contains("API") && !value.is_empty() && !value.starts_with("${") {
                    return true;
                }
            }
        }
        false
    }

    /// Get the capability this API provides
    pub fn capability(&self) -> Option<String> {
        self.datum.provides.as_ref()?.capability.clone()
    }

    /// Get the protocol this API implements
    pub fn protocol(&self) -> Option<String> {
        self.datum.protocol.clone()
    }

    /// Check if this API implements a specific protocol
    pub fn implements_protocol(&self, protocol: &str) -> bool {
        if let Some(implements) = &self.datum.implements {
            implements.iter().any(|p| p == protocol)
        } else {
            false
        }
    }

    /// Check if this API provides a specific capability
    pub fn provides_capability(&self, capability: &str) -> bool {
        self.capability().as_deref() == Some(capability)
    }
}

impl TryFrom<(&str, &str)> for ApiDatum {
    type Error = anyhow::Error;

    fn try_from((name, path): (&str, &str)) -> Result<Self, Self::Error> {
        Self::from_config(name, path)
    }
}

impl DatumChecker for ApiDatum {
    fn is_installed(&self) -> bool {
        // API datums are "installed" if their dependencies are satisfied
        // For now, we'll assume they're available if the datum exists
        true
    }

    fn current_version(&self) -> Option<String> {
        // APIs don't have traditional versions, show protocol version
        if let Some(protocol) = &self.datum.protocol {
            Some(format!("Protocol: {}", protocol))
        } else {
            Some("API available".to_string())
        }
    }

    fn desired_version(&self) -> Option<String> {
        self.datum.desires.clone()
    }

    fn version_status(&self) -> VersionStatus {
        if DatumChecker::is_installed(self) {
            VersionStatus::Unknown // APIs are just available/not available
        } else {
            VersionStatus::Missing
        }
    }
}

impl StatusProvider for ApiDatum {
    fn name(&self) -> &str {
        &self.datum.name
    }

    fn subsystem(&self) -> &str {
        "api"
    }

    fn hint(&self) -> &str {
        &self.datum.hint
    }

    fn is_disabled(&self) -> bool {
        false // APIs are not disabled by default
    }
}
