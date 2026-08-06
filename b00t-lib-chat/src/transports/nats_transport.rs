//! NATS transport implementation for agent communication.
//!
//! Provides cloud-native, high-scale messaging via NATS.io for
//! distributed b00t agents across different hosts/regions.
//!
//! All subscriptions are fused into a single inbound mpsc channel so `recv()`
//! correctly performs fan-in across every subscribed channel (the previous
//! implementation only polled the first subscriber). The active subscription
//! list is tracked in a sync `RwLock` so `subscriptions()` returns real data
//! instead of an empty vec.

use crate::error::{ChatError, ChatResult};
use crate::ipc_transport::{BroadcastTransport, IpcTransport, TransportKind};
use crate::message::ChatMessage;
use crate::metrics::{ChatMetrics, LatencyTimer};
use async_nats::{Client, Subscriber};
use async_trait::async_trait;
use futures::StreamExt;
use std::sync::{Arc, RwLock as StdRwLock};
use tokio::sync::{Mutex as AsyncMutex, RwLock, mpsc};
use tracing::{debug, info};

/// NATS transport for distributed agent messaging.
#[derive(Debug, Clone)]
pub struct NatsTransport {
    client: Arc<RwLock<Option<Client>>>,
    url: String,
    /// Channels currently subscribed (sync so `subscriptions()` can read it).
    subscriptions: Arc<StdRwLock<Vec<String>>>,
    /// Fused inbound messages from all subscriber tasks.
    inbox_tx: mpsc::UnboundedSender<ChatMessage>,
    inbox_rx: Arc<AsyncMutex<mpsc::UnboundedReceiver<ChatMessage>>>,
    /// Per-channel forwarding tasks, aborted on unsubscribe/close.
    forwards: Arc<RwLock<Vec<(String, tokio::task::JoinHandle<()>)>>>,
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
        let (inbox_tx, inbox_rx) = mpsc::unbounded_channel();
        Self {
            client: Arc::new(RwLock::new(None)),
            url: url.into(),
            subscriptions: Arc::new(StdRwLock::new(Vec::new())),
            inbox_tx,
            inbox_rx: Arc::new(AsyncMutex::new(inbox_rx)),
            forwards: Arc::new(RwLock::new(Vec::new())),
        }
    }

    /// Connect to NATS server.
    pub async fn connect(&self) -> ChatResult<()> {
        let mut client_guard = self.client.write().await;

        if client_guard.is_some() {
            return Ok(()); // Already connected
        }

        let options = async_nats::ConnectOptions::new();
        let client = options.connect(&self.url).await.map_err(|e| {
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
        client_guard.clone().ok_or(ChatError::NotConnected)
    }

    /// Convert ChatMessage to NATS subject format.
    ///
    /// Maps `channel.sender` to NATS subject hierarchy.
    fn message_to_subject(message: &ChatMessage) -> String {
        format!("b00t.agents.{}.{}", message.channel, message.sender)
    }

    /// Spawn a task that forwards every message from `subscriber` into the
    /// fused inbound channel.
    fn forward(&self, mut subscriber: Subscriber) {
        let inbox_tx = self.inbox_tx.clone();
        tokio::spawn(async move {
            while let Some(msg) = subscriber.next().await {
                let parsed: ChatMessage = match serde_json::from_slice(&msg.payload) {
                    Ok(m) => m,
                    Err(e) => {
                        debug!("NATS: dropping undecodable message: {}", e);
                        continue;
                    }
                };
                if inbox_tx.send(parsed).is_err() {
                    break; // transport dropped
                }
            }
        });
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
        let mut inbox = self.inbox_rx.lock().await;
        match inbox.recv().await {
            Some(message) => {
                ChatMetrics::global().record_message_received("nats", &message.channel);
                timer.stop();
                Ok(Some(message))
            }
            None => Ok(None),
        }
    }

    async fn close(&self) -> ChatResult<()> {
        let mut client_guard = self.client.write().await;
        *client_guard = None;

        for (_, handle) in self.forwards.write().await.drain(..) {
            handle.abort();
        }
        self.subscriptions.write().unwrap().clear();

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

        self.forward(subscriber);

        self.subscriptions
            .write()
            .unwrap()
            .push(channel.to_string());

        ChatMetrics::global().record_transport_operation("nats", "subscribe");
        info!("Subscribed to NATS subject: {}", subject);
        Ok(())
    }

    async fn unsubscribe(&mut self, channel: &str) -> ChatResult<()> {
        self.subscriptions.write().unwrap().retain(|s| s != channel);

        let mut forwards = self.forwards.write().await;
        if let Some(idx) = forwards.iter().position(|(ch, _)| ch == channel) {
            let (_, handle) = forwards.remove(idx);
            handle.abort();
        }

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
        self.subscriptions.read().unwrap().clone()
    }
}
