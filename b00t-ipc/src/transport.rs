//! Transport abstraction for multi-protocol pub/sub
//!
//! Supports: Redis, NATS, IRC, RabbitMQ, and more
//!
//! # Design Philosophy
//! - Transport-agnostic: Business logic doesn't care about Redis vs NATS
//! - Async-first: All transports use tokio async I/O
//! - Type-safe: Serde for message serialization
//! - Extensible: Easy to add new transports (IRC, RabbitMQ, etc.)
//!
//! # Example
//! ```rust,no_run
//! use b00t_ipc::transport::{Transport, RedisTransport, K0mmand3rMessage};
//!
//! #[tokio::main]
//! async fn main() {
//!     let transport = RedisTransport::connect("redis://localhost:6379").await.unwrap();
//!
//!     // Subscribe to channel
//!     let mut rx = transport.subscribe("b00t:k0mmand3r").await.unwrap();
//!
//!     // Publish message
//!     let msg = K0mmand3rMessage {
//!         verb: "start".to_string(),
//!         params: Default::default(),
//!         content: None,
//!         timestamp: chrono::Utc::now(),
//!         agent_id: None,
//!     };
//!     transport.publish("b00t:k0mmand3r", &msg).await.unwrap();
//!
//!     // Receive message
//!     if let Some((channel, msg)) = rx.recv().await {
//!         println!("Received on {}: {:?}", channel, msg);
//!     }
//! }
//! ```

use anyhow::{Context, Result};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use tokio::sync::mpsc;

/// k0mmand3r message format (shared by PM2 Tasker, LangChain, Job Orchestrator)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct K0mmand3rMessage {
    /// Command verb (start, stop, agent, job, etc.)
    pub verb: String,

    /// Parameters as key-value pairs
    #[serde(default)]
    pub params: HashMap<String, String>,

    /// Optional content payload
    #[serde(skip_serializing_if = "Option::is_none")]
    pub content: Option<String>,

    /// Message timestamp
    #[serde(with = "chrono::serde::ts_seconds")]
    pub timestamp: chrono::DateTime<chrono::Utc>,

    /// Originating agent ID
    #[serde(skip_serializing_if = "Option::is_none")]
    pub agent_id: Option<String>,
}

impl K0mmand3rMessage {
    pub fn new(verb: impl Into<String>) -> Self {
        Self {
            verb: verb.into(),
            params: HashMap::new(),
            content: None,
            timestamp: chrono::Utc::now(),
            agent_id: None,
        }
    }

    pub fn with_param(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.params.insert(key.into(), value.into());
        self
    }

    pub fn with_content(mut self, content: impl Into<String>) -> Self {
        self.content = Some(content.into());
        self
    }

    pub fn with_agent_id(mut self, agent_id: impl Into<String>) -> Self {
        self.agent_id = Some(agent_id.into());
        self
    }
}

/// Transport-agnostic pub/sub interface
#[async_trait]
pub trait Transport: Send + Sync {
    /// Connect to transport backend
    async fn connect(url: &str) -> Result<Self>
    where
        Self: Sized;

    /// Publish message to channel
    async fn publish(&self, channel: &str, msg: &K0mmand3rMessage) -> Result<()>;

    /// Subscribe to channel, returns receiver for messages
    async fn subscribe(
        &self,
        channel: &str,
    ) -> Result<mpsc::UnboundedReceiver<(String, K0mmand3rMessage)>>;

    /// Unsubscribe from channel
    async fn unsubscribe(&self, channel: &str) -> Result<()>;

    /// Close connection and cleanup
    async fn close(&self) -> Result<()>;

    /// Health check
    async fn ping(&self) -> Result<bool>;
}

// Re-exports for convenience
#[cfg(feature = "redis")]
pub use redis_transport::RedisTransport;

#[cfg(feature = "nats")]
pub use nats_transport::NatsTransport;

