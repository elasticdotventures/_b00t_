//! Hive transport abstraction — broker-neutral substrate for the intra-hive mesh.
//!
//! The mesh needs only a small, broker-shaped surface: connect, publish to a
//! subject, and subscribe to a subject's stream. `HiveTransport` is that
//! surface. Concrete impls are pluggable:
//!
//! - [`NatsHiveTransport`] (feature `nats`, default) — the current NATS broker.
//! - [`MemoryHiveTransport`] — in-process pub/sub, broker-free; backs tests and
//!   the gossip-first discovery path with no external broker.
//! - [`IrohHiveTransport`] (feature `iroh`) — strategic NodeId/QUIC substrate;
//!   stub until the iroh backend is wired.
//!
//! `NatsMeshNode` holds an `Arc<dyn HiveTransport>`, so the broker is fully
//! behind the abstraction and can be `#[cfg]`-disabled or swapped (gossip is the
//! initial discovery that hands off/establishes to whichever broker is active).

use crate::error::{ChatError, ChatResult};
use crate::ipc_transport::TransportKind;
use async_trait::async_trait;
use futures::StreamExt;
use futures::stream::BoxStream;
use std::fmt::Debug;
use std::sync::Arc;

/// Broker-neutral surface the intra-hive mesh requires.
///
/// Subjects follow the `b00t.hive.mesh.*` namespace (see `mesh.rs`). Reply
/// semantics are expressed by publishing to a subject (the querier's own inbox),
/// so no broker-specific reply primitive is needed.
#[async_trait]
pub trait HiveTransport: Send + Sync + Debug {
    /// Establish the broker connection for `url` (URL form is transport-defined).
    async fn connect(&self, url: &str) -> ChatResult<()>;
    /// Fire-and-forget publish of raw bytes to a subject.
    async fn publish(&self, subject: &str, payload: &[u8]) -> ChatResult<()>;
    /// Subscribe to a subject; yields raw frame payloads as they arrive.
    async fn subscribe(&self, subject: &str) -> ChatResult<BoxStream<'static, Vec<u8>>>;
    /// Transport kind, for telemetry.
    fn kind(&self) -> TransportKind;
    /// Whether a connection is currently established.
    async fn is_available(&self) -> bool;
    /// Tear down the connection.
    async fn close(&self) -> ChatResult<()>;
}

#[cfg(feature = "nats")]
pub use nats_impl::NatsHiveTransport;

#[cfg(feature = "nats")]
mod nats_impl {
    use super::*;
    use tokio::sync::RwLock;

    /// NATS-backed hive transport (the default broker).
    #[derive(Debug, Default)]
    pub struct NatsHiveTransport {
        client: Arc<RwLock<Option<async_nats::Client>>>,
    }

    impl NatsHiveTransport {
        pub fn new() -> Self {
            Self::default()
        }
    }

    #[async_trait]
    impl HiveTransport for NatsHiveTransport {
        async fn connect(&self, url: &str) -> ChatResult<()> {
            let client = async_nats::connect(url)
                .await
                .map_err(|e| ChatError::Nats(e.to_string()))?;
            *self.client.write().await = Some(client);
            Ok(())
        }

        async fn publish(&self, subject: &str, payload: &[u8]) -> ChatResult<()> {
            let client = {
                let guard = self.client.read().await;
                guard
                    .as_ref()
                    .ok_or_else(|| ChatError::Other("hive transport not connected".into()))?
                    .clone()
            };
            let bytes = bytes::Bytes::copy_from_slice(payload);
            client
                .publish(subject.to_string(), bytes)
                .await
                .map_err(|e| ChatError::Nats(e.to_string()))
        }

        async fn subscribe(&self, subject: &str) -> ChatResult<BoxStream<'static, Vec<u8>>> {
            let client = {
                let guard = self.client.read().await;
                guard
                    .as_ref()
                    .ok_or_else(|| ChatError::Other("hive transport not connected".into()))?
                    .clone()
            };
            let sub = client
                .subscribe(subject.to_string())
                .await
                .map_err(|e| ChatError::Nats(e.to_string()))?;
            let stream = sub.map(|m| m.payload.to_vec());
            Ok(Box::pin(stream))
        }

        fn kind(&self) -> TransportKind {
            TransportKind::Nats
        }

