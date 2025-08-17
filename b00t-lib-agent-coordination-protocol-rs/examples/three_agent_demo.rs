//! Three-agent ACP coordination demo
//! 
//! Agent 1 & 2: Propose random numbers
//! Agent 3: Receives proposals and adds them together
//! 
//! Demonstrates step-based coordination with PROPOSE/STATUS message flow

use b00t_acp::{AgentConfig, MessageType, ACPMessage};
use serde_json::json;
use tokio::time::{sleep, Duration};
use tracing::{info, error, Level};
use tracing_subscriber;
use std::sync::Arc;
use tokio::sync::Mutex;
use std::collections::HashMap;
use rand::Rng;

/// Shared message bus for demo (replaces real NATS)
type MessageBus = Arc<Mutex<HashMap<String, Vec<ACPMessage>>>>;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize logging
    tracing_subscriber::fmt()
        .with_max_level(Level::INFO)
        .init();

    info!("🥾 Starting three-agent ACP coordination demo");

    // Create shared message bus for agents to communicate
    let message_bus: MessageBus = Arc::new(Mutex::new(HashMap::new()));

    // Spawn three agents concurrently
    let (agent1_handle, agent2_handle, agent3_handle) = tokio::join!(
        spawn_number_proposer_agent("agent1", 1, Arc::clone(&message_bus)),
        spawn_number_proposer_agent("agent2", 2, Arc::clone(&message_bus)),
        spawn_calculator_agent(Arc::clone(&message_bus))
    );

    // Check results
    match (agent1_handle, agent2_handle, agent3_handle) {
        (Ok(_), Ok(_), Ok(sum)) => {
            info!("🎉 Demo completed successfully! Final sum: {}", sum);
        }
        _ => {
            error!("❌ Demo failed - one or more agents had errors");
        }
    }

    Ok(())
}

/// Agent that proposes a random number
async fn spawn_number_proposer_agent(
    agent_id: &str, 
    agent_num: u32,
    message_bus: MessageBus
) -> Result<i32, Box<dyn std::error::Error + Send + Sync>> {
    let config = AgentConfig::new(
        agent_id.to_string(),
        "demo://localhost".to_string(),
        "demo.namespace".to_string(),
    )
    .with_role("number-proposer".to_string())
    .with_timeout(5000);

    // Generate random number for this agent
    let mut rng = rand::thread_rng();
    let random_number: i32 = rng.gen_range(1..=100);
    
    info!("🎲 Agent {} starting with number: {}", agent_num, random_number);

    // Send STATUS: Agent ready
    let status_msg = ACPMessage::status(
        agent_id.to_string(),
        1,
        json!({
            "description": format!("Agent {} ready with number {}", agent_num, random_number),
            "agent_type": "number-proposer",
            "ready": true
        })
    );
    
    publish_message(&message_bus, &status_msg).await;
    info!("📢 Agent {} sent STATUS: ready", agent_num);

    // Wait a moment for all agents to be ready
    sleep(Duration::from_millis(100)).await;

    // Send PROPOSE: Random number
    let propose_msg = ACPMessage::propose(
        agent_id.to_string(),
        1,
        json!({
            "action": "propose_number",
            "number": random_number,
            "agent": agent_num,
            "description": format!("Agent {} proposes number {}", agent_num, random_number)
        })
    );
    
    publish_message(&message_bus, &propose_msg).await;
    info!("💡 Agent {} sent PROPOSE: number {}", agent_num, random_number);

    // Send STEP: Complete step 1
    let step_msg = ACPMessage::step_complete(agent_id.to_string(), 1);
    publish_message(&message_bus, &step_msg).await;
    info!("✅ Agent {} completed step 1", agent_num);

    Ok(random_number)
}

