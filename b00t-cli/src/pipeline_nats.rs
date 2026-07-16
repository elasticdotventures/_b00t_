// 🤓 NATS transport adapters — route pipeline stage data through the ACP pub/sub
//    mesh (GH #729).  Provides:
//
//    Traits:
//      NatsTransport      — publish + subscribe (sync façade over async backends)
//      NatsSubscription   — blocking message iterator
//
//    Implementations:
//      AsyncNatsTransport — wraps async_nats::Client (production)
//      MockNatsTransport  — in-memory channel pairs (tests, no server needed)
//      NatsClientAdapter  — bridges existing NatsClient trait → NatsTransport
//
//    Routing:
//      NatsStageRouter    — subject naming + stage output routing
//
//    Subject convention:
//      pipeline.{run_id}.{stage_name}.{direction}.{media_type}

use crate::pipeline_executor::NatsClient;
use crate::pipeline_types::{PortDirection, PortMediaType, StagePort};
use anyhow::{anyhow, Context, Result};
use futures::StreamExt;
use std::collections::HashMap;
use std::future::Future;
use std::sync::{Arc, Mutex};

fn block_on_tokio<F, T>(future: F) -> Result<T>
where
    F: Future<Output = Result<T>> + Send + 'static,
    T: Send + 'static,
{
    let run = move || -> Result<T> {
        tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .context("failed to build tokio runtime")?
            .block_on(future)
    };

    if tokio::runtime::Handle::try_current().is_ok() {
        std::thread::spawn(run)
            .join()
            .map_err(|_| anyhow!("tokio bridge thread panicked"))?
    } else {
        run()
    }
}

// ── NatsTransport trait ─────────────────────────────────────────────────────────

/// Abstraction over NATS pub/sub for pipeline stage transport.
///
/// Methods are synchronous; async implementations use `Handle::block_on`
/// internally.  The sync interface keeps `NatsStageRouter` simple and
/// avoids async-trait overhead in the router hot path.
pub trait NatsTransport: Send + Sync {
    /// Publish a payload to a subject.
    fn publish(&self, subject: &str, payload: &[u8]) -> Result<()>;

    /// Subscribe to a subject, returning a subscription handle.
    fn subscribe(&self, subject: &str) -> Result<Box<dyn NatsSubscription>>;
}

// ── NatsSubscription trait ─────────────────────────────────────────────────────

/// A NATS subscription that yields messages one at a time.
///
/// `next` blocks until a message is available or the subscription is closed.
pub trait NatsSubscription: Send {
    /// Return the next message payload, or `None` if the subscription is closed.
    fn next(&mut self) -> Option<Vec<u8>>;
}

// ── AsyncNatsTransport ─────────────────────────────────────────────────────────

/// Production NATS transport backed by `async_nats::Client`.
///
/// Each subject subscription spawns a tokio task that forwards messages from
/// the async subscriber into a `std::sync::mpsc` channel so the sync
/// `NatsSubscription::next()` can read them without blocking the runtime.
pub struct AsyncNatsTransport {
    client: Arc<async_nats::Client>,
}

impl AsyncNatsTransport {
    pub fn new(client: async_nats::Client) -> Self {
        Self {
            client: Arc::new(client),
        }
    }
}

struct AsyncNatsSubscription {
    rx: std::sync::mpsc::Receiver<Vec<u8>>,
}

impl NatsSubscription for AsyncNatsSubscription {
    fn next(&mut self) -> Option<Vec<u8>> {
        self.rx.recv().ok()
    }
}

impl NatsTransport for AsyncNatsTransport {
    fn publish(&self, subject: &str, payload: &[u8]) -> Result<()> {
        let client = self.client.clone();
        let subject = subject.to_string();
        let payload = payload.to_vec();
        block_on_tokio(async move {
            client
                .publish(subject, payload.into())
                .await
                .context("async_nats publish failed")
        })?;
        Ok(())
    }

    fn subscribe(&self, subject: &str) -> Result<Box<dyn NatsSubscription>> {
        let client = self.client.clone();
        let subject = subject.to_string();

        let subscriber = block_on_tokio(async move {
            client
                .subscribe(subject)
                .await
                .context("async_nats subscribe failed")
        })?;

        let (tx, rx) = std::sync::mpsc::channel::<Vec<u8>>();
        tokio::spawn(async move {
            let mut sub = subscriber;
            while let Some(msg) = sub.next().await {
                if tx.send(msg.payload.to_vec()).is_err() {
                    break; // receiver dropped
                }
            }
        });

        Ok(Box::new(AsyncNatsSubscription { rx }))
    }
}

