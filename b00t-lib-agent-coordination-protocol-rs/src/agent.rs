//! High-level agent implementation for ACP coordination

use crate::{
    ACPMessage, MessageType, StepBarrier, NatsTransport,
    ACPError, Result, JsonValue
};
use crate::transport::NatsConfig;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::Mutex;
use tokio::time::{timeout, Duration, sleep};
use tracing::{debug, info, warn, error};
use uuid::Uuid;

/// Agent configuration
#[derive(Debug, Clone)]
pub struct AgentConfig {
    /// Unique agent identifier (e.g., "claude.124435")
    pub agent_id: String,
    /// NATS server URL
    pub nats_url: String,
    /// GitHub user namespace (e.g., "account.elasticdotventures")
    pub namespace: String,
    /// JWT token for NATS authentication
    pub jwt_token: Option<String>,
    /// Agent role (ai-assistant, ci-cd, monitoring, etc.)
    pub role: String,
    /// Default timeout for operations in milliseconds
    pub timeout_ms: u64,
}

impl AgentConfig {
    /// Create new agent config
    pub fn new(agent_id: String, nats_url: String, namespace: String) -> Self {
        Self {
            agent_id,
            nats_url,
            namespace,
            jwt_token: None,
            role: "ai-assistant".to_string(),
            timeout_ms: 30000,
        }
    }

    /// Set JWT token for authentication
    pub fn with_jwt(mut self, jwt_token: String) -> Self {
        self.jwt_token = Some(jwt_token);
        self
    }

    /// Set agent role
    pub fn with_role(mut self, role: String) -> Self {
        self.role = role;
        self
    }

    /// Set default timeout
    pub fn with_timeout(mut self, timeout_ms: u64) -> Self {
        self.timeout_ms = timeout_ms;
        self
    }

    /// Validate configuration
    pub fn validate(&self) -> Result<()> {
        if self.agent_id.is_empty() {
            return Err(ACPError::invalid_config("agent_id cannot be empty"));
        }
        if self.nats_url.is_empty() {
            return Err(ACPError::invalid_config("nats_url cannot be empty"));
        }
        if self.namespace.is_empty() {
            return Err(ACPError::invalid_config("namespace cannot be empty"));
        }
        if self.timeout_ms == 0 {
            return Err(ACPError::invalid_config("timeout_ms must be greater than 0"));
        }
        Ok(())
    }
}

/// High-level agent for ACP coordination
pub struct Agent {
    config: AgentConfig,
    transport: NatsTransport,
    step_barrier: Arc<Mutex<StepBarrier>>,
    message_handlers: Arc<Mutex<HashMap<MessageType, Box<dyn Fn(&ACPMessage) + Send + Sync>>>>,
    running: Arc<Mutex<bool>>,
}

impl Agent {
    /// Create new agent
    pub async fn new(config: AgentConfig) -> Result<Self> {
        config.validate()?;

        let nats_config = NatsConfig {
            url: config.nats_url.clone(),
            jwt_token: config.jwt_token.clone(),
            timeout_ms: config.timeout_ms,
            max_reconnect_attempts: Some(5),
            reconnect_delay_ms: 2000,
        };

        let transport = NatsTransport::new(nats_config).await?;
        let step_barrier = Arc::new(Mutex::new(
            StepBarrier::new(vec![config.agent_id.clone()], config.timeout_ms)
        ));

        info!("Agent '{}' initialized", config.agent_id);

        Ok(Self {
            config,
            transport,
            step_barrier,
            message_handlers: Arc::new(Mutex::new(HashMap::new())),
            running: Arc::new(Mutex::new(false)),
        })
    }