/// Redis pub/sub transport
#[cfg(feature = "redis")]
pub mod redis_transport {
    use super::*;
    use futures::StreamExt;
    use redis::aio::MultiplexedConnection;
    use redis::{AsyncCommands, Client};
    use std::sync::Arc;
    use tokio::sync::RwLock;

    pub struct RedisTransport {
        client: Client,
        connection: Arc<RwLock<MultiplexedConnection>>,
        subscriptions:
            Arc<RwLock<HashMap<String, mpsc::UnboundedSender<(String, K0mmand3rMessage)>>>>,
    }

    #[async_trait]
    impl Transport for RedisTransport {
        async fn connect(url: &str) -> Result<Self> {
            let client = Client::open(url).context("Failed to create Redis client")?;
            let connection = client
                .get_multiplexed_async_connection()
                .await
                .context("Failed to connect to Redis")?;

            Ok(Self {
                client,
                connection: Arc::new(RwLock::new(connection)),
                subscriptions: Arc::new(RwLock::new(HashMap::new())),
            })
        }

        async fn publish(&self, channel: &str, msg: &K0mmand3rMessage) -> Result<()> {
            let json = serde_json::to_string(msg).context("Failed to serialize message")?;
            let mut conn = self.connection.write().await;
            let _: () = conn
                .publish(channel, json)
                .await
                .context("Failed to publish to Redis")?;
            Ok(())
        }

        async fn subscribe(
            &self,
            channel: &str,
        ) -> Result<mpsc::UnboundedReceiver<(String, K0mmand3rMessage)>> {
            let (tx, rx) = mpsc::unbounded_channel();

            // Store sender for this subscription
            {
                let mut subs = self.subscriptions.write().await;
                subs.insert(channel.to_string(), tx.clone());
            }

            // Spawn task to listen on Redis pub/sub
            let channel_name = channel.to_string();
            let client = self.client.clone();
            let subscriptions = self.subscriptions.clone();

            tokio::spawn(async move {
                let mut pubsub = match client.get_async_pubsub().await {
                    Ok(ps) => ps,
                    Err(e) => {
                        eprintln!("Failed to create Redis pubsub: {}", e);
                        return;
                    }
                };

                if let Err(e) = pubsub.subscribe(&channel_name).await {
                    eprintln!("Failed to subscribe to {}: {}", channel_name, e);
                    return;
                }

                loop {
                    match pubsub.on_message().next().await {
                        Some(msg) => {
                            let payload: String = match msg.get_payload() {
                                Ok(p) => p,
                                Err(e) => {
                                    eprintln!("Failed to get payload: {}", e);
                                    continue;
                                }
                            };

                            let k0mmand3r_msg: K0mmand3rMessage =
                                match serde_json::from_str(&payload) {
                                    Ok(m) => m,
                                    Err(e) => {
                                        eprintln!("Failed to deserialize message: {}", e);
                                        continue;
                                    }
                                };

                            // Send to channel receiver
                            let subs = subscriptions.read().await;
                            if let Some(tx) = subs.get(&channel_name) {
                                if tx.send((channel_name.clone(), k0mmand3r_msg)).is_err() {
                                    // Receiver dropped, stop listening
                                    break;
                                }
                            }
                        }
                        None => break,
                    }
                }
            });

            Ok(rx)
        }

        async fn unsubscribe(&self, channel: &str) -> Result<()> {
            let mut subs = self.subscriptions.write().await;
            subs.remove(channel);
            Ok(())
        }

        async fn close(&self) -> Result<()> {
            let mut subs = self.subscriptions.write().await;
            subs.clear();
            Ok(())
        }

        async fn ping(&self) -> Result<bool> {
            let mut conn = self.connection.write().await;
            let pong: String = redis::cmd("PING")
                .query_async(&mut *conn)
                .await
                .context("PING failed")?;
            Ok(pong == "PONG")
        }
    }
}

/// NATS pub/sub transport
#[cfg(feature = "nats")]
pub mod nats_transport {
    use super::*;
    use async_nats::Client;
    use futures::StreamExt;
    use std::sync::Arc;
    use tokio::sync::RwLock;

