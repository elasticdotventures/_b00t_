/// Hive registry — manages discovery and tracking of remote hives.
///
/// A hive is a collection of agents accessible via an A2A HTTP endpoint.
/// The `HiveRegistry` tracks known hives, discovers remote agents via
/// well-known URLs, and provides cross-hive agent lookup by skill.
use crate::agent_card::AgentCard;
use crate::http_transport::A2aHttpTransport;
use std::collections::HashMap;
use std::time::{Duration, Instant};
use url::Url;

/// A remote hive discovered via A2A HTTP.
#[derive(Debug, Clone)]
pub struct RemoteHive {
    /// Unique identifier for this hive
    pub id: String,
    /// Base URL of the hive's A2A endpoint
    pub url: Url,
    /// Agent cards discovered from this hive
    pub agent_cards: Vec<AgentCard>,
    /// When this hive was last seen/alive
    pub last_seen: Instant,
}

/// Registry of known hives (local + remote) for cross-hive agent discovery.
#[derive(Debug, Clone)]
pub struct HiveRegistry {
    /// The local agent's own card
    local_card: AgentCard,
    /// Remote hives indexed by ID
    remote_hives: HashMap<String, RemoteHive>,
}

impl HiveRegistry {
    /// Create a new `HiveRegistry` with the local agent card.
    pub fn new(local_card: AgentCard) -> Self {
        Self {
            local_card,
            remote_hives: HashMap::new(),
        }
    }

    /// Add or update a remote hive with its agent cards.
    pub fn add_remote(&mut self, id: String, url: Url, cards: Vec<AgentCard>) {
        self.remote_hives.insert(
            id.clone(),
            RemoteHive {
                id,
                url,
                agent_cards: cards,
                last_seen: Instant::now(),
            },
        );
    }

    /// Discover a remote hive by its well-known URL.
    ///
    /// Fetches agent cards from `{url}/.well-known/agent-cards` and registers
    /// the hive.
    pub async fn discover_remote(&mut self, url: &Url) -> Result<(), Box<dyn std::error::Error>> {
        let cards = A2aHttpTransport::discover_remote(url).await?;
        let id = format!(
            "{}:{}",
            url.host_str().unwrap_or("unknown"),
            url.port().unwrap_or(80)
        );
        self.add_remote(id, url.clone(), cards);
        Ok(())
    }

    /// Get all agents across all hives (local + remote) matching a skill name.
    ///
    /// Returns a list of `(hive_id, agent_card)` tuples.
    pub fn find_agents_by_skill(&self, skill_name: &str) -> Vec<(String, AgentCard)> {
        let mut results = Vec::new();

        // Check local card
        if self
            .local_card
            .skills
            .iter()
            .any(|s| s.id == skill_name || s.name == skill_name)
        {
            results.push(("local".to_string(), self.local_card.clone()));
        }

        // Check remote hives
        for (id, hive) in &self.remote_hives {
            for card in &hive.agent_cards {
                if card
                    .skills
                    .iter()
                    .any(|s| s.id == skill_name || s.name == skill_name)
                {
                    results.push((id.clone(), card.clone()));
                }
            }
        }

        results
    }

    /// Get the local agent card.
    pub fn local_card(&self) -> &AgentCard {
        &self.local_card
    }

    /// Get a mutable reference to the local card.
    pub fn local_card_mut(&mut self) -> &mut AgentCard {
        &mut self.local_card
    }

    /// Get all remote hives.
    pub fn remote_hives(&self) -> &HashMap<String, RemoteHive> {
        &self.remote_hives
    }

    /// Number of known hives (including local).
    pub fn hive_count(&self) -> usize {
        1 + self.remote_hives.len() // +1 for local
    }

    /// Number of remote hives.
    pub fn remote_count(&self) -> usize {
        self.remote_hives.len()
    }

    /// Remove stale hives that haven't been seen in longer than `max_age`.
    ///
    /// Returns the number of hives pruned.
    pub fn prune_stale(&mut self, max_age: Duration) -> usize {
        let now = Instant::now();
        let before = self.remote_hives.len();
        self.remote_hives
            .retain(|_, hive| now.duration_since(hive.last_seen) < max_age);
        before - self.remote_hives.len()
    }