    /// Start agent message processing
    pub async fn start(&self) -> Result<()> {
        let mut running = self.running.lock().await;
        if *running {
            return Ok(());
        }
        *running = true;
        drop(running);

        info!("Starting agent '{}'", self.config.agent_id);

        // Subscribe to agent-specific messages
        let agent_subject = format!("{}.agents.{}.{}.>", 
            self.config.namespace, 
            self.config.role,
            self.config.agent_id
        );
        
        let mut subscriber = self.transport.subscribe(&agent_subject).await?;

        // Subscribe to step coordination messages
        let step_subject = format!("{}.acp.>", self.config.namespace);
        let mut step_subscriber = self.transport.subscribe(&step_subject).await?;

        // Message processing loop
        tokio::spawn({
            let handlers = Arc::clone(&self.message_handlers);
            let running = Arc::clone(&self.running);
            let step_barrier = Arc::clone(&self.step_barrier);
            
            async move {
                while *running.lock().await {
                    tokio::select! {
                        // Handle regular messages
                        msg_result = subscriber.next_message(1000) => {
                            match msg_result {
                                Ok(Some(msg)) => {
                                    debug!("Received message: {:?}", msg.message_type);
                                    if let Some(handler) = handlers.lock().await.get(&msg.message_type) {
                                        handler(&msg);
                                    }
                                }
                                Ok(None) => break,
                                Err(ACPError::ReceiveTimeout) => continue,
                                Err(e) => {
                                    error!("Error receiving message: {}", e);
                                    break;
                                }
                            }
                        }
                        
                        // Handle step coordination messages
                        step_result = step_subscriber.next_message(1000) => {
                            match step_result {
                                Ok(Some(msg)) => {
                                    if msg.message_type == MessageType::Step {
                                        let mut barrier = step_barrier.lock().await;
                                        barrier.record_step_completion(msg.step, msg.agent_id);
                                        
                                        if barrier.try_advance_step() {
                                            info!("Advanced to step {}", barrier.current_step());
                                        }
                                    }
                                }
                                Ok(None) => break,
                                Err(ACPError::ReceiveTimeout) => continue,
                                Err(e) => {
                                    error!("Error receiving step message: {}", e);
                                    break;
                                }
                            }
                        }
                    }
                }
            }
        });

        Ok(())
    }

    /// Stop agent
    pub async fn stop(&self) -> Result<()> {
        let mut running = self.running.lock().await;
        *running = false;
        info!("Stopped agent '{}'", self.config.agent_id);
        Ok(())
    }

    /// Send STATUS message
    pub async fn send_status(&self, description: &str, payload: JsonValue) -> Result<()> {
        let step = self.current_step().await;
        let mut full_payload = payload;
        full_payload["description"] = serde_json::Value::String(description.to_string());
        
        let message = ACPMessage::status(
            self.config.agent_id.clone(),
            step,
            full_payload
        );

        let subject = format!("{}.acp.{}.{}.status", 
            self.config.namespace,
            step,
            self.config.agent_id
        );

        self.transport.publish_to_subject(&subject, &message).await?;
        debug!("Sent STATUS: {}", description);
        Ok(())
    }

    /// Send PROPOSE message
    pub async fn send_propose(&self, action: &str, payload: JsonValue) -> Result<()> {
        let step = self.current_step().await;
        let mut full_payload = payload;
        full_payload["action"] = serde_json::Value::String(action.to_string());
        
        let message = ACPMessage::propose(
            self.config.agent_id.clone(),
            step,
            full_payload
        );

        let subject = format!("{}.acp.{}.{}.propose", 
            self.config.namespace,
            step,
            self.config.agent_id
        );

        self.transport.publish_to_subject(&subject, &message).await?;
        debug!("Sent PROPOSE: {}", action);
        Ok(())
    }

    /// Complete current step
    pub async fn complete_step(&self) -> Result<()> {
        let step = self.current_step().await;
        
        let message = ACPMessage::step_complete(
            self.config.agent_id.clone(),
            step
        );

        let subject = format!("{}.acp.{}.{}.step", 
            self.config.namespace,
            step,
            self.config.agent_id
        );

        self.transport.publish_to_subject(&subject, &message).await?;
        
        // Record our own step completion
        {
            let mut barrier = self.step_barrier.lock().await;
            barrier.record_step_completion(step, self.config.agent_id.clone());
        }

        info!("Completed step {}", step);
        Ok(())
    }

