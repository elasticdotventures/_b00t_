//! NATS transport implementation for agent communication.
//!
//! Provides cloud-native, high-scale messaging via NATS.io for
//! distributed b00t agents across different hosts/regions.

use crate::error::{ChatError, ChatResult};
use crate::ipc_transport::{BroadcastTransport, IpcTransport, TransportKind};
use crate::message::ChatMessage;
use crate::metrics::{ChatMetrics, LatencyTimer};
use async_nats::{Client, ConnectOptions, Subscriber};
use async_trait::async_trait;
use futures::StreamExt;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{debug, info};

/// NATS transport for distributed agent messaging.
#[derive(Debug, Clone)]
pub struct NatsTransport {
    client: Arc<RwLock<Option<Client>>>,
    url: String,
    subscriptions: Arc<RwLock<Vec<String>>>,
    subscribers: Arc<RwLock<Vec<Subscriber>>>,
}

impl NatsTransport {
    /// Create a new NATS transport with connection URL.
    ///
    /// # Examples
    /// ```no_run
    /// use b00t_chat::transports::NatsTransport;
    ///
    /// let transport = NatsTransport::new("nats://localhost:4222");
    /// ```
    pub fn new(url: impl Into<String>) -> Self {
        Self {
            client: Arc::new(RwLock::new(None)),
            url: url.into(),
            subscriptions: Arc::new(RwLock::new(Vec::new())),
            subscribers: Arc::new(RwLock::new(Vec::new())),
        }
    }

    /// Connect to NATS server.
    pub async fn connect(&self) -> ChatResult<()> {
        let mut client_guard = self.client.write().await;

        if client_guard.is_some() {
            return Ok(()); // Already connected
        }

        let options = ConnectOptions::new();
        let client = options
            .connect(&self.url)
            .await
            .map_err(|e| {
                ChatMetrics::global().record_connection_error("nats", "connection_failed");
                ChatError::Other(format!("NATS connection failed: {}", e))
            })?;

        *client_guard = Some(client);
        ChatMetrics::global().record_connection_opened("nats");
        info!("Connected to NATS server: {}", self.url);
        Ok(())
    }

    /// Get the underlying NATS client.
    async fn get_client(&self) -> ChatResult<Client> {
        let client_guard = self.client.read().await;
        client_guard
            .clone()
            .ok_or(ChatError::NotConnected)
    }

    /// Convert ChatMessage to NATS subject format.
    ///
    /// Maps `channel.sender` to NATS subject hierarchy.
    fn message_to_subject(message: &ChatMessage) -> String {
        format!("b00t.agents.{}.{}", message.channel, message.sender)
    }
}

#[async_trait]
impl IpcTransport for NatsTransport {
    async fn send(&self, message: &ChatMessage) -> ChatResult<()> {
        let timer = LatencyTimer::send("nats");
        let client = self.get_client().await?;
        let subject = Self::message_to_subject(message);
        let payload = serde_json::to_vec(message)?;

        let result = client
            .publish(subject.clone(), payload.into())
            .await
            .map_err(|e| {
                ChatMetrics::global().record_message_failed("nats", "publish_failed");
                ChatError::Other(format!("NATS publish failed: {}", e))
            });

        if result.is_ok() {
            ChatMetrics::global().record_message_sent("nats", &message.channel);
            timer.stop();
            debug!("Published to NATS subject: {}", subject);
        }

        result
    }

    async fn recv(&self) -> ChatResult<Option<ChatMessage>> {
        let timer = LatencyTimer::recv("nats");
        let mut subscribers = self.subscribers.write().await;

        if subscribers.is_empty() {
            return Ok(None);
        }

        // Poll first subscriber for messages
        if let Some(subscriber) = subscribers.first_mut() {
            match subscriber.next().await {
                Some(msg) => {
                    let message: ChatMessage = serde_json::from_slice(&msg.payload)?;
                    ChatMetrics::global().record_message_received("nats", &message.channel);
                    timer.stop();
                    Ok(Some(message))
                }
                None => Ok(None),
            }
        } else {
            Ok(None)
        }
    }

    async fn close(&self) -> ChatResult<()> {
        let mut client_guard = self.client.write().await;
        *client_guard = None;

        let mut subscribers = self.subscribers.write().await;
        subscribers.clear();

        ChatMetrics::global().record_connection_closed("nats");
        info!("Closed NATS connection");
        Ok(())
    }

    fn kind(&self) -> TransportKind {
        TransportKind::Nats
    }

    async fn is_available(&self) -> bool {
        self.client.read().await.is_some()
    }
}

#[async_trait]
impl BroadcastTransport for NatsTransport {
    async fn subscribe(&mut self, channel: &str) -> ChatResult<()> {
        if !self.is_available().await {
            self.connect().await?;
        }

        let client = self.get_client().await?;
        let subject = format!("b00t.agents.{}.>", channel);

        let subscriber = client
            .subscribe(subject.clone())
            .await
            .map_err(|e| ChatError::Other(format!("NATS subscribe failed: {}", e)))?;

        self.subscriptions.write().await.push(channel.to_string());
        self.subscribers.write().await.push(subscriber);

        ChatMetrics::global().record_transport_operation("nats", "subscribe");
        info!("Subscribed to NATS subject: {}", subject);
        Ok(())
    }

    async fn unsubscribe(&mut self, channel: &str) -> ChatResult<()> {
        let mut subscriptions = self.subscriptions.write().await;
        subscriptions.retain(|s| s != channel);

        // Note: async_nats subscribers automatically unsubscribe on drop
        debug!("Unsubscribed from NATS channel: {}", channel);
        Ok(())
    }

    async fn publish(&self, channel: &str, message: &ChatMessage) -> ChatResult<()> {
        let client = self.get_client().await?;
        let subject = format!("b00t.agents.{}.broadcast", channel);
        let payload = serde_json::to_vec(message)?;

        client
            .publish(subject.clone(), payload.into())
            .await
            .map_err(|e| ChatError::Other(format!("NATS broadcast failed: {}", e)))?;

        debug!("Broadcast to NATS subject: {}", subject);
        Ok(())
    }

    fn subscriptions(&self) -> Vec<String> {
        // Note: This is a synchronous method but subscriptions is behind RwLock
        // We can't await here, so we return empty vec
        // TODO: Consider making this async or using a different pattern
        Vec::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_nats_transport_creation() {
        let transport = NatsTransport::new("nats://localhost:4222");
        assert_eq!(transport.kind(), TransportKind::Nats);
    }

    #[test]
    fn test_message_to_subject_conversion() {
        let message = ChatMessage::new("mission.alpha", "frontend", "test");
        let subject = NatsTransport::message_to_subject(&message);
        assert_eq!(subject, "b00t.agents.mission.alpha.frontend");
    }

    #[tokio::test]
    async fn test_transport_not_connected_initially() {
        let transport = NatsTransport::new("nats://localhost:4222");
        assert!(!transport.is_available().await);
    }
}
