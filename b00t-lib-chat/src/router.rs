//! Unified message router with polymorphic transport selection.
//!
//! Routes messages to the appropriate transport (local sockets, Redis, NATS)
//! based on destination type and agent availability.

use crate::discovery::SocketRegistry;
use crate::error::{ChatError, ChatResult};
use crate::ipc_transport::{AgentEndpoint, DiscoverableTransport, TransportKind};
use crate::message::ChatMessage;
use crate::transport::LocalSocketTransport;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{debug, info, warn};

/// Message destination selector.
#[derive(Debug, Clone)]
pub enum Destination {
    /// Send to a specific agent by ID.
    Agent(String),
    /// Broadcast to all agents in a crew.
    Crew(String),
    /// Broadcast to all known agents.
    Broadcast,
    /// Send to a specific endpoint URI.
    Direct(String),
}

/// Unified message router that selects the best transport for each message.
///
/// Routes messages using a priority system:
/// 1. Local Unix sockets (fastest, same-host)
/// 2. Redis pub/sub (distributed, persistent)
/// 3. NATS (cloud-native, high-scale)
pub struct MessageRouter {
    registry: Arc<RwLock<SocketRegistry>>,
    #[allow(dead_code)]
    redis_available: bool,
    #[allow(dead_code)]
    nats_available: bool,
}

impl MessageRouter {
    /// Create a new message router with a socket registry.
    pub fn new(registry: SocketRegistry) -> Self {
        Self {
            registry: Arc::new(RwLock::new(registry)),
            redis_available: false,
            nats_available: false,
        }
    }

    /// Enable Redis transport.
    pub fn with_redis(mut self) -> Self {
        self.redis_available = true;
        self
    }

    /// Enable NATS transport.
    pub fn with_nats(mut self) -> Self {
        self.nats_available = true;
        self
    }

    /// Route a message to its destination.
    pub async fn route(&self, message: &ChatMessage, destination: &Destination) -> ChatResult<()> {
        match destination {
            Destination::Agent(agent_id) => {
                self.route_to_agent(message, agent_id).await
            }
            Destination::Crew(crew_id) => {
                self.route_to_crew(message, crew_id).await
            }
            Destination::Broadcast => {
                self.route_broadcast(message).await
            }
            Destination::Direct(uri) => {
                self.route_direct(message, uri).await
            }
        }
    }

    /// Route message to a specific agent.
    async fn route_to_agent(&self, message: &ChatMessage, agent_id: &str) -> ChatResult<()> {
        let registry = self.registry.read().await;

        // Try to find agent in registry
        if let Some(endpoint) = registry.get_agent(agent_id).await {
            match endpoint.transport_kind {
                TransportKind::UnixSocket => {
                    // Use local socket transport
                    let transport = LocalSocketTransport::new(Some(endpoint.endpoint_uri.into()))?;
                    transport.send(message).await?;
                    info!("📨 Routed to {} via Unix socket", agent_id);
                    Ok(())
                }
                _ => {
                    // Fall back to other transports
                    warn!("Transport {:?} not yet implemented, using fallback", endpoint.transport_kind);
                    self.route_fallback(message, agent_id).await
                }
            }
        } else {
            warn!("Agent {} not found in registry", agent_id);
            Err(ChatError::AgentNotFound(agent_id.to_string()))
        }
    }

    /// Route message to all agents in a crew.
    async fn route_to_crew(&self, message: &ChatMessage, crew_id: &str) -> ChatResult<()> {
        let registry = self.registry.read().await;
        let all_agents = registry.discover_agents().await?;

        let mut sent_count = 0;
        let mut errors = Vec::new();

        for endpoint in all_agents {
            // Filter by crew metadata if available
            if let Some(metadata) = &endpoint.metadata {
                if let Some(agent_crew) = metadata.get("crew").and_then(|v| v.as_str()) {
                    if agent_crew == crew_id {
                        if let Err(e) = self.route_to_endpoint(message, &endpoint).await {
                            errors.push((endpoint.agent_id.clone(), e));
                        } else {
                            sent_count += 1;
                        }
                    }
                }
            }
        }

        info!("📡 Broadcast to crew '{}': {} agents reached", crew_id, sent_count);

        if sent_count == 0 && !errors.is_empty() {
            Err(ChatError::Other(format!(
                "Failed to reach any crew members: {:?}",
                errors
            )))
        } else {
            Ok(())
        }
    }