// ── MockNatsTransport ─────────────────────────────────────────────────────────

/// In-memory mock NATS transport using per-subject broadcast channels.
///
/// `publish` sends to every active subscriber on that subject.
/// `subscribe` creates a new subscriber that receives all future messages.
///
/// Fully self-contained — no NATS server required.
pub struct MockNatsTransport {
    /// Map of subject → list of senders (one per subscriber).
    subscribers: Arc<Mutex<HashMap<String, Vec<std::sync::mpsc::Sender<Vec<u8>>>>>>,
}

impl MockNatsTransport {
    pub fn new() -> Self {
        Self {
            subscribers: Arc::new(Mutex::new(HashMap::new())),
        }
    }
}

impl Default for MockNatsTransport {
    fn default() -> Self {
        Self::new()
    }
}

struct MockNatsSubscription {
    rx: std::sync::mpsc::Receiver<Vec<u8>>,
}

impl NatsSubscription for MockNatsSubscription {
    fn next(&mut self) -> Option<Vec<u8>> {
        // try_recv, not recv: this mock is fully in-memory and synchronous —
        // publish() has already delivered any pending message by the time
        // next() is called, so there's nothing to block-wait for. A blocking
        // recv() here hangs forever on the (deliberately, by-design) empty
        // case — messages published before subscribe(), or genuinely no
        // message sent — because the sender lives inside the still-alive
        // MockNatsTransport and never gets dropped to unblock recv().
        self.rx.try_recv().ok()
    }
}

impl NatsTransport for MockNatsTransport {
    fn publish(&self, subject: &str, payload: &[u8]) -> Result<()> {
        let subscribers = self.subscribers.lock().expect("mock nats lock");
        if let Some(senders) = subscribers.get(subject) {
            let payload = payload.to_vec();
            for tx in senders {
                // Ignore send errors — subscriber may have dropped
                let _ = tx.send(payload.clone());
            }
        }
        Ok(())
    }

    fn subscribe(&self, subject: &str) -> Result<Box<dyn NatsSubscription>> {
        let (tx, rx) = std::sync::mpsc::channel();
        let mut subscribers = self.subscribers.lock().expect("mock nats lock");
        subscribers
            .entry(subject.to_string())
            .or_default()
            .push(tx);
        Ok(Box::new(MockNatsSubscription { rx }))
    }
}

// ── NatsClientAdapter ──────────────────────────────────────────────────────────

/// Adapts the existing async `NatsClient` trait to the sync `NatsTransport` trait.
///
/// This is the bridge that wires `NatsStageRouter` into `PipelineExecutor` via
/// the existing `NatsClient` field — no new connection management needed.
pub struct NatsClientAdapter {
    client: Arc<dyn NatsClient>,
}

impl NatsClientAdapter {
    pub fn new(client: Arc<dyn NatsClient>) -> Self {
        Self { client }
    }
}

struct NatsClientAdapterSubscription {
    /// Buffered messages received via the one-shot NatsClient::subscribe.
    buffer: std::vec::IntoIter<Vec<u8>>,
}

impl NatsSubscription for NatsClientAdapterSubscription {
    fn next(&mut self) -> Option<Vec<u8>> {
        self.buffer.next()
    }
}

impl NatsTransport for NatsClientAdapter {
    fn publish(&self, subject: &str, payload: &[u8]) -> Result<()> {
        let client = self.client.clone();
        let subject = subject.to_string();
        let payload = payload.to_vec();
        block_on_tokio(async move { client.publish(&subject, payload).await })
            .context("NatsClient publish failed")?;
        Ok(())
    }

    fn subscribe(&self, subject: &str) -> Result<Box<dyn NatsSubscription>> {
        let client = self.client.clone();
        let subject = subject.to_string();
        let data = block_on_tokio(async move { client.subscribe(&subject).await })
            .context("NatsClient subscribe failed")?;
        let buffer = data.into_iter().collect::<Vec<Vec<u8>>>();
        Ok(Box::new(NatsClientAdapterSubscription {
            buffer: buffer.into_iter(),
        }))
    }
}

