//! Integration test: session bus round-trip
//!
//! Starts B00tService on session bus, connects client proxy, calls ping/status.
//! Requires a running D-Bus session bus (standard on any Linux desktop/CI).
//!
//! Run: cargo test --features dbus -p b00t-ipc --test dbus_session_test

#[cfg(feature = "dbus")]
mod session_roundtrip {
    use b00t_ipc::dbus_client;
    use b00t_ipc::dbus_interface::{B00tService, StackResult, dbus_hive_bridge};
    use std::path::PathBuf;

    /// Register minimal bridge stubs for testing (no real hive ops)
    fn register_test_bridge() {
        dbus_hive_bridge::register(
            // capture — return fake snapshot JSON
            || {
                Ok(r#"{"ram_total_gb":31.0,"ram_available_gb":12.0,"cpu_cores":16,"timestamp":"test"}"#.to_string())
            },
            // activate — echo back the profile name
            |profile: &str, _dir: &std::path::Path, _force: bool| {
                Ok(StackResult {
                    success: true,
                    log: vec![format!("test-activated: {profile}")],
                })
            },
            // deactivate
            |profile: &str, _dir: &std::path::Path| {
                Ok(StackResult {
                    success: true,
                    log: vec![format!("test-deactivated: {profile}")],
                })
            },
        );
    }

    #[tokio::test]
    async fn ping_roundtrip() {
        register_test_bridge();

        // Use a unique bus name to avoid conflicts with running service
        let test_bus = format!(
            "com.promptexecution.b00t1.test{}",
            std::process::id()
        );

        let service = B00tService::new(PathBuf::from("/tmp/b00t-test"));

        // Try to connect to session bus — skip if unavailable (headless CI)
        let conn = match zbus::connection::Builder::session() {
            Ok(builder) => {
                match builder
                    .name(test_bus.as_str())
                    .expect("valid bus name")
                    .serve_at("/com/promptexecution/b00t1", service)
                    .expect("valid path")
                    .build()
                    .await
                {
                    Ok(c) => c,
                    Err(e) => {
                        eprintln!("skipping: session bus unavailable: {e}");
                        return;
                    }
                }
            }
            Err(e) => {
                eprintln!("skipping: session bus unavailable: {e}");
                return;
            }
        };

        // Build a client proxy pointing at our test bus name
        let proxy = zbus::proxy::Builder::<dbus_client::B00tControlProxy>::new(&conn)
            .destination(test_bus.as_str())
            .expect("valid dest")
            .path("/com/promptexecution/b00t1")
            .expect("valid path")
            .build()
            .await
            .expect("proxy build");

        // Test ping
        let pong = proxy.ping().await.expect("ping");
        assert_eq!(pong, "pong");

        // Test get_system_status
        let status_json = proxy.get_system_status().await.expect("status");
        assert!(status_json.contains("ram_total_gb"));

        // Test stack_activate
        let activate_json = proxy
            .stack_activate("test-profile", false)
            .await
            .expect("activate");
        let result: StackResult = serde_json::from_str(&activate_json).unwrap();
        assert!(result.success);
        assert!(result.log[0].contains("test-activated"));
    }
}
