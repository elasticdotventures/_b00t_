//! Pipeline Store NATS Adapter — expose store backends as NATS request-reply subjects.
//!
//! Enables multi-agent store access through NATS request-reply pattern.
//!
//! # Subject convention
//! - `store.get.<key>`       → reply with value bytes (empty if not found)
//! - `store.set.<key>`       → set value from payload, reply with "ok"
//! - `store.del.<key>`       → delete key, reply with "ok"
//! - `store.list.<prefix>`   → reply with JSON array of matching keys
//!
//! Subscribe to `store.>` (NATS wildcard) and dispatch based on the verb in the
//! second token.  Keys may contain dots (preserved by `splitn(3, '.')`).
//!
//! # Testing
//! Use `MemoryStoreBackend` as a lightweight in-memory backend for unit tests.
//! The test module provides a channel-based `MockNatsClient` that does not need
//! a running NATS server.

use anyhow::{Context, Result};
use async_trait::async_trait;
use futures::StreamExt;
use std::collections::HashMap;
use std::sync::{Arc, RwLock};

// ── Internal NATS operation traits (for testability) ──────────────────────────

/// Internal: async subscription that yields one message at a time.
#[async_trait]
trait NatsSubscriptionOp: Send {
    /// Return the next (subject, payload, reply_subject), or `None` when closed.
    async fn next(&mut self) -> Option<(String, Vec<u8>, Option<String>)>;
}

/// Internal: NATS client operations needed by the adapter.
#[async_trait]
trait NatsClientOp: Send + Sync {
    /// Subscribe to a subject pattern (NATS wildcards supported).
    async fn subscribe(&self, subject: &str) -> Result<Box<dyn NatsSubscriptionOp>>;

    /// Publish a payload to a subject.
    async fn publish(&self, subject: &str, payload: Vec<u8>) -> Result<()>;
}

// ── RealNatsClient (wraps async_nats::Client) ───────────────────────────────────

/// Adapter that bridges `async_nats::Client` to `NatsClientOp`.
struct RealNatsClient(async_nats::Client);

#[async_trait]
impl NatsClientOp for RealNatsClient {
    async fn subscribe(&self, subject: &str) -> Result<Box<dyn NatsSubscriptionOp>> {
        let sub = self
            .0
            .subscribe(subject.to_string())
            .await
            .context("async_nats subscribe failed")?;
        Ok(Box::new(RealNatsSubscription(sub)))
    }

    async fn publish(&self, subject: &str, payload: Vec<u8>) -> Result<()> {
        self.0
            .publish(subject.to_string(), payload.into())
            .await
            .context("async_nats publish failed")?;
        Ok(())
    }
}

struct RealNatsSubscription(async_nats::Subscriber);

#[async_trait]
impl NatsSubscriptionOp for RealNatsSubscription {
    async fn next(&mut self) -> Option<(String, Vec<u8>, Option<String>)> {
        let msg = self.0.next().await?;
        Some((
            msg.subject.to_string(),
            msg.payload.to_vec(),
            msg.reply.map(|s| s.to_string()),
        ))
    }
}

// ── StoreBackend trait ─────────────────────────────────────────────────────────

/// Abstract key-value store backend.
///
/// All methods are fallible (`anyhow::Result`) to accommodate both in-memory
/// and persistent (SQLite, S3, etc.) implementations.
pub trait StoreBackend: Send + Sync {
    /// Get a value by key. Returns `None` if the key does not exist.
    fn get(&self, key: &str) -> Result<Option<Vec<u8>>>;

    /// Set a value for a key. Overwrites any existing value.
    fn set(&self, key: &str, value: Vec<u8>) -> Result<()>;

    /// Delete a key. No-op if the key does not exist.
    fn delete(&self, key: &str) -> Result<()>;

    /// List all keys with the given prefix, sorted alphabetically.
    fn list(&self, prefix: &str) -> Result<Vec<String>>;
}

// ── MemoryStoreBackend ─────────────────────────────────────────────────────────

/// In-memory implementation of `StoreBackend` backed by `HashMap` + `RwLock`.
///
/// Thread-safe via `Arc<RwLock<HashMap>>`.  Suitable for testing and
/// single-node deployments where persistence is not required.
pub struct MemoryStoreBackend {
    data: Arc<RwLock<HashMap<String, Vec<u8>>>>,
}

