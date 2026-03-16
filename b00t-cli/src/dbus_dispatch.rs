//! DBus client dispatch — feature-gated module for routing hive operations
//! through the central b00t.service when available.
//!
//! Falls back silently when DBus is unreachable, so callers can use
//! `dbus_available()` to decide whether to dispatch or go direct.

use anyhow::Result;
use std::sync::OnceLock;

static DBUS_AVAILABLE: OnceLock<bool> = OnceLock::new();

/// Check if the b00t DBus service is reachable (cached after first probe)
pub fn dbus_available() -> bool {
    *DBUS_AVAILABLE.get_or_init(|| {
        let rt = match tokio::runtime::Handle::try_current() {
            Ok(_handle) => {
                // Already in a tokio context — can't block_on, so spawn and check
                // 🤓 This path shouldn't normally hit since hive.rs calls are sync
                return false;
            }
            Err(_) => match tokio::runtime::Runtime::new() {
                Ok(rt) => rt,
                Err(_) => return false,
            },
        };
        rt.block_on(async {
            match b00t_ipc::dbus_client::connect_auto().await {
                Ok(proxy) => proxy.ping().await.is_ok(),
                Err(_) => false,
            }
        })
    })
}

/// Activate a hive profile via DBus — returns the log lines
pub fn dbus_stack_activate(profile: &str, force: bool) -> Result<Vec<String>> {
    let rt = tokio::runtime::Runtime::new()?;
    rt.block_on(async {
        let proxy = b00t_ipc::dbus_client::connect_auto().await?;
        let json = proxy.stack_activate(profile, force).await?;
        let result: b00t_ipc::dbus_interface::StackResult = serde_json::from_str(&json)?;
        if result.success {
            Ok(result.log)
        } else {
            anyhow::bail!("DBus activate failed: {}", result.log.join("; "));
        }
    })
}

/// Deactivate a hive profile via DBus
pub fn dbus_stack_deactivate(profile: &str) -> Result<Vec<String>> {
    let rt = tokio::runtime::Runtime::new()?;
    rt.block_on(async {
        let proxy = b00t_ipc::dbus_client::connect_auto().await?;
        let json = proxy.stack_deactivate(profile).await?;
        let result: b00t_ipc::dbus_interface::StackResult = serde_json::from_str(&json)?;
        if result.success {
            Ok(result.log)
        } else {
            anyhow::bail!("DBus deactivate failed: {}", result.log.join("; "));
        }
    })
}

/// Start a systemd unit via DBus
pub fn dbus_service_start(unit: &str) -> Result<bool> {
    let rt = tokio::runtime::Runtime::new()?;
    rt.block_on(async {
        let proxy = b00t_ipc::dbus_client::connect_auto().await?;
        Ok(proxy.service_start(unit).await?)
    })
}

/// Stop a systemd unit via DBus
pub fn dbus_service_stop(unit: &str) -> Result<bool> {
    let rt = tokio::runtime::Runtime::new()?;
    rt.block_on(async {
        let proxy = b00t_ipc::dbus_client::connect_auto().await?;
        Ok(proxy.service_stop(unit).await?)
    })
}
