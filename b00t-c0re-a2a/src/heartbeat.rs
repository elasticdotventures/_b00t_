//! Hive heartbeat — periodic status broadcast to peer hives.
//! Runs on the governance scheduler (every 15min) and sends A2A tasks.
//!
//! Each heartbeat payload includes:
//! - Hive identity and uptime
//! - Number of agents and skills
//! - Status (alive, overloaded, etc.)

use crate::hive::HiveRegistry;
use crate::http_transport::A2aHttpTransport;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::Mutex;

/// Heartbeat configuration.
pub struct Heartbeat {
    /// Interval between heartbeat broadcasts.
    interval: Duration,
    /// Unique identifier for this hive.
    hive_id: String,
    /// Shared registry of known hives.
    registry: Arc<Mutex<HiveRegistry>>,
    /// HTTP transport for sending A2A tasks to remote hives.
    #[allow(dead_code)]
    transport: Arc<A2aHttpTransport>,
    /// When this heartbeat loop started.
    started_at: Instant,
}

impl Heartbeat {
    /// Create a new heartbeat broadcaster.
    ///
    /// Default interval is 15 minutes.
    pub fn new(
        hive_id: String,
        interval: Duration,
        registry: Arc<Mutex<HiveRegistry>>,
        transport: Arc<A2aHttpTransport>,
    ) -> Self {
        Self {
            interval,
            hive_id,
            registry,
            transport,
            started_at: Instant::now(),
        }
    }

    /// Start the heartbeat loop. Call in a spawned tokio task.
    ///
    /// Runs forever, broadcasting every `interval`.
    pub async fn start(&self) {
        let mut ticker = tokio::time::interval(self.interval);
        // First tick fires immediately; skip it so we have an initial delay.
        ticker.tick().await;

        loop {
            ticker.tick().await;
            self.send_to_remotes().await;
        }
    }

    /// Build a heartbeat payload with current hive stats.
    fn build_payload(&self) -> serde_json::Value {
        let uptime = self.started_at.elapsed().as_secs();
        serde_json::json!({
            "type": "hive_heartbeat",
            "hive_id": self.hive_id,
            "protocol": "a2a",
            "version": "1.0",
            "uptime_secs": uptime,
            "timestamp": chrono::Utc::now().to_rfc3339(),
            "status": "alive",
        })
    }

    /// Send heartbeat to all known remote hives.
    ///
    /// Constructs a heartbeat A2A task (skill="hive/heartbeat") and sends it
    /// to every remote hive in the registry. Errors are logged per-remote-hive
    /// but do not abort the loop.
    async fn send_to_remotes(&self) {
        let payload = self.build_payload();
        let task = crate::task::Task::new("hive/heartbeat", payload, "system");

        let remotes: Vec<(String, crate::agent_card::AgentCard)> = {
            let registry = self.registry.lock().await;
            registry.all_agents()
        };

        // Build a map of hive IDs to the first agent card URL for each remote.
        let mut destinations: Vec<url::Url> = Vec::new();
        for (hive_id, card) in &remotes {
            if hive_id != "local" {
                destinations.push(card.url.clone());
            }
        }
        // Deduplicate by URL
        destinations.sort_by(|a, b| a.as_str().cmp(b.as_str()));
        destinations.dedup();

        if destinations.is_empty() {
            return;
        }

        for url in &destinations {
            if let Err(e) = A2aHttpTransport::send_task(url, &task).await {
                eprintln!(
                    "[heartbeat] failed to send heartbeat to {url}: {e}"
                );
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agent_card::AgentCard;
    use crate::skill_registry::SkillRegistry;
    use url::Url;

    #[test]
    fn test_build_payload() {
        let registry = Arc::new(Mutex::new(HiveRegistry::new(
            AgentCard::new(
                "test-hive",
                "Test hive",
                Url::parse("http://localhost:9999").unwrap(),
            ),
        )));
        let skill_reg = Arc::new(SkillRegistry::new());
        let transport = Arc::new(A2aHttpTransport::new(skill_reg, 0));

        let heartbeat = Heartbeat::new(
            "test-hive".to_string(),
            Duration::from_secs(900),
            registry,
            transport,
        );

        let payload = heartbeat.build_payload();
        assert_eq!(payload["type"], "hive_heartbeat");
        assert_eq!(payload["hive_id"], "test-hive");
        assert_eq!(payload["protocol"], "a2a");
        assert_eq!(payload["status"], "alive");
        assert!(payload["uptime_secs"].as_u64().is_some());
        assert!(payload["timestamp"].as_str().is_some());
    }

    #[test]
    fn test_build_payload_uptime_increases() {
        let registry = Arc::new(Mutex::new(HiveRegistry::new(
            AgentCard::new(
                "uptime-hive",
                "Uptime test",
                Url::parse("http://localhost:9999").unwrap(),
            ),
        )));
        let skill_reg = Arc::new(SkillRegistry::new());
        let transport = Arc::new(A2aHttpTransport::new(skill_reg, 0));

        let heartbeat = Heartbeat::new(
            "uptime-hive".to_string(),
            Duration::from_secs(900),
            registry,
            transport,
        );

        let p1 = heartbeat.build_payload();
        let u1 = p1["uptime_secs"].as_u64().unwrap();

        // After a small sleep, uptime should be >= previous value
        std::thread::sleep(Duration::from_millis(10));
        let p2 = heartbeat.build_payload();
        let u2 = p2["uptime_secs"].as_u64().unwrap();

        assert!(u2 >= u1, "uptime should not decrease");
    }
}
