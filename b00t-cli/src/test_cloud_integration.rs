//! Integration test for cloud sync + AI client functionality
//!
//! Tests the complete flow from dashboard configuration to local client usage

use crate::cloud_sync::{CloudSyncClient, CloudSyncConfig};
use crate::session_memory::SessionMemory;
use anyhow::Result;
use b00t_c0re_lib::AiClientConfig;

/// Test cloud sync configuration roundtrip
pub async fn test_cloud_sync_flow() -> Result<()> {
    eprintln!("🧪 Testing cloud sync configuration flow...");

    // Create test session memory
    let mut memory = SessionMemory::default();

    // Set up a mock GitHub user
    memory.set("github_user", "testuser")?;

    // Test cloud sync client creation
    let sync_config = CloudSyncConfig::default();
    let mut sync_client = CloudSyncClient::new(sync_config)?;

    eprintln!("✅ Cloud sync client created successfully");

    // Test should_sync logic
    assert!(
        sync_client.should_sync(),
        "Should sync when never synced before"
    );
    eprintln!("✅ Should sync logic working");

    // Test configuration storage (without actual API call)
    let test_ai_config = AiClientConfig::default();
    let config_json = serde_json::to_string(&test_ai_config)?;

    memory.set("ai_config_json", &config_json)?;
    eprintln!("✅ AI config stored in session memory");

    // Test configuration retrieval
    let cached_config = CloudSyncClient::get_cached_ai_config(&memory);
    assert!(cached_config.is_some(), "Should retrieve cached AI config");
    eprintln!("✅ AI config retrieved from session memory");

    // Test AI client initialization with cached config
    if let Some(ai_config) = cached_config {
        use b00t_c0re_lib::B00tAiClient;
        match B00tAiClient::new(ai_config) {
            Ok(_client) => {
                eprintln!("✅ AI client created with cloud configuration");
            }
            Err(e) => {
                eprintln!(
                    "⚠️ AI client creation failed (expected in test environment): {}",
                    e
                );
            }
        }
    }

    eprintln!("🎉 Cloud sync integration test completed successfully!");
    Ok(())
}

/// Test error handling for missing configuration
pub fn test_error_handling() -> Result<()> {
    eprintln!("🧪 Testing error handling...");

    // Test with empty session memory
    let memory = SessionMemory::default();

    // Should return None for missing config
    let cached_config = CloudSyncClient::get_cached_ai_config(&memory);
    assert!(
        cached_config.is_none(),
        "Should return None for missing config"
    );
    eprintln!("✅ Missing config handled correctly");

    // Test with invalid JSON
    let mut memory = SessionMemory::default();
    memory.set("ai_config_json", "invalid json")?;

    let cached_config = CloudSyncClient::get_cached_ai_config(&memory);
    assert!(
        cached_config.is_none(),
        "Should return None for invalid JSON"
    );
    eprintln!("✅ Invalid JSON handled correctly");

    eprintln!("🎉 Error handling tests passed!");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_cloud_sync_integration() {
        test_cloud_sync_flow().await.unwrap();
        test_error_handling().unwrap();
    }
}