/// Agent that receives proposals and calculates sum
async fn spawn_calculator_agent(
    message_bus: MessageBus
) -> Result<i32, Box<dyn std::error::Error + Send + Sync>> {
    let agent_id = "agent3";
    let config = AgentConfig::new(
        agent_id.to_string(),
        "demo://localhost".to_string(),
        "demo.namespace".to_string(),
    )
    .with_role("calculator".to_string())
    .with_timeout(5000);

    info!("🧮 Calculator agent starting");

    // Send STATUS: Agent ready
    let status_msg = ACPMessage::status(
        agent_id.to_string(),
        1,
        json!({
            "description": "Calculator agent ready to receive proposals",
            "agent_type": "calculator",
            "ready": true
        })
    );
    
    publish_message(&message_bus, &status_msg).await;
    info!("📢 Calculator sent STATUS: ready");

    // Wait for proposals from other agents
    let proposals = wait_for_proposals(&message_bus, 2, Duration::from_secs(3)).await?;
    
    if proposals.len() != 2 {
        return Err(format!("Expected 2 proposals, got {}", proposals.len()).into());
    }

    // Extract numbers from proposals
    let mut numbers = Vec::new();
    for proposal in &proposals {
        if let Some(number) = proposal.payload["number"].as_i64() {
            numbers.push(number as i32);
            info!("📥 Calculator received: {} from {}", number, proposal.agent_id);
        }
    }

    // Calculate sum
    let sum: i32 = numbers.iter().sum();
    info!("🧮 Calculator computed: {} + {} = {}", numbers[0], numbers[1], sum);

    // Send STATUS: Calculation result
    let result_msg = ACPMessage::status(
        agent_id.to_string(),
        1,
        json!({
            "description": format!("Calculated sum: {}", sum),
            "action": "calculation_complete",
            "inputs": numbers,
            "result": sum,
            "agent_type": "calculator"
        })
    );
    
    publish_message(&message_bus, &result_msg).await;
    info!("📊 Calculator sent STATUS: result {}", sum);

    // Send STEP: Complete step 1
    let step_msg = ACPMessage::step_complete(agent_id.to_string(), 1);
    publish_message(&message_bus, &step_msg).await;
    info!("✅ Calculator completed step 1");

    Ok(sum)
}

/// Publish message to shared bus (simulates NATS)
async fn publish_message(message_bus: &MessageBus, message: &ACPMessage) {
    let mut bus = message_bus.lock().await;
    let message_type_id = match message.message_type {
        MessageType::Status => 0,
        MessageType::Propose => 1,
        MessageType::Step => 2,
    };
    let key = format!("step.{}.{}", message.step, message_type_id);
    bus.entry(key).or_insert_with(Vec::new).push(message.clone());
}

/// Wait for PROPOSE messages from other agents
async fn wait_for_proposals(
    message_bus: &MessageBus, 
    expected_count: usize,
    timeout_duration: Duration
) -> Result<Vec<ACPMessage>, Box<dyn std::error::Error + Send + Sync>> {
    let start_time = std::time::Instant::now();
    
    loop {
        // Check for timeout
        if start_time.elapsed() > timeout_duration {
            return Err("Timeout waiting for proposals".into());
        }

        // Check message bus for PROPOSE messages
        {
            let bus = message_bus.lock().await;
            let key = "step.1.1".to_string(); // MessageType::Propose = 1
            
            if let Some(messages) = bus.get(&key) {
                let proposals: Vec<ACPMessage> = messages.iter()
                    .filter(|msg| msg.payload.get("action").and_then(|v| v.as_str()) == Some("propose_number"))
                    .cloned()
                    .collect();
                
                if proposals.len() >= expected_count {
                    return Ok(proposals);
                }
            }
        }

        // Small delay before checking again
        sleep(Duration::from_millis(50)).await;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_three_agent_demo() {
        // This test would run the demo but we'll keep it simple
        let message_bus: MessageBus = Arc::new(Mutex::new(HashMap::new()));
        
        // Test message publishing
        let test_msg = ACPMessage::status(
            "test-agent".to_string(),
            1,
            json!({"test": true})
        );
        
        publish_message(&message_bus, &test_msg).await;
        
        let bus = message_bus.lock().await;
        assert!(!bus.is_empty());
    }

    #[tokio::test]
    async fn test_proposal_waiting() {
        let message_bus: MessageBus = Arc::new(Mutex::new(HashMap::new()));
        
        // Add a proposal message
        let proposal = ACPMessage::propose(
            "test-agent".to_string(),
            1,
            json!({
                "action": "propose_number",
                "number": 42
            })
        );
        
        publish_message(&message_bus, &proposal).await;
        
        // Wait for proposals (should find 1)
        let result = wait_for_proposals(&message_bus, 1, Duration::from_millis(100)).await;
        assert!(result.is_ok());
        
        let proposals = result.unwrap();
        assert_eq!(proposals.len(), 1);
        assert_eq!(proposals[0].payload["number"], 42);
    }
}