impl MemoryStoreBackend {
    pub fn new() -> Self {
        Self {
            data: Arc::new(RwLock::new(HashMap::new())),
        }
    }
}

impl Default for MemoryStoreBackend {
    fn default() -> Self {
        Self::new()
    }
}

impl StoreBackend for MemoryStoreBackend {
    fn get(&self, key: &str) -> Result<Option<Vec<u8>>> {
        let map = self
            .data
            .read()
            .map_err(|e| anyhow::anyhow!("RwLock poisoned: {e}"))?;
        Ok(map.get(key).cloned())
    }

    fn set(&self, key: &str, value: Vec<u8>) -> Result<()> {
        let mut map = self
            .data
            .write()
            .map_err(|e| anyhow::anyhow!("RwLock poisoned: {e}"))?;
        map.insert(key.to_string(), value);
        Ok(())
    }

    fn delete(&self, key: &str) -> Result<()> {
        let mut map = self
            .data
            .write()
            .map_err(|e| anyhow::anyhow!("RwLock poisoned: {e}"))?;
        map.remove(key);
        Ok(())
    }

    fn list(&self, prefix: &str) -> Result<Vec<String>> {
        let map = self
            .data
            .read()
            .map_err(|e| anyhow::anyhow!("RwLock poisoned: {e}"))?;
        let mut keys: Vec<String> = map
            .keys()
            .filter(|k| k.starts_with(prefix))
            .cloned()
            .collect();
        keys.sort();
        Ok(keys)
    }
}

// ── StoreNatsAdapter ───────────────────────────────────────────────────────────

/// Expose a `StoreBackend` via NATS request-reply subjects.
///
/// Subscribe to `store.>` and dispatch each incoming message to the appropriate
/// backend operation based on the subject convention:
///
/// | Subject                | Action                         | Reply payload     |
/// |------------------------|--------------------------------|-------------------|
/// | `store.get.<key>`      | `StoreBackend::get(key)`       | value bytes       |
/// | `store.set.<key>`      | `StoreBackend::set(key, body)` | `"ok"`            |
/// | `store.del.<key>`      | `StoreBackend::delete(key)`    | `"ok"`            |
/// | `store.list.<prefix>`  | `StoreBackend::list(prefix)`   | JSON string array |
pub struct StoreNatsAdapter {
    store: Arc<dyn StoreBackend>,
    nc: Arc<dyn NatsClientOp>,
}

impl StoreNatsAdapter {
    /// Create a new adapter backed by the given `store` and a real NATS client.
    pub fn new(store: Arc<dyn StoreBackend>, nc: async_nats::Client) -> Self {
        Self {
            store,
            nc: Arc::new(RealNatsClient(nc)),
        }
    }

    /// Create an adapter with a mock NATS client (test-only).
    #[cfg(test)]
    pub fn with_mock(store: Arc<dyn StoreBackend>, mock: impl NatsClientOp + 'static) -> Self {
        Self {
            store,
            nc: Arc::new(mock),
        }
    }

    /// Subscribe to `store.>` and handle incoming request-reply messages forever.
    ///
    /// Each message is dispatched to `handle_request`, and the response is
    /// published on the NATS reply subject (or a generated inbox if missing).
    pub async fn serve(&self) -> Result<()> {
        let mut subscription = self
            .nc
            .subscribe("store.>")
            .await
            .context("Failed to subscribe to store.>")?;

        while let Some((subject, payload, reply_to)) = subscription.next().await {
            // Use the NATS reply subject, or fall back to a generated inbox.
            let reply = reply_to.unwrap_or_else(|| format!("_inbox.{}", uuid::Uuid::new_v4()));

            let response = match self.handle_request(&subject, &payload).await {
                Ok(data) => data,
                Err(e) => format!("error:{e}").into_bytes(),
            };

            self.nc
                .publish(&reply, response)
                .await
                .context("Failed to publish reply")?;
        }

        Ok(())
    }