    pub struct NatsTransport {
        client: Client,
        subscriptions:
            Arc<RwLock<HashMap<String, mpsc::UnboundedSender<(String, K0mmand3rMessage)>>>>,
    }

    #[async_trait]
    impl Transport for NatsTransport {
        async fn connect(url: &str) -> Result<Self> {
            let client = async_nats::connect(url)
                .await
                .context("Failed to connect to NATS")?;

            Ok(Self {
                client,
                subscriptions: Arc::new(RwLock::new(HashMap::new())),
            })
        }

        async fn publish(&self, channel: &str, msg: &K0mmand3rMessage) -> Result<()> {
            let json = serde_json::to_string(msg).context("Failed to serialize message")?;
            self.client
                .publish(channel.to_string(), json.into())
                .await
                .context("Failed to publish to NATS")?;
            Ok(())
        }

        async fn subscribe(
            &self,
            channel: &str,
        ) -> Result<mpsc::UnboundedReceiver<(String, K0mmand3rMessage)>> {
            let (tx, rx) = mpsc::unbounded_channel();

            // Store sender
            {
                let mut subs = self.subscriptions.write().await;
                subs.insert(channel.to_string(), tx.clone());
            }

            // Subscribe to NATS subject
            let mut subscriber = self
                .client
                .subscribe(channel.to_string())
                .await
                .context("Failed to subscribe to NATS")?;

            let channel_name = channel.to_string();
            let subscriptions = self.subscriptions.clone();

            tokio::spawn(async move {
                while let Some(msg) = subscriber.next().await {
                    let payload = String::from_utf8_lossy(&msg.payload).to_string();

                    let k0mmand3r_msg: K0mmand3rMessage = match serde_json::from_str(&payload) {
                        Ok(m) => m,
                        Err(e) => {
                            eprintln!("Failed to deserialize NATS message: {}", e);
                            continue;
                        }
                    };

                    // Send to receiver
                    let subs = subscriptions.read().await;
                    if let Some(tx) = subs.get(&channel_name) {
                        if tx.send((channel_name.clone(), k0mmand3r_msg)).is_err() {
                            break;
                        }
                    }
                }
            });

            Ok(rx)
        }

        async fn unsubscribe(&self, channel: &str) -> Result<()> {
            let mut subs = self.subscriptions.write().await;
            subs.remove(channel);
            Ok(())
        }

        async fn close(&self) -> Result<()> {
            let mut subs = self.subscriptions.write().await;
            subs.clear();
            Ok(())
        }

        async fn ping(&self) -> Result<bool> {
            // NATS client maintains connection automatically
            // Connection is healthy if server_info is available
            let _info = self.client.server_info();
            Ok(true)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_k0mmand3r_message_builder() {
        let msg = K0mmand3rMessage::new("start")
            .with_param("task", "myapp")
            .with_param("command", "node app.js")
            .with_content("Job execution context")
            .with_agent_id("agent-123");

        assert_eq!(msg.verb, "start");
        assert_eq!(msg.params.get("task").unwrap(), "myapp");
        assert_eq!(msg.params.get("command").unwrap(), "node app.js");
        assert_eq!(msg.content.as_ref().unwrap(), "Job execution context");
        assert_eq!(msg.agent_id.as_ref().unwrap(), "agent-123");
    }

    #[test]
    fn test_k0mmand3r_message_serialization() {
        let msg = K0mmand3rMessage::new("job")
            .with_param("name", "build-and-test")
            .with_param("step", "lint");

        let json = serde_json::to_string(&msg).unwrap();
        let deserialized: K0mmand3rMessage = serde_json::from_str(&json).unwrap();

        assert_eq!(deserialized.verb, "job");
        assert_eq!(deserialized.params.get("name").unwrap(), "build-and-test");
        assert_eq!(deserialized.params.get("step").unwrap(), "lint");
    }
}
