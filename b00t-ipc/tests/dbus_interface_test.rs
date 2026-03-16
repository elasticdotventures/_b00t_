//! Unit tests for DBus interface types — no DBus connection required

#[cfg(feature = "dbus")]
mod dbus_types {
    use b00t_ipc::dbus_interface::StackResult;

    #[test]
    fn stack_result_serialize_roundtrip() {
        let result = StackResult {
            success: true,
            log: vec![
                "stop b00t-hive-idle.service".into(),
                "start b00t-hive-qwen3.service".into(),
                "profile 'inference-qwen3' activated".into(),
            ],
        };

        let json = serde_json::to_string(&result).unwrap();
        let parsed: StackResult = serde_json::from_str(&json).unwrap();

        assert!(parsed.success);
        assert_eq!(parsed.log.len(), 3);
        assert!(parsed.log[2].contains("activated"));
    }

    #[test]
    fn stack_result_failure() {
        let result = StackResult {
            success: false,
            log: vec!["resource gate failed: need 16GB free, have 4GB".into()],
        };

        let json = serde_json::to_string(&result).unwrap();
        let parsed: StackResult = serde_json::from_str(&json).unwrap();

        assert!(!parsed.success);
        assert_eq!(parsed.log.len(), 1);
    }

    #[test]
    fn stack_result_empty_log() {
        let result = StackResult {
            success: true,
            log: vec![],
        };

        let json = serde_json::to_string(&result).unwrap();
        assert!(json.contains("\"log\":[]"));
    }

    #[test]
    fn stack_result_deserialize_from_literal() {
        let json = r#"{"success":true,"log":["pong"]}"#;
        let result: StackResult = serde_json::from_str(json).unwrap();
        assert!(result.success);
        assert_eq!(result.log, vec!["pong"]);
    }
}