    /// Handle a single store request.
    ///
    /// Parses `subject` into verb + key, dispatches to the backend, and returns
    /// the response bytes.  This is a pure async function — no NATS I/O — making
    /// it directly testable.
    pub async fn handle_request(&self, subject: &str, payload: &[u8]) -> Result<Vec<u8>> {
        handle_store_request(&*self.store, subject, payload)
    }
}

// ── Pure request handler ───────────────────────────────────────────────────────

/// Core dispatch logic: parse subject → call backend → return response bytes.
///
/// Extracted from `StoreNatsAdapter::handle_request` so it can be unit-tested
/// without any NATS dependency.
fn handle_store_request(
    store: &dyn StoreBackend,
    subject: &str,
    payload: &[u8],
) -> Result<Vec<u8>> {
    let parts: Vec<&str> = subject.splitn(3, '.').collect();
    if parts.len() < 3 || parts[0] != "store" {
        anyhow::bail!("Unknown store subject: {subject}");
    }

    let verb = parts[1];
    let key_or_prefix = parts[2];

    match verb {
        "get" => {
            let value = store.get(key_or_prefix).context("Store get failed")?;
            Ok(value.unwrap_or_default())
        }
        "set" => {
            store
                .set(key_or_prefix, payload.to_vec())
                .context("Store set failed")?;
            Ok(b"ok".to_vec())
        }
        "del" => {
            store.delete(key_or_prefix).context("Store delete failed")?;
            Ok(b"ok".to_vec())
        }
        "list" => {
            let keys = store.list(key_or_prefix).context("Store list failed")?;
            let json = serde_json::to_string(&keys).context("Failed to serialize key list")?;
            Ok(json.into_bytes())
        }
        _ => anyhow::bail!("Unknown store verb: {verb}"),
    }
}