        async fn is_available(&self) -> bool {
            self.client.read().await.is_some()
        }

        async fn close(&self) -> ChatResult<()> {
            *self.client.write().await = None;
            Ok(())
        }
    }
}

/// In-process pub/sub transport — fully broker-free.
///
/// All clones share one registry (Arc), so multiple `NatsMeshNode`s built from
/// cloned `MemoryHiveTransport`s communicate with no external broker. This backs
/// the gossip-first discovery path and broker-free tests.
#[derive(Debug, Clone, Default)]
pub struct MemoryHiveTransport {
    subs: Arc<
        tokio::sync::Mutex<
            std::collections::HashMap<String, Vec<tokio::sync::broadcast::Sender<Vec<u8>>>>,
        >,
    >,
}

impl MemoryHiveTransport {
    pub fn new() -> Self {
        Self::default()
    }
}

#[async_trait]
impl HiveTransport for MemoryHiveTransport {
    async fn connect(&self, _url: &str) -> ChatResult<()> {
        Ok(())
    }

    async fn publish(&self, subject: &str, payload: &[u8]) -> ChatResult<()> {
        let subs = self.subs.lock().await;
        if let Some(senders) = subs.get(subject) {
            for s in senders {
                let _ = s.send(payload.to_vec());
            }
        }
        Ok(())
    }

    async fn subscribe(&self, subject: &str) -> ChatResult<BoxStream<'static, Vec<u8>>> {
        use tokio_stream::wrappers::BroadcastStream;
        let (tx, rx) = tokio::sync::broadcast::channel(1024);
        self.subs
            .lock()
            .await
            .entry(subject.to_string())
            .or_default()
            .push(tx);
        // Drop lagged/closed signals; keep only live payloads.
        let stream = BroadcastStream::new(rx).filter_map(|r| async move {
            match r {
                Ok(v) => Some(v),
                Err(_) => None,
            }
        });
        Ok(Box::pin(stream))
    }

    fn kind(&self) -> TransportKind {
        TransportKind::Memory
    }

    async fn is_available(&self) -> bool {
        true
    }

    async fn close(&self) -> ChatResult<()> {
        self.subs.lock().await.clear();
        Ok(())
    }
}

/// Iroh-backed hive transport (feature `iroh`) — strategic NodeId/QUIC substrate.
///
/// Stub: the iroh crate is not yet wired. Enabling feature `iroh` compiles this
/// placeholder; real endpoint dialing + topic gossip is the follow-up that makes
/// iroh a drop-in alternate to the NATS broker for intra-hive discovery.
#[cfg(feature = "iroh")]
pub use iroh_impl::IrohHiveTransport;

#[cfg(feature = "iroh")]
mod iroh_impl {
    use super::*;

    #[derive(Debug, Default)]
    pub struct IrohHiveTransport {
        _node: Option<()>,
    }

    impl IrohHiveTransport {
        pub fn new() -> Self {
            Self::default()
        }
    }

    #[async_trait]
    impl HiveTransport for IrohHiveTransport {
        async fn connect(&self, _url: &str) -> ChatResult<()> {
            Err(ChatError::Other(
                "iroh HiveTransport not yet implemented — wire the iroh node/endpoint and topic gossip"
                    .into(),
            ))
        }
        async fn publish(&self, _subject: &str, _payload: &[u8]) -> ChatResult<()> {
            Err(ChatError::Other(
                "iroh HiveTransport not yet implemented".into(),
            ))
        }
        async fn subscribe(&self, _subject: &str) -> ChatResult<BoxStream<'static, Vec<u8>>> {
            Err(ChatError::Other(
                "iroh HiveTransport not yet implemented".into(),
            ))
        }
        fn kind(&self) -> TransportKind {
            TransportKind::Iroh
        }
        async fn is_available(&self) -> bool {
            false
        }
        async fn close(&self) -> ChatResult<()> {
            Ok(())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn memory_transport_delivers_published_frames() {
        let shared = Arc::new(MemoryHiveTransport::new());
        let sub = shared.subscribe("b00t.hive.mesh.node.a").await.unwrap();
        shared
            .publish("b00t.hive.mesh.node.a", b"hello")
            .await
            .unwrap();
        let mut sub = sub;
        let got = sub.next().await.unwrap();
        assert_eq!(got, b"hello");
    }
}
