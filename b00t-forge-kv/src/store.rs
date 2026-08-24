//! In-process key-value store backing the RESP2 server. Deliberately no
//! external database — the whole point of ForgeKV is to be a b00t-native
//! drop-in for `RedisComms` without an extra process/dependency to install
//! and operate on a hive node.

use std::collections::HashMap;
use std::time::{Duration, Instant};
use tokio::sync::{broadcast, RwLock};

struct Entry {
    value: Vec<u8>,
    expires_at: Option<Instant>,
}

impl Entry {
    fn is_expired(&self) -> bool {
        self.expires_at.is_some_and(|t| Instant::now() >= t)
    }
}

#[derive(Default)]
pub struct Store {
    strings: RwLock<HashMap<String, Entry>>,
    hashes: RwLock<HashMap<String, HashMap<String, String>>>,
    /// One broadcast channel per pub/sub channel name. Created lazily on
    /// first PUBLISH or SUBSCRIBE, never removed (channel names are a small,
    /// known set in practice — hive coordination, not user-generated).
    channels: RwLock<HashMap<String, broadcast::Sender<Vec<u8>>>>,
}

impl Store {
    pub fn new() -> Self {
        Self::default()
    }

    // ── strings ──────────────────────────────────────────────────────────

    pub async fn set(&self, key: &str, value: Vec<u8>, ttl: Option<Duration>) {
        let expires_at = ttl.map(|d| Instant::now() + d);
        self.strings
            .write()
            .await
            .insert(key.to_string(), Entry { value, expires_at });
    }

    pub async fn get(&self, key: &str) -> Option<Vec<u8>> {
        let mut strings = self.strings.write().await;
        match strings.get(key) {
            Some(e) if e.is_expired() => {
                strings.remove(key);
                None
            }
            Some(e) => Some(e.value.clone()),
            None => None,
        }
    }

    pub async fn del(&self, keys: &[String]) -> i64 {
        let mut strings = self.strings.write().await;
        let mut hashes = self.hashes.write().await;
        let mut count = 0i64;
        for key in keys {
            if strings.remove(key).is_some() {
                count += 1;
            }
            if hashes.remove(key).is_some() {
                count += 1;
            }
        }
        count
    }

    pub async fn exists(&self, keys: &[String]) -> i64 {
        let mut strings = self.strings.write().await;
        let hashes = self.hashes.read().await;
        let mut count = 0i64;
        for key in keys {
            let string_hit = match strings.get(key.as_str()) {
                Some(e) if e.is_expired() => {
                    strings.remove(key.as_str());
                    false
                }
                Some(_) => true,
                None => false,
            };
            if string_hit || hashes.contains_key(key.as_str()) {
                count += 1;
            }
        }
        count
    }

    pub async fn expire(&self, key: &str, ttl: Duration) -> bool {
        let mut strings = self.strings.write().await;
        match strings.get_mut(key) {
            Some(e) if !e.is_expired() => {
                e.expires_at = Some(Instant::now() + ttl);
                true
            }
            _ => false,
        }
    }