// ── Tests ──────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;
    use std::time::Duration;
    use tokio::sync::broadcast;

    // ── MockNatsClient (channel-based, no NATS server needed) ──────────────

    /// In-memory mock NATS client backed by `tokio::sync::broadcast`.
    ///
    /// - `subscribe()` returns a receiver that gets all messages sent via
    ///   `broadcast::Sender`.
    /// - `publish()` appends to `published` for assertions.
    struct MockNatsClient {
        msg_tx: broadcast::Sender<(String, Vec<u8>, Option<String>)>,
        published: Arc<Mutex<Vec<(String, Vec<u8>)>>>,
    }

    impl MockNatsClient {
        fn new() -> Self {
            let (tx, _) = broadcast::channel(256);
            Self {
                msg_tx: tx,
                published: Arc::new(Mutex::new(Vec::new())),
            }
        }

        /// Clone the sender to inject messages from outside the adapter.
        fn inject_tx(&self) -> broadcast::Sender<(String, Vec<u8>, Option<String>)> {
            self.msg_tx.clone()
        }

        /// Drain published messages for assertions.
        fn take_published(&self) -> Vec<(String, Vec<u8>)> {
            std::mem::take(&mut *self.published.lock().unwrap())
        }
    }

    #[async_trait]
    impl NatsClientOp for MockNatsClient {
        async fn subscribe(&self, _subject: &str) -> Result<Box<dyn NatsSubscriptionOp>> {
            Ok(Box::new(MockNatsSubscription(self.msg_tx.subscribe())))
        }

        async fn publish(&self, subject: &str, payload: Vec<u8>) -> Result<()> {
            self.published
                .lock()
                .unwrap()
                .push((subject.to_string(), payload));
            Ok(())
        }
    }

    struct MockNatsSubscription(broadcast::Receiver<(String, Vec<u8>, Option<String>)>);

    #[async_trait]
    impl NatsSubscriptionOp for MockNatsSubscription {
        async fn next(&mut self) -> Option<(String, Vec<u8>, Option<String>)> {
            self.0.recv().await.ok()
        }
    }

    // ── Helper ─────────────────────────────────────────────────────────────

    /// Short sleep to let async tasks process messages.
    async fn tick() {
        tokio::time::sleep(Duration::from_millis(50)).await;
    }

    fn make_store() -> Arc<dyn StoreBackend> {
        Arc::new(MemoryStoreBackend::new()) as Arc<dyn StoreBackend>
    }

    // ── MemoryStoreBackend tests ───────────────────────────────────────────

    #[test]
    fn memory_set_get_round_trip() {
        let store = MemoryStoreBackend::new();
        store.set("key1", b"value1".to_vec()).unwrap();
        assert_eq!(store.get("key1").unwrap(), Some(b"value1".to_vec()));
    }

    #[test]
    fn memory_get_missing_returns_none() {
        let store = MemoryStoreBackend::new();
        assert_eq!(store.get("nonexistent").unwrap(), None);
    }

    #[test]
    fn memory_delete_removes_key() {
        let store = MemoryStoreBackend::new();
        store.set("key1", b"v".to_vec()).unwrap();
        store.delete("key1").unwrap();
        assert_eq!(store.get("key1").unwrap(), None);
    }

    #[test]
    fn memory_delete_nonexistent_is_noop() {
        let store = MemoryStoreBackend::new();
        store.delete("nonexistent").unwrap(); // should not error
    }

    #[test]
    fn memory_list_prefix() {
        let store = MemoryStoreBackend::new();
        store.set("a.one", b"1".to_vec()).unwrap();
        store.set("a.two", b"2".to_vec()).unwrap();
        store.set("b.one", b"3".to_vec()).unwrap();

        let keys = store.list("a.").unwrap();
        assert_eq!(keys, vec!["a.one", "a.two"]);
    }

    #[test]
    fn memory_list_empty_prefix_returns_all() {
        let store = MemoryStoreBackend::new();
        store.set("x", b"1".to_vec()).unwrap();
        store.set("y", b"2".to_vec()).unwrap();
        assert_eq!(store.list("").unwrap(), vec!["x", "y"]);
    }

    #[test]
    fn memory_list_no_match() {
        let store = MemoryStoreBackend::new();
        store.set("a", b"1".to_vec()).unwrap();
        assert!(store.list("b.").unwrap().is_empty());
    }

    #[test]
    fn memory_overwrite() {
        let store = MemoryStoreBackend::new();
        store.set("k", b"old".to_vec()).unwrap();
        store.set("k", b"new".to_vec()).unwrap();
        assert_eq!(store.get("k").unwrap(), Some(b"new".to_vec()));
    }

    #[test]
    fn memory_concurrent_access() {
        let store = Arc::new(MemoryStoreBackend::new());
        let mut handles = vec![];

        for i in 0..20 {
            let s = store.clone();
            handles.push(std::thread::spawn(move || {
                s.set(&format!("key{i}"), format!("val{i}").into_bytes())
                    .unwrap();
                let _ = s.get(&format!("key{i}"));
            }));
        }

        for h in handles {
            h.join().expect("thread panicked");
        }

        assert_eq!(store.list("").unwrap().len(), 20);
    }

    // ── handle_request unit tests (no NATS) ────────────────────────────────

    #[tokio::test]
    async fn handle_set_then_get() {
        let store = make_store();
        let adapter = StoreNatsAdapter::with_mock(store, MockNatsClient::new());

        let resp = adapter
            .handle_request("store.set.mykey", b"hello world")
            .await
            .unwrap();
        assert_eq!(resp, b"ok");

        let resp = adapter
            .handle_request("store.get.mykey", b"")
            .await
            .unwrap();
        assert_eq!(resp, b"hello world");
    }

    #[tokio::test]
    async fn handle_get_missing_key_returns_empty() {
        let store = make_store();
        let adapter = StoreNatsAdapter::with_mock(store, MockNatsClient::new());

        let resp = adapter
            .handle_request("store.get.missing", b"")
            .await
            .unwrap();
        assert!(resp.is_empty());
    }

    #[tokio::test]
    async fn handle_delete_removes_value() {
        let store = make_store();
        let adapter = StoreNatsAdapter::with_mock(store, MockNatsClient::new());

        adapter
            .handle_request("store.set.tmp", b"data")
            .await
            .unwrap();
        adapter.handle_request("store.del.tmp", b"").await.unwrap();

        let resp = adapter.handle_request("store.get.tmp", b"").await.unwrap();
        assert!(resp.is_empty());
    }

    #[tokio::test]
    async fn handle_list_with_prefix() {
        let store = make_store();
        let adapter = StoreNatsAdapter::with_mock(store, MockNatsClient::new());

        adapter
            .handle_request("store.set.a.one", b"1")
            .await
            .unwrap();
        adapter
            .handle_request("store.set.a.two", b"2")
            .await
            .unwrap();
        adapter
            .handle_request("store.set.b.one", b"3")
            .await
            .unwrap();

        let resp = adapter.handle_request("store.list.a.", b"").await.unwrap();
        let keys: Vec<String> = serde_json::from_slice(&resp).unwrap();
        assert_eq!(keys, vec!["a.one", "a.two"]);
    }

    #[tokio::test]
    async fn handle_list_empty_prefix() {
        let store = make_store();
        let adapter = StoreNatsAdapter::with_mock(store, MockNatsClient::new());

        adapter.handle_request("store.set.x", b"1").await.unwrap();
        adapter.handle_request("store.set.y", b"2").await.unwrap();

        let resp = adapter.handle_request("store.list.", b"").await.unwrap();
        let keys: Vec<String> = serde_json::from_slice(&resp).unwrap();
        assert_eq!(keys, vec!["x", "y"]);
    }

    #[tokio::test]
    async fn handle_unknown_subject_returns_error() {
        let store = make_store();
        let adapter = StoreNatsAdapter::with_mock(store, MockNatsClient::new());

        let result = adapter.handle_request("unknown.subject", b"").await;
        assert!(result.is_err());
        assert!(
            format!("{:?}", result).contains("Unknown store subject"),
            "error should mention unknown subject"
        );
    }

    #[tokio::test]
    async fn handle_unknown_verb_returns_error() {
        let store = make_store();
        let adapter = StoreNatsAdapter::with_mock(store, MockNatsClient::new());

        let result = adapter.handle_request("store.unknown.key", b"").await;
        assert!(result.is_err());
        assert!(
            format!("{:?}", result).contains("Unknown store verb"),
            "error should mention unknown verb"
        );
    }

    #[tokio::test]
    async fn handle_empty_key_set_get_del() {
        let store = make_store();
        let adapter = StoreNatsAdapter::with_mock(store, MockNatsClient::new());

        // set with empty key
        let resp = adapter
            .handle_request("store.set.", b"empty-key-data")
            .await
            .unwrap();
        assert_eq!(resp, b"ok");

        // get with empty key
        let resp = adapter.handle_request("store.get.", b"").await.unwrap();
        assert_eq!(resp, b"empty-key-data");

        // del with empty key
        let resp = adapter.handle_request("store.del.", b"").await.unwrap();
        assert_eq!(resp, b"ok");

        // verify gone
        let resp = adapter.handle_request("store.get.", b"").await.unwrap();
        assert!(resp.is_empty());
    }

    #[tokio::test]
    async fn handle_key_with_dots_preserved() {
        let store = make_store();
        let adapter = StoreNatsAdapter::with_mock(store, MockNatsClient::new());

        adapter
            .handle_request("store.set.nested.key.path", b"deep")
            .await
            .unwrap();
        let resp = adapter
            .handle_request("store.get.nested.key.path", b"")
            .await
            .unwrap();
        assert_eq!(resp, b"deep");
    }

    // ── serve() round-trip via mock NATS ───────────────────────────────────

    #[tokio::test]
    async fn serve_set_and_verify_store_state() {
        // Test that serve() actually writes to the store by checking through
        // the shared Arc<dyn StoreBackend>.
        let store_ref: Arc<MemoryStoreBackend> = Arc::new(MemoryStoreBackend::new());
        let store: Arc<dyn StoreBackend> = store_ref.clone();

        let mock = MockNatsClient::new();
        let inject = mock.inject_tx();
        let published = mock.published.clone();
        let adapter = StoreNatsAdapter::with_mock(store, mock);

        let handle = tokio::spawn(async move {
            let _ = adapter.serve().await;
        });

        tick().await;

        // Inject a SET request via NATS
        inject
            .send((
                "store.set.mykey".to_string(),
                b"hello-world".to_vec(),
                Some("reply.1".to_string()),
            ))
            .unwrap();
        tick().await;

        // Directly check the store
        let val = store_ref.get("mykey").unwrap();
        assert_eq!(val, Some(b"hello-world".to_vec()));

        // Check the reply was published
        let msgs = published.lock().unwrap();
        assert!(msgs.iter().any(|(s, p)| s == "reply.1" && p == b"ok"));

        handle.abort();
    }

    #[tokio::test]
    async fn serve_list_returns_json() {
        let store_ref: Arc<MemoryStoreBackend> = Arc::new(MemoryStoreBackend::new());
        store_ref.set("a.x", b"1".to_vec()).unwrap();
        store_ref.set("a.y", b"2".to_vec()).unwrap();
        let store: Arc<dyn StoreBackend> = store_ref;

        let mock = MockNatsClient::new();
        let inject = mock.inject_tx();
        let published = mock.published.clone();
        let adapter = StoreNatsAdapter::with_mock(store, mock);

        let handle = tokio::spawn(async move {
            let _ = adapter.serve().await;
        });

        tick().await;

        inject
            .send((
                "store.list.a.".to_string(),
                b"".to_vec(),
                Some("reply.list".to_string()),
            ))
            .unwrap();
        tick().await;

        let msgs = published.lock().unwrap();
        let list_msg = msgs
            .iter()
            .find(|(s, _)| s == "reply.list")
            .expect("should have reply on reply.list");
        let keys: Vec<String> = serde_json::from_slice(&list_msg.1).unwrap();
        assert_eq!(keys, vec!["a.x", "a.y"]);

        handle.abort();
    }

    #[tokio::test]
    async fn serve_unknown_subject_returns_error_reply() {
        let store = make_store();
        let mock = MockNatsClient::new();
        let inject = mock.inject_tx();
        let published = mock.published.clone();
        let adapter = StoreNatsAdapter::with_mock(store, mock);

        let handle = tokio::spawn(async move {
            let _ = adapter.serve().await;
        });

        tick().await;

        inject
            .send((
                "unknown.subject".to_string(),
                b"".to_vec(),
                Some("reply.err".to_string()),
            ))
            .unwrap();
        tick().await;

        let msgs = published.lock().unwrap();
        let err_msg = msgs
            .iter()
            .find(|(s, _)| s == "reply.err")
            .expect("should have reply on reply.err");
        assert!(
            std::str::from_utf8(&err_msg.1)
                .unwrap_or("")
                .starts_with("error:"),
            "reply should start with 'error:'"
        );

        handle.abort();
    }

    #[tokio::test]
    async fn serve_delete_then_get() {
        let store_ref: Arc<MemoryStoreBackend> = Arc::new(MemoryStoreBackend::new());
        store_ref.set("tmp", b"to-delete".to_vec()).unwrap();
        let store: Arc<dyn StoreBackend> = store_ref.clone();

        let mock = MockNatsClient::new();
        let inject = mock.inject_tx();
        let published = mock.published.clone();
        let adapter = StoreNatsAdapter::with_mock(store, mock);

        let handle = tokio::spawn(async move {
            let _ = adapter.serve().await;
        });

        tick().await;

        // Delete via NATS
        inject
            .send((
                "store.del.tmp".to_string(),
                b"".to_vec(),
                Some("reply.del".to_string()),
            ))
            .unwrap();
        tick().await;

        assert!(store_ref.get("tmp").unwrap().is_none());

        let msgs = published.lock().unwrap();
        assert!(msgs.iter().any(|(s, p)| s == "reply.del" && p == b"ok"));

        handle.abort();
    }

    #[tokio::test]
    async fn serve_concurrent_requests() {
        let store_ref: Arc<MemoryStoreBackend> = Arc::new(MemoryStoreBackend::new());
        let store: Arc<dyn StoreBackend> = store_ref.clone();

        let mock = MockNatsClient::new();
        let inject = mock.inject_tx();
        let adapter = StoreNatsAdapter::with_mock(store, mock);

        let handle = tokio::spawn(async move {
            let _ = adapter.serve().await;
        });

        tick().await;

        // Send 10 concurrent SET requests
        for i in 0..10 {
            let tx = inject.clone();
            tokio::spawn(async move {
                tx.send((
                    format!("store.set.key{i}"),
                    format!("val{i}").into_bytes(),
                    Some(format!("reply.{i}")),
                ))
                .ok();
            });
        }
        tick().await;

        // Verify all keys were written
        let keys = store_ref.list("").unwrap();
        assert_eq!(keys.len(), 10);

        handle.abort();
    }
}