    /// Get all agents across all hives (including local).
    pub fn all_agents(&self) -> Vec<(String, AgentCard)> {
        let mut agents = Vec::new();
        agents.push(("local".to_string(), self.local_card.clone()));
        for (id, hive) in &self.remote_hives {
            for card in &hive.agent_cards {
                agents.push((id.clone(), card.clone()));
            }
        }
        agents
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agent_card::Skill;
    use std::time::Duration;

    fn sample_card(name: &str) -> AgentCard {
        AgentCard::new(
            name,
            &format!("Agent {}", name),
            Url::parse("http://localhost:9999").unwrap(),
        )
        .with_skill(Skill::new(
            "ping",
            "Ping",
            "Ping test",
            serde_json::json!({}),
            serde_json::json!({}),
        ))
    }

    #[test]
    fn test_new_registry() {
        let card = sample_card("local-agent");
        let registry = HiveRegistry::new(card.clone());
        assert_eq!(registry.hive_count(), 1);
        assert_eq!(registry.local_card().name, "local-agent");
        assert_eq!(registry.remote_count(), 0);
    }

    #[test]
    fn test_add_remote() {
        let local = sample_card("local");
        let mut registry = HiveRegistry::new(local);

        let remote_card = sample_card("remote-1");
        let url = Url::parse("http://remote-hive:8080").unwrap();
        registry.add_remote("hive-1".to_string(), url.clone(), vec![remote_card.clone()]);

        assert_eq!(registry.hive_count(), 2);
        assert_eq!(registry.remote_count(), 1);

        let agents = registry.all_agents();
        assert_eq!(agents.len(), 2); // local + 1 remote
    }

    #[test]
    fn test_find_agents_by_skill() {
        let local = AgentCard::new(
            "local",
            "Local agent",
            Url::parse("http://localhost:1").unwrap(),
        )
        .with_skill(Skill::new(
            "code-gen",
            "Code Generator",
            "Generates code",
            serde_json::json!({}),
            serde_json::json!({}),
        ));

        let mut registry = HiveRegistry::new(local);

        let remote_card = AgentCard::new(
            "remote-bot",
            "Remote bot",
            Url::parse("http://remote:8080").unwrap(),
        )
        .with_skill(Skill::new(
            "translate",
            "Translator",
            "Translates text",
            serde_json::json!({}),
            serde_json::json!({}),
        ));

        registry.add_remote(
            "hive-b".to_string(),
            Url::parse("http://hive-b:8080").unwrap(),
            vec![remote_card],
        );

        // Find by local skill ID
        let results = registry.find_agents_by_skill("code-gen");
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].0, "local");

        // Find by remote skill ID
        let results = registry.find_agents_by_skill("translate");
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].0, "hive-b");

        // Find nonexistent
        let results = registry.find_agents_by_skill("nonexistent");
        assert!(results.is_empty());
    }

    #[test]
    fn test_prune_stale() {
        let local = sample_card("local");
        let mut registry = HiveRegistry::new(local);

        registry.add_remote(
            "fresh".to_string(),
            Url::parse("http://fresh:8080").unwrap(),
            vec![sample_card("fresh-agent")],
        );

        // Add a stale hive by setting a very old last_seen
        registry.add_remote(
            "stale".to_string(),
            Url::parse("http://stale:8080").unwrap(),
            vec![sample_card("stale-agent")],
        );

        // Manually set the stale hive's last_seen to the past
        if let Some(hive) = registry.remote_hives.get_mut("stale") {
            hive.last_seen = Instant::now() - Duration::from_secs(7200); // 2 hours ago
        }

        assert_eq!(registry.remote_count(), 2);

        // Prune with 1-hour max age
        let pruned = registry.prune_stale(Duration::from_secs(3600));
        assert_eq!(pruned, 1);
        assert_eq!(registry.remote_count(), 1);
        assert!(registry.remote_hives.contains_key("fresh"));
        assert!(!registry.remote_hives.contains_key("stale"));
    }

    #[test]
    fn test_hive_count_after_prune() {
        let local = sample_card("local");
        let mut registry = HiveRegistry::new(local);

        registry.add_remote(
            "hive-a".to_string(),
            Url::parse("http://a:8080").unwrap(),
            vec![sample_card("a")],
        );

        registry.add_remote(
            "hive-b".to_string(),
            Url::parse("http://b:8080").unwrap(),
            vec![sample_card("b")],
        );

        assert_eq!(registry.hive_count(), 3); // local + 2 remote

        // Mark hive-a as stale
        if let Some(hive) = registry.remote_hives.get_mut("hive-a") {
            hive.last_seen = Instant::now() - Duration::from_secs(7200);
        }

        registry.prune_stale(Duration::from_secs(3600));
        assert_eq!(registry.hive_count(), 2); // local + 1 remote
    }
}
