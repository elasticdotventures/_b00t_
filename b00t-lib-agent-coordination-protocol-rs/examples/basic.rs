//! Basic example of b00t ACP usage

use b00t_acp::{Agent, AgentConfig, MessageType};
use serde_json::json;
use tokio::time::{sleep, Duration};
use tracing::{info, Level};
use tracing_subscriber;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize logging
    tracing_subscriber::fmt()
        .with_max_level(Level::INFO)
        .init();

    info!("Starting b00t-acp basic example");

    // Create agent configuration
    let config = AgentConfig::new(
        "example-agent".to_string(),
        "nats://c010.promptexecution.com:4222".to_string(),
        "account.elasticdotventures".to_string(),
    )
    .with_role("ai-assistant".to_string())
    .with_timeout(30000);

    // Create and start agent
    let agent = Agent::new(config).await?;
    agent.start().await?;

    info!("Agent started, current step: {}", agent.current_step().await);

    // Send a status message
    agent.send_status("Agent initialized and ready", json!({
        "version": "0.1.0",
        "capabilities": ["coordination", "messaging"],
        "ready": true
    })).await?;

    // Send a proposal
    agent.send_propose("execute_command", json!({
        "command": "git status",
        "working_dir": "/tmp",
        "priority": "normal"
    })).await?;

    // Wait a bit for messages to be processed
    sleep(Duration::from_secs(2)).await;

    // Complete the current step
    let current_step = agent.current_step().await;
    agent.complete_step().await?;

    info!("Completed step {}", current_step);

    // Simulate waiting for other agents (will timeout since we're alone)
    info!("Waiting for step completion (will timeout after 5 seconds)...");
    match agent.wait_for_step_complete(current_step).await {
        Ok(_) => info!("Step completed by all agents"),
        Err(e) => info!("Step wait timed out as expected: {}", e),
    }

    // Show final step
    info!("Final step: {}", agent.current_step().await);

    // Stop agent gracefully
    agent.stop().await?;
    info!("Agent stopped");

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_agent_lifecycle() {
        let config = AgentConfig::new(
            "test-agent".to_string(),
            "nats://localhost:4222".to_string(), // Use local NATS for testing
            "account.test".to_string(),
        );

        // Note: This test requires a running NATS server
        // In CI/CD, you would start a NATS container for testing
        if let Ok(agent) = Agent::new(config).await {
            assert!(!agent.is_connected().await); // Should not be connected yet
            
            if agent.start().await.is_ok() {
                assert!(agent.is_connected().await);
                let _ = agent.stop().await;
            }
        }
    }
}