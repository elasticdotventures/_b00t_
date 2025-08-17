//! ACP (Agent Coordination Protocol) commands for b00t-cli
//! 
//! Allows operators to send messages to agents and coordinate step-based workflows

use anyhow::{Result, Context};
use clap::Subcommand;
use serde_json::{json, Value};
use std::collections::HashMap;
use tracing::{info, warn, error};
use b00t_acp::{AgentConfig, ACPMessage, MessageType};

/// ACP (Agent Coordination Protocol) commands
#[derive(Subcommand, Debug)]
pub enum AcpCommands {
    /// Send STATUS message to announce operator state
    Status {
        /// Message description
        #[arg(short, long)]
        description: String,
        /// Additional payload as JSON
        #[arg(short, long)]
        payload: Option<String>,
        /// Target step number
        #[arg(short, long, default_value = "1")]
        step: u64,
        /// Agent namespace (defaults to current user)
        #[arg(short, long)]
        namespace: Option<String>,
    },
    
    /// Send PROPOSE message to suggest an action
    Propose {
        /// Action to propose
        action: String,
        /// Action payload as JSON
        #[arg(short, long)]
        payload: Option<String>,
        /// Target step number
        #[arg(short, long, default_value = "1")]
        step: u64,
        /// Agent namespace (defaults to current user)
        #[arg(short, long)]
        namespace: Option<String>,
    },
    
    /// Complete current step and advance
    Step {
        /// Target step number to complete
        #[arg(short, long, default_value = "1")]
        step: u64,
        /// Agent namespace (defaults to current user)
        #[arg(short, long)]
        namespace: Option<String>,
    },
    
    /// Listen for messages in a namespace
    Listen {
        /// Agent namespace to listen to
        #[arg(short, long)]
        namespace: Option<String>,
        /// Step to listen to (all steps if not specified)
        #[arg(short, long)]
        step: Option<u64>,
        /// Message type filter (STATUS, PROPOSE, STEP)
        #[arg(short, long)]
        message_type: Option<String>,
        /// Timeout in seconds
        #[arg(short, long, default_value = "30")]
        timeout: u64,
    },
    
    /// Show agent coordination status
    Show {
        /// Agent namespace (defaults to current user)
        #[arg(short, long)]
        namespace: Option<String>,
        /// Show details for specific step
        #[arg(short, long)]
        step: Option<u64>,
    },
    
    /// Send custom message to specific subject
    Send {
        /// NATS subject to send to
        subject: String,
        /// Message type (STATUS, PROPOSE, STEP)
        #[arg(short, long, default_value = "STATUS")]
        message_type: String,
        /// Message payload as JSON
        #[arg(short, long)]
        payload: String,
        /// Step number
        #[arg(short, long, default_value = "1")]
        step: u64,
    },
}

impl AcpCommands {
    pub async fn execute(&self) -> Result<()> {
        match self {
            AcpCommands::Status { description, payload, step, namespace } => {
                self.send_status(description, payload.as_deref(), *step, namespace.as_deref()).await
            }
            AcpCommands::Propose { action, payload, step, namespace } => {
                self.send_propose(action, payload.as_deref(), *step, namespace.as_deref()).await
            }
            AcpCommands::Step { step, namespace } => {
                self.send_step(*step, namespace.as_deref()).await
            }
            AcpCommands::Listen { namespace, step, message_type, timeout } => {
                self.listen_messages(namespace.as_deref(), *step, message_type.as_deref(), *timeout).await
            }
            AcpCommands::Show { namespace, step } => {
                self.show_status(namespace.as_deref(), *step).await
            }
            AcpCommands::Send { subject, message_type, payload, step } => {
                self.send_custom(subject, message_type, payload, *step).await
            }
        }
    }

    async fn send_status(&self, description: &str, payload: Option<&str>, step: u64, namespace: Option<&str>) -> Result<()> {
        let config = self.get_agent_config("operator", namespace).await?;
        let agent_id = format!("operator.{}", whoami::username());
        
        // Parse payload or use default
        let payload_json = if let Some(p) = payload {
            serde_json::from_str(p).context("Invalid JSON payload")?
        } else {
            json!({
                "description": description,
                "timestamp": chrono::Utc::now().to_rfc3339(),
                "operator": whoami::username()
            })
        };

        let message = ACPMessage::status(agent_id, step, payload_json);
        
        info!("📢 Sending STATUS to step {}: {}", step, description);
        info!("🎯 Subject: {}", message.subject());
        
        // For now, just display the message (stub implementation)
        println!("📨 ACP STATUS Message:");
        println!("   Step: {}", message.step);
        println!("   Agent: {}", message.agent_id);
        println!("   Subject: {}", message.subject());
        println!("   Payload: {}", serde_json::to_string_pretty(&message.payload)?);
        
        Ok(())
    }

    async fn send_propose(&self, action: &str, payload: Option<&str>, step: u64, namespace: Option<&str>) -> Result<()> {
        let config = self.get_agent_config("operator", namespace).await?;
        let agent_id = format!("operator.{}", whoami::username());
        
        // Parse payload or use default
        let mut payload_json = if let Some(p) = payload {
            serde_json::from_str(p).context("Invalid JSON payload")?
        } else {
            json!({})
        };
        
        // Add action and operator info
        payload_json["action"] = json!(action);
        payload_json["timestamp"] = json!(chrono::Utc::now().to_rfc3339());
        payload_json["operator"] = json!(whoami::username());

        let message = ACPMessage::propose(agent_id, step, payload_json);
        
        info!("💡 Sending PROPOSE to step {}: {}", step, action);
        info!("🎯 Subject: {}", message.subject());
        
        println!("📨 ACP PROPOSE Message:");
        println!("   Step: {}", message.step);
        println!("   Agent: {}", message.agent_id);
        println!("   Action: {}", action);
        println!("   Subject: {}", message.subject());
        println!("   Payload: {}", serde_json::to_string_pretty(&message.payload)?);
        
        Ok(())
    }

