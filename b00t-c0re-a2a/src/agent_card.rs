use serde::{Deserialize, Serialize};
use url::Url;

/// Reputation scoring for an agent.
/// Tracks across missions and affects recruitment ranking.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentReputation {
    /// Composite score from -10 to +10
    pub score: f64,
    /// Number of missions successfully completed
    pub missions_completed: u64,
    /// Number of missions abandoned
    pub missions_abandoned: u64,
    /// Number of times designated as ballast (bottleneck / liability)
    pub times_ballast: u64,
    /// Number of captain endorsements received
    pub captain_endorsements: u64,
}

impl AgentReputation {
    /// Create a new neutral reputation (score 0.0).
    pub fn new() -> Self {
        Self {
            score: 0.0,
            missions_completed: 0,
            missions_abandoned: 0,
            times_ballast: 0,
            captain_endorsements: 0,
        }
    }

    /// Record a successful mission completion (+1.0, max 10.0).
    pub fn record_completion(&mut self) {
        self.missions_completed += 1;
        self.score = (self.score + 1.0).clamp(-10.0, 10.0);
    }

    /// Record an abandoned mission (-2.0, min -10.0).
    pub fn record_abandon(&mut self) {
        self.missions_abandoned += 1;
        self.score = (self.score - 2.0).clamp(-10.0, 10.0);
    }

    /// Record ballast designation (-3.0, min -10.0).
    pub fn record_ballast(&mut self) {
        self.times_ballast += 1;
        self.score = (self.score - 3.0).clamp(-10.0, 10.0);
    }

    /// Receive a captain endorsement (+1.5, max 10.0).
    pub fn endorse(&mut self) {
        self.captain_endorsements += 1;
        self.score = (self.score + 1.5).clamp(-10.0, 10.0);
    }

    /// Calculate effective reputation weight (0.0–1.0) for recruitment scoring.
    /// Maps score [-10, 10] linearly to [0.0, 1.0].
    pub fn recruitment_weight(&self) -> f64 {
        (self.score + 10.0) / 20.0
    }
}

impl Default for AgentReputation {
    fn default() -> Self {
        Self::new()
    }
}

/// An A2A Agent Card — the discovery document for agent-to-agent communication.
///
/// Every A2A-compliant agent publishes an Agent Card so that other agents can
/// discover its identity, capabilities, endpoint, and authentication requirements.
/// This follows the A2A v1.0 specification.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentCard {
    /// Human-readable name of the agent
    pub name: String,

    /// Description of what the agent does
    pub description: String,

    /// URL where this agent can be reached (e.g. `stdio://` or `http://`)
    pub url: Url,

    /// Skills this agent offers
    pub skills: Vec<Skill>,

    /// Authentication schemes the agent supports
    pub authentication: Vec<AuthenticationScheme>,

    /// Optional default skill id used when none is specified by the caller
    #[serde(skip_serializing_if = "Option::is_none")]
    pub default_skill: Option<String>,

    /// Reputation tracking for this agent
    #[serde(default)]
    pub reputation: AgentReputation,
}

/// A single skill offered by an agent, with JSON Schema for its input/output.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Skill {
    /// Unique identifier for this skill within the agent
    pub id: String,

    /// Human-readable name
    pub name: String,

    /// Description of what this skill does
    pub description: String,

    /// JSON Schema describing the expected input format
    pub input_schema: serde_json::Value,

    /// JSON Schema describing the output format
    pub output_schema: serde_json::Value,
}

/// Authentication scheme required to invoke this agent's skills.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthenticationScheme {
    /// Scheme identifier (e.g. `"none"`, `"bearer"`, `"oauth2"`)
    pub scheme: String,

    /// Optional credential data (e.g. API key, token endpoint URL)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub credentials: Option<String>,
}

impl AgentCard {
    /// Create a new `AgentCard` with the minimum required fields.
    pub fn new(name: &str, description: &str, url: Url) -> Self {
        Self {
            name: name.to_string(),
            description: description.to_string(),
            url,
            skills: Vec::new(),
            authentication: Vec::new(),
            default_skill: None,
            reputation: AgentReputation::new(),
        }
    }

