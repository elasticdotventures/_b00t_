//! MQTT transport implementation for IoT and edge agent communication.
//!
//! Provides lightweight messaging via MQTT protocol for resource-constrained
//! agents and edge deployments.

use crate::error::{ChatError, ChatResult};
use crate::ipc_transport::{BroadcastTransport, IpcTransport, TransportKind};
use crate::message::ChatMessage;
use crate::metrics::{ChatMetrics, LatencyTimer};
use async_trait::async_trait;
use rumqttc::{AsyncClient, ConnectionError, Event, EventLoop, MqttOptions, Packet, QoS};
use std::sync::Arc;
use tokio::sync::{mpsc, RwLock};
use tracing::{debug, error, info, warn};

/// MQTT transport for IoT and edge agent communication.
#[derive(Debug)]
pub struct MqttTransport {
    client: Arc<RwLock<Option<AsyncClient>>>,
    broker_url: String,
    client_id: String,
    subscriptions: Arc<RwLock<Vec<String>>>,
    message_rx: Arc<RwLock<Option<mpsc::UnboundedReceiver<ChatMessage>>>>,
    message_tx: mpsc::UnboundedSender<ChatMessage>,
}

impl MqttTransport {
    /// Create a new MQTT transport.
    ///
    /// # Arguments
    /// * `broker_url` - MQTT broker address (e.g., "localhost:1883")
    /// * `client_id` - Unique client identifier
    pub fn new(broker_url: impl Into<String>, client_id: impl Into<String>) -> Self {
        let (tx, rx) = mpsc::unbounded_channel();

        Self {
            client: Arc::new(RwLock::new(None)),
            broker_url: broker_url.into(),
            client_id: client_id.into(),
            subscriptions: Arc::new(RwLock::new(Vec::new())),
            message_rx: Arc::new(RwLock::new(Some(rx))),
            message_tx: tx,
        }
    }

    /// Connect to MQTT broker.
    pub async fn connect(&self) -> ChatResult<()> {
        let mut client_guard = self.client.write().await;

        if client_guard.is_some() {
            return Ok(()); // Already connected
        }

        let parts: Vec<&str> = self.broker_url.split(':').collect();
        let host = parts.first().ok_or_else(|| ChatError::Other("Invalid broker URL".to_string()))?;
        let port = parts.get(1).and_then(|p| p.parse::<u16>().ok()).unwrap_or(1883);

        let mut mqtt_options = MqttOptions::new(&self.client_id, *host, port);
        mqtt_options.set_keep_alive(std::time::Duration::from_secs(30));

        let (client, event_loop) = AsyncClient::new(mqtt_options, 10);

        *client_guard = Some(client);

        // Spawn event loop processor (EventLoop stays in the spawned task)
        self.spawn_event_processor(event_loop).await;

        ChatMetrics::global().record_connection_opened("mqtt");
        info!("Connected to MQTT broker: {}", self.broker_url);
        Ok(())
    }

    /// Spawn background task to process MQTT events.
    /// EventLoop is moved into the task and never shared.
    async fn spawn_event_processor(&self, mut event_loop: EventLoop) {
        let message_tx = self.message_tx.clone();

        tokio::spawn(async move {
            loop {
                let poll_result: Result<Event, ConnectionError> = event_loop.poll().await;
                match poll_result {
                    Ok(Event::Incoming(Packet::Publish(publish))) => {
                        // Deserialize message from payload
                        match serde_json::from_slice::<ChatMessage>(&publish.payload) {
                            Ok(message) => {
                                ChatMetrics::global().record_message_received("mqtt", &message.channel);
                                if message_tx.send(message).is_err() {
                                    warn!("Message receiver dropped");
                                    break;
                                }
                            }
                            Err(e) => {
                                error!("Failed to deserialize MQTT message: {}", e);
                            }
                        }
                    }
                    Ok(_) => {} // Other events
                    Err(e) => {
                        error!("MQTT event loop error: {}", e);
                        tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;
                    }
                }
            }
        });
    }

    /// Get the underlying MQTT client.
    async fn get_client(&self) -> ChatResult<AsyncClient> {
        let client_guard = self.client.read().await;
        client_guard
            .clone()
            .ok_or(ChatError::NotConnected)
    }

    /// Convert ChatMessage to MQTT topic format.
    fn message_to_topic(message: &ChatMessage) -> String {
        format!("b00t/agents/{}/{}", message.channel, message.sender)
    }
}