    /// Wait for step to complete (all agents)
    pub async fn wait_for_step_complete(&self, step: u64) -> Result<()> {
        let timeout_duration = Duration::from_millis(self.config.timeout_ms);
        
        let result = timeout(timeout_duration, async {
            loop {
                {
                    let barrier = self.step_barrier.lock().await;
                    if barrier.is_step_complete(step) {
                        return Ok::<(), ACPError>(());
                    }
                    
                    let pending = barrier.pending_agents(step);
                    if !pending.is_empty() {
                        debug!("Waiting for agents to complete step {}: {:?}", step, pending);
                    }
                }
                
                sleep(Duration::from_millis(100)).await;
            }
        }).await;

        match result {
            Ok(_) => {
                info!("Step {} completed by all agents", step);
                Ok(())
            }
            Err(_) => {
                warn!("Step {} timed out, forcing advancement", step);
                let mut barrier = self.step_barrier.lock().await;
                barrier.force_advance_step();
                Err(ACPError::StepTimeout { 
                    step, 
                    timeout_ms: self.config.timeout_ms 
                })
            }
        }
    }

    /// Get current step number
    pub async fn current_step(&self) -> u64 {
        let barrier = self.step_barrier.lock().await;
        barrier.current_step()
    }

    /// Add agent to coordination group
    pub async fn add_agent(&self, agent_id: String) -> Result<()> {
        let mut barrier = self.step_barrier.lock().await;
        
        if barrier.known_agents().contains(&agent_id) {
            return Err(ACPError::AgentAlreadyExists { agent_id });
        }
        
        barrier.add_agent(agent_id.clone());
        info!("Added agent '{}' to coordination group", agent_id);
        Ok(())
    }

    /// Remove agent from coordination group
    pub async fn remove_agent(&self, agent_id: &str) -> Result<()> {
        let mut barrier = self.step_barrier.lock().await;
        
        if !barrier.known_agents().contains(&agent_id.to_string()) {
            return Err(ACPError::AgentNotFound { 
                agent_id: agent_id.to_string() 
            });
        }
        
        barrier.remove_agent(agent_id);
        info!("Removed agent '{}' from coordination group", agent_id);
        Ok(())
    }

    /// Get list of known agents
    pub async fn known_agents(&self) -> Vec<String> {
        let barrier = self.step_barrier.lock().await;
        barrier.known_agents().to_vec()
    }

    /// Get connection status
    pub async fn is_connected(&self) -> bool {
        self.transport.is_connected().await
    }

    /// Register message handler
    pub async fn on_message<F>(&self, message_type: MessageType, handler: F)
    where
        F: Fn(&ACPMessage) + Send + Sync + 'static,
    {
        let mut handlers = self.message_handlers.lock().await;
        handlers.insert(message_type, Box::new(handler));
    }

    /// Send custom message to specific subject
    pub async fn send_message(&self, subject: &str, message: &ACPMessage) -> Result<()> {
        self.transport.publish_to_subject(subject, message).await
    }

    /// Request-response pattern
    pub async fn request(&self, subject: &str, message: &ACPMessage) -> Result<ACPMessage> {
        self.transport.request(subject, message, self.config.timeout_ms).await
    }

    /// Get agent configuration
    pub fn config(&self) -> &AgentConfig {
        &self.config
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_agent_config_validation() {
        let config = AgentConfig::new(
            "".to_string(), // Empty agent_id should fail
            "nats://localhost:4222".to_string(),
            "account.test".to_string()
        );
        
        assert!(config.validate().is_err());
        
        let valid_config = AgentConfig::new(
            "test-agent".to_string(),
            "nats://localhost:4222".to_string(),
            "account.test".to_string()
        );
        
        assert!(valid_config.validate().is_ok());
    }

    #[test]
    fn test_agent_config_builder() {
        let config = AgentConfig::new(
            "test-agent".to_string(),
            "nats://localhost:4222".to_string(),
            "account.test".to_string()
        )
        .with_jwt("jwt-token".to_string())
        .with_role("ci-cd".to_string())
        .with_timeout(60000);
        
        assert_eq!(config.jwt_token, Some("jwt-token".to_string()));
        assert_eq!(config.role, "ci-cd");
        assert_eq!(config.timeout_ms, 60000);
    }
}