    async fn send_step(&self, step: u64, namespace: Option<&str>) -> Result<()> {
        let config = self.get_agent_config("operator", namespace).await?;
        let agent_id = format!("operator.{}", whoami::username());

        let message = ACPMessage::step_complete(agent_id, step);
        
        info!("✅ Completing step {}", step);
        info!("🎯 Subject: {}", message.subject());
        
        println!("📨 ACP STEP Message:");
        println!("   Step: {}", message.step);
        println!("   Agent: {}", message.agent_id);
        println!("   Subject: {}", message.subject());
        println!("   Payload: {}", serde_json::to_string_pretty(&message.payload)?);
        
        Ok(())
    }

    async fn listen_messages(&self, namespace: Option<&str>, step: Option<u64>, message_type: Option<&str>, timeout: u64) -> Result<()> {
        let config = self.get_agent_config("listener", namespace).await?;
        
        let subject_pattern = match (step, message_type) {
            (Some(s), Some(mt)) => {
                let mt_lower = mt.to_lowercase();
                format!("{}.acp.{}.*.{}", config.namespace, s, mt_lower)
            }
            (Some(s), None) => {
                format!("{}.acp.{}.>", config.namespace, s)
            }
            (None, Some(mt)) => {
                let mt_lower = mt.to_lowercase();
                format!("{}.acp.*.*.{}", config.namespace, mt_lower)
            }
            (None, None) => {
                format!("{}.acp.>", config.namespace)
            }
        };

        info!("👂 Listening for messages on: {}", subject_pattern);
        info!("⏱️  Timeout: {} seconds", timeout);
        
        println!("🎧 ACP Message Listener");
        println!("   Subject Pattern: {}", subject_pattern);
        println!("   Timeout: {} seconds", timeout);
        println!("   Namespace: {}", config.namespace);
        
        // Stub implementation - in real version would connect to NATS
        println!("⚠️  Stub implementation - would listen for real NATS messages");
        println!("📡 To enable real NATS listening, configure connection to c010.promptexecution.com:4222");
        
        Ok(())
    }

    async fn show_status(&self, namespace: Option<&str>, step: Option<u64>) -> Result<()> {
        let config = self.get_agent_config("status", namespace).await?;
        
        println!("📊 ACP Coordination Status");
        println!("   Namespace: {}", config.namespace);
        println!("   Agent ID: {}", config.agent_id);
        println!("   NATS URL: {}", config.nats_url);
        
        if let Some(s) = step {
            println!("   Step Filter: {}", s);
        }
        
        // Stub implementation - would query real agent states
        println!("\n🤖 Known Agents:");
        println!("   operator.{} - Ready", whoami::username());
        println!("   (Add real agent discovery with NATS integration)");
        
        println!("\n📈 Step Status:");
        println!("   Current Step: 1");
        println!("   Completed Steps: 0");
        println!("   Pending Agents: 0");
        
        Ok(())
    }

    async fn send_custom(&self, subject: &str, message_type: &str, payload: &str, step: u64) -> Result<()> {
        let agent_id = format!("operator.{}", whoami::username());
        
        // Parse message type
        let msg_type = match message_type.to_uppercase().as_str() {
            "STATUS" => MessageType::Status,
            "PROPOSE" => MessageType::Propose,
            "STEP" => MessageType::Step,
            _ => return Err(anyhow::anyhow!("Invalid message type. Use STATUS, PROPOSE, or STEP")),
        };
        
        // Parse payload
        let payload_json: Value = serde_json::from_str(payload)
            .context("Invalid JSON payload")?;
        
        let message = match msg_type {
            MessageType::Status => ACPMessage::status(agent_id, step, payload_json),
            MessageType::Propose => ACPMessage::propose(agent_id, step, payload_json),
            MessageType::Step => ACPMessage::step_complete(agent_id, step),
        };
        
        info!("📤 Sending custom message to: {}", subject);
        
        println!("📨 ACP Custom Message:");
        println!("   Subject: {}", subject);
        println!("   Type: {}", message_type);
        println!("   Step: {}", step);
        println!("   Agent: {}", message.agent_id);
        println!("   Payload: {}", serde_json::to_string_pretty(&message.payload)?);
        
        Ok(())
    }

    async fn get_agent_config(&self, role: &str, namespace: Option<&str>) -> Result<AgentConfig> {
        let username = whoami::username();
        let agent_id = format!("{}.{}", role, username);
        
        let namespace = namespace
            .map(|ns| ns.to_string())
            .unwrap_or_else(|| format!("account.{}", username));
        
        // TODO: Get NATS URL and JWT from configuration
        let nats_url = std::env::var("NATS_URL")
            .unwrap_or_else(|_| "nats://c010.promptexecution.com:4222".to_string());
        
        let config = AgentConfig::new(agent_id, nats_url, namespace)
            .with_role(role.to_string())
            .with_timeout(30000);
        
        // TODO: Add JWT token from b00t-website provisioning
        // if let Ok(jwt) = std::env::var("NATS_JWT") {
        //     config = config.with_jwt(jwt);
        // }
        
        Ok(config)
    }
}

/// Helper function to get current user's namespace
pub fn get_current_namespace() -> String {
    let username = whoami::username();
    format!("account.{}", username)
}

/// Helper function to create operator agent ID
pub fn get_operator_agent_id() -> String {
    let username = whoami::username();
    format!("operator.{}", username)
}