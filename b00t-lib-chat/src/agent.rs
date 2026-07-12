use crate::{
    ACPMessage, StepBarrier,
    ChatError, ChatResult, JsonValue,
    ChatClient,
};
use std::sync::Arc;
use tokio::sync::Mutex;
use tokio::time::{timeout, Duration, sleep};
use tracing::{debug, info, warn};

#[derive(Debug, Clone)]
pub struct AgentConfig {
    pub agent_id: String,
    pub nats_url: String,
    pub nats_user: Option<String>,
    pub nats_password: Option<String>,
    pub namespace: String,
    pub role: String,
    pub timeout_ms: u64,
}

impl AgentConfig {
    pub fn new(agent_id: String, nats_url: String, namespace: String) -> Self {
        let nats_url = if nats_url.is_empty() {
            std::env::var("NATS_URL")
                .unwrap_or_else(|_| "nats://localhost:4222".to_string())
        } else {
            nats_url
        };

        let nats_user = std::env::var("B00T_HIVE_NATS_USER").ok();
        let nats_password = std::env::var("B00T_HIVE_NATS_PASSWORD").ok();

        Self {
            agent_id,
            nats_url,
            nats_user,
            nats_password,
            namespace,
            role: "ai-assistant".to_string(),
            timeout_ms: 30000,
        }
    }

    pub fn with_role(mut self, role: String) -> Self { self.role = role; self }
    pub fn with_timeout(mut self, timeout_ms: u64) -> Self { self.timeout_ms = timeout_ms; self }

    pub fn from_env(agent_id: String, namespace: String) -> Self {
        Self::new(agent_id, String::new(), namespace)
    }

    pub fn validate(&self) -> ChatResult<()> {
        if self.agent_id.is_empty() {
            return Err(ChatError::Other("agent_id cannot be empty".into()));
        }
        if self.nats_url.is_empty() {
            return Err(ChatError::Other("nats_url cannot be empty".into()));
        }
        if self.namespace.is_empty() {
            return Err(ChatError::Other("namespace cannot be empty".into()));
        }
        if self.timeout_ms == 0 {
            return Err(ChatError::Other("timeout_ms must be greater than 0".into()));
        }
        Ok(())
    }
}

pub struct Agent {
    config: AgentConfig,
    client: ChatClient,
    step_barrier: Arc<Mutex<StepBarrier>>,
    running: Arc<Mutex<bool>>,
}

impl Agent {
    pub async fn new(config: AgentConfig) -> ChatResult<Self> {
        config.validate()?;

        let client = ChatClient::nats(
            Some(config.nats_url.clone()),
            config.nats_user.clone(),
            config.nats_password.clone(),
        )?;

        let step_barrier = Arc::new(Mutex::new(
            StepBarrier::new(vec![config.agent_id.clone()], config.timeout_ms)
        ));

        info!("Agent '{}' initialized on NATS", config.agent_id);

        Ok(Self {
            config,
            client,
            step_barrier,
            running: Arc::new(Mutex::new(false)),
        })
    }

    pub async fn start(&self) -> ChatResult<()> {
        let mut running = self.running.lock().await;
        if *running { return Ok(()); }
        *running = true;
        info!("Agent '{}' started", self.config.agent_id);
        Ok(())
    }

    pub async fn stop(&self) -> ChatResult<()> {
        let mut running = self.running.lock().await;
        *running = false;
        info!("Agent '{}' stopped", self.config.agent_id);
        Ok(())
    }

    pub async fn send_status(&self, description: &str, payload: JsonValue) -> ChatResult<()> {
        let step = self.current_step().await;
        let mut full_payload = payload;
        full_payload["description"] = serde_json::Value::String(description.to_string());

        let message = ACPMessage::status(self.config.agent_id.clone(), step, full_payload);
        self.publish_acp(&message).await?;
        debug!("Sent STATUS: {}", description);
        Ok(())
    }

    pub async fn send_propose(&self, action: &str, payload: JsonValue) -> ChatResult<()> {
        let step = self.current_step().await;
        let mut full_payload = payload;
        full_payload["action"] = serde_json::Value::String(action.to_string());

        let message = ACPMessage::propose(self.config.agent_id.clone(), step, full_payload);
        self.publish_acp(&message).await?;
        debug!("Sent PROPOSE: {}", action);
        Ok(())
    }

    pub async fn complete_step(&self) -> ChatResult<()> {
        let step = self.current_step().await;
        let message = ACPMessage::step_complete(self.config.agent_id.clone(), step);
        self.publish_acp(&message).await?;

        let mut barrier = self.step_barrier.lock().await;
        barrier.record_step_completion(step, self.config.agent_id.clone());
        info!("Completed step {}", step);
        Ok(())
    }

    pub async fn wait_for_step_complete(&self, step: u64) -> ChatResult<()> {
        let timeout_duration = Duration::from_millis(self.config.timeout_ms);
        let result = timeout(timeout_duration, async {
            loop {
                {
                    let barrier = self.step_barrier.lock().await;
                    if barrier.is_step_complete(step) { return Ok(()); }
                    let pending = barrier.pending_agents(step);
                    if !pending.is_empty() {
                        debug!("Waiting for agents to complete step {}: {:?}", step, pending);
                    }
                }
                sleep(Duration::from_millis(100)).await;
            }
        }).await;

        match result {
            Ok(Ok(())) => Ok(()),
            Ok(Err(e)) => Err(e),
            Err(_) => {
                warn!("Step {} timed out", step);
                let mut barrier = self.step_barrier.lock().await;
                barrier.force_advance_step();
                Err(ChatError::Other(format!("Step {} timed out after {}ms", step, self.config.timeout_ms)))
            }
        }
    }

    pub async fn current_step(&self) -> u64 {
        self.step_barrier.lock().await.current_step()
    }

    pub async fn add_agent(&self, agent_id: String) -> ChatResult<()> {
        let mut barrier = self.step_barrier.lock().await;
        if barrier.known_agents().contains(&agent_id) {
            return Err(ChatError::AgentNotFound(agent_id));
        }
        barrier.add_agent(agent_id);
        Ok(())
    }

    pub async fn known_agents(&self) -> Vec<String> {
        self.step_barrier.lock().await.known_agents().to_vec()
    }

    pub async fn send_message(&self, _subject: &str, message: &ACPMessage) -> ChatResult<()> {
        self.publish_acp(message).await
    }

    pub fn config(&self) -> &AgentConfig { &self.config }
    pub fn client(&self) -> &ChatClient { &self.client }

    async fn publish_acp(&self, message: &ACPMessage) -> ChatResult<()> {
        let payload = serde_json::to_vec(message)?;
        let subject = message.subject();
        self.client.send_raw(&subject, &payload).await
    }
}
