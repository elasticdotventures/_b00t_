//! k0mmand3r IPC integration for job orchestration
//!
//! Enables job control via slash commands over Redis/NATS/etc:
//! - `/job run build-and-test`
//! - `/job status build-and-test`
//! - `/job stop build-and-test`
//!
//! # Architecture
//!
//! ```text
//! User/Agent → /job run myapp
//!     ↓
//! k0mmand3r Parser
//!     ↓
//! Redis/NATS (b00t:job channel)
//!     ↓
//! JobIpcListener (this module)
//!     ↓
//! Job Orchestrator (commands/job.rs)
//!     ↓
//! Status Updates → b00t:job:status
//! ```
//!
//! # Message Format
//!
//! ```json
//! {
//!   "verb": "job",
//!   "params": {
//!     "action": "run",           // run, status, stop, plan
//!     "name": "build-and-test",  // job name
//!     "from_step": "lint",       // optional
//!     "to_step": "test",         // optional
//!     "resume": "true",          // optional
//!     "dry_run": "false"         // optional
//!   },
//!   "agent_id": "agent-123",
//!   "timestamp": "2025-11-17T15:00:00Z"
//! }
//! ```

use anyhow::{Context, Result};
use b00t_ipc::transport::{K0mmand3rMessage, Transport};
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{error, info, warn};

use crate::commands::job::{
    get_job_plan_json, get_job_status_json, list_jobs_json, run_job_internal, stop_job_internal,
};

/// Job IPC listener configuration
#[derive(Debug, Clone)]
pub struct JobIpcConfig {
    /// Transport URL (redis://..., nats://..., etc.)
    pub transport_url: String,

    /// Command channel to subscribe to
    pub command_channel: String,

    /// Status channel to publish to
    pub status_channel: String,

    /// Working directory for job execution
    pub work_dir: String,
}

impl Default for JobIpcConfig {
    fn default() -> Self {
        Self {
            transport_url: "redis://localhost:6379".to_string(),
            command_channel: "b00t:job".to_string(),
            status_channel: "b00t:job:status".to_string(),
            work_dir: ".".to_string(),
        }
    }
}

/// Job IPC listener state
pub struct JobIpcListener<T: Transport> {
    config: JobIpcConfig,
    transport: Arc<T>,
    running: Arc<RwLock<bool>>,
}