    /// Broadcast message to all known agents.
    async fn route_broadcast(&self, message: &ChatMessage) -> ChatResult<()> {
        let registry = self.registry.read().await;
        let all_agents = registry.discover_agents().await?;

        let mut sent_count = 0;

        for endpoint in all_agents {
            if let Err(e) = self.route_to_endpoint(message, &endpoint).await {
                warn!("Failed to send to {}: {}", endpoint.agent_id, e);
            } else {
                sent_count += 1;
            }
        }

        info!("📢 Broadcast: {} agents reached", sent_count);
        Ok(())
    }

    /// Route to a specific endpoint URI.
    async fn route_direct(&self, message: &ChatMessage, uri: &str) -> ChatResult<()> {
        let transport = LocalSocketTransport::new(Some(uri.into()))?;
        transport.send(message).await?;
        debug!("📨 Direct send to {}", uri);
        Ok(())
    }

    /// Route to a discovered endpoint.
    async fn route_to_endpoint(&self, message: &ChatMessage, endpoint: &AgentEndpoint) -> ChatResult<()> {
        match endpoint.transport_kind {
            TransportKind::UnixSocket => {
                let transport = LocalSocketTransport::new(Some(endpoint.endpoint_uri.clone().into()))?;
                transport.send(message).await
            }
            _ => {
                // Future: implement Redis/NATS transports
                Err(ChatError::Other(format!(
                    "Transport {:?} not implemented",
                    endpoint.transport_kind
                )))
            }
        }
    }

    /// Fallback routing when primary transport fails.
    async fn route_fallback(&self, _message: &ChatMessage, agent_id: &str) -> ChatResult<()> {
        // Future: try Redis pub/sub, then NATS
        warn!("No fallback transport available for {}", agent_id);
        Err(ChatError::NotConnected)
    }

    /// Get the socket registry for direct access.
    pub fn registry(&self) -> Arc<RwLock<SocketRegistry>> {
        self.registry.clone()
    }
}

/// Builder for MessageRouter with fluent configuration.
pub struct MessageRouterBuilder {
    registry: Option<SocketRegistry>,
    enable_redis: bool,
    enable_nats: bool,
}

impl MessageRouterBuilder {
    pub fn new() -> Self {
        Self {
            registry: None,
            enable_redis: false,
            enable_nats: false,
        }
    }

    pub fn with_registry(mut self, registry: SocketRegistry) -> Self {
        self.registry = Some(registry);
        self
    }

    pub fn enable_redis(mut self) -> Self {
        self.enable_redis = true;
        self
    }

    pub fn enable_nats(mut self) -> Self {
        self.enable_nats = true;
        self
    }

    pub fn build(self) -> ChatResult<MessageRouter> {
        let registry = self.registry.ok_or_else(|| {
            ChatError::Other("MessageRouter requires a SocketRegistry".to_string())
        })?;

        let mut router = MessageRouter::new(registry);

        if self.enable_redis {
            router = router.with_redis();
        }

        if self.enable_nats {
            router = router.with_nats();
        }

        Ok(router)
    }
}

impl Default for MessageRouterBuilder {
    fn default() -> Self {
        Self::new()
    }
}

/// Macro for sending messages with automatic routing.
#[macro_export]
macro_rules! route_message {
    ($router:expr, $msg:expr => agent $agent_id:expr) => {
        $router.route(&$msg, &$crate::router::Destination::Agent($agent_id.to_string())).await
    };
    ($router:expr, $msg:expr => crew $crew_id:expr) => {
        $router.route(&$msg, &$crate::router::Destination::Crew($crew_id.to_string())).await
    };
    ($router:expr, $msg:expr => broadcast) => {
        $router.route(&$msg, &$crate::router::Destination::Broadcast).await
    };
    ($router:expr, $msg:expr => direct $uri:expr) => {
        $router.route(&$msg, &$crate::router::Destination::Direct($uri.to_string())).await
    };
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::SocketRegistryBuilder;

    #[tokio::test]
    async fn test_router_creation() {
        let registry = SocketRegistryBuilder::new()
            .with_system_dir()
            .build();

        let router = MessageRouterBuilder::new()
            .with_registry(registry)
            .enable_redis()
            .build()
            .unwrap();

        assert!(router.redis_available);
    }

    #[tokio::test]
    async fn test_destination_types() {
        let dest_agent = Destination::Agent("alpha".to_string());
        let dest_crew = Destination::Crew("backend".to_string());
        let dest_broadcast = Destination::Broadcast;

        match dest_agent {
            Destination::Agent(id) => assert_eq!(id, "alpha"),
            _ => panic!("Wrong destination type"),
        }

        match dest_crew {
            Destination::Crew(id) => assert_eq!(id, "backend"),
            _ => panic!("Wrong destination type"),
        }

        matches!(dest_broadcast, Destination::Broadcast);
    }
}
