//! DBus client proxy for b00t hive control
//!
//! Generated proxy mirrors the `com.promptexecution.b00t1` interface.
//! Used by CLI commands and agents to call the central b00t.service
//! without sudo.

/// Well-known bus name
pub const BUS_NAME: &str = "com.promptexecution.b00t1";
/// Object path
pub const OBJECT_PATH: &str = "/com/promptexecution/b00t1";

#[zbus::proxy(
    interface = "com.promptexecution.b00t1",
    default_service = "com.promptexecution.b00t1",
    default_path = "/com/promptexecution/b00t1"
)]
pub trait B00tControl {
    /// Liveness check
    async fn ping(&self) -> zbus::Result<String>;

    /// JSON SystemSnapshot
    async fn get_system_status(&self) -> zbus::Result<String>;

    /// systemctl status output for a unit
    async fn service_status(&self, unit: &str) -> zbus::Result<String>;

    /// Start a systemd unit — returns true on success
    async fn service_start(&self, unit: &str) -> zbus::Result<bool>;

    /// Stop a systemd unit — returns true on success
    async fn service_stop(&self, unit: &str) -> zbus::Result<bool>;

    /// Activate hive profile — returns JSON StackResult
    async fn stack_activate(&self, profile: &str, force: bool) -> zbus::Result<String>;

    /// Deactivate hive profile — returns JSON StackResult
    async fn stack_deactivate(&self, profile: &str) -> zbus::Result<String>;
}

/// Connect to b00t DBus service on the system bus
pub async fn connect_system() -> zbus::Result<B00tControlProxy<'static>> {
    let connection = zbus::Connection::system().await?;
    B00tControlProxy::new(&connection).await
}

/// Connect to b00t DBus service on the session bus (dev/test)
pub async fn connect_session() -> zbus::Result<B00tControlProxy<'static>> {
    let connection = zbus::Connection::session().await?;
    B00tControlProxy::new(&connection).await
}

/// Try system bus first, fall back to session bus
pub async fn connect_auto() -> zbus::Result<B00tControlProxy<'static>> {
    match connect_system().await {
        Ok(proxy) => Ok(proxy),
        Err(_) => connect_session().await,
    }
}