impl<T: Transport + 'static> JobIpcListener<T> {
    /// Create new job IPC listener
    pub async fn new(config: JobIpcConfig) -> Result<Self> {
        let transport = T::connect(&config.transport_url)
            .await
            .context("Failed to connect to transport")?;

        Ok(Self {
            config,
            transport: Arc::new(transport),
            running: Arc::new(RwLock::new(false)),
        })
    }

    /// Start listening for job commands
    pub async fn start(&self) -> Result<()> {
        info!(
            "🎧 Starting job IPC listener on channel: {}",
            self.config.command_channel
        );

        // Mark as running
        {
            let mut running = self.running.write().await;
            *running = true;
        }

        // Subscribe to command channel
        let mut rx = self
            .transport
            .subscribe(&self.config.command_channel)
            .await
            .context("Failed to subscribe to command channel")?;

        // Process messages
        while *self.running.read().await {
            match rx.recv().await {
                Some((channel, msg)) => {
                    info!("📨 Received job command on {}: {:?}", channel, msg.verb);
                    if let Err(e) = self.handle_command(msg).await {
                        error!("Failed to handle job command: {}", e);
                    }
                }
                None => {
                    warn!("Command channel closed");
                    break;
                }
            }
        }

        info!("Job IPC listener stopped");
        Ok(())
    }

    /// Stop listening
    pub async fn stop(&self) -> Result<()> {
        let mut running = self.running.write().await;
        *running = false;
        Ok(())
    }

    /// Handle incoming job command
    async fn handle_command(&self, msg: K0mmand3rMessage) -> Result<()> {
        let action = msg
            .params
            .get("action")
            .map(|s| s.as_str())
            .unwrap_or("run");

        match action {
            "run" => self.handle_run(msg).await,
            "status" => self.handle_status(msg).await,
            "stop" => self.handle_stop(msg).await,
            "plan" => self.handle_plan(msg).await,
            "list" => self.handle_list(msg).await,
            _ => {
                warn!("Unknown job action: {}", action);
                self.publish_error(&msg, format!("Unknown action: {}", action))
                    .await
            }
        }
    }

    /// Handle job run command
    async fn handle_run(&self, msg: K0mmand3rMessage) -> Result<()> {
        let name = msg.params.get("name").context("Missing 'name' parameter")?;

        let from_step = msg.params.get("from_step").map(|s| s.as_str());
        let to_step = msg.params.get("to_step").map(|s| s.as_str());
        let resume = msg
            .params
            .get("resume")
            .map(|s| s == "true")
            .unwrap_or(false);
        let dry_run = msg
            .params
            .get("dry_run")
            .map(|s| s == "true")
            .unwrap_or(false);
        let no_checkpoint = msg
            .params
            .get("no_checkpoint")
            .map(|s| s == "true")
            .unwrap_or(false);

        // Parse env vars (env_KEY=VALUE format)
        let env_vars: Vec<String> = msg
            .params
            .iter()
            .filter(|(k, _)| k.starts_with("env_"))
            .map(|(k, v)| format!("{}={}", &k[4..], v))
            .collect();

        info!("🚀 Running job: {}", name);

        // Publish starting status
        self.publish_status(&msg, "started", &format!("Starting job: {}", name))
            .await?;

        // Execute job (calling existing job command implementation)
        let result = run_job_internal(
            &self.config.work_dir,
            name,
            from_step,
            to_step,
            dry_run,
            no_checkpoint,
            resume,
            &env_vars,
        )
        .await;

        // Publish completion status
        match result {
            Ok(_) => {
                self.publish_status(
                    &msg,
                    "completed",
                    &format!("Job {} completed successfully", name),
                )
                .await?;
            }
            Err(e) => {
                self.publish_error(&msg, format!("Job {} failed: {}", name, e))
                    .await?;
            }
        }

        Ok(())
    }

    /// Handle job status command
    async fn handle_status(&self, msg: K0mmand3rMessage) -> Result<()> {
        let name = msg.params.get("name");
        let all = msg.params.get("all").map(|s| s == "true").unwrap_or(false);

        info!("📊 Getting job status: {:?}", name);

        // Get status (calling existing job status implementation)
        let status_output =
            get_job_status_json(&self.config.work_dir, name.map(|s| s.as_str()), all).await?;

        // Publish status response
        self.publish_status(&msg, "status_response", &status_output)
            .await?;

        Ok(())
    }

    /// Handle job stop command
    async fn handle_stop(&self, msg: K0mmand3rMessage) -> Result<()> {
        let name = msg.params.get("name");
        let all = msg.params.get("all").map(|s| s == "true").unwrap_or(false);

        info!("🛑 Stopping job: {:?}", name);

        // Stop job (calling existing job stop implementation)
        let result = stop_job_internal(&self.config.work_dir, name.map(|s| s.as_str()), all).await;

        match result {
            Ok(_) => {
                self.publish_status(&msg, "stopped", "Job stopped successfully")
                    .await?;
            }
            Err(e) => {
                self.publish_error(&msg, format!("Failed to stop job: {}", e))
                    .await?;
            }
        }

        Ok(())
    }

    /// Handle job plan command
    async fn handle_plan(&self, msg: K0mmand3rMessage) -> Result<()> {
        let name = msg.params.get("name").context("Missing 'name' parameter")?;

        info!("📋 Getting job plan: {}", name);

        // Get plan (calling existing job plan implementation)
        let plan_output = get_job_plan_json(&self.config.work_dir, name).await?;

        // Publish plan response
        self.publish_status(&msg, "plan_response", &plan_output)
            .await?;

        Ok(())
    }

    /// Handle job list command
    async fn handle_list(&self, msg: K0mmand3rMessage) -> Result<()> {
        info!("📝 Listing jobs");

        // List jobs (calling existing job list implementation)
        let list_output = list_jobs_json(&self.config.work_dir).await?;

        // Publish list response
        self.publish_status(&msg, "list_response", &list_output)
            .await?;

        Ok(())
    }

    /// Publish status update
    async fn publish_status(
        &self,
        original_msg: &K0mmand3rMessage,
        status: &str,
        message: &str,
    ) -> Result<()> {
        let status_msg = K0mmand3rMessage::new("job_status")
            .with_param("status", status)
            .with_param(
                "job_name",
                original_msg
                    .params
                    .get("name")
                    .map(|s| s.as_str())
                    .unwrap_or(""),
            )
            .with_content(message)
            .with_agent_id(
                original_msg
                    .agent_id
                    .clone()
                    .unwrap_or_else(|| "job-orchestrator".to_string()),
            );

        self.transport
            .publish(&self.config.status_channel, &status_msg)
            .await
            .context("Failed to publish status")?;

        Ok(())
    }

    /// Publish error
    async fn publish_error(&self, original_msg: &K0mmand3rMessage, error: String) -> Result<()> {
        self.publish_status(original_msg, "error", &error).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_job_ipc_config_default() {
        let config = JobIpcConfig::default();
        assert_eq!(config.transport_url, "redis://localhost:6379");
        assert_eq!(config.command_channel, "b00t:job");
        assert_eq!(config.status_channel, "b00t:job:status");
    }

    // 🤓 Integration tests would require Redis/NATS running
    // Use #[ignore] or separate integration test suite
}