    /// Create a new `AgentCard` with an initial reputation.
    pub fn new_with_reputation(
        name: &str,
        description: &str,
        url: Url,
        reputation: AgentReputation,
    ) -> Self {
        Self {
            name: name.to_string(),
            description: description.to_string(),
            url,
            skills: Vec::new(),
            authentication: Vec::new(),
            default_skill: None,
            reputation,
        }
    }

    /// Add a skill to this agent card.
    pub fn with_skill(mut self, skill: Skill) -> Self {
        self.skills.push(skill);
        self
    }

    /// Set the default skill ID.
    pub fn with_default_skill(mut self, skill_id: &str) -> Self {
        self.default_skill = Some(skill_id.to_string());
        self
    }

    /// Add an authentication scheme.
    pub fn with_auth(mut self, scheme: AuthenticationScheme) -> Self {
        self.authentication.push(scheme);
        self
    }

    /// Find a skill by its ID.
    pub fn find_skill(&self, skill_id: &str) -> Option<&Skill> {
        self.skills.iter().find(|s| s.id == skill_id)
    }
}

impl Skill {
    /// Create a new `Skill`.
    pub fn new(
        id: &str,
        name: &str,
        description: &str,
        input_schema: serde_json::Value,
        output_schema: serde_json::Value,
    ) -> Self {
        Self {
            id: id.to_string(),
            name: name.to_string(),
            description: description.to_string(),
            input_schema,
            output_schema,
        }
    }
}

impl AuthenticationScheme {
    /// Create a new "none" authentication scheme.
    pub fn none() -> Self {
        Self {
            scheme: "none".to_string(),
            credentials: None,
        }
    }

    /// Create a new bearer token authentication scheme.
    pub fn bearer(token: &str) -> Self {
        Self {
            scheme: "bearer".to_string(),
            credentials: Some(token.to_string()),
        }
    }

    /// Create a new OAuth2 authentication scheme.
    pub fn oauth2(credentials: &str) -> Self {
        Self {
            scheme: "oauth2".to_string(),
            credentials: Some(credentials.to_string()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use url::Url;

    #[test]
    fn test_agent_card_new() {
        let url = Url::parse("stdio://my-agent").unwrap();
        let card = AgentCard::new("test-agent", "A test agent", url.clone());
        assert_eq!(card.name, "test-agent");
        assert_eq!(card.description, "A test agent");
        assert_eq!(card.url, url);
        assert!(card.skills.is_empty());
        assert!(card.default_skill.is_none());
    }

    #[test]
    fn test_agent_card_with_skill() {
        let url = Url::parse("stdio://my-agent").unwrap();
        let card = AgentCard::new("test", "desc", url)
            .with_skill(Skill::new("s1", "Skill 1", "Does stuff", serde_json::json!({}), serde_json::json!({})))
            .with_default_skill("s1");
        assert_eq!(card.skills.len(), 1);
        assert_eq!(card.default_skill, Some("s1".to_string()));
        assert!(card.find_skill("s1").is_some());
        assert!(card.find_skill("s2").is_none());
    }

    #[test]
    fn test_serialization_roundtrip() {
        let url = Url::parse("http://localhost:8080").unwrap();
        let card = AgentCard::new("agent-x", "Agent X", url)
            .with_skill(Skill::new("code", "Code Gen", "Generates code", serde_json::json!({"type": "object"}), serde_json::json!({"type": "object"})))
            .with_auth(AuthenticationScheme::bearer("tok_123"));
        let json = serde_json::to_string_pretty(&card).unwrap();
        let deserialized: AgentCard = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.name, "agent-x");
        assert_eq!(deserialized.skills.len(), 1);
        assert_eq!(deserialized.skills[0].id, "code");
        assert_eq!(deserialized.authentication.len(), 1);
        assert_eq!(deserialized.authentication[0].scheme, "bearer");
    }

    #[test]
    fn test_auth_schemes() {
        let none = AuthenticationScheme::none();
        assert_eq!(none.scheme, "none");
        assert!(none.credentials.is_none());

        let bearer = AuthenticationScheme::bearer("tok_123");
        assert_eq!(bearer.scheme, "bearer");
        assert_eq!(bearer.credentials.as_deref(), Some("tok_123"));

        let oauth = AuthenticationScheme::oauth2("client_cred");
        assert_eq!(oauth.scheme, "oauth2");
        assert_eq!(oauth.credentials.as_deref(), Some("client_cred"));
    }
}