// ── NatsStageRouter ────────────────────────────────────────────────────────────

/// Routes pipeline stage outputs to NATS subjects following the ACP naming
/// convention: `pipeline.{run_id}.{stage_name}.{direction}.{media_type}`.
///
/// Wraps a `NatsTransport` implementation so it can be backed by a real NATS
/// connection, a mock, or an adapter over the existing `NatsClient` trait.
pub struct NatsStageRouter {
    transport: Box<dyn NatsTransport>,
    run_id: String,
}

impl NatsStageRouter {
    /// Create a new router for a given pipeline run.
    pub fn new(transport: Box<dyn NatsTransport>, run_id: &str) -> Self {
        Self {
            transport,
            run_id: run_id.to_string(),
        }
    }

    /// Build the NATS subject string for a stage port.
    ///
    /// Convention: `pipeline.{run_id}.{stage_name}.{direction}.{media_type}`
    /// where `direction` is "input" or "output" and `media_type` is the
    /// kebab-case representation of `PortMediaType`.
    pub fn subject_for(&self, stage: &str, port: &StagePort) -> String {
        let direction = match port.direction {
            PortDirection::Input => "input",
            PortDirection::Output => "output",
        };
        let media_type = media_type_to_str(&port.media_type);
        format!(
            "pipeline.{}.{}.{}.{}",
            self.run_id, stage, direction, media_type
        )
    }

    /// Publish stage output data to the correct NATS subject.
    ///
    /// `from` is the source stage name, `to` the destination stage name,
    /// `port` is the output port with direction/media_type, and `data` is
    /// the raw payload.
    pub fn route_output(
        &self,
        from: &str,
        _to: &str,
        port: &StagePort,
        data: &[u8],
    ) -> Result<()> {
        let subject = self.subject_for(from, port);
        self.transport
            .publish(&subject, data)
            .with_context(|| format!("route_output failed for '{subject}'"))
    }

    /// Subscribe to a stage's input port, returning a message stream.
    ///
    /// The returned subscription yields all data published to the subject
    /// matching the stage name and the given port.
    pub fn subscribe_stage(
        &self,
        stage: &str,
        port: &StagePort,
    ) -> Result<Box<dyn NatsSubscription>> {
        let subject = self.subject_for(stage, port);
        self.transport
            .subscribe(&subject)
            .with_context(|| format!("subscribe_stage failed for '{subject}'"))
    }

    /// Return a reference to the underlying transport (for advanced use).
    pub fn transport(&self) -> &dyn NatsTransport {
        self.transport.as_ref()
    }
}

// ── Helpers ────────────────────────────────────────────────────────────────────

/// Convert a `PortMediaType` to its kebab-case string representation.
fn media_type_to_str(mt: &PortMediaType) -> &'static str {
    match mt {
        PortMediaType::Video => "video",
        PortMediaType::Audio => "audio",
        PortMediaType::Image => "image",
        PortMediaType::Json => "json",
        PortMediaType::Parquet => "parquet",
        PortMediaType::Bytes => "bytes",
        PortMediaType::Error => "error",
    }
}