    /// INCRBY semantics: missing key treated as 0, value must parse as i64.
    pub async fn incrby(&self, key: &str, delta: i64) -> Result<i64, &'static str> {
        let mut strings = self.strings.write().await;
        let current = match strings.get(key) {
            Some(e) if e.is_expired() => 0,
            Some(e) => std::str::from_utf8(&e.value)
                .ok()
                .and_then(|s| s.parse::<i64>().ok())
                .ok_or("value is not an integer or out of range")?,
            None => 0,
        };
        let next = current
            .checked_add(delta)
            .ok_or("increment or decrement would overflow")?;
        strings.insert(
            key.to_string(),
            Entry {
                value: next.to_string().into_bytes(),
                expires_at: None,
            },
        );
        Ok(next)
    }

    // ── hashes ───────────────────────────────────────────────────────────

    /// Returns true if `field` is new (matches Redis HSET's "1 if new" reply).
    pub async fn hset(&self, key: &str, field: &str, value: &str) -> bool {
        let mut hashes = self.hashes.write().await;
        let map = hashes.entry(key.to_string()).or_default();
        map.insert(field.to_string(), value.to_string()).is_none()
    }

    pub async fn hget(&self, key: &str, field: &str) -> Option<String> {
        self.hashes
            .read()
            .await
            .get(key)
            .and_then(|m| m.get(field))
            .cloned()
    }

    pub async fn hgetall(&self, key: &str) -> Vec<(String, String)> {
        self.hashes
            .read()
            .await
            .get(key)
            .map(|m| m.iter().map(|(k, v)| (k.clone(), v.clone())).collect())
            .unwrap_or_default()
    }

    // ── pub/sub ──────────────────────────────────────────────────────────

    /// Publish to `channel`, returning the number of current subscribers
    /// (matches Redis PUBLISH's reply). A channel with zero subscribers has
    /// no sender yet — that's correctly 0, not an error.
    pub async fn publish(&self, channel: &str, message: Vec<u8>) -> i64 {
        let channels = self.channels.read().await;
        match channels.get(channel) {
            Some(tx) => tx.send(message).map(|n| n as i64).unwrap_or(0),
            None => 0,
        }
    }

    pub async fn subscribe(&self, channel: &str) -> broadcast::Receiver<Vec<u8>> {
        let mut channels = self.channels.write().await;
        channels
            .entry(channel.to_string())
            .or_insert_with(|| broadcast::channel(256).0)
            .subscribe()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn set_get_roundtrip() {
        let store = Store::new();
        store.set("k", b"v".to_vec(), None).await;
        assert_eq!(store.get("k").await, Some(b"v".to_vec()));
    }

    #[tokio::test]
    async fn expired_key_reads_as_missing() {
        let store = Store::new();
        store.set("k", b"v".to_vec(), Some(Duration::from_millis(1))).await;
        tokio::time::sleep(Duration::from_millis(20)).await;
        assert_eq!(store.get("k").await, None);
    }

    #[tokio::test]
    async fn del_counts_strings_and_hashes() {
        let store = Store::new();
        store.set("s", b"v".to_vec(), None).await;
        store.hset("h", "f", "v").await;
        assert_eq!(store.del(&["s".into(), "h".into(), "missing".into()]).await, 2);
        assert_eq!(store.get("s").await, None);
    }

    #[tokio::test]
    async fn exists_treats_expired_as_absent() {
        let store = Store::new();
        store.set("k", b"v".to_vec(), Some(Duration::from_millis(1))).await;
        tokio::time::sleep(Duration::from_millis(20)).await;
        assert_eq!(store.exists(&["k".into()]).await, 0);
    }

    #[tokio::test]
    async fn incrby_treats_missing_key_as_zero_and_persists() {
        let store = Store::new();
        assert_eq!(store.incrby("counter", 5).await, Ok(5));
        assert_eq!(store.incrby("counter", -2).await, Ok(3));
    }

    #[tokio::test]
    async fn incrby_rejects_non_integer_value() {
        let store = Store::new();
        store.set("k", b"not-a-number".to_vec(), None).await;
        assert!(store.incrby("k", 1).await.is_err());
    }

    #[tokio::test]
    async fn hset_reports_new_vs_existing_field() {
        let store = Store::new();
        assert!(store.hset("h", "f", "v1").await);
        assert!(!store.hset("h", "f", "v2").await);
        assert_eq!(store.hget("h", "f").await, Some("v2".to_string()));
    }

    #[tokio::test]
    async fn hgetall_returns_all_fields() {
        let store = Store::new();
        store.hset("h", "a", "1").await;
        store.hset("h", "b", "2").await;
        let mut fields = store.hgetall("h").await;
        fields.sort();
        assert_eq!(fields, vec![("a".to_string(), "1".to_string()), ("b".to_string(), "2".to_string())]);
    }

    #[tokio::test]
    async fn publish_before_any_subscriber_returns_zero() {
        let store = Store::new();
        assert_eq!(store.publish("ch", b"hi".to_vec()).await, 0);
    }

    #[tokio::test]
    async fn subscriber_receives_published_message() {
        let store = Store::new();
        let mut rx = store.subscribe("ch").await;
        assert_eq!(store.publish("ch", b"hello".to_vec()).await, 1);
        assert_eq!(rx.recv().await.unwrap(), b"hello".to_vec());
    }

    #[tokio::test]
    async fn expire_extends_ttl_on_existing_key_only() {
        let store = Store::new();
        assert!(!store.expire("missing", Duration::from_secs(10)).await);
        store.set("k", b"v".to_vec(), None).await;
        assert!(store.expire("k", Duration::from_millis(1)).await);
        tokio::time::sleep(Duration::from_millis(20)).await;
        assert_eq!(store.get("k").await, None);
    }
}