#[async_trait]
impl IpcTransport for MqttTransport {
    async fn send(&self, message: &ChatMessage) -> ChatResult<()> {
        let timer = LatencyTimer::send("mqtt");
        let client = self.get_client().await?;
        let topic = Self::message_to_topic(message);
        let payload = serde_json::to_vec(message)?;

        let result = client
            .publish(topic.clone(), QoS::AtLeastOnce, false, payload)
            .await
            .map_err(|e| {
                ChatMetrics::global().record_message_failed("mqtt", "publish_failed");
                ChatError::Other(format!("MQTT publish failed: {}", e))
            });

        if result.is_ok() {
            ChatMetrics::global().record_message_sent("mqtt", &message.channel);
            timer.stop();
            debug!("Published to MQTT topic: {}", topic);
        }

        result
    }

    async fn recv(&self) -> ChatResult<Option<ChatMessage>> {
        let mut rx_guard = self.message_rx.write().await;

        if let Some(ref mut rx) = *rx_guard {
            match rx.try_recv() {
                Ok(msg) => Ok(Some(msg)),
                Err(mpsc::error::TryRecvError::Empty) => Ok(None),
                Err(mpsc::error::TryRecvError::Disconnected) => {
                    Err(ChatError::Other("Message channel disconnected".to_string()))
                }
            }
        } else {
            Ok(None)
        }
    }

    async fn close(&self) -> ChatResult<()> {
        let mut client_guard = self.client.write().await;
        *client_guard = None;

        // EventLoop is owned by the spawned task, will be dropped when task exits
        ChatMetrics::global().record_connection_closed("mqtt");
        info!("Closed MQTT connection");
        Ok(())
    }

    fn kind(&self) -> TransportKind {
        TransportKind::Tcp // MQTT uses TCP, could add MqttTransportKind variant
    }

    async fn is_available(&self) -> bool {
        self.client.read().await.is_some()
    }
}

#[async_trait]
impl BroadcastTransport for MqttTransport {
    async fn subscribe(&mut self, channel: &str) -> ChatResult<()> {
        if !self.is_available().await {
            self.connect().await?;
        }

        let client = self.get_client().await?;
        let topic = format!("b00t/agents/{}/#", channel);

        client
            .subscribe(&topic, QoS::AtLeastOnce)
            .await
            .map_err(|e| ChatError::Other(format!("MQTT subscribe failed: {}", e)))?;

        self.subscriptions.write().await.push(channel.to_string());

        ChatMetrics::global().record_transport_operation("mqtt", "subscribe");
        info!("Subscribed to MQTT topic: {}", topic);
        Ok(())
    }

    async fn unsubscribe(&mut self, channel: &str) -> ChatResult<()> {
        let client = self.get_client().await?;
        let topic = format!("b00t/agents/{}/#", channel);

        client
            .unsubscribe(&topic)
            .await
            .map_err(|e| ChatError::Other(format!("MQTT unsubscribe failed: {}", e)))?;

        let mut subscriptions = self.subscriptions.write().await;
        subscriptions.retain(|s| s != channel);

        debug!("Unsubscribed from MQTT topic: {}", topic);
        Ok(())
    }

    async fn publish(&self, channel: &str, message: &ChatMessage) -> ChatResult<()> {
        let client = self.get_client().await?;
        let topic = format!("b00t/agents/{}/broadcast", channel);
        let payload = serde_json::to_vec(message)?;

        client
            .publish(topic.clone(), QoS::AtLeastOnce, false, payload)
            .await
            .map_err(|e| ChatError::Other(format!("MQTT broadcast failed: {}", e)))?;

        debug!("Broadcast to MQTT topic: {}", topic);
        Ok(())
    }

    fn subscriptions(&self) -> Vec<String> {
        // Note: Synchronous method with async data - return empty vec
        // TODO: Consider making this async
        Vec::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mqtt_transport_creation() {
        let transport = MqttTransport::new("localhost:1883", "test-client");
        assert_eq!(transport.kind(), TransportKind::Tcp);
    }

    #[test]
    fn test_message_to_topic_conversion() {
        let message = ChatMessage::new("mission.alpha", "frontend", "test");
        let topic = MqttTransport::message_to_topic(&message);
        assert_eq!(topic, "b00t/agents/mission.alpha/frontend");
    }

    #[tokio::test]
    async fn test_transport_not_connected_initially() {
        let transport = MqttTransport::new("localhost:1883", "test");
        assert!(!transport.is_available().await);
    }
}