// ── Tests ──────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pipeline_types::PortDirection;

    fn output_port(mt: PortMediaType) -> StagePort {
        StagePort {
            direction: PortDirection::Output,
            media_type: mt,
            description: None,
        }
    }

    fn input_port(mt: PortMediaType) -> StagePort {
        StagePort {
            direction: PortDirection::Input,
            media_type: mt,
            description: None,
        }
    }

    // ── MockTransport: publish / subscribe round-trip ─────────────────────

    #[test]
    fn mock_transport_round_trip() {
        let transport = MockNatsTransport::new();
        let subject = "pipeline.test-run.stage-a.output.video";

        // Subscribe *before* publish: per the mock's documented semantics
        // (see mock_transport_publish_before_subscribe_lost below), a
        // subscriber only receives messages published after it subscribes.
        let mut sub = transport.subscribe(subject).expect("subscribe");
        transport
            .publish(subject, b"hello-nats")
            .expect("publish");

        let msg = sub.next().expect("should receive message");
        assert_eq!(msg, b"hello-nats");
    }

    // ── MockTransport: multiple subscribers on same subject ──────────────

    #[test]
    fn mock_transport_multiple_subscribers() {
        let transport = MockNatsTransport::new();
        let subject = "pipeline.test-run.stage-b.output.audio";

        let mut sub_a = transport.subscribe(subject).expect("subscribe a");
        let mut sub_b = transport.subscribe(subject).expect("subscribe b");

        transport
            .publish(subject, b"fan-out-test")
            .expect("publish");

        assert_eq!(sub_a.next(), Some(b"fan-out-test".to_vec()));
        assert_eq!(sub_b.next(), Some(b"fan-out-test".to_vec()));
    }

    // ── MockTransport: works without NATS server (no panics) ────────────

    #[test]
    fn mock_transport_no_server_needed() {
        let transport = MockNatsTransport::new();
        // Should not panic or error — fully in-memory.
        transport
            .publish("pipeline.x.y.output.bytes", b"data")
            .expect("publish without server");

        let mut sub = transport
            .subscribe("pipeline.x.y.output.bytes")
            .expect("subscribe without server");

        // The publish happened before subscribe, so this message is lost
        // (mock only delivers messages published after subscribe).
        assert!(sub.next().is_none());
    }

    // ── Subject naming convention ───────────────────────────────────────

    #[test]
    fn subject_for_output_video() {
        let router = NatsStageRouter::new(
            Box::new(MockNatsTransport::new()),
            "run-abc",
        );
        let port = output_port(PortMediaType::Video);
        let subject = router.subject_for("transcode", &port);
        assert_eq!(subject, "pipeline.run-abc.transcode.output.video");
    }

    #[test]
    fn subject_for_input_json() {
        let router = NatsStageRouter::new(
            Box::new(MockNatsTransport::new()),
            "run-xyz",
        );
        let port = input_port(PortMediaType::Json);
        let subject = router.subject_for("analyze", &port);
        assert_eq!(subject, "pipeline.run-xyz.analyze.input.json");
    }

    #[test]
    fn subject_for_output_bytes() {
        let router = NatsStageRouter::new(
            Box::new(MockNatsTransport::new()),
            "test-42",
        );
        let port = output_port(PortMediaType::Bytes);
        let subject = router.subject_for("ingest", &port);
        assert_eq!(subject, "pipeline.test-42.ingest.output.bytes");
    }

    #[test]
    fn subject_for_all_media_types() {
        let router = NatsStageRouter::new(
            Box::new(MockNatsTransport::new()),
            "run-99",
        );
        for (mt, expected) in &[
            (PortMediaType::Video, "video"),
            (PortMediaType::Audio, "audio"),
            (PortMediaType::Image, "image"),
            (PortMediaType::Json, "json"),
            (PortMediaType::Parquet, "parquet"),
            (PortMediaType::Bytes, "bytes"),
            (PortMediaType::Error, "error"),
        ] {
            let port = output_port(mt.clone());
            let subject = router.subject_for("stage", &port);
            assert!(
                subject.ends_with(expected),
                "expected subject to end with '{}', got '{}'",
                expected,
                subject
            );
        }
    }

    // ── Router: round-trip through MockTransport ────────────────────────

    #[test]
    fn router_publish_and_subscribe_round_trip() {
        let transport = MockNatsTransport::new();
        let router = NatsStageRouter::new(Box::new(transport), "round-trip");

        let port = output_port(PortMediaType::Bytes);
        let subject = router.subject_for("source", &port);

        // Subscribe first (mock delivers only post-subscribe messages).
        let mut sub = router.subscribe_stage("source", &port).expect("subscribe");

        // Publish via router.
        router
            .route_output("source", "dest", &port, b"routed-data")
            .expect("route_output");

        let msg = sub.next().expect("should receive routed data");
        assert_eq!(msg, b"routed-data");
    }

    // ── NatsClientAdapter bridge ────────────────────────────────────────

    #[tokio::test]
    async fn nats_client_adapter_round_trip() {
        // NatsClientAdapter's methods are sync-but-block_on-internally (see
        // the module doc) — calling them directly from this #[tokio::test]
        // async fn would nest block_on inside the runtime already driving
        // this test ("Cannot start a runtime from within a runtime").
        // spawn_blocking moves the whole sync sequence onto a blocking-pool
        // thread, where that internal block_on is safe.
        let mock_nats = Arc::new(crate::pipeline_executor::MockNatsClient::new())
            as Arc<dyn NatsClient>;
        let adapter = NatsClientAdapter::new(mock_nats.clone());
        let subject = "pipeline.adapter-test.stage.output.bytes";

        let msg = tokio::task::spawn_blocking(move || {
            adapter
                .publish(subject, b"adapter-payload")
                .expect("adapter publish");
            let mut sub = adapter.subscribe(subject).expect("adapter subscribe");
            sub.next().expect("should receive from adapter")
        })
        .await
        .expect("blocking task panicked");

        assert_eq!(msg, b"adapter-payload");
    }

    // ── MockTransport publish before subscribe ──────────────────────────

    #[test]
    fn mock_transport_publish_before_subscribe_lost() {
        // Messages published before a subscription is created are not
        // delivered (by design — mock uses live channels).
        let transport = MockNatsTransport::new();
        transport
            .publish("pipeline.test.s.output.bytes", b"early")
            .expect("publish early");

        let mut sub = transport
            .subscribe("pipeline.test.s.output.bytes")
            .expect("subscribe late");
        assert!(sub.next().is_none(), "early messages should be lost");
    }

    // ── Multiple publishes on same subject ──────────────────────────────

    #[test]
    fn mock_transport_multiple_publishes() {
        let transport = MockNatsTransport::new();
        let subject = "pipeline.multi-pub.stage.output.image";

        let mut sub = transport.subscribe(subject).expect("subscribe");

        for i in 0..5 {
            transport
                .publish(subject, format!("msg-{}", i).as_bytes())
                .expect("publish");
        }

        for i in 0..5 {
            let msg = sub.next().unwrap_or_else(|| panic!("expected msg-{}", i));
            assert_eq!(msg, format!("msg-{}", i).as_bytes());
        }
    }

    // ── Router with different run_ids produces different subjects ───────

    #[test]
    fn different_run_ids_different_subjects() {
        let router_a = NatsStageRouter::new(
            Box::new(MockNatsTransport::new()),
            "run-alpha",
        );
        let router_b = NatsStageRouter::new(
            Box::new(MockNatsTransport::new()),
            "run-beta",
        );

        let port = output_port(PortMediaType::Json);
        let sub_a = router_a.subject_for("stage-1", &port);
        let sub_b = router_b.subject_for("stage-1", &port);

        assert_ne!(sub_a, sub_b, "different run_ids must differ");
        assert!(sub_a.contains("run-alpha"));
        assert!(sub_b.contains("run-beta"));
    }

    // ── NatsClientAdapter handles empty subscription ────────────────────

    #[tokio::test]
    async fn nats_client_adapter_no_message() {
        // See nats_client_adapter_round_trip for why this is spawn_blocking.
        let mock_nats = Arc::new(crate::pipeline_executor::MockNatsClient::new())
            as Arc<dyn NatsClient>;
        let adapter = NatsClientAdapter::new(mock_nats);

        let received = tokio::task::spawn_blocking(move || {
            let mut sub = adapter
                .subscribe("pipeline.empty.sub.output.bytes")
                .expect("subscribe to empty");
            sub.next()
        })
        .await
        .expect("blocking task panicked");

        assert!(received.is_none(), "no message should be available");
    }

    // ── NatsClientAdapter: subject isolation ────────────────────────────

    #[tokio::test]
    async fn nats_client_adapter_subject_isolation() {
        // See nats_client_adapter_round_trip for why this is spawn_blocking.
        let mock_nats = Arc::new(crate::pipeline_executor::MockNatsClient::new())
            as Arc<dyn NatsClient>;
        let adapter = NatsClientAdapter::new(mock_nats);

        let received = tokio::task::spawn_blocking(move || {
            adapter
                .publish("pipeline.isolation.a.output.bytes", b"data-a")
                .expect("publish a");

            let mut sub_b = adapter
                .subscribe("pipeline.isolation.b.output.bytes")
                .expect("subscribe b");
            sub_b.next()
        })
        .await
        .expect("blocking task panicked");

        // Subject B should have no messages (isolation).
        assert!(received.is_none(), "subjects must be isolated");
    }
}
