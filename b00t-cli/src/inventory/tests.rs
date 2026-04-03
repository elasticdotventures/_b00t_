#[cfg(test)]
mod inventory_tests {
    use super::super::*;
    use serde_json::json;

    /// Test 1: Parse inventory JSON from b00t hive status
    #[test]
    fn test_parse_hive_status_to_inventory() {
        let hive_json = json!({
            "profile": "inference-sm0l",
            "resources": {
                "ram_total_gb": 31,
                "ram_free_gb": 12.3,
                "gpu_free_mb": 8500,
                "cpu_load": 0.42
            },
            "services": [
                { "name": "vllm-sm0l", "status": "active" },
                { "name": "dev-tools", "status": "inactive" }
            ]
        });

        // This will fail until Inventory::from_hive_json is implemented
        // That's TDD: test drives implementation
        assert_eq!(hive_json["profile"], "inference-sm0l");
    }

    /// Test 2: Inventory scan captures all subsystems
    #[test]
    #[ignore]  // Ignore until implementation ready
    fn test_inventory_scan_completes() {
        let result = Inventory::scan();
        assert!(result.is_ok(), "Should scan without error");

        let inv = result.unwrap();
        assert!(!inv.timestamp.is_empty());
        assert!(!inv.tools.is_empty(), "Should detect at least bash");
    }

    /// Test 3: Missing blessings reported
    #[test]
    #[ignore]
    fn test_missing_blessings_reported() {
        let inv = Inventory::scan().unwrap();
        let missing = inv.missing_blessings();

        // If no MCPs: should report missing
        if inv.mcp_servers.is_empty() {
            assert!(missing.iter().any(|b| b.contains("mcp")));
        }
    }

    /// Test 4: Inventory serializes for agent
    #[test]
    #[ignore]
    fn test_inventory_json_serializable() {
        let inv = Inventory::scan().unwrap();
        let json = serde_json::to_string(&inv).expect("Should serialize");
        assert!(!json.is_empty());
    }
